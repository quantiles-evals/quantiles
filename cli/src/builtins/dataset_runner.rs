use anyhow::Result;
use futures::stream::{self, StreamExt, TryStreamExt};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tqdm::pbar;

use crate::dataset::{DatasetInfo, DatasetManager};

/// Batched dataset iterator with an optional progress bar.
///
/// Shared infrastructure for builtins that walk a dataset row-by-row.
pub(crate) struct DatasetRunner<'a> {
    manager: &'a DatasetManager,
    dataset_id: &'a str,
    info: &'a DatasetInfo,
    limit: usize,
    desc: Option<&'a str>,
    batch_size: usize,
    quiet: bool,
}

impl<'a> DatasetRunner<'a> {
    /// Create a new runner.
    pub(crate) fn new(
        manager: &'a DatasetManager,
        dataset_id: &'a str,
        info: &'a DatasetInfo,
        limit: usize,
    ) -> Self {
        Self {
            manager,
            dataset_id,
            info,
            limit,
            desc: None,
            batch_size: 100,
            quiet: false,
        }
    }

    /// Set a description for the progress bar.
    pub(crate) fn desc(mut self, desc: &'a str) -> Self {
        self.desc = Some(desc);
        self
    }

    /// Suppress the progress bar (useful for non-interactive / JSON mode).
    pub(crate) fn set_quiet(mut self, yes: bool) -> Self {
        self.quiet = yes;
        self
    }

    /// Iterate concurrently over rows within each batch, collecting results.
    ///
    /// The callback receives the global row index and the row value.
    /// Concurrency is limited to `max_workers` within each batch.
    pub(crate) async fn for_each_concurrent<F, Fut, T>(
        self,
        max_workers: usize,
        process_row: F,
    ) -> Result<Vec<T>>
    where
        F: Fn(usize, Value) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut processed = 0;
        let mut offset = 0;
        let mut results = Vec::with_capacity(self.limit);
        let mut pb = if self.quiet {
            None
        } else {
            Some(pbar(Some(self.limit)).desc(self.desc))
        };

        while processed < self.limit {
            let batch_limit = self.batch_size.min(self.limit - processed);
            let rows = self
                .manager
                .batch(
                    self.dataset_id,
                    &self.info.config,
                    &self.info.selected_split,
                    offset,
                    batch_limit,
                    self.info.revision.as_deref(),
                )
                .await?;

            if rows.is_empty() {
                break;
            }

            let actual_rows: Vec<Value> = rows.into_iter().take(self.limit - processed).collect();
            let count = actual_rows.len();

            {
                let mut stream = stream::iter(actual_rows.into_iter().enumerate())
                    .map(|(batch_i, row)| {
                        let f = process_row.clone();
                        let global_i = processed + batch_i;
                        async move { f(global_i, row).await }
                    })
                    .buffer_unordered(max_workers);

                while let Some(result) = stream.try_next().await? {
                    results.push(result);
                    if let Some(ref mut pb) = pb {
                        pb.update(1)?;
                    }
                }
            }

            processed += count;
            offset += count;
        }

        Ok(results)
    }

    /// Deserialize each raw JSON row before invoking the concurrent row callback, and return
    /// an error when any deserialization or callback fails.
    ///
    /// All deserialization error will include the dataset's row index for hopefully-easier
    /// debugging.
    pub(crate) async fn for_each_deserialized<Row, F, Fut, T>(
        self,
        max_workers: usize,
        process_row: F,
    ) -> Result<Vec<T>>
    where
        Row: DeserializeOwned,
        F: Fn(usize, Row) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<T>>,
    {
        self.for_each_concurrent(max_workers, move |i, row| {
            let process_row = process_row.clone();
            async move {
                let row = serde_json::from_value(row)
                    .map_err(|error| anyhow::anyhow!("row {i}: invalid row data: {error}"))?;
                process_row(i, row).await
            }
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::cache::{self, DatasetCache};
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Deserialize)]
    struct FixtureRow {
        index: i64,
    }

    async fn setup_fixture(
        cache: &DatasetCache,
        dataset_id: &str,
        config: &str,
        split: &str,
        offset: usize,
        limit: usize,
        rows: Vec<Value>,
    ) {
        let key = cache::cache_key(dataset_id, config, split, None);
        let path = cache.batch_path(&key, offset, limit);
        cache.write_batch(&path, &rows).await.unwrap();
    }

    fn fixture_info(total_rows: usize, config: &str, split: &str) -> DatasetInfo {
        DatasetInfo {
            total_rows: Some(total_rows),
            available_splits: vec![split.to_string()],
            selected_split: split.to_string(),
            config: config.to_string(),
            revision: None,
        }
    }

    #[tokio::test]
    async fn for_each_concurrent_collects_all_rows() {
        let tmpdir = tempfile::tempdir().unwrap();
        let manager = DatasetManager::new_with_cache_dir(tmpdir.path().to_path_buf()).unwrap();

        let rows: Vec<Value> = (0..8).map(|i| json!({"index": i})).collect();

        setup_fixture(
            &manager.cache,
            "fixture/test",
            "cfg",
            "train",
            0,
            8,
            rows.clone(),
        )
        .await;

        let info = fixture_info(8, "cfg", "train");
        let results: Vec<(usize, i64)> = DatasetRunner::new(&manager, "fixture/test", &info, 8)
            .for_each_concurrent(4, |i, row| async move {
                let idx = row["index"].as_i64().unwrap();
                Ok((i, idx))
            })
            .await
            .unwrap();

        assert_eq!(results.len(), 8);
        let mut sorted = results;
        sorted.sort_by_key(|(i, _)| *i);
        #[expect(clippy::cast_possible_wrap)]
        for (i, (global_idx, row_idx)) in sorted.iter().enumerate() {
            assert_eq!(*global_idx, i);
            assert_eq!(*row_idx, i as i64);
        }
    }

    #[tokio::test]
    async fn for_each_deserialized_passes_typed_rows_to_callback() {
        let tmpdir = tempfile::tempdir().unwrap();
        let manager = DatasetManager::new_with_cache_dir(tmpdir.path().to_path_buf()).unwrap();
        let rows: Vec<Value> = (0..3).map(|i| json!({"index": i})).collect();

        setup_fixture(&manager.cache, "fixture/typed", "cfg", "train", 0, 3, rows).await;

        let info = fixture_info(3, "cfg", "train");
        let results = DatasetRunner::new(&manager, "fixture/typed", &info, 3)
            .for_each_deserialized(2, |i, row: FixtureRow| async move { Ok((i, row.index)) })
            .await
            .unwrap();

        assert_eq!(results.len(), 3);
        assert!(
            results
                .iter()
                .all(|(i, value)| i64::try_from(*i) == Ok(*value))
        );
    }

    #[tokio::test]
    async fn for_each_deserialized_reports_invalid_row_index() {
        let tmpdir = tempfile::tempdir().unwrap();
        let manager = DatasetManager::new_with_cache_dir(tmpdir.path().to_path_buf()).unwrap();
        let rows = vec![json!({"index": "not-an-integer"})];

        setup_fixture(
            &manager.cache,
            "fixture/invalid",
            "cfg",
            "train",
            0,
            1,
            rows,
        )
        .await;

        let info = fixture_info(1, "cfg", "train");
        let error = DatasetRunner::new(&manager, "fixture/invalid", &info, 1)
            .for_each_deserialized(1, |_, row: FixtureRow| async move { Ok(row.index) })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("row 0: invalid row data"));
    }
}
