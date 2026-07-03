use std::sync::Arc;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::builtins::common::{
    emit_accuracy_metrics, extract_text, get_max_workers, hash_input, resolve_sampler,
    run_timed_step,
};
use crate::builtins::dataset_runner::DatasetRunner;
use crate::builtins::input::set_builtin_run_input;
use crate::builtins::output::set_builtin_run_output;
use crate::builtins::{BuiltinContext, BuiltinWorkflow};
use crate::dataset::DatasetManager;
use crate::llm::random::RandomSampler;

/// Input deserialized from the JSON assembled by `commands::run`.
#[derive(Debug, Deserialize)]
struct CustomNoCodeInput {
    style: crate::config::CustomNoCodeStyle,
    dataset: String,
    #[serde(default)]
    model: Option<crate::llm::Sampler>,
    #[serde(flatten)]
    qa: crate::config::CustomNoCodeQaConfig,
}

/// Per-row step output stored as JSON in the step record.
#[derive(Debug, Serialize, Deserialize)]
struct RowOutput {
    input: String,
    response: String,
    golden: String,
    is_correct: bool,
}

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

    #[expect(clippy::too_many_arguments)]
    async fn evaluate_row(
        &self,
        i: usize,
        row: &serde_json::Value,
        prompt_column: &str,
        golden_column: &str,
        template_str: &str,
        env: &jinja::Environment<'_>,
        model: Option<&crate::llm::Sampler>,
        llm: &std::sync::Arc<dyn crate::llm::LLMSampler>,
        db: &sea_orm::DatabaseConnection,
        metrics_store: &crate::metrics_store::MetricsStore,
        run_id: i64,
    ) -> Result<bool> {
        let prompt = extract_text(row, prompt_column)
            .with_context(|| format!("row {i}: missing prompt column `{prompt_column}`"))?;
        let golden = extract_text(row, golden_column)
            .with_context(|| format!("row {i}: missing golden column `{golden_column}`"))?;

        let rendered = env
            .render_str(template_str, jinja::context!(prompt => &prompt))
            .with_context(|| format!("row {i}: failed to render prompt template"))?;

        let model_str = model
            .as_ref()
            .map_or("random".to_string(), std::string::ToString::to_string);
        let input_hash = hash_input(&format!(
            "{rendered}\nmodel={model_str}\nworkflow={}",
            self.name()
        ));
        let step_key = format!("row-{i}");

        let (output, step_id) =
            run_timed_step(db, metrics_store, run_id, &step_key, &input_hash, async {
                let model_response = llm
                    .sample(&rendered)
                    .await
                    .with_context(|| format!("failed to sample LLM for row {i}"))?;

                let is_correct = is_exact_match(&model_response, &golden);

                Ok(RowOutput {
                    input: rendered.clone(),
                    response: model_response,
                    golden,
                    is_correct,
                })
            })
            .await?;

        if let Some(step_id) = step_id {
            metrics_store
                .emit(
                    run_id,
                    Some(step_id),
                    "is_correct",
                    if output.is_correct { 1.0 } else { 0.0 },
                    None,
                )
                .await;
        }

        Ok(output.is_correct)
    }
}

#[async_trait::async_trait]
impl BuiltinWorkflow for CustomNoCodeBuiltin {
    fn name(&self) -> String {
        self.name.clone()
    }

