use std::collections::HashMap;

use anyhow::{Context, Result, bail};

use crate::builtins::common::{compute_statistics, emit_accuracy_metrics};

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
enum SampleResultKind {
    ExactMatch(ExactMatchSampleResultParams),
    MultipleChoice(MultipleChoiceSampleResultParams),
}

#[derive(Clone, Debug)]
pub(super) struct SampleResult(SampleResultKind);

impl SampleResult {
    /// Construct an exact-match result from its correctness value.
    pub(super) const fn exact_match(is_correct: bool) -> Self {
        Self(SampleResultKind::ExactMatch(ExactMatchSampleResultParams {
            is_correct,
        }))
    }

    /// Construct a multiple-choice result with its golden and optionally parsed labels.
    pub(super) fn multiple_choice(
        is_correct: bool,
        golden_label: String,
        predicted_label: Option<String>,
    ) -> Self {
        Self(SampleResultKind::MultipleChoice(
            MultipleChoiceSampleResultParams {
                is_correct,
                golden_label,
                predicted_label,
            },
        ))
    }

    /// Return whether this sample's parsed response matched its golden answer.
    const fn is_correct(&self) -> bool {
        match &self.0 {
            SampleResultKind::ExactMatch(params) => params.is_correct,
            SampleResultKind::MultipleChoice(params) => params.is_correct,
        }
    }

    /// Return whether the response could be parsed for this scoring style.
    const fn response_parsed(&self) -> bool {
        match &self.0 {
            SampleResultKind::ExactMatch(_) => true,
            SampleResultKind::MultipleChoice(params) => params.predicted_label.is_some(),
        }
    }
}

#[expect(clippy::cast_precision_loss)]
/// Emit correctness, parsing, latency, and optional classification aggregates for a run.
pub(super) async fn emit_aggregate_metrics(
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
            .map(|result| match &result.0 {
                SampleResultKind::MultipleChoice(params) => Ok(params.clone()),
                SampleResultKind::ExactMatch(_) => {
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

/// Build a golden-label-by-predicted-label count matrix with a final unparsed column.
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
/// Emit per-label, macro, weighted, and confusion-matrix metrics for multiple-choice results.
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

/// Divide two metric values, returning zero when the denominator is effectively zero.
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

    #[tokio::test]
    /// Verifies all aggregate metric families and their expected values are emitted.
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
            SampleResult::multiple_choice(true, "A".to_owned(), Some("A".to_owned())),
            SampleResult::multiple_choice(false, "A".to_owned(), Some("B".to_owned())),
            SampleResult::multiple_choice(false, "B".to_owned(), None),
        ];
        let choice_labels = ["A".to_owned(), "B".to_owned()];

        emit_aggregate_metrics(&metrics_store, run_id, &results, Some(&choice_labels))
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
}
