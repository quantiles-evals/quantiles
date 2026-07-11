use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::builtins::common::{
    compute_statistics, extract_text, get_max_workers, hash_input, run_timed_step,
};
use crate::builtins::dataset_runner::DatasetRunner;
use crate::builtins::input::set_builtin_run_input;
use crate::builtins::output::set_builtin_run_output;
use crate::builtins::{BuiltinContext, BuiltinWorkflow};
use crate::dataset::{DatasetManager, resolve_hf_dataset_source};
use crate::llm::LLMSampler;
use crate::llm::random::RandomSampler;
use crate::similarity::{
    SimilarityMetric, SimilarityMetricName, levenshtein::LevenshteinSimilarity,
    vector::CosineSimilarity,
};

/// Configuration shared by all similarity-based builtins.
///
/// All common fields (`limit`, `model`, `max_workers`) live in [`BuiltinConfig`]
/// and are flattened during deserialization so that the TOML surface stays flat.
#[derive(Debug, Default, Deserialize)]
struct SimilarityConfig {
    #[serde(flatten)]
    base: crate::builtins::common::BuiltinConfig,
    /// Similarity metric name. Defaults to `cosine`.
    #[serde(default)]
    metric: SimilarityMetricName,
}

/// Per-row step output stored as JSON in the step record.
#[derive(Debug, Serialize, Deserialize)]
struct RowOutput {
    input: String,
    response: String,
    target: String,
    similarity_name: String,
    similarity_score: f64,
}

/// Parameterised builtin for benchmarks that score LLM responses with a
/// similarity metric.
#[derive(Clone, Copy)]
pub struct SimilarityBenchmark {
    name: &'static str,
    dataset_source: &'static str,
    input_field: &'static str,
    target_field: &'static str,
}

/// `simpleqa-verified` builtin.
pub const SIMPLEQA: SimilarityBenchmark = SimilarityBenchmark {
    name: "simpleqa-verified",
    dataset_source: "hf://quantiles/simpleqa-verified",
    input_field: "problem",
    target_field: "answer",
};

/// `financebench` builtin.
pub const FINANCEBENCH: SimilarityBenchmark = SimilarityBenchmark {
    name: "financebench",
    dataset_source: "hf://quantiles/financebench",
    input_field: "question",
    target_field: "answer",
};

#[expect(clippy::too_many_lines)]
#[async_trait::async_trait]
impl BuiltinWorkflow for SimilarityBenchmark {
    fn name(&self) -> &'static str {
        self.name
    }

    async fn execute(&self, ctx: BuiltinContext<'_>) -> Result<()> {
        let config: SimilarityConfig = ctx
            .input
            .map(serde_json::from_str)
            .transpose()
            .context("invalid builtin input JSON")?
            .unwrap_or_default();

        if config.base.limit == Some(0) {
            bail!("limit must be > 0");
        }

        let metric: Box<dyn SimilarityMetric> = match config.metric {
            SimilarityMetricName::Levenshtein => Box::new(LevenshteinSimilarity),
            SimilarityMetricName::Cosine => Box::new(CosineSimilarity::try_new()?),
        };

        let llm: Arc<dyn LLMSampler> = match config.base.model {
            None => Arc::new(RandomSampler::new(80)),
            Some(ref sampler) => sampler.resolve()?,
        };

        let manager = DatasetManager::new()?;
        let dataset_source = config
            .base
            .dataset
            .as_deref()
            .unwrap_or(self.dataset_source);
        let dataset_id = resolve_hf_dataset_source(dataset_source)?;
        let info = manager.init(dataset_id, None, None, None).await?;

        let total = info
            .total_rows
            .context("could not determine dataset size; pass an explicit limit")?;
        let limit = config.base.limit.unwrap_or(total).min(total);

        let db = ctx.db;
        let run_id = ctx.run_id;

        set_builtin_run_input(
            db,
            run_id,
            dataset_source,
            config.base.model.as_ref(),
            limit,
            config.base.max_workers,
        )
        .await?;

        let metric_name = config.metric.to_string();
        let llm = &llm;
        let metric = &metric;
        let max_workers = config.base.max_workers.unwrap_or_else(get_max_workers);

        let scores = DatasetRunner::new(&manager, dataset_id, &info, limit)
            .desc(self.name)
            .set_quiet(ctx.quiet)
            .for_each_concurrent(max_workers, |i, row| {
                let metric_name = metric_name.clone();
                async move {
                    let input = extract_text(&row, self.input_field)
                        .with_context(|| format!("row {i}: missing '{}'", self.input_field))?;
                    let target = extract_text(&row, self.target_field)
                        .with_context(|| format!("row {i}: missing '{}'", self.target_field))?;

                    let input_hash = hash_input(&input);
                    let step_key = format!("row-{i}");

                    let (output, step_id) = run_timed_step(
                        db,
                        ctx.metrics_store,
                        run_id,
                        &step_key,
                        &input_hash,
                        async {
                            let response = llm
                                .sample(&input)
                                .await
                                .with_context(|| format!("failed to sample LLM for row {i}"))?;

                            let similarity_score =
                                metric.compute(&response, &target).await.with_context(|| {
                                    format!("failed to compute similarity for row {i}")
                                })?;

                            Ok(RowOutput {
                                input: input.clone(),
                                response,
                                target: target.clone(),
                                similarity_name: metric_name.clone(),
                                similarity_score,
                            })
                        },
                    )
                    .await?;

                    if let Some(step_id) = step_id {
                        ctx.metrics_store
                            .emit(
                                ctx.run_id,
                                Some(step_id),
                                "similarity_score",
                                output.similarity_score,
                                None,
                            )
                            .await;
                    }

                    Ok(output.similarity_score)
                }
            })
            .await?;

        // Emit aggregate metrics
        if !scores.is_empty() {
            let stats = compute_statistics(&scores);

            ctx.metrics_store
                .emit(ctx.run_id, None, "mean_similarity", stats.mean, None)
                .await;
            ctx.metrics_store
                .emit(ctx.run_id, None, "stdev_similarity", stats.std, None)
                .await;
            ctx.metrics_store
                .emit(
                    ctx.run_id,
                    None,
                    "variance_similarity",
                    stats.variance,
                    None,
                )
                .await;
            ctx.metrics_store
                .emit(ctx.run_id, None, "median_similarity", stats.median, None)
                .await;
            ctx.metrics_store
                .emit(ctx.run_id, None, "min_similarity", stats.min, None)
                .await;
            ctx.metrics_store
                .emit(ctx.run_id, None, "max_similarity", stats.max, None)
                .await;
            ctx.metrics_store
                .emit(ctx.run_id, None, "p99_similarity", stats.p99, None)
                .await;
            ctx.metrics_store
                .emit(ctx.run_id, None, "p95_similarity", stats.p95, None)
                .await;
        }

        set_builtin_run_output(ctx.db, ctx.run_id, scores.len()).await?;

        Ok(())
    }
}
