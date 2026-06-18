use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use arrow::array::{Array, Float64Array, Int64Array, StringArray, StringViewArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use crossbeam_queue::SegQueue;
use datafusion::prelude::*;
use itertools::Itertools;
use papaya::HashMap;
use parquet::arrow::ArrowWriter;

use crate::db::MetricPointSummary;
use crate::time::{format_utc, now_utc};

/// In-memory buffer of metric points waiting to be flushed to Parquet.
#[derive(Debug, Clone)]
struct BufferedMetric {
    run_id: i64,
    step_id: Option<i64>,
    metric_name: String,
    metric_value: f64,
    unit: Option<String>,
    created_at: time::OffsetDateTime,
}

/// Store for eval-run metrics backed by Parquet files.
///
/// Metrics are buffered in memory per run and flushed to a single Parquet
/// file when the run is completed or failed. Reads are served via datafusion
/// SQL over the per-run Parquet files.
#[derive(Debug, Clone)]
pub struct MetricsStore {
    /// Mapping from `run_id` to queue of buffered metrics.
    buffer: Arc<HashMap<i64, SegQueue<BufferedMetric>>>,
    dir: PathBuf,
}

impl MetricsStore {
    /// Open a metrics store backed by `dir`.
    ///
    /// The directory is created if it does not already exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the metrics store couldn't be opened.
    pub fn new(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create metrics dir {}", dir.display()))?;
        Ok(Self {
            buffer: Arc::new(HashMap::new()),
            dir,
        })
    }

    /// Buffer a metric point for later flush.
    pub async fn emit(
        &self,
        run_id: i64,
        step_id: Option<i64>,
        metric_name: &str,
        metric_value: f64,
        unit: Option<&str>,
    ) {
        let buffer = &self.buffer;
        let buffer_guard = buffer.guard();
        let run_queue = buffer.get_or_insert_with(run_id, || SegQueue::new(), &buffer_guard);
        run_queue.push(BufferedMetric {
            run_id,
            step_id,
            metric_name: metric_name.to_owned(),
            metric_value,
            unit: unit.map(String::from),
            created_at: now_utc(),
        });
    }

    /// Persist all buffered metrics for `run_id` to a Parquet file.
    ///
    /// If no metrics were buffered for this run, the call is a no-op.
    ///
    /// # Errors
    ///
    /// Returns an error if the flush failed.
    pub async fn flush(&self, run_id: i64) -> Result<()> {
        let buffer = &self.buffer;
        let buffer_guard = buffer.guard();
        let Some(metrics_q) = buffer.remove(&run_id, &buffer_guard) else {
            return Ok(());
        };

        let mut metrics = Vec::new();
        while let Some(metric) = metrics_q.pop() {
            metrics.push(metric);
        }

        if metrics.is_empty() {
            return Ok(());
        }

        let path = self.dir.join(format!("run_{run_id}.parquet"));
        let file = std::fs::File::create(&path)
            .with_context(|| format!("failed to create {}", path.display()))?;

        let schema = Arc::new(Schema::new(vec![
            Field::new("run_id", DataType::Int64, false),
            Field::new("step_id", DataType::Int64, true),
            Field::new("metric_name", DataType::Utf8, false),
            Field::new("metric_value", DataType::Float64, false),
            Field::new("unit", DataType::Utf8, true),
            Field::new("created_at", DataType::Utf8, false),
            // TODO: migrate to DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into()))
            // so that timestamps are stored natively in Parquet rather than as strings.
        ]));

        let (run_ids, step_ids, metric_names, metric_values, units, created_ats) = {
            let metrics_iter = metrics.into_iter();

            let vals_vec = metrics_iter.map(|m| {
                (
                    m.run_id,
                    m.step_id,
                    m.metric_name,
                    m.metric_value,
                    m.unit,
                    format_utc(m.created_at),
                )
            });
            let unzipped: (
                Vec<i64>,
                Vec<Option<i64>>,
                Vec<String>,
                Vec<f64>,
                Vec<Option<String>>,
                Vec<String>,
            ) = vals_vec.into_iter().multiunzip();

            let run_ids = Int64Array::from(unzipped.0);
            let step_ids = Int64Array::from(unzipped.1);
            let metric_names = StringArray::from(unzipped.2);
            let metric_values = Float64Array::from(unzipped.3);
            let units = StringArray::from(unzipped.4);
            let created_ats = StringArray::from(unzipped.5);
            (
                run_ids,
                step_ids,
                metric_names,
                metric_values,
                units,
                created_ats,
            )
        };

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(run_ids),
                Arc::new(step_ids),
                Arc::new(metric_names),
                Arc::new(metric_values),
                Arc::new(units),
                Arc::new(created_ats),
            ],
        )
        .context("failed to build metric RecordBatch")?;

        let mut writer =
            ArrowWriter::try_new(file, schema, None).context("failed to create ArrowWriter")?;
        writer
            .write(&batch)
            .context("failed to write metric batch")?;
        writer.close().context("failed to close ArrowWriter")?;

        Ok(())
    }

    /// Query all metrics for a single run via datafusion.
    ///
    /// # Errors
    ///
    /// Returns an error if metric summaries couldn't be listed for the given run.
    ///
    /// TODO: define a `RunID` type, rather than passing i64's down the stack
    pub async fn list_for_run(&self, run_id: i64) -> Result<Vec<MetricPointSummary>> {
        self.query_metrics(run_id, false).await
    }

    /// Query only aggregate metrics (those without a `step_id`) for a single
    /// run via datafusion.
    ///
    /// # Errors
    ///
    /// Returns an error if aggregate metric summaries couldn't be listed for
    /// the given run.
    pub async fn list_aggregate_for_run(&self, run_id: i64) -> Result<Vec<MetricPointSummary>> {
        self.query_metrics(run_id, true).await
    }

    async fn query_metrics(
        &self,
        run_id: i64,
        aggregate_only: bool,
    ) -> Result<Vec<MetricPointSummary>> {
        let path = self.dir.join(format!("run_{run_id}.parquet"));
        if !path.exists() {
            return Ok(Vec::new());
        }
        let path_str = path.to_str().context("metrics path is not valid UTF-8")?;

        let ctx = SessionContext::new();
        let mut df = ctx
            .read_parquet(path_str, ParquetReadOptions::default())
            .await
            .context("failed to read metrics parquet")?
            .filter(col("run_id").eq(lit(run_id)))
            .context("failed to filter metrics")?;

        if aggregate_only {
            df = df
                .filter(col("step_id").is_null())
                .context("failed to filter aggregate metrics")?;
        }

        let df = df
            .select(vec![
                col("metric_name"),
                col("metric_value"),
                col("unit"),
                col("step_id"),
                col("created_at"),
            ])
            .context("failed to select metric columns")?
            .sort(vec![
                col("metric_name").sort(true, false),
                col("created_at").sort(true, false),
            ])
            .context("failed to sort metrics")?;

        let batches = df
            .collect()
            .await
            .context("failed to collect metric batches")?;
        let mut results = Vec::new();

        for batch in &batches {
            let metric_names = batch
                .column_by_name("metric_name")
                .and_then(|c| c.as_any().downcast_ref::<StringViewArray>())
                .context("metric_name column missing or wrong type")?;
            let metric_values = batch
                .column_by_name("metric_value")
                .and_then(|c| c.as_any().downcast_ref::<Float64Array>())
                .context("metric_value column missing or wrong type")?;
            let units = batch
                .column_by_name("unit")
                .and_then(|c| c.as_any().downcast_ref::<StringViewArray>())
                .context("unit column missing or wrong type")?;
            let created_ats = batch
                .column_by_name("created_at")
                .and_then(|c| c.as_any().downcast_ref::<StringViewArray>())
                .context("created_at column missing or wrong type")?;
            let step_ids = batch
                .column_by_name("step_id")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .context("step_id column missing or wrong type")?;

            let rfc3339 = time::format_description::well_known::Rfc3339;

            for i in 0..batch.num_rows() {
                let created_at_str = created_ats.value(i);
                let created_at = time::OffsetDateTime::parse(created_at_str, &rfc3339)
                    .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
                results.push(MetricPointSummary {
                    metric_name: metric_names.value(i).to_owned(),
                    metric_value: metric_values.value(i),
                    unit: if units.is_null(i) {
                        None
                    } else {
                        Some(units.value(i).to_owned())
                    },
                    step_id: if step_ids.is_null(i) {
                        None
                    } else {
                        Some(step_ids.value(i))
                    },
                    created_at,
                });
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use crate::db::{create_run, init_workspace, open_workspace};

    use super::*;

    #[tokio::test]
    async fn emit_and_flush_roundtrip() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let root = tmpdir.path();
        init_workspace(root).await?;
        let db = open_workspace(root).await?;
        let store = MetricsStore::new(crate::db::metrics_dir(root))?;

        let run_id = create_run(&db, "demo", None).await?;
        store
            .emit(run_id, None, "latency_ms", 120.5, Some("ms"))
            .await;
        store.emit(run_id, None, "tokens_used", 42.0, None).await;
        store.flush(run_id).await?;

        let metrics = store.list_for_run(run_id).await?;
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[0].metric_name, "latency_ms");
        assert!((metrics[0].metric_value - 120.5).abs() < f64::EPSILON);
        assert_eq!(metrics[0].unit.as_deref(), Some("ms"));
        assert_eq!(metrics[1].metric_name, "tokens_used");
        assert!((metrics[1].metric_value - 42.0).abs() < f64::EPSILON);
        assert_eq!(metrics[1].unit.as_deref(), None);

        Ok(())
    }

    #[tokio::test]
    async fn list_for_run_returns_empty_when_no_file() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let root = tmpdir.path();
        init_workspace(root).await?;
        let store = MetricsStore::new(crate::db::metrics_dir(root))?;

        let metrics = store.list_for_run(9999).await?;
        assert!(metrics.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn flush_no_emit_is_noop() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let root = tmpdir.path();
        init_workspace(root).await?;
        let store = MetricsStore::new(crate::db::metrics_dir(root))?;

        store.flush(1).await?;

        let path = store.dir.join("run_1.parquet");
        assert!(
            !path.exists(),
            "flush without emit should not create a file"
        );

        Ok(())
    }

    #[tokio::test]
    async fn emit_with_step_id_roundtrip() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let root = tmpdir.path();
        init_workspace(root).await?;
        let db = open_workspace(root).await?;
        let store = MetricsStore::new(crate::db::metrics_dir(root))?;

        let run_id = create_run(&db, "demo", None).await?;
        store
            .emit(run_id, Some(7), "accuracy", 0.95, Some("percent"))
            .await;
        store.flush(run_id).await?;

        let metrics = store.list_for_run(run_id).await?;
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].metric_name, "accuracy");
        assert!((metrics[0].metric_value - 0.95).abs() < f64::EPSILON);
        assert_eq!(metrics[0].unit.as_deref(), Some("percent"));

        Ok(())
    }

    #[tokio::test]
    async fn run_isolation() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let root = tmpdir.path();
        init_workspace(root).await?;
        let db = open_workspace(root).await?;
        let store = MetricsStore::new(crate::db::metrics_dir(root))?;

        let run_a = create_run(&db, "demo", None).await?;
        let run_b = create_run(&db, "demo", None).await?;

        store.emit(run_a, None, "a_metric", 1.0, None).await;
        store.emit(run_b, None, "b_metric", 2.0, None).await;

        store.flush(run_a).await?;

        let metrics_a = store.list_for_run(run_a).await?;
        assert_eq!(metrics_a.len(), 1);
        assert_eq!(metrics_a[0].metric_name, "a_metric");

        let metrics_b = store.list_for_run(run_b).await?;
        assert!(metrics_b.is_empty(), "run_b should not be flushed yet");

        store.flush(run_b).await?;
        let metrics_b = store.list_for_run(run_b).await?;
        assert_eq!(metrics_b.len(), 1);
        assert_eq!(metrics_b[0].metric_name, "b_metric");

        Ok(())
    }

    #[tokio::test]
    async fn sorting_order() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let root = tmpdir.path();
        init_workspace(root).await?;
        let db = open_workspace(root).await?;
        let store = MetricsStore::new(crate::db::metrics_dir(root))?;

        let run_id = create_run(&db, "demo", None).await?;
        store.emit(run_id, None, "zebra", 1.0, None).await;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        store.emit(run_id, None, "alpha", 2.0, None).await;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        store.emit(run_id, None, "alpha", 3.0, None).await;
        store.flush(run_id).await?;

        let metrics = store.list_for_run(run_id).await?;
        assert_eq!(metrics.len(), 3);
        assert_eq!(metrics[0].metric_name, "alpha");
        assert!((metrics[0].metric_value - 2.0).abs() < f64::EPSILON);
        assert_eq!(metrics[1].metric_name, "alpha");
        assert!((metrics[1].metric_value - 3.0).abs() < f64::EPSILON);
        assert_eq!(metrics[2].metric_name, "zebra");

        Ok(())
    }

    #[tokio::test]
    async fn double_flush_preserves_first_flush() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let root = tmpdir.path();
        init_workspace(root).await?;
        let db = open_workspace(root).await?;
        let store = MetricsStore::new(crate::db::metrics_dir(root))?;

        let run_id = create_run(&db, "demo", None).await?;
        store.emit(run_id, None, "latency", 100.0, None).await;
        store.flush(run_id).await?;

        let metrics_first = store.list_for_run(run_id).await?;
        assert_eq!(metrics_first.len(), 1);

        store.flush(run_id).await?;

        let metrics_second = store.list_for_run(run_id).await?;
        assert_eq!(metrics_second.len(), 1);
        assert!((metrics_second[0].metric_value - 100.0).abs() < f64::EPSILON);

        Ok(())
    }

    #[tokio::test]
    async fn concurrent_emits_to_same_run() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let root = tmpdir.path();
        init_workspace(root).await?;
        let db = open_workspace(root).await?;
        let store = MetricsStore::new(crate::db::metrics_dir(root))?;

        let run_id = create_run(&db, "demo", None).await?;
        let store = Arc::new(store);

        let mut handles = Vec::new();
        for i in 0..50 {
            let store = store.clone();
            let step_id = i64::from(i);
            let metric_val = f64::from(i);
            handles.push(tokio::spawn(async move {
                store
                    .emit(run_id, Some(step_id), "concurrent", metric_val, None)
                    .await;
            }));
        }

        for h in handles {
            h.await?;
        }

        store.flush(run_id).await?;

        let metrics = store.list_for_run(run_id).await?;
        assert_eq!(metrics.len(), 50);

        Ok(())
    }
}
