use anyhow::Result;
use qt::db::{MetricPointSummary, StepSummary};

/// Compute the custom no-code aggregate metrics enabled by the `json` parameter, and
/// append them to `metrics` without persisting them.
///
/// # Errors
///
/// Returns an error if the stored custom no-code configuration is incompatible with the requested
/// metrics or a recorded step output cannot be decoded.
pub(super) fn append_requested_output_metrics(
    metrics: &mut Vec<MetricPointSummary>,
    input: Option<&str>,
    steps: &[StepSummary],
    json: bool,
    created_at: time::OffsetDateTime,
) -> Result<()> {
    let derived = qt::builtins::compute_custom_nocode_output_metrics(input, steps, json)?;
    metrics.extend(derived.into_iter().map(|metric| MetricPointSummary {
        metric_name: metric.name,
        metric_value: metric.value,
        unit: None,
        step_id: None,
        created_at,
    }));
    Ok(())
}