    async fn execute(&self, ctx: BuiltinContext<'_>) -> Result<()> {
        let config = parse_input(ctx.input)?;

        let (template_str, env) = load_template(&config.qa.prompt_template_file)?;

        let max_workers = config.qa.max_workers.unwrap_or_else(get_max_workers);

        let llm = resolve_sampler(config.model.as_ref(), || Arc::new(RandomSampler::new(80)))?;

        let (manager, info, limit) =
            resolve_dataset_limit(&config.dataset, config.qa.limit).await?;

        set_builtin_run_input(
            ctx.db,
            ctx.run_id,
            config.model.as_ref(),
            limit,
            config.qa.max_workers,
        )
        .await?;

        let db = ctx.db.clone();
        let model = config.model.clone();
        let run_id = ctx.run_id;
        let dataset = config.dataset.clone();
        let prompt_column = config.qa.prompt_column.clone();
        let golden_column = config.qa.golden_column.clone();
        let template_str = Arc::new(template_str);

        let name = self.name();
        let results = DatasetRunner::new(&manager, &dataset, &info, limit)
            .desc(&name)
            .set_quiet(ctx.quiet)
            .for_each_concurrent(max_workers, move |i, row| {
                let llm = Arc::clone(&llm);
                let db = db.clone();
                let model = model.clone();
                let template_str = Arc::clone(&template_str);
                let prompt_column = prompt_column.clone();
                let golden_column = golden_column.clone();
                let env = env.clone();
                async move {
                    self.evaluate_row(
                        i,
                        &row,
                        &prompt_column,
                        &golden_column,
                        &template_str,
                        &env,
                        model.as_ref(),
                        &llm,
                        &db,
                        ctx.metrics_store,
                        run_id,
                    )
                    .await
                }
            })
            .await?;

        let total_count = results.len();
        emit_accuracy_metrics(ctx.metrics_store, ctx.run_id, results).await;

        set_builtin_run_output(ctx.db, ctx.run_id, total_count).await?;

        Ok(())
    }
}

fn parse_input(input: Option<&str>) -> Result<CustomNoCodeInput> {
    let config: CustomNoCodeInput = input
        .map(serde_json::from_str)
        .transpose()
        .context("invalid builtin input JSON")?
        .context("custom_nocode benchmark requires input configuration")?;

    match config.style {
        crate::config::CustomNoCodeStyle::Qa => {}
    }

    if config.qa.limit == Some(0) {
        bail!("limit must be > 0");
    }

    Ok(config)
}

fn load_template(path: &str) -> Result<(String, jinja::Environment<'_>)> {
    let template_str = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read prompt template file `{path}`"))?;
    let env = jinja::Environment::new();
    env.render_str(&template_str, jinja::context!(prompt => ""))
        .with_context(|| format!("invalid jinja syntax in prompt template file `{path}`"))?;
    Ok((template_str, env))
}

async fn resolve_dataset_limit(
    dataset: &str,
    limit: Option<usize>,
) -> Result<(DatasetManager, crate::dataset::DatasetInfo, usize)> {
    let manager = DatasetManager::new()?;
    let info = manager.init(dataset, None, None, None).await?;

    let total = info
        .total_rows
        .context("could not determine dataset size; pass an explicit limit")?;
    let limit = limit.unwrap_or(total).min(total);

    Ok((manager, info, limit))
}

