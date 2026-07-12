use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::builtins::common::{
    compute_statistics, emit_accuracy_metrics, get_max_workers, hash_input, resolve_sampler,
    run_timed_step,
};
use crate::builtins::dataset_runner::DatasetRunner;
use crate::builtins::input::set_builtin_run_input;
use crate::builtins::output::set_builtin_run_output;
use crate::builtins::{BuiltinContext, BuiltinWorkflow};
use crate::dataset::DatasetManager;
use crate::llm::random::RandomSampler;
use crate::llm::random_label::RandomLabelSampler;

/// Input deserialized from the JSON assembled by `commands::run`.
#[derive(Debug, Deserialize)]
struct CustomNoCodeInput {
    dataset: crate::config::CustomNoCodeDatasetConfig,
    #[serde(default)]
    model: Option<crate::llm::Sampler>,
    limit: Option<usize>,
    max_workers: Option<usize>,
    prompt_template_file: String,
    style: crate::config::CustomNoCodeStyleConfig,
}

/// Per-row step output stored as JSON in the step record.
#[derive(Debug, Serialize, Deserialize)]
struct RowOutput {
    input: String,
    response: String,
    parsed_response: Option<String>,
    golden: String,
    is_correct: bool,
}

#[derive(Clone, Debug, Serialize)]
struct PromptChoice {
    label: String,
    text: String,
}

struct PreparedRow {
    choices: Vec<PromptChoice>,
    golden: String,
}

#[derive(Clone, Copy, Debug)]
struct ExactMatchSampleResultParams {
    is_correct: bool,
}

#[derive(Clone, Debug)]
struct MultipleChoiceSampleResultParams {
    is_correct: bool,
    golden_label: String,
    /// The configured label parsed from the response, or `None` when parsing fails.
    /// Unparsed predictions show up in the 'unparsed' column in the confusion matrix,
    /// and surface accordingly in classification metrics.
    predicted_label: Option<String>,
}

#[derive(Clone, Debug)]
enum SampleResult {
    ExactMatch(ExactMatchSampleResultParams),
    MultipleChoice(MultipleChoiceSampleResultParams),
}

impl SampleResult {
    const fn is_correct(&self) -> bool {
        match self {
            Self::ExactMatch(params) => params.is_correct,
            Self::MultipleChoice(params) => params.is_correct,
        }
    }

    const fn response_parsed(&self) -> bool {
        match self {
            Self::ExactMatch(_) => true,
            Self::MultipleChoice(params) => params.predicted_label.is_some(),
        }
    }
}

