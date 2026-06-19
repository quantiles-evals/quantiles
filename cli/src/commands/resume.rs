use std::time::Instant;

use anyhow::{Result, bail};

use qt::builtins;
use qt::db;
use qt::metrics_store::MetricsStore;

/// Resume an existing eval run.
///
/// # Errors
///
/// Returns an error when the run does not exist, is already completed, the
/// config file is missing or invalid, or execution fails.
pub async fn resume(run_id: i64, json: bool, process_start: Instant) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root = db::resolve_workspace_root(&cwd, true).await?;
    let db = db::open_workspace(&root).await?;
    let metrics_store = MetricsStore::new(db::metrics_dir(&root))?;

    let run = db::get_run(&db, run_id).await?;
    if run.status == db::RunStatus::Completed {
        bail!(
            "run {run_id} is already completed; \
             create a new run or resume a running/failed one"
        );
    }

    let workflow_name = run.workflow_name.as_str();
    let stored_input = run.input.as_deref();

    let config = qt::config::load()?;
    let bench_config = config.benchmarks.get(workflow_name);

    db::resume_run(&db, run_id).await?;
    if !json {
        println!("Resuming eval run {run_id} ({workflow_name})");
    }

    match bench_config {
        Some(bench) => {
            bench.validate()?;
            match bench.type_ {
                qt::config::BenchmarkType::Builtin => {
                    super::run::execute_builtin(
                        &db,
                        &metrics_store,
                        run_id,
                        workflow_name,
                        stored_input,
                        json,
                        process_start,
                    )
                    .await
                }
                qt::config::BenchmarkType::CustomCode => {
                    let command = bench.command.as_ref().unwrap();
                    super::run::execute_custom(
                        run_id,
                        workflow_name,
                        stored_input,
                        command,
                        json,
                        process_start,
                        None,
                    )
                    .await
                }
            }
        }
        None => {
            if builtins::resolve(workflow_name).is_some() {
                super::run::execute_builtin(
                    &db,
                    &metrics_store,
                    run_id,
                    workflow_name,
                    stored_input,
                    json,
                    process_start,
                )
                .await
            } else {
                bail!(
                    "no config section found for benchmark `{workflow_name}`; \
                     cannot resume custom eval without config"
                );
            }
        }
    }
}
