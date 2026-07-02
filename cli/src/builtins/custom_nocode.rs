use std::sync::Arc;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::builtins::common::{get_max_workers, extract_text, hash_input, run_timed_step};
use crate::builtins::dataset_runner::DatasetRunner;
use crate::builtins::input::set_builtin_run_input;
use crate::builtins::output::set_builtin_run_output;
use crate::builtins::{BuiltinContext, BuiltinWorkflow};
use crate::dataset::DatasetManager;
use crate::llm::LLMSampler;
use crate::llm::random::RandomSampler;

/// Input deserialized from the JSON assembled by `commands::run`.
#[derive(Debug, Default, Deserialize)]
struct CustomNoCodeInput {
    style: String,
    dataset: String,
    #[serde(default)]
    model: Option<crate::llm::Sampler>,
    prompt_template_file: String,
    prompt_column: String,
    golden_column: String,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    max_workers: Option<usize>,
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
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait::async_trait]
impl BuiltinWorkflow for CustomNoCodeBuiltin {
    fn name(&self) -> String {
        self.name.clone()
    }

    #[expect(clippy::too_many_lines)]
    async fn execute(&self, ctx: BuiltinContext<'_>) -> Result<()> {
        let config: CustomNoCodeInput = ctx
            .input
            .map(serde_json::from_str)
            .transpose()
            .context("invalid builtin input JSON")?
            .unwrap_or_default();

        if config.style != "qa" {
            bail!(
                "unsupported custom_nocode style `{}`; only `qa` is supported",
                config.style
            );
        }

        if config.limit == Some(0) {
            bail!("limit must be > 0");
        }

        // Load template file and validate syntax.
        let template_str = std::fs::read_to_string(&config.prompt_template_file)
            .with_context(|| {
                format!(
                    "failed to read prompt template file `{}`",
                    config.prompt_template_file
                )
            })?;
        let env = jinja::Environment::new();
        env.render_str(&template_str, jinja::context!(prompt => ""))
            .with_context(|| {
                format!(
                    "invalid jinja syntax in prompt template file `{}`",
                    config.prompt_template_file
                )
            })?;

        let max_workers = config.max_workers.unwrap_or_else(get_max_workers);

        let llm: Arc<dyn LLMSampler> = match config.model {
            None => Arc::new(RandomSampler::new(80)),
            Some(ref sampler) => sampler.resolve()?,
        };

        let manager = DatasetManager::new()?;
        let info = manager
            .init(&config.dataset, None, None, None)
            .await?;

        let total = info
            .total_rows
            .context("could not determine dataset size; pass an explicit limit")?;
        let limit = config.limit.unwrap_or(total).min(total);

        set_builtin_run_input(
            ctx.db,
            ctx.run_id,
            config.model.as_ref(),
            limit,
            config.max_workers,
        )
        .await?;

        let db = ctx.db.clone();
        let model = config.model.clone();
        let run_id = ctx.run_id;
        let dataset = config.dataset.clone();
        let prompt_column = config.prompt_column.clone();
        let golden_column = config.golden_column.clone();
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
                    let prompt = extract_text(&row, &prompt_column)
                        .with_context(|| {
                            format!("row {i}: missing prompt column `{prompt_column}`")
                        })?;
                    let golden = extract_text(&row, &golden_column)
                        .with_context(|| {
                            format!("row {i}: missing golden column `{golden_column}`")
                        })?;

                    let rendered = env
                        .render_str(&template_str, jinja::context!(prompt => &prompt))
                        .with_context(|| {
                            format!("row {i}: failed to render prompt template")
                        })?;

                    let model_str = model
                        .as_ref()
                        .map_or("random".to_string(), std::string::ToString::to_string);
                    let input_hash = hash_input(&format!(
                        "{rendered}\nmodel={model_str}\nworkflow={}"
                        , self.name()));
                    let step_key = format!("row-{i}");

                    let (output, step_id) = run_timed_step(
                        &db,
                        ctx.metrics_store,
                        run_id,
                        &step_key,
                        &input_hash,
                        async {
                            let model_response = llm
                                .sample(&rendered)
                                .await
                                .with_context(|| {
                                    format!("failed to sample LLM for row {i}")
                                })?;

                            let is_correct = is_exact_match(&model_response, &golden);

                            Ok(RowOutput {
                                input: rendered.clone(),
                                response: model_response,
                                golden,
                                is_correct,
                            })
                        },
                    )
                    .await?;

                    if let Some(step_id) = step_id {
                        ctx.metrics_store
                            .emit(
                                ctx.run_id,
                                Some(step_id),
                                "is_correct",
                                if output.is_correct { 1.0 } else { 0.0 },
                                None,
                            )
                            .await;
                    }

                    Ok::<_, anyhow::Error>(output.is_correct)
                }
            })
            .await?;

        let mut correct_count: usize = 0;
        let total_count = results.len();
        for is_correct in results {
            if is_correct {
                correct_count += 1;
            }
        }

        #[expect(clippy::cast_precision_loss)]
        if total_count > 0 {
            let accuracy = correct_count as f64 / total_count as f64;

            ctx.metrics_store
                .emit(ctx.run_id, None, "accuracy", accuracy, None)
                .await;
            ctx.metrics_store
                .emit(
                    ctx.run_id,
                    None,
                    "correct_count",
                    correct_count as f64,
                    None,
                )
                .await;
            ctx.metrics_store
                .emit(ctx.run_id, None, "total_count", total_count as f64, None)
                .await;
        }

        set_builtin_run_output(ctx.db, ctx.run_id, total_count).await?;

        Ok(())
    }
}

/// Case-sensitive exact-match comparison after trimming whitespace.
fn is_exact_match(response: &str, golden: &str) -> bool {
    response.trim() == golden.trim()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