/// Arguments for the [`CustomNoCodeBuiltin::evaluate_row`] method
struct EvaluateRowArgs<'a> {
    /// The row index
    i: usize,
    /// The row value
    row: &'a serde_json::Value,
    /// Exact-match or multiple-choice task configuration.
    style: &'a crate::config::CustomNoCodeStyleConfig,
    /// The prompt template
    template_str: &'a str,
    /// The pre-constructed jinja template env
    env: &'a jinja::Environment<'a>,
    /// The name of the model we're sampling (used for cache key hashing only)
    model_name: &'a str,
    /// The actual model to sample
    llm: &'a std::sync::Arc<dyn crate::llm::LLMSampler>,
    /// The connection to the metadata DB
    db: &'a sea_orm::DatabaseConnection,
    /// Metrics storage
    metrics_store: &'a crate::metrics_store::MetricsStore,
    /// The run ID
    run_id: i64,
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

    async fn evaluate_row(&self, args: EvaluateRowArgs<'_>) -> Result<SampleResult> {
        let prepared = prepare_row(args.i, args.row, args.style)?;

        let rendered = args
            .env
            .render_str(
                args.template_str,
                jinja::context!(row => args.row, choices => &prepared.choices),
            )
            .with_context(|| format!("row {}: failed to render prompt template", args.i))?;

        let input_hash = hash_input(&format!(
            "{rendered}\ngolden={}\nmodel={}\nworkflow={}",
            prepared.golden,
            args.model_name,
            self.name()
        ));
        let step_key = format!("row-{}", args.i);

        let (output, step_id) = run_timed_step(
            args.db,
            args.metrics_store,
            args.run_id,
            &step_key,
            &input_hash,
            async {
                let model_response = args
                    .llm
                    .sample(&rendered)
                    .await
                    .with_context(|| format!("failed to sample LLM for row {}", args.i))?;

                let parsed_response = match args.style {
                    crate::config::CustomNoCodeStyleConfig::MultipleChoice {
                        choice_labels,
                        ..
                    } => extract_choice_label(&model_response, choice_labels),
                    crate::config::CustomNoCodeStyleConfig::ExactMatch { .. } => {
                        Some(model_response.trim().to_owned())
                    }
                };
                let is_correct = parsed_response.as_deref() == Some(prepared.golden.trim());

                Ok(RowOutput {
                    input: rendered.clone(),
                    response: model_response,
                    parsed_response,
                    golden: prepared.golden,
                    is_correct,
                })
            },
        )
        .await?;

        if let Some(step_id) = step_id {
            args.metrics_store
                .emit(
                    args.run_id,
                    Some(step_id),
                    "is_correct",
                    if output.is_correct { 1.0 } else { 0.0 },
                    None,
                )
                .await;
            args.metrics_store
                .emit(
                    args.run_id,
                    Some(step_id),
                    "response_parsed",
                    if output.parsed_response.is_some() {
                        1.0
                    } else {
                        0.0
                    },
                    None,
                )
                .await;
        }

        Ok(match args.style {
            crate::config::CustomNoCodeStyleConfig::ExactMatch { .. } => {
                SampleResult::ExactMatch(ExactMatchSampleResultParams {
                    is_correct: output.is_correct,
                })
            }
            crate::config::CustomNoCodeStyleConfig::MultipleChoice { .. } => {
                SampleResult::MultipleChoice(MultipleChoiceSampleResultParams {
                    is_correct: output.is_correct,
                    golden_label: output.golden,
                    predicted_label: output.parsed_response,
                })
            }
        })
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
        let llm = resolve_custom_nocode_sampler(config.model.as_ref(), &config.style)?;

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
            .for_each_concurrent(max_workers, move |i, row| {
                let llm = Arc::clone(&llm);
                let db = db.clone();
                let model_name = model_name.clone();
                let template_str = Arc::clone(&template_str);
                let style = Arc::clone(&style);
                let env = env.clone();
                async move {
                    let args = EvaluateRowArgs {
                        i,
                        row: &row,
                        style: &style,
                        template_str: &template_str,
                        env: &env,
                        model_name: &model_name,
                        llm: &llm,
                        db: &db,
                        metrics_store: ctx.metrics_store,
                        run_id,
                    };
                    self.evaluate_row(args).await
                }
            })
            .await?;

        let total_count = results.len();
        emit_custom_nocode_aggregate_metrics(
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

#[expect(clippy::cast_precision_loss)]
async fn emit_custom_nocode_aggregate_metrics(
    metrics_store: &crate::metrics_store::MetricsStore,
    run_id: i64,
    results: &[SampleResult],
    choice_labels: Option<&[String]>,
) -> Result<()> {
    if results.is_empty() {
        return Ok(());
    }

    emit_accuracy_metrics(
        metrics_store,
        run_id,
        results.iter().map(SampleResult::is_correct),
    )
    .await;

    let incorrect_count = results.iter().filter(|result| !result.is_correct()).count();
    let parsed_response_count = results
        .iter()
        .filter(|result| result.response_parsed())
        .count();
    let unparsed_response_count = results.len() - parsed_response_count;
    let parse_rate = parsed_response_count as f64 / results.len() as f64;

    metrics_store
        .emit(
            run_id,
            None,
            "incorrect_count",
            incorrect_count as f64,
            None,
        )
        .await;
    metrics_store
        .emit(
            run_id,
            None,
            "parsed_response_count",
            parsed_response_count as f64,
            None,
        )
        .await;
    metrics_store
        .emit(
            run_id,
            None,
            "unparsed_response_count",
            unparsed_response_count as f64,
            None,
        )
        .await;
    metrics_store
        .emit(run_id, None, "parse_rate", parse_rate, None)
        .await;

    if let Some(choice_labels) = choice_labels {
        let multiple_choice_results = results
            .iter()
            .map(|result| match result {
                SampleResult::MultipleChoice(params) => Ok(params.clone()),
                SampleResult::ExactMatch(_) => {
                    bail!("multiple-choice aggregate received an exact-match sample")
                }
            })
            .collect::<Result<Vec<_>>>()?;
        emit_multiple_choice_aggregate_metrics(
            metrics_store,
            run_id,
            &multiple_choice_results,
            choice_labels,
        )
        .await?;
    }

    let latency_values = metrics_store
        .sample_metric_values(run_id, "latency_ms")
        .await?;
    if latency_values.is_empty() {
        return Ok(());
    }
    let stats = compute_statistics(&latency_values);
    for (metric_name, metric_value) in [
        ("mean_latency_ms", stats.mean),
        ("median_latency_ms", stats.median),
        ("p95_latency_ms", stats.p95),
        ("p99_latency_ms", stats.p99),
        ("min_latency_ms", stats.min),
        ("max_latency_ms", stats.max),
    ] {
        metrics_store
            .emit(run_id, None, metric_name, metric_value, Some("ms"))
            .await;
    }

    Ok(())
}

fn build_confusion_matrix(
    results: &[MultipleChoiceSampleResultParams],
    choice_labels: &[String],
) -> Result<Vec<Vec<usize>>> {
    if choice_labels.is_empty() {
        bail!("multiple-choice aggregate requires at least one choice label");
    }
    let label_indices: HashMap<&str, usize> = choice_labels
        .iter()
        .enumerate()
        .map(|(index, label)| (label.as_str(), index))
        .collect();
    let unparsed_index = choice_labels.len();
    let mut matrix = vec![vec![0usize; choice_labels.len() + 1]; choice_labels.len()];

    for result in results {
        let golden_index = *label_indices
            .get(result.golden_label.as_str())
            .with_context(|| format!("unknown golden choice label `{}`", result.golden_label))?;
        let predicted_index = result
            .predicted_label
            .as_deref()
            .map(|label| {
                label_indices
                    .get(label)
                    .copied()
                    .with_context(|| format!("unknown predicted choice label `{label}`"))
            })
            .transpose()?
            .unwrap_or(unparsed_index);
        matrix[golden_index][predicted_index] += 1;
    }

    Ok(matrix)
}

#[expect(clippy::cast_precision_loss)]
async fn emit_multiple_choice_aggregate_metrics(
    metrics_store: &crate::metrics_store::MetricsStore,
    run_id: i64,
    results: &[MultipleChoiceSampleResultParams],
    choice_labels: &[String],
) -> Result<()> {
    let confusion_matrix = build_confusion_matrix(results, choice_labels)?;
    let unparsed_index = choice_labels.len();
    let total = results.len() as f64;
    let mut macro_precision = 0.0;
    let mut macro_recall = 0.0;
    let mut macro_f1 = 0.0;
    let mut weighted_precision = 0.0;
    let mut weighted_recall = 0.0;
    let mut weighted_f1 = 0.0;

    for label_index in 0..choice_labels.len() {
        let true_positive = confusion_matrix[label_index][label_index] as f64;
        let false_positive = confusion_matrix
            .iter()
            .enumerate()
            .filter(|(golden_index, _)| *golden_index != label_index)
            .map(|(_, row)| row[label_index])
            .sum::<usize>() as f64;
        let support = confusion_matrix[label_index].iter().sum::<usize>() as f64;
        let false_negative = support - true_positive;
        let precision = safe_ratio(true_positive, true_positive + false_positive);
        let recall = safe_ratio(true_positive, true_positive + false_negative);
        let f1 = safe_ratio(2.0 * precision * recall, precision + recall);
        let weight = support / total;

        macro_precision += precision;
        macro_recall += recall;
        macro_f1 += f1;
        weighted_precision += precision * weight;
        weighted_recall += recall * weight;
        weighted_f1 += f1 * weight;

        for (metric_name, metric_value) in [
            (format!("precision_label_{label_index}"), precision),
            (format!("recall_label_{label_index}"), recall),
            (format!("f1_label_{label_index}"), f1),
            (format!("support_label_{label_index}"), support),
        ] {
            metrics_store
                .emit(run_id, None, &metric_name, metric_value, None)
                .await;
        }

        for (predicted_index, count) in confusion_matrix[label_index]
            .iter()
            .take(choice_labels.len())
            .enumerate()
        {
            metrics_store
                .emit(
                    run_id,
                    None,
                    &format!("confusion_matrix_{label_index}_{predicted_index}"),
                    *count as f64,
                    None,
                )
                .await;
        }
        metrics_store
            .emit(
                run_id,
                None,
                &format!("confusion_matrix_{label_index}_unparsed"),
                confusion_matrix[label_index][unparsed_index] as f64,
                None,
            )
            .await;
    }

    let label_count = choice_labels.len() as f64;
    for (metric_name, metric_value) in [
        ("macro_precision", macro_precision / label_count),
        ("macro_recall", macro_recall / label_count),
        ("macro_f1", macro_f1 / label_count),
        ("weighted_precision", weighted_precision),
        ("weighted_recall", weighted_recall),
        ("weighted_f1", weighted_f1),
    ] {
        metrics_store
            .emit(run_id, None, metric_name, metric_value, None)
            .await;
    }

    Ok(())
}

fn safe_ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator.abs() < f64::EPSILON {
        0.0
    } else {
        numerator / denominator
    }
}

fn resolve_custom_nocode_sampler(
    model: Option<&crate::llm::Sampler>,
    style: &crate::config::CustomNoCodeStyleConfig,
) -> Result<Arc<dyn crate::llm::LLMSampler>> {
    if model.is_none_or(|sampler| matches!(sampler, crate::llm::Sampler::Random))
        && let crate::config::CustomNoCodeStyleConfig::MultipleChoice { choice_labels, .. } = style
    {
        return Ok(Arc::new(RandomLabelSampler::new(choice_labels)));
    }

    resolve_sampler(model, || Arc::new(RandomSampler::new(80)))
}

fn parse_input(input: Option<&str>) -> Result<CustomNoCodeInput> {
    let config: CustomNoCodeInput = input
        .map(serde_json::from_str)
        .transpose()
        .context("invalid builtin input JSON")?
        .context("custom_nocode benchmark requires input configuration")?;

    if config.limit == Some(0) {
        bail!("limit must be > 0");
    }

    Ok(config)
}

fn load_template(path: &str) -> Result<(String, jinja::Environment<'_>)> {
    let template_str = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read prompt template file `{path}`"))?;
    let env = jinja::Environment::new();
    env.render_str(
        &template_str,
        jinja::context!(row => serde_json::json!({}), choices => Vec::<PromptChoice>::new()),
    )
    .with_context(|| format!("invalid jinja syntax in prompt template file `{path}`"))?;
    Ok((template_str, env))
}

async fn resolve_dataset_limit(
    dataset: &str,
    dataset_config: Option<&str>,
    split: Option<&str>,
    revision: Option<&str>,
    limit: Option<usize>,
) -> Result<(DatasetManager, crate::dataset::DatasetInfo, usize)> {
    let manager = DatasetManager::new()?;
    let info = manager
        .init(dataset, dataset_config, split, revision)
        .await?;

    let total = info
        .total_rows
        .context("could not determine dataset size; pass an explicit limit")?;
    let limit = limit.unwrap_or(total).min(total);

    Ok((manager, info, limit))
}

fn prepare_row(
    row_index: usize,
    row: &serde_json::Value,
    style: &crate::config::CustomNoCodeStyleConfig,
) -> Result<PreparedRow> {
    match style {
        crate::config::CustomNoCodeStyleConfig::ExactMatch { golden_column } => {
            let golden = extract_scalar(row, golden_column).with_context(|| {
                format!("row {row_index}: missing golden column `{golden_column}`")
            })?;
            Ok(PreparedRow {
                choices: Vec::new(),
                golden,
            })
        }
        crate::config::CustomNoCodeStyleConfig::MultipleChoice {
            choices: choice_source,
            answer,
            choice_labels,
            shuffle,
        } => {
            let choice_values = extract_choices(row_index, row, choice_source, choice_labels)?;
            if choice_values.is_empty() || choice_values.len() > choice_labels.len() {
                bail!(
                    "row {row_index}: found {} choices but configured only {} labels",
                    choice_values.len(),
                    choice_labels.len()
                );
            }
            let labels = &choice_labels[..choice_values.len()];

            let correct_index = resolve_correct_index(
                row_index,
                row,
                choice_source,
                answer,
                labels,
                &choice_values,
            )?;
            let mut indexed_choices: Vec<(String, bool)> = choice_values
                .into_iter()
                .enumerate()
                .map(|(index, text)| (text, index == correct_index))
                .collect();

            if let Some(shuffle) = shuffle {
                let seed = extract_scalar(row, &shuffle.seed_column).with_context(|| {
                    format!(
                        "row {row_index}: missing shuffle seed column `{}`",
                        shuffle.seed_column
                    )
                })?;
                deterministic_shuffle(&mut indexed_choices, &seed);
            }

            let mut golden = None;
            let choices = indexed_choices
                .into_iter()
                .zip(labels)
                .map(|((text, is_correct), label)| {
                    if is_correct {
                        golden = Some(label.clone());
                    }
                    PromptChoice {
                        label: label.clone(),
                        text,
                    }
                })
                .collect();

            Ok(PreparedRow {
                choices,
                golden: golden.context("correct choice disappeared while assigning labels")?,
            })
        }
    }
}

fn extract_choices(
    row_index: usize,
    row: &serde_json::Value,
    source: &crate::config::CustomNoCodeChoiceSource,
    labels: &[String],
) -> Result<Vec<String>> {
    match source {
        crate::config::CustomNoCodeChoiceSource::Columns(
            crate::config::CustomNoCodeChoiceColumns { columns },
        ) => columns
            .iter()
            .map(|column| {
                extract_scalar(row, column)
                    .with_context(|| format!("row {row_index}: missing choice column `{column}`"))
            })
            .collect(),
        crate::config::CustomNoCodeChoiceSource::Column(
            crate::config::CustomNoCodeChoiceColumn { column },
        ) => {
            let value = row
                .get(column)
                .with_context(|| format!("row {row_index}: missing choices column `{column}`"))?;
            let owned;
            let value = if let Some(encoded) = value.as_str() {
                owned = serde_json::from_str(encoded).unwrap_or_else(|_| value.clone());
                &owned
            } else {
                value
            };

            match value {
                serde_json::Value::Array(values) => values
                    .iter()
                    .map(value_to_scalar)
                    .collect::<Option<Vec<_>>>()
                    .with_context(|| format!("row {row_index}: choices column `{column}` contains non-scalar values")),
                serde_json::Value::Object(values) => labels
                    .iter()
                    .map(|label| {
                        values.get(label).and_then(value_to_scalar).with_context(|| {
                            format!("row {row_index}: choices object `{column}` has no scalar `{label}` value")
                        })
                    })
                    .collect(),
                _ => bail!("row {row_index}: choices column `{column}` must be an array or object"),
            }
        }
    }
}

fn resolve_correct_index(
    row_index: usize,
    row: &serde_json::Value,
    choice_source: &crate::config::CustomNoCodeChoiceSource,
    answer: &crate::config::CustomNoCodeAnswerSource,
    labels: &[String],
    choices: &[String],
) -> Result<usize> {
    match answer {
        crate::config::CustomNoCodeAnswerSource::IndexColumn(
            crate::config::CustomNoCodeIndexAnswer {
                index_column: column,
                index_base,
            },
        ) => {
            let raw = row.get(column).with_context(|| {
                format!("row {row_index}: missing golden index column `{column}`")
            })?;
            let index = raw
                .as_u64()
                .or_else(|| raw.as_str().and_then(|value| value.parse().ok()))
                .with_context(|| {
                    format!("row {row_index}: golden index column `{column}` is not an integer")
                })?;
            let index = usize::try_from(index)
                .ok()
                .and_then(|index| index.checked_sub(*index_base))
                .with_context(|| {
                    format!("row {row_index}: golden index is below configured base")
                })?;
            if index >= choices.len() {
                bail!("row {row_index}: golden index {index} is outside the choice list");
            }
            Ok(index)
        }
        crate::config::CustomNoCodeAnswerSource::CorrectChoiceColumn(
            crate::config::CustomNoCodeCorrectChoiceAnswer {
                correct_choice_column: column,
            },
        ) => {
            let crate::config::CustomNoCodeChoiceSource::Columns(
                crate::config::CustomNoCodeChoiceColumns { columns },
            ) = choice_source
            else {
                bail!("correct-choice answer requires column-backed choices");
            };
            columns
                .iter()
                .position(|candidate| candidate == column)
                .context("validated correct choice is absent from choice columns")
        }
        crate::config::CustomNoCodeAnswerSource::LabelColumn(
            crate::config::CustomNoCodeLabelAnswer {
                label_column: column,
            },
        ) => {
            let golden = extract_scalar(row, column)
                .with_context(|| format!("row {row_index}: missing golden column `{column}`"))?;
            labels
                .iter()
                .position(|label| label.eq_ignore_ascii_case(golden.trim()))
                .with_context(|| {
                    format!("row {row_index}: golden label `{golden}` is not in `choice_labels`")
                })
        }
    }
}

fn extract_scalar(row: &serde_json::Value, key: &str) -> Option<String> {
    row.get(key).and_then(value_to_scalar)
}

fn value_to_scalar(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn deterministic_shuffle<T>(values: &mut [T], seed: &str) {
    for upper in (1..values.len()).rev() {
        let hash = hash_input(&format!("custom-nocode-choice-shuffle-v1:{seed}:{upper}"));
        let random = u64::from_str_radix(&hash, 16).unwrap_or_default();
        let index =
            usize::try_from(random % u64::try_from(upper + 1).unwrap_or(1)).unwrap_or_default();
        values.swap(upper, index);
    }
}

fn extract_choice_label(response: &str, labels: &[String]) -> Option<String> {
    let trimmed = response.trim();
    if let Some(label) = labels
        .iter()
        .find(|label| label.eq_ignore_ascii_case(trimmed))
    {
        return Some(label.clone());
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    for token in tokens.iter().rev().take(8) {
        let cleaned = token.trim_matches(|character: char| !character.is_ascii_alphanumeric());
        if let Some(label) = labels
            .iter()
            .find(|label| label.eq_ignore_ascii_case(cleaned))
        {
            return Some(label.clone());
        }
    }
    None
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
    fn render_template_with_row_variable() {
        let template = "Answer this: {{ row.question }}";
        let env = jinja::Environment::new();
        let rendered = env
            .render_str(
                template,
                jinja::context!(row => json!({"question": "what is 2+2"})),
            )
            .unwrap();
        assert_eq!(rendered, "Answer this: what is 2+2");
    }

    #[test]
    fn render_template_preserves_newlines() {
        let template = "Question:\n{{ row.question }}\nAnswer:";
        let env = jinja::Environment::new();
        let rendered = env
            .render_str(
                template,
                jinja::context!(row => json!({"question": "hello"})),
            )
            .unwrap();
        assert_eq!(rendered, "Question:\nhello\nAnswer:");
    }

    #[test]
    fn extracts_multiple_choice_labels() {
        let labels = vec!["A".to_owned(), "B".to_owned(), "C".to_owned()];
        assert_eq!(extract_choice_label("B", &labels).as_deref(), Some("B"));
        assert_eq!(
            extract_choice_label("The answer is (c).", &labels).as_deref(),
            Some("C")
        );
        assert_eq!(extract_choice_label("unknown", &labels), None);
    }

    #[tokio::test]
    async fn multiple_choice_random_sampler_uses_configured_labels() {
        let style = multiple_choice_config(
            crate::config::CustomNoCodeChoiceSource::Column(
                crate::config::CustomNoCodeChoiceColumn {
                    column: "options".to_owned(),
                },
            ),
            crate::config::CustomNoCodeAnswerSource::LabelColumn(
                crate::config::CustomNoCodeLabelAnswer {
                    label_column: "answer".to_owned(),
                },
            ),
        );
        let configured_random = crate::llm::Sampler::Random;

        for model in [None, Some(&configured_random)] {
            let sampler = resolve_custom_nocode_sampler(model, &style).unwrap();
            for _ in 0..100 {
                let response = sampler.sample("ignored prompt").await.unwrap();
                assert!(matches!(response.as_str(), "A" | "B" | "C" | "D"));
            }
        }
    }

    #[tokio::test]
    async fn exact_match_random_sampler_remains_alphanumeric() {
        let style = crate::config::CustomNoCodeStyleConfig::ExactMatch {
            golden_column: "answer".to_owned(),
        };
        let sampler =
            resolve_custom_nocode_sampler(Some(&crate::llm::Sampler::Random), &style).unwrap();

        for _ in 0..100 {
            let response = sampler.sample("ignored prompt").await.unwrap();
            assert!(!response.is_empty());
            assert!(response.len() <= 80);
            assert!(
                response
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
            );
        }
    }

    #[tokio::test]
    async fn emits_extended_aggregate_metrics() {
        let tmpdir = tempfile::tempdir().unwrap();
        let metrics_store =
            crate::metrics_store::MetricsStore::new(tmpdir.path().join("metrics")).unwrap();
        let run_id = 17;
        for (step_id, latency) in [(1, 10.0), (2, 20.0), (3, 30.0)] {
            metrics_store
                .emit(run_id, Some(step_id), "latency_ms", latency, Some("ms"))
                .await;
        }
        let results = [
            SampleResult::MultipleChoice(MultipleChoiceSampleResultParams {
                is_correct: true,
                golden_label: "A".to_owned(),
                predicted_label: Some("A".to_owned()),
            }),
            SampleResult::MultipleChoice(MultipleChoiceSampleResultParams {
                is_correct: false,
                golden_label: "A".to_owned(),
                predicted_label: Some("B".to_owned()),
            }),
            SampleResult::MultipleChoice(MultipleChoiceSampleResultParams {
                is_correct: false,
                golden_label: "B".to_owned(),
                predicted_label: None,
            }),
        ];
        let choice_labels = ["A".to_owned(), "B".to_owned()];

        emit_custom_nocode_aggregate_metrics(
            &metrics_store,
            run_id,
            &results,
            Some(&choice_labels),
        )
        .await
        .unwrap();
        metrics_store.flush(run_id).await.unwrap();
        let metrics = metrics_store.list_aggregate_for_run(run_id).await.unwrap();
        let value = |name: &str| {
            metrics
                .iter()
                .find(|metric| metric.metric_name == name)
                .unwrap()
                .metric_value
        };
        let assert_metric = |name: &str, expected: f64| {
            let actual = value(name);
            assert!(
                (actual - expected).abs() < 1e-12,
                "expected {name}={expected}, got {actual}"
            );
        };

        assert_metric("accuracy", 1.0 / 3.0);
        assert_metric("correct_count", 1.0);
        assert_metric("incorrect_count", 2.0);
        assert_metric("total_count", 3.0);
        assert_metric("parsed_response_count", 2.0);
        assert_metric("unparsed_response_count", 1.0);
        assert_metric("parse_rate", 2.0 / 3.0);
        assert_metric("mean_latency_ms", 20.0);
        assert_metric("median_latency_ms", 20.0);
        assert_metric("p95_latency_ms", 29.0);
        assert_metric("p99_latency_ms", 29.8);
        assert_metric("min_latency_ms", 10.0);
        assert_metric("max_latency_ms", 30.0);
        assert_metric("precision_label_0", 1.0);
        assert_metric("recall_label_0", 0.5);
        assert_metric("f1_label_0", 2.0 / 3.0);
        assert_metric("support_label_0", 2.0);
        assert_metric("precision_label_1", 0.0);
        assert_metric("recall_label_1", 0.0);
        assert_metric("f1_label_1", 0.0);
        assert_metric("support_label_1", 1.0);
        assert_metric("macro_precision", 0.5);
        assert_metric("macro_recall", 0.25);
        assert_metric("macro_f1", 1.0 / 3.0);
        assert_metric("weighted_precision", 2.0 / 3.0);
        assert_metric("weighted_recall", 1.0 / 3.0);
        assert_metric("weighted_f1", 4.0 / 9.0);
        assert_metric("confusion_matrix_0_0", 1.0);
        assert_metric("confusion_matrix_0_1", 1.0);
        assert_metric("confusion_matrix_0_unparsed", 0.0);
        assert_metric("confusion_matrix_1_0", 0.0);
        assert_metric("confusion_matrix_1_1", 0.0);
        assert_metric("confusion_matrix_1_unparsed", 1.0);
    }

    fn multiple_choice_config(
        choices: crate::config::CustomNoCodeChoiceSource,
        answer: crate::config::CustomNoCodeAnswerSource,
    ) -> crate::config::CustomNoCodeStyleConfig {
        crate::config::CustomNoCodeStyleConfig::MultipleChoice {
            choices,
            answer,
            choice_labels: ["A", "B", "C", "D"].map(str::to_owned).to_vec(),
            shuffle: None,
        }
    }

    #[test]
    fn prepares_medqa_object_choices() {
        let config = multiple_choice_config(
            crate::config::CustomNoCodeChoiceSource::Column(
                crate::config::CustomNoCodeChoiceColumn {
                    column: "options".to_owned(),
                },
            ),
            crate::config::CustomNoCodeAnswerSource::LabelColumn(
                crate::config::CustomNoCodeLabelAnswer {
                    label_column: "answer_idx".to_owned(),
                },
            ),
        );
        let row = json!({
            "options": {"A": "alpha", "B": "beta", "C": "gamma", "D": "delta"},
            "answer_idx": "C"
        });

        let prepared = prepare_row(0, &row, &config).unwrap();
        assert_eq!(prepared.golden, "C");
        assert_eq!(prepared.choices[2].text, "gamma");
    }

    #[test]
    fn prepares_mmlu_pro_array_choices() {
        let config = multiple_choice_config(
            crate::config::CustomNoCodeChoiceSource::Column(
                crate::config::CustomNoCodeChoiceColumn {
                    column: "options".to_owned(),
                },
            ),
            crate::config::CustomNoCodeAnswerSource::LabelColumn(
                crate::config::CustomNoCodeLabelAnswer {
                    label_column: "answer".to_owned(),
                },
            ),
        );
        let row = json!({"options": ["alpha", "beta", "gamma"], "answer": "B"});

        let prepared = prepare_row(0, &row, &config).unwrap();
        assert_eq!(prepared.golden, "B");
        assert_eq!(prepared.choices[1].text, "beta");
    }

    #[test]
    fn prepares_medmcqa_indexed_choice_columns() {
        let config = multiple_choice_config(
            crate::config::CustomNoCodeChoiceSource::Columns(
                crate::config::CustomNoCodeChoiceColumns {
                    columns: ["opa", "opb", "opc", "opd"].map(str::to_owned).to_vec(),
                },
            ),
            crate::config::CustomNoCodeAnswerSource::IndexColumn(
                crate::config::CustomNoCodeIndexAnswer {
                    index_column: "cop".to_owned(),
                    index_base: 0,
                },
            ),
        );
        let row = json!({"opa": "alpha", "opb": "beta", "opc": "gamma", "opd": "delta", "cop": 1});

        let prepared = prepare_row(0, &row, &config).unwrap();
        assert_eq!(prepared.golden, "B");
        assert_eq!(prepared.choices[1].text, "beta");
    }

    #[test]
    fn prepares_gpqa_with_deterministic_shuffle() {
        let config = crate::config::CustomNoCodeStyleConfig::MultipleChoice {
            choices: crate::config::CustomNoCodeChoiceSource::Columns(
                crate::config::CustomNoCodeChoiceColumns {
                    columns: [
                        "Correct Answer",
                        "Incorrect Answer 1",
                        "Incorrect Answer 2",
                        "Incorrect Answer 3",
                    ]
                    .map(str::to_owned)
                    .to_vec(),
                },
            ),
            answer: crate::config::CustomNoCodeAnswerSource::CorrectChoiceColumn(
                crate::config::CustomNoCodeCorrectChoiceAnswer {
                    correct_choice_column: "Correct Answer".to_owned(),
                },
            ),
            choice_labels: ["A", "B", "C", "D"].map(str::to_owned).to_vec(),
            shuffle: Some(crate::config::CustomNoCodeShuffleConfig {
                seed_column: "Record ID".to_owned(),
            }),
        };
        let row = json!({
            "Correct Answer": "correct",
            "Incorrect Answer 1": "wrong one",
            "Incorrect Answer 2": "wrong two",
            "Incorrect Answer 3": "wrong three",
            "Record ID": "gpqa-row-1"
        });

        let first = prepare_row(0, &row, &config).unwrap();
        let second = prepare_row(0, &row, &config).unwrap();
        assert_eq!(
            serde_json::to_value(&first.choices).unwrap(),
            serde_json::to_value(&second.choices).unwrap()
        );
        let correct = first
            .choices
            .iter()
            .find(|choice| choice.label == first.golden)
            .unwrap();
        assert_eq!(correct.text, "correct");
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
        std::fs::write(&template_path, "{{ row.question }}\nAnswer:").unwrap();

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
        let response_parsed_count = all_metrics
            .iter()
            .filter(|m| m.metric_name == "response_parsed")
            .count();
        assert_eq!(response_parsed_count, 2);

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
