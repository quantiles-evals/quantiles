use std::sync::Arc;

use anyhow::{Context, Result, bail};

use crate::builtins::common::{get_max_workers, hash_input, run_timed_step};
use crate::builtins::dataset_runner::DatasetRunner;
use crate::builtins::input::set_builtin_run_input;
use crate::builtins::output::set_builtin_run_output;
use crate::builtins::{BuiltinContext, BuiltinWorkflow};
use crate::dataset::DatasetManager;
use crate::llm::LLMSampler;
use crate::llm::random_label::RandomLabelSampler;

use config::{PubMedQAConfig, RowOutput};
use data::transform_pubmedqa_row;
use eval::{build_prompt, extract_label_from_response};

mod config;
mod data;
mod eval;

/// `PubMedQA` builtin using the quantiles/PubMedQA dataset.
pub struct PubmedqaBuiltin;

#[expect(clippy::too_many_lines)]
#[async_trait::async_trait]
impl BuiltinWorkflow for PubmedqaBuiltin {
    fn name(&self) -> String {
        "pubmedqa".to_string()
    }

    async fn execute(&self, ctx: BuiltinContext<'_>) -> Result<()> {
        let config: PubMedQAConfig = ctx
            .input
            .map(serde_json::from_str)
            .transpose()
            .context("invalid builtin input JSON")?
            .unwrap_or_default();

        if config.base.limit == Some(0) {
            bail!("limit must be > 0");
        }

        let max_workers = config.base.max_workers.unwrap_or_else(get_max_workers);

        let llm: Arc<dyn LLMSampler> = match config.base.model {
            None => Arc::new(RandomLabelSampler::new(&["yes", "no", "maybe"])),
            Some(ref sampler) => sampler.resolve()?,
        };

        let manager = DatasetManager::new()?;
        let dataset_id = "quantiles/PubMedQA";
        let info = manager
            .init(dataset_id, Some("pqa_labeled"), Some("train"), None)
            .await?;

        let total = info
            .total_rows
            .context("could not determine dataset size; pass an explicit limit")?;
        let limit = config.base.limit.unwrap_or(total).min(total);

        set_builtin_run_input(
            ctx.db,
            ctx.run_id,
            config.base.model.as_ref(),
            limit,
            config.base.max_workers,
        )
        .await?;

        let db = ctx.db.clone();
        let model = config.base.model.clone();
        let run_id = ctx.run_id;

        let results = DatasetRunner::new(&manager, dataset_id, &info, limit)
            .desc(self.name())
            .set_quiet(ctx.quiet)
            .for_each_concurrent(max_workers, move |i, row| {
                let llm = Arc::clone(&llm);
                let db = db.clone();
                let model = model.clone();
                async move {
                    let row = transform_pubmedqa_row(&row)
                        .with_context(|| format!("row {i}: invalid row data"))?;

                    let prompt = build_prompt(&row.question, &row.context);
                    let model_str = model
                        .as_ref()
                        .map_or("random_label".to_string(), std::string::ToString::to_string);
                    let input_hash = hash_input(&format!(
                        "{prompt}\nmodel={model_str}\nsampler=pubmedqa-random-label-v1"
                    ));
                    let step_key = format!("eval-{}", row.sample_id);

                    let (output, step_id) = run_timed_step(
                        &db,
                        ctx.metrics_store,
                        run_id,
                        &step_key,
                        &input_hash,
                        async {
                            let model_response = llm
                                .sample(&prompt)
                                .await
                                .with_context(|| format!("failed to sample LLM for row {i}"))?;

                            let prediction = extract_label_from_response(&model_response);
                            let is_correct = prediction.as_ref() == Some(&row.gold_answer);

                            Ok(RowOutput {
                                sample_id: row.sample_id.clone(),
                                question: row.question,
                                context: row.context,
                                gold_answer: row.gold_answer,
                                prediction,
                                is_correct,
                                model_response,
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
