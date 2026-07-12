use std::time::Instant;

use anyhow::{Context, Result, bail};

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
                qt::config::BenchmarkConfig::CustomCode(c) => {
                    Ok(ResumePlan::CustomCode(c.command.clone()))
                }
                qt::config::BenchmarkConfig::Builtin(_)
                | qt::config::BenchmarkConfig::CustomNoCode(_) => Ok(ResumePlan::Builtin),
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
            let builtin: Box<dyn builtins::BuiltinWorkflow> = match bench_config {
                Some(qt::config::BenchmarkConfig::CustomNoCode(_)) => {
                    Box::new(builtins::CustomNoCodeBuiltin::new(workflow_name.to_owned()))
                }
                _ => builtins::resolve(workflow_name)
                    .with_context(|| format!("builtin `{workflow_name}` not found"))?,
            };
            let custom_nocode_input = match bench_config {
                Some(qt::config::BenchmarkConfig::CustomNoCode(config)) => {
                    Some(super::run::assemble_custom_nocode_input(config, None))
                }
                _ => None,
            };
            super::run::execute_builtin(super::run::ExecuteBuiltinArgs {
                db: &db,
                metrics_store: &metrics_store,
                run_id,
                workflow_name,
                builtin,
                input: custom_nocode_input.as_deref().or(stored_input),
                json,
                process_start,
            })
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

    /// A `custom_nocode` benchmark should plan to resume as a builtin so that the
    /// CLI can re-run the no-code workflow natively without spawning an external command.
    #[test]
    fn plan_resume_custom_nocode_with_config() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let bench = qt::config::BenchmarkConfig::CustomNoCode(Box::new(
            qt::config::CustomNoCodeBenchmarkConfig {
                type_: "custom_nocode".to_owned(),
                params: qt::config::CustomNoCodeParams {
                    dataset: qt::config::CustomNoCodeDatasetConfig {
                        name: "quantiles/simpleqa-verified".to_owned(),
                        config_name: None,
                        split: None,
                        revision: None,
                    },
                    model: Some(qt::llm::Sampler::Random {}),
                    prompt_template_file: file.path().to_str().unwrap().to_owned(),
                    limit: None,
                    max_workers: None,
                    style: qt::config::CustomNoCodeStyleConfig::ExactMatch {
                        golden_column: "answer".to_owned(),
                    },
                },
            },
        ));
        let plan = plan_resume("nocode_custom", &RunStatus::Failed, Some(&bench)).unwrap();
        assert!(matches!(plan, ResumePlan::Builtin));
    }

    /// A failed `custom_nocode` run can be resumed and re-execute successfully
    /// through the `CustomNoCodeBuiltin`, verifying that the resume path wires
    /// the correct builtin and `ExecuteBuiltinArgs`.
    #[tokio::test]
    async fn resume_failed_custom_nocode_run_re_executes_successfully() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let tmpdir = tempfile::tempdir().unwrap();
        let root = tmpdir.path();
        let cache_dir = root.join("cache");

        // Save original env var values so we can restore them later.
        let orig_hf = std::env::var("HF_DATASETS_SERVER").ok();
        let orig_cache = std::env::var("QUANTILES_DATASET_CACHE_DIR").ok();
        unsafe {
            std::env::set_var("HF_DATASETS_SERVER", server.uri());
            std::env::set_var("QUANTILES_DATASET_CACHE_DIR", cache_dir.as_os_str());
        }

        // Mock HF dataset server endpoints used by DatasetManager::init().
        Mock::given(method("GET"))
            .and(path("/splits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "splits": [{"config": "default", "split": "train"}]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/size"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "size": {"splits": [{"num_rows": 2}]}
            })))
            .mount(&server)
            .await;

        // Initialize workspace with SQLite DB and metrics dir.
        qt::db::init_workspace(root).await.unwrap();
        let db = qt::db::open_workspace(root).await.unwrap();
        let metrics_store =
            qt::metrics_store::MetricsStore::new(qt::db::metrics_dir(root)).unwrap();

        // Write a Jinja template file.
        let template_path = root.join("template.txt");
        std::fs::write(&template_path, "{{ row.question }}\nAnswer:").unwrap();

        // Pre-populate the dataset cache so no network fetch is needed for rows.
        let cache = qt::dataset::cache::DatasetCache::new(cache_dir);
        let rows = vec![
            serde_json::json!({"question": "what is 2+2", "answer": "4"}),
            serde_json::json!({"question": "what is 3+3", "answer": "6"}),
        ];
        let key = qt::dataset::cache::cache_key("fixture/qa", "default", "train", None);
        let batch_path = cache.batch_path(&key, 0, 2);
        cache.write_batch(&batch_path, &rows).await.unwrap();

        // Write a quantiles.toml so resume can load config.
        std::fs::write(
            root.join("quantiles.toml"),
            format!(
                r#"
[benchmarks.nocode_resume_test]
type = "custom_nocode"
style = {{ type = "exact_match", golden_column = "answer" }}
dataset = {{ name = "fixture/qa" }}
model = "random"
prompt_template_file = "{}"
limit = 2
"#,
                template_path.to_str().unwrap()
            ),
        )
        .unwrap();

        // A started no-code run stores normalized display input, so resume must
        // reconstruct the executable configuration from quantiles.toml.
        let input_json = serde_json::to_string(&serde_json::json!({
            "model": "demo-builtin",
            "num_samples": 2,
        }))
        .unwrap();

        let workflow_name = "nocode_resume_test";
        let run_id = qt::db::create_run(&db, workflow_name, Some(&input_json))
            .await
            .unwrap();

        // Mark the run as failed so it can be resumed.
        qt::db::fail_run(&db, &metrics_store, run_id, "simulated failure")
            .await
            .unwrap();

        let run_before = qt::db::get_run(&db, run_id).await.unwrap();
        assert_eq!(run_before.status, qt::db::RunStatus::Failed);

        // Change cwd so resume finds the config file.
        let orig_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(root).unwrap();

        resume(run_id, true, std::time::Instant::now())
            .await
            .unwrap();

        // Restore cwd.
        std::env::set_current_dir(orig_cwd).unwrap();

        // Verify the run is now completed.
        let run_after = qt::db::get_run(&db, run_id).await.unwrap();
        assert_eq!(run_after.status, qt::db::RunStatus::Completed);

        // Verify aggregate metrics were written.
        metrics_store.flush(run_id).await.unwrap();
        let agg = metrics_store.list_aggregate_for_run(run_id).await.unwrap();
        let names: Vec<&str> = agg.iter().map(|m| m.metric_name.as_str()).collect();
        assert!(names.contains(&"accuracy"));
        assert!(names.contains(&"correct_count"));
        assert!(names.contains(&"total_count"));

        // Verify steps were persisted in SQLite.
        let steps = qt::db::list_steps_for_run(&db, run_id).await.unwrap();
        assert_eq!(steps.len(), 2);

        // Restore environment variables.
        unsafe {
            match &orig_hf {
                Some(v) => std::env::set_var("HF_DATASETS_SERVER", v),
                None => std::env::remove_var("HF_DATASETS_SERVER"),
            }
            match &orig_cache {
                Some(v) => std::env::set_var("QUANTILES_DATASET_CACHE_DIR", v),
                None => std::env::remove_var("QUANTILES_DATASET_CACHE_DIR"),
            }
        }
    }
}
