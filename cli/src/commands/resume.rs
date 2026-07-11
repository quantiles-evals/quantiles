use std::time::Instant;

use anyhow::{Result, bail};

use qt::builtins;
use qt::db;
use qt::db::RunStatus;
use qt::metrics_store::MetricsStore;

/// Result of planning how to resume a run.
#[derive(Debug)]
pub(crate) enum ResumePlan {
    Builtin,
    CustomCode(Vec<String>),
}

/// Plan how to resume a run without doing any IO.
///
/// # Errors
///
/// Returns an error when the run is already completed, the benchmark config is
/// invalid, or there is no way to resume the workflow.
pub(crate) fn plan_resume(
    workflow_name: &str,
    run_status: &RunStatus,
    bench_config: Option<&qt::config::BenchmarkConfig>,
) -> Result<ResumePlan> {
    if *run_status == RunStatus::Completed {
        bail!(
            "run is already completed; \
             create a new run or resume a running/failed one"
        );
    }

    match bench_config {
        Some(bench) => {
            bench.validate()?;
            match bench {
                qt::config::BenchmarkConfig::Builtin(_) => Ok(ResumePlan::Builtin),
                qt::config::BenchmarkConfig::CustomCode(c) => {
                    Ok(ResumePlan::CustomCode(c.command.clone()))
                }
            }
        }
        None => {
            if builtins::resolve(workflow_name).is_some() {
                Ok(ResumePlan::Builtin)
            } else {
                bail!(
                    "no config section found for benchmark `{workflow_name}`; \
                     cannot resume custom eval without config"
                );
            }
        }
    }
}

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
    if run.status == RunStatus::Completed {
        // TODO: output error in JSON if json == true
        bail!(
            "run {run_id} is already completed; \
             create a new run or resume a running/failed one"
        );
    }

    let workflow_name = run.workflow_name.as_str();
    let stored_input = run.input.as_deref();

    let config = qt::config::load()?;
    let bench_config = config.benchmarks.get(workflow_name);

    let plan = plan_resume(workflow_name, &run.status, bench_config)?;

    db::resume_run(&db, run_id).await?;
    if !json {
        println!("Resuming eval run {run_id} ({workflow_name})");
    }

    // TODO: we always re-read the command from the config file on resume.
    // This means that if the config file is edited between `qt run` and
    // `qt resume`, the resumed run will use the updated command. It may be
    // wise to revisit this policy.
    match plan {
        ResumePlan::Builtin => {
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
        ResumePlan::CustomCode(command) => {
            super::run::execute_custom(
                run_id,
                workflow_name,
                stored_input,
                &command,
                json,
                process_start,
                None,
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resuming a run whose status is `completed` must be rejected before any execution
    /// begins, because a completed run cannot be meaningfully resumed.
    #[test]
    fn plan_resume_completed_run_errors() {
        let bench = qt::config::BenchmarkConfig::Builtin(qt::config::BuiltinBenchmarkConfig {
            type_: "builtin".to_owned(),
            samples: None,
            dataset: None,
            model: None,
            max_workers: None,
        });
        let err = plan_resume("demo", &RunStatus::Completed, Some(&bench)).unwrap_err();
        assert!(err.to_string().contains("already completed"));
    }

    /// A builtin benchmark with a valid config section should plan to resume as a builtin,
    /// using the stored input from the database.
    #[test]
    fn plan_resume_builtin_with_config() {
        let bench = qt::config::BenchmarkConfig::Builtin(qt::config::BuiltinBenchmarkConfig {
            type_: "builtin".to_owned(),
            samples: Some(10),
            dataset: None,
            model: None,
            max_workers: None,
        });
        let plan = plan_resume("demo", &RunStatus::Failed, Some(&bench)).unwrap();
        assert!(matches!(plan, ResumePlan::Builtin));
    }

    /// A builtin benchmark that has no config section can still be resumed by name lookup,
    /// falling back to the hardcoded builtin registry.
    #[test]
    fn plan_resume_builtin_without_config() {
        let plan = plan_resume("pubmedqa", &RunStatus::Failed, None).unwrap();
        assert!(matches!(plan, ResumePlan::Builtin));
    }

    /// A `custom_code` benchmark with a config section should plan to resume by re-running
    /// the command array from the config file with the stored DB input.
    #[test]
    fn plan_resume_custom_code_with_config() {
        let bench =
            qt::config::BenchmarkConfig::CustomCode(qt::config::CustomCodeBenchmarkConfig {
                type_: "custom_code".to_owned(),
                command: vec!["python".to_owned(), "eval.py".to_owned()],
                input: None,
            });
        let plan = plan_resume("my-eval", &RunStatus::Failed, Some(&bench)).unwrap();
        assert!(matches!(&plan, ResumePlan::CustomCode(cmd) if cmd == &["python", "eval.py"]));
    }

    /// A `custom_code` benchmark without a config section cannot be resumed because the
    /// CLI has no source of truth for what command to execute.
    #[test]
    fn plan_resume_custom_code_without_config_errors() {
        let err = plan_resume("my-eval", &RunStatus::Failed, None).unwrap_err();
        assert!(err.to_string().contains("no config section found"));
    }

    /// An unknown workflow name with neither a config section nor a builtin match must
    /// fail immediately with a clear "no config section found" message.
    #[test]
    fn plan_resume_unknown_without_config_errors() {
        let err = plan_resume("unknown-eval", &RunStatus::Failed, None).unwrap_err();
        assert!(err.to_string().contains("no config section found"));
    }

    /// Even when a config section is present, it must pass `validate()` before resume
    /// proceeds, so invalid configs (e.g. empty command) are caught early.
    #[test]
    fn plan_resume_validates_config() {
        let bench =
            qt::config::BenchmarkConfig::CustomCode(qt::config::CustomCodeBenchmarkConfig {
                type_: "custom_code".to_owned(),
                command: vec![],
                input: None,
            });
        let err = plan_resume("my-eval", &RunStatus::Failed, Some(&bench)).unwrap_err();
        assert!(err.to_string().contains("non-empty `command`"));
    }
}
