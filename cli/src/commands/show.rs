use anyhow::Result;
use comfy_table::{Cell, ContentArrangement, Table, presets::NOTHING};
use qt::time::{format_duration, format_utc};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use qt::db::{self, MetricPointSummary, StepSummary, WorkflowRun};
use qt::metrics_store::MetricsStore;

/// JSON payload emitted by `qt show --json`.
#[derive(Serialize)]
struct ShowJsonOutput {
    run: db::WorkflowRun,
    metrics: Vec<db::MetricPointSummary>,
    samples: Vec<Value>,
}

pub async fn show(run_id: i64, json: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root = db::resolve_workspace_root(&cwd, false).await?;
    let db = db::open_workspace(&root).await?;
    let metrics_store = MetricsStore::new(db::metrics_dir(&root))?;
    let (run, steps, metrics) = tokio::try_join!(
        db::get_run(&db, run_id),
        db::list_steps_for_run(&db, run_id),
        metrics_store.list_for_run(run_id),
    )?;

    if json {
        print_json(run, steps, metrics)?;
    } else {
        println!("Run {}", run.id);
        println!("  eval:        {}", run.workflow_name);
        println!("  status:      {}", run.status);
        println!("  created:     {}", format_utc(run.started_at));
        println!(
            "  duration:    {}",
            run.finished_at.map_or_else(
                || "-".to_string(),
                |dt| format_duration(dt - run.started_at)
            )
        );
        println!("  input:       {}", run.input.as_deref().unwrap_or("-"));
        println!("  output:      {}", run.output.as_deref().unwrap_or("-"));
        println!("  error:       {}", run.error.as_deref().unwrap_or("-"));

        let aggregate_metrics: Vec<db::MetricPointSummary> = metrics
            .iter()
            .filter(|m| m.step_id.is_none())
            .cloned()
            .collect();
        print_metrics(&aggregate_metrics);
        println!("\nRun 'qt show {run_id} --json' for sample-level details.");
    }
    Ok(())
}

fn print_json(
    run: WorkflowRun,
    steps: Vec<StepSummary>,
    metrics: Vec<MetricPointSummary>,
) -> Result<()> {
    let mut samples = Vec::new();
    for step in steps {
        let mut sample = step
            .output
            .as_deref()
            .and_then(|output| serde_json::from_str::<Value>(output).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        sample.insert("step_key".to_string(), Value::String(step.step_key.clone()));
        sample.insert("status".to_string(), Value::String(step.status.to_string()));
        sample.insert(
            "input_hash".to_string(),
            Value::String(step.input_hash.clone()),
        );
        if let Some(ref err) = step.error {
            sample.insert("error".to_string(), Value::String(err.clone()));
        }
        sample.insert(
            "started_at".to_string(),
            Value::String(format_utc(step.started_at)),
        );
        if let Some(finished) = step.finished_at {
            sample.insert(
                "finished_at".to_string(),
                Value::String(format_utc(finished)),
            );
        }

        // TODO: This is O(steps * metrics). Consider moving the filter into
        // DataFusion (e.g. list_metrics_for_step) or pre-computing a
        // HashMap<i64, HashMap<String, f64>> once up-front.
        let step_metrics: HashMap<String, f64> = metrics
            .iter()
            .filter(|m| m.step_id == Some(step.id))
            .map(|m| (m.metric_name.clone(), m.metric_value))
            .collect();
        sample.insert("metrics".to_string(), serde_json::to_value(step_metrics)?);

        samples.push(Value::Object(sample));
    }

    let output = ShowJsonOutput {
        run,
        metrics,
        samples,
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn print_metrics(metrics: &[db::MetricPointSummary]) {
    println!();
    if metrics.is_empty() {
        println!("Aggregated Metrics");
        println!("  No metrics found.");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(NOTHING)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["NAME", "VALUE", "UNIT"]);

    for metric in metrics {
        table.add_row(vec![
            Cell::new(&metric.metric_name),
            Cell::new(metric.metric_value),
            Cell::new(metric.unit.as_deref().unwrap_or("-")),
        ]);
    }

    println!("Metrics");
    println!("{table}");
}
