use std::sync::Arc;

use anyhow::Result;

use self::data::{DatasetRow, prepare_row};
use self::evaluation::{EvaluateRowArgs, evaluate_row};
use self::metrics::emit_aggregate_metrics;
use self::runtime::{load_template, parse_input, resolve_dataset_limit, resolve_sampler_for_style};
use crate::builtins::common::get_max_workers;
use crate::builtins::dataset_runner::DatasetRunner;
use crate::builtins::input::set_builtin_run_input;
use crate::builtins::output::set_builtin_run_output;
use crate::builtins::{BuiltinContext, BuiltinWorkflow};

mod data;
mod evaluation;
mod metrics;
mod runtime;

/// No-code custom benchmark builtin.
pub struct CustomNoCodeBuiltin {
    name: String,
}

impl CustomNoCodeBuiltin {
    /// Create a new builtin with the workflow name from the config file.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait::async_trait]
impl BuiltinWorkflow for CustomNoCodeBuiltin {
    fn name(&self) -> String {
        self.name.clone()
    }

    async fn execute(&self, ctx: BuiltinContext<'_>) -> Result<()> {
        let config = parse_input(ctx.input)?;
        let (template_str, env) = load_template(&config.prompt_template_file)?;
        let max_workers = config.max_workers.unwrap_or_else(get_max_workers);
        let llm = resolve_sampler_for_style(config.model.as_ref(), &config.style)?;

        let (manager, info, limit) = resolve_dataset_limit(
            &config.dataset.name,
            config.dataset.config_name.as_deref(),
            config.dataset.split.as_deref(),
            config.dataset.revision.as_deref(),
            config.limit,
        )
        .await?;
        set_builtin_run_input(
            ctx.db,
            ctx.run_id,
            config.model.as_ref(),
            limit,
            config.max_workers,
        )
        .await?;

        let db = ctx.db.clone();
        let model_name = config
            .model
            .as_ref()
            .map_or("random".to_string(), std::string::ToString::to_string);
        let run_id = ctx.run_id;
        let dataset = config.dataset.name.clone();
        let choice_labels = match &config.style {
            crate::config::CustomNoCodeStyleConfig::ExactMatch { .. } => None,
            crate::config::CustomNoCodeStyleConfig::MultipleChoice { choice_labels, .. } => {
                Some(choice_labels.clone())
            }
        };
        let style = Arc::new(config.style.clone());
        let template_str = Arc::new(template_str);

        let name = self.name();
        let results = DatasetRunner::new(&manager, &dataset, &info, limit)
            .desc(&name)
            .set_quiet(ctx.quiet)
            .for_each_deserialized(max_workers, move |i, row: DatasetRow| {
                let llm = Arc::clone(&llm);
                let db = db.clone();
                let model_name = model_name.clone();
                let template_str = Arc::clone(&template_str);
                let style = Arc::clone(&style);
                let env = env.clone();
                async move {
                    let prepared = prepare_row(i, &row, &style)?;
                    let args = EvaluateRowArgs {
                        i,
                        row: &row,
                        prepared,
                        template_str: &template_str,
                        env: &env,
                        model_name: &model_name,
                        llm: &llm,
                        db: &db,
                        metrics_store: ctx.metrics_store,
                        run_id,
                    };
                    evaluate_row(&self.name, args).await
                }
            })
            .await?;

        let total_count = results.len();
        emit_aggregate_metrics(
            ctx.metrics_store,
            ctx.run_id,
            &results,
            choice_labels.as_deref(),
        )
        .await?;

        set_builtin_run_output(ctx.db, ctx.run_id, total_count).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    /// Verifies the builtin exposes the benchmark name supplied at construction.
    fn builtin_name_returns_configured_name() {
        let builtin = CustomNoCodeBuiltin::new("my-benchmark".to_owned());
        assert_eq!(builtin.name(), "my-benchmark");
    }

    #[tokio::test]
    /// Verifies execution fails before dataset access when the prompt template is invalid.
    async fn execute_rejects_invalid_jinja_template() {
        let tmpdir = tempfile::tempdir().unwrap();
        let root = tmpdir.path();
        crate::db::init_workspace(root).await.unwrap();
        let db = crate::db::open_workspace(root).await.unwrap();
        let metrics_store =
            crate::metrics_store::MetricsStore::new(crate::db::metrics_dir(root)).unwrap();

        let template_path = root.join("bad.txt");
        std::fs::write(&template_path, "{{ unclosed").unwrap();

        let input_json = serde_json::to_string(&json!({
            "style": {"type": "exact_match", "golden_column": "a"},
            "dataset": {"name": "fixture/qa"},
            "prompt_template_file": template_path.to_str().unwrap(),
        }))
        .unwrap();

        let run_id = crate::db::create_run(&db, "test", Some(&input_json))
            .await
            .unwrap();

        let workflow_name = "test".to_owned();
        let builtin = CustomNoCodeBuiltin::new(workflow_name.clone());
        let result = builtin
            .execute(BuiltinContext {
                db: &db,
                metrics_store: &metrics_store,
                run_id,
                workflow_name: &workflow_name,
                input: Some(&input_json),
                quiet: true,
            })
            .await;

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("invalid jinja syntax"),
            "unexpected error: {msg}"
        );
    }

