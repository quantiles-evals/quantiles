mod metrics;
mod report;
mod run;
mod step;

use anyhow::{Context, Result};
use qt::db;
use qt::metrics_store::MetricsStore;

use crate::commands::compare::{report::CompareReport, run::RunData};

pub async fn compare(left_id: i64, right_id: i64, json: bool) -> Result<()> {
    if left_id == right_id {
        anyhow::bail!("cannot compare a run with itself");
    }

    let cwd = std::env::current_dir()?;
    let root = db::resolve_workspace_root(&cwd, false).await?;
    let db = db::open_workspace(&root).await?;
    let metrics_store = MetricsStore::new(db::metrics_dir(&root))?;

    let a = RunData::fetch_by_id(&db, &metrics_store, left_id)
        .await
        .with_context(|| format!("failed to load data for run {left_id}"))?;
    let b = RunData::fetch_by_id(&db, &metrics_store, right_id)
        .await
        .with_context(|| format!("failed to load data for run {right_id}"))?;

    let report = CompareReport::build(a, b);
    let differs = report.differs;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        report.print();
        // only print the notification about running `qt compare --json ...`
        // if we're not already in JSON mode
        println!("\nRun 'qt compare {left_id} {right_id} --json' for sample-level details");
    }

    if differs {
        std::process::exit(1);
    }

    Ok(())
}