/// Case-sensitive exact-match comparison after trimming whitespace.
fn is_exact_match(response: &str, golden: &str) -> bool {
    response.trim() == golden.trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn builtin_name_returns_configured_name() {
        let builtin = CustomNoCodeBuiltin::new("my-benchmark".to_owned());
        assert_eq!(builtin.name(), "my-benchmark");
    }

    #[test]
    fn render_template_with_prompt_variable() {
        let template = "Answer this: {{ prompt }}";
        let env = jinja::Environment::new();
        let rendered = env
            .render_str(template, jinja::context!(prompt => "what is 2+2"))
            .unwrap();
        assert_eq!(rendered, "Answer this: what is 2+2");
    }

    #[test]
    fn render_template_preserves_newlines() {
        let template = "Question:\n{{ prompt }}\nAnswer:";
        let env = jinja::Environment::new();
        let rendered = env
            .render_str(template, jinja::context!(prompt => "hello"))
            .unwrap();
        assert_eq!(rendered, "Question:\nhello\nAnswer:");
    }

    #[test]
    fn exact_match_case_sensitive() {
        assert!(is_exact_match("hello", "hello"));
        assert!(!is_exact_match("Hello", "hello"));
    }

    #[test]
    fn exact_match_trims_whitespace() {
        assert!(is_exact_match("  hello  ", "hello"));
        assert!(is_exact_match("hello", "  hello  "));
    }

    #[tokio::test]
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
            "style": "qa",
            "dataset": "fixture/qa",
            "prompt_template_file": template_path.to_str().unwrap(),
            "prompt_column": "q",
            "golden_column": "a",
        }))
        .unwrap();

        let run_id = crate::db::create_run(&db, "test", Some(&input_json))
            .await
            .unwrap();

        let workflow_name = "test".to_owned();
        let builtin = CustomNoCodeBuiltin::new(workflow_name.clone());
        let result = builtin
            .execute(super::BuiltinContext {
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
    async fn execute_records_metrics_and_steps_with_fixture() {
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

        // Initialize workspace with SQLite DB and metrics dir.
        crate::db::init_workspace(root).await.unwrap();
        let db = crate::db::open_workspace(root).await.unwrap();
        let metrics_store =
            crate::metrics_store::MetricsStore::new(crate::db::metrics_dir(root)).unwrap();

        // Write a Jinja template file.
        let template_path = root.join("template.txt");
        std::fs::write(&template_path, "{{ prompt }}\nAnswer:").unwrap();

        // Pre-populate the dataset cache so no network fetch is needed for rows.
        let cache = crate::dataset::cache::DatasetCache::new(cache_dir);
        let rows = vec![
            json!({"question": "what is 2+2", "answer": "4"}),
            json!({"question": "what is 3+3", "answer": "6"}),
        ];
        let key = crate::dataset::cache::cache_key("fixture/qa", "default", "train", None);
        let batch_path = cache.batch_path(&key, 0, 2);
        cache.write_batch(&batch_path, &rows).await.unwrap();

        // Assemble the input JSON that execute() expects.
        let input_json = serde_json::to_string(&json!({
            "style": "qa",
            "dataset": "fixture/qa",
            "model": "random",
            "prompt_template_file": template_path.to_str().unwrap(),
            "prompt_column": "question",
            "golden_column": "answer",
            "limit": 2,
        }))
        .unwrap();

        let run_id = crate::db::create_run(&db, "test_nocode", Some(&input_json))
            .await
            .unwrap();

        let workflow_name = "test_nocode".to_owned();
        let builtin = CustomNoCodeBuiltin::new(workflow_name.clone());
        builtin
            .execute(super::BuiltinContext {
                db: &db,
                metrics_store: &metrics_store,
                run_id,
                workflow_name: &workflow_name,
                input: Some(&input_json),
                quiet: true,
            })
            .await
            .unwrap();

        // Flush buffered metrics to Parquet so we can read them back.
        metrics_store.flush(run_id).await.unwrap();

        // Verify aggregate metrics were written.
        let agg = metrics_store.list_aggregate_for_run(run_id).await.unwrap();
        let names: Vec<&str> = agg.iter().map(|m| m.metric_name.as_str()).collect();
        assert!(names.contains(&"accuracy"));
        assert!(names.contains(&"correct_count"));
        assert!(names.contains(&"total_count"));

        let total_metric = agg.iter().find(|m| m.metric_name == "total_count").unwrap();
        assert_eq!(total_metric.metric_value as i64, 2);

        // Random sampler responses won't match "4" or "6", so correctness is 0.
        let correct_metric = agg
            .iter()
            .find(|m| m.metric_name == "correct_count")
            .unwrap();
        assert_eq!(correct_metric.metric_value as i64, 0);

        // Verify per-step metrics were recorded for both rows.
        let all_metrics = metrics_store.list_for_run(run_id).await.unwrap();
        let is_correct_count = all_metrics
            .iter()
            .filter(|m| m.metric_name == "is_correct")
            .count();
        assert_eq!(is_correct_count, 2);

        // Verify steps were persisted in SQLite.
        let steps = crate::db::list_steps_for_run(&db, run_id).await.unwrap();
        assert_eq!(steps.len(), 2);

        // Execute a second time to verify step caching reuses existing records.
        let builtin2 = CustomNoCodeBuiltin::new(workflow_name.clone());
        builtin2
            .execute(super::BuiltinContext {
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
