use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::builtins::common::compute_statistics;
use crate::config::{CustomNoCodeMetricName, CustomNoCodeParams, CustomNoCodeStyleConfig};
use crate::db::StepSummary;

#[derive(Clone, Copy, Debug)]
pub(super) struct SampleResult {
    is_correct: bool,
}

impl SampleResult {
    pub(super) const fn new(is_correct: bool) -> Self {
        Self { is_correct }
    }
}

/// An aggregate metric computed for output without being persisted.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputMetric {
    pub name: String,
    pub value: f64,
}

#[derive(Debug, Deserialize)]
struct StoredRowOutput {
    parsed_response: Option<String>,
    golden: String,
}

#[derive(Clone, Debug)]
struct MultipleChoiceResult {
    golden_label: String,
    predicted_label: Option<String>,
}

/// Persist only the default accuracy and latency aggregates.
pub(super) async fn emit_default_aggregate_metrics(
    metrics_store: &crate::metrics_store::MetricsStore,
    run_id: i64,
    results: &[SampleResult],
) -> Result<()> {
    if results.is_empty() {
        return Ok(());
    }

    #[expect(clippy::cast_precision_loss)]
    let accuracy =
        results.iter().filter(|result| result.is_correct).count() as f64 / results.len() as f64;
    metrics_store
        .emit(run_id, None, "accuracy", accuracy, None)
        .await;

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

/// Return whether the stored custom no-code input requests derived metrics for this output mode.
#[must_use]
pub fn output_metrics_requested(input: Option<&str>, json: bool) -> bool {
    parse_config(input).is_some_and(|config| {
        config
            .metrics
            .iter()
            .any(|selection| selection.requested_for(json))
    })
}

/// Compute configured aggregate metrics from durable step outputs without persisting them.
///
/// # Errors
///
/// Returns an error when a requested metric is incompatible with the stored configuration,
/// or when a durable step output cannot be decoded into a multiple-choice result.
pub fn compute_output_metrics(
    input: Option<&str>,
    steps: &[StepSummary],
    json: bool,
) -> Result<Vec<OutputMetric>> {
    let Some(config) = parse_config(input) else {
        return Ok(Vec::new());
    };
    let requested = config
        .metrics
        .iter()
        .copied()
        .filter(|selection| selection.requested_for(json))
        .collect::<Vec<_>>();
    if requested.is_empty() {
        return Ok(Vec::new());
    }

    let CustomNoCodeStyleConfig::MultipleChoice { choice_labels, .. } = &config.style else {
        bail!("configured output metrics require a multiple-choice evaluation");
    };
    let results = steps
        .iter()
        .filter_map(|step| step.output.as_deref().map(|output| (step, output)))
        .map(|(step, output)| {
            let output: StoredRowOutput = serde_json::from_str(output)
                .with_context(|| format!("failed to parse output for step `{}`", step.step_key))?;
            Ok(MultipleChoiceResult {
                golden_label: output.golden,
                predicted_label: output.parsed_response,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if results.is_empty() {
        return Ok(Vec::new());
    }

    let matrix = build_confusion_matrix(&results, choice_labels)?;
    let mut metrics = Vec::new();
    for selection in requested {
        match selection.name() {
            CustomNoCodeMetricName::F1 => {
                metrics.extend(compute_f1_metrics(&matrix));
            }
            CustomNoCodeMetricName::Confusion => {
                metrics.extend(confusion_metrics(&matrix));
            }
        }
    }
    Ok(metrics)
}

fn parse_config(input: Option<&str>) -> Option<CustomNoCodeParams> {
    input.and_then(|input| serde_json::from_str(input).ok())
}

fn build_confusion_matrix(
    results: &[MultipleChoiceResult],
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
fn compute_f1_metrics(matrix: &[Vec<usize>]) -> Vec<OutputMetric> {
    let label_count = matrix.len();
    let total = matrix.iter().flatten().sum::<usize>() as f64;
    let mut macro_f1 = 0.0;
    let mut weighted_f1 = 0.0;
    let mut metrics = Vec::with_capacity(label_count + 2);

    for label_index in 0..label_count {
        let true_positive = matrix[label_index][label_index] as f64;
        let false_positive = matrix
            .iter()
            .enumerate()
            .filter(|(golden_index, _)| *golden_index != label_index)
            .map(|(_, row)| row[label_index])
            .sum::<usize>() as f64;
        let support = matrix[label_index].iter().sum::<usize>() as f64;
        let precision = safe_ratio(true_positive, true_positive + false_positive);
        let recall = safe_ratio(true_positive, support);
        let f1 = safe_ratio(2.0 * precision * recall, precision + recall);
        macro_f1 += f1;
        weighted_f1 += f1 * safe_ratio(support, total);
        metrics.push(OutputMetric {
            name: format!("f1_label_{label_index}"),
            value: f1,
        });
    }

    metrics.push(OutputMetric {
        name: "macro_f1".to_owned(),
        value: macro_f1 / label_count as f64,
    });
    metrics.push(OutputMetric {
        name: "weighted_f1".to_owned(),
        value: weighted_f1,
    });
    metrics
}

#[expect(clippy::cast_precision_loss)]
fn confusion_metrics(matrix: &[Vec<usize>]) -> Vec<OutputMetric> {
    let label_count = matrix.len();
    let mut metrics = Vec::with_capacity(label_count * (label_count + 1));
    for (golden_index, row) in matrix.iter().enumerate() {
        for (predicted_index, count) in row.iter().take(label_count).enumerate() {
            metrics.push(OutputMetric {
                name: format!("confusion_matrix_{golden_index}_{predicted_index}"),
                value: *count as f64,
            });
        }
        metrics.push(OutputMetric {
            name: format!("confusion_matrix_{golden_index}_unparsed"),
            value: row[label_count] as f64,
        });
    }
    metrics
}

fn safe_ratio(numerator: f64, denominator: f64) -> f64 {
    if denominator.abs() < f64::EPSILON {
        0.0
    } else {
        numerator / denominator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::StepStatus;
    use serde_json::json;
    use time::OffsetDateTime;

    #[tokio::test]
    async fn persists_only_default_aggregates() {
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
            SampleResult::new(true),
            SampleResult::new(false),
            SampleResult::new(false),
        ];

        emit_default_aggregate_metrics(&metrics_store, run_id, &results)
            .await
            .unwrap();
        metrics_store.flush(run_id).await.unwrap();
        let names = metrics_store
            .list_aggregate_for_run(run_id)
            .await
            .unwrap()
            .into_iter()
            .map(|metric| metric.metric_name)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "accuracy",
                "max_latency_ms",
                "mean_latency_ms",
                "median_latency_ms",
                "min_latency_ms",
                "p95_latency_ms",
                "p99_latency_ms",
            ]
        );
    }

    #[test]
    fn computes_requested_f1_and_confusion_metrics() {
        let results = [
            MultipleChoiceResult {
                golden_label: "A".to_owned(),
                predicted_label: Some("A".to_owned()),
            },
            MultipleChoiceResult {
                golden_label: "A".to_owned(),
                predicted_label: Some("B".to_owned()),
            },
            MultipleChoiceResult {
                golden_label: "B".to_owned(),
                predicted_label: None,
            },
        ];
        let matrix = build_confusion_matrix(&results, &["A".to_owned(), "B".to_owned()]).unwrap();
        let f1 = compute_f1_metrics(&matrix);
        let confusion = confusion_metrics(&matrix);

        let macro_f1 = f1.iter().find(|m| m.name == "macro_f1").unwrap().value;
        assert!((macro_f1 - 1.0 / 3.0).abs() < f64::EPSILON);
        let unparsed = confusion
            .iter()
            .find(|m| m.name == "confusion_matrix_1_unparsed")
            .unwrap()
            .value;
        assert!((unparsed - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn computes_only_metric_families_requested_for_output_mode() {
        let input = json!({
            "dataset": {"name": "fixture/qa"},
            "prompt_template_file": "prompts/qa.txt",
            "style": {
                "type": "multiple_choice",
                "choices": {"column": "options"},
                "answer": {"label_column": "answer"},
                "choice_labels": ["A", "B"]
            },
            "metrics": [
                "f1",
                {"name": "confusion", "show": "all"}
            ]
        })
        .to_string();
        let steps = vec![
            step(1, "A", Some("A")),
            step(2, "A", Some("B")),
            step(3, "B", None),
        ];

        let human = compute_output_metrics(Some(&input), &steps, false).unwrap();
        assert!(
            human
                .iter()
                .all(|metric| metric.name.starts_with("confusion_matrix_"))
        );
        assert!(!human.is_empty());

        let json = compute_output_metrics(Some(&input), &steps, true).unwrap();
        assert!(json.iter().any(|metric| metric.name == "macro_f1"));
        assert!(
            json.iter()
                .any(|metric| metric.name == "confusion_matrix_0_0")
        );
    }

    #[test]
    fn skips_step_parsing_when_no_output_metrics_are_requested() {
        let input = json!({
            "dataset": {"name": "fixture/qa"},
            "prompt_template_file": "prompts/qa.txt",
            "style": {
                "type": "multiple_choice",
                "choices": {"column": "options"},
                "answer": {"label_column": "answer"},
                "choice_labels": ["A", "B"]
            }
        })
        .to_string();
        let malformed_step = StepSummary {
            id: 1,
            step_key: "row-0".to_owned(),
            input_hash: "hash".to_owned(),
            status: StepStatus::Completed,
            output: Some("not JSON".to_owned()),
            error: None,
            started_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: Some(OffsetDateTime::UNIX_EPOCH),
        };

        assert!(!output_metrics_requested(Some(&input), true));
        assert!(
            compute_output_metrics(Some(&input), &[malformed_step], true)
                .unwrap()
                .is_empty()
        );
    }

    fn step(id: i64, golden: &str, parsed_response: Option<&str>) -> StepSummary {
        StepSummary {
            id,
            step_key: format!("row-{}", id - 1),
            input_hash: format!("hash-{id}"),
            status: StepStatus::Completed,
            output: Some(
                json!({
                    "input": "question",
                    "response": parsed_response.unwrap_or("unparsed"),
                    "parsed_response": parsed_response,
                    "golden": golden,
                    "is_correct": parsed_response == Some(golden)
                })
                .to_string(),
            ),
            error: None,
            started_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: Some(OffsetDateTime::UNIX_EPOCH),
        }
    }
}