    #[expect(clippy::too_many_lines)]
    #[expect(clippy::cast_possible_truncation)]
    #[tokio::test]
    /// Verifies end-to-end execution, metric persistence, and cached-step reuse.
    async fn execute_records_metrics_and_steps_with_fixture() {
        let server = MockServer::start().await;
        let tmpdir = tempfile::tempdir().unwrap();
        let root = tmpdir.path();
        let cache_dir = root.join("cache");

        let orig_hf = std::env::var("HF_DATASETS_SERVER").ok();
        let orig_cache = std::env::var("QUANTILES_DATASET_CACHE_DIR").ok();
        unsafe {
            std::env::set_var("HF_DATASETS_SERVER", server.uri());
            std::env::set_var("QUANTILES_DATASET_CACHE_DIR", cache_dir.as_os_str());
        }

        Mock::given(method("GET"))
            .and(path("/splits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "splits": [{"config": "default", "split": "train"}]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/size"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "size": {"splits": [{"num_rows": 2}]}
            })))
            .mount(&server)
            .await;

        crate::db::init_workspace(root).await.unwrap();
        let db = crate::db::open_workspace(root).await.unwrap();
        let metrics_store =
            crate::metrics_store::MetricsStore::new(crate::db::metrics_dir(root)).unwrap();

        let template_path = root.join("template.txt");
        std::fs::write(&template_path, "{{ row.question }}\nAnswer:").unwrap();

        let cache = crate::dataset::cache::DatasetCache::new(cache_dir);
        let rows = vec![
            json!({"question": "what is 2+2", "answer": "4"}),
            json!({"question": "what is 3+3", "answer": "6"}),
        ];
        let key = crate::dataset::cache::cache_key("fixture/qa", "default", "train", None);
        let batch_path = cache.batch_path(&key, 0, 2);
        cache.write_batch(&batch_path, &rows).await.unwrap();

        let input_json = serde_json::to_string(&json!({
            "style": {"type": "exact_match", "golden_column": "answer"},
            "dataset": {"name": "fixture/qa"},
            "model": "random",
            "prompt_template_file": template_path.to_str().unwrap(),
            "limit": 2,
        }))
        .unwrap();

        let run_id = crate::db::create_run(&db, "test_nocode", Some(&input_json))
            .await
            .unwrap();

        let workflow_name = "test_nocode".to_owned();
        let builtin = CustomNoCodeBuiltin::new(workflow_name.clone());
        builtin
            .execute(BuiltinContext {
                db: &db,
                metrics_store: &metrics_store,
                run_id,
                workflow_name: &workflow_name,
                input: Some(&input_json),
                quiet: true,
            })
            .await
            .unwrap();

        metrics_store.flush(run_id).await.unwrap();

        let agg = metrics_store.list_aggregate_for_run(run_id).await.unwrap();
        let names: Vec<&str> = agg.iter().map(|m| m.metric_name.as_str()).collect();
        assert!(names.contains(&"accuracy"));
        assert!(names.contains(&"correct_count"));
        assert!(names.contains(&"incorrect_count"));
        assert!(names.contains(&"total_count"));
        assert!(names.contains(&"parsed_response_count"));
        assert!(names.contains(&"unparsed_response_count"));
        assert!(names.contains(&"parse_rate"));
        assert!(names.contains(&"mean_latency_ms"));
        assert!(names.contains(&"median_latency_ms"));
        assert!(names.contains(&"p95_latency_ms"));
        assert!(names.contains(&"p99_latency_ms"));
        assert!(names.contains(&"min_latency_ms"));
        assert!(names.contains(&"max_latency_ms"));

        let total_metric = agg.iter().find(|m| m.metric_name == "total_count").unwrap();
        assert_eq!(total_metric.metric_value as i64, 2);

        let correct_metric = agg
            .iter()
            .find(|m| m.metric_name == "correct_count")
            .unwrap();
        assert_eq!(correct_metric.metric_value as i64, 0);

        let all_metrics = metrics_store.list_for_run(run_id).await.unwrap();
        let is_correct_count = all_metrics
            .iter()
            .filter(|m| m.metric_name == "is_correct")
            .count();
        assert_eq!(is_correct_count, 2);
        let response_parsed_count = all_metrics
            .iter()
            .filter(|m| m.metric_name == "response_parsed")
            .count();
        assert_eq!(response_parsed_count, 2);

        let steps = crate::db::list_steps_for_run(&db, run_id).await.unwrap();
        assert_eq!(steps.len(), 2);

        let builtin2 = CustomNoCodeBuiltin::new(workflow_name.clone());
        builtin2
            .execute(BuiltinContext {
                db: &db,
                metrics_store: &metrics_store,
                run_id,
                workflow_name: &workflow_name,
                input: Some(&input_json),
                quiet: true,
            })
            .await
            .unwrap();

        let steps2 = crate::db::list_steps_for_run(&db, run_id).await.unwrap();
        assert_eq!(
            steps2.len(),
            2,
            "second execution should reuse cached steps instead of creating new ones"
        );

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
