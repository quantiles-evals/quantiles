use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use crate::db::steps::{self, StepDecision};
use crate::llm::Sampler;
use crate::metrics_store::MetricsStore;

/// Fields shared by every builtin benchmark config. When adding a new builtin,
/// embed this with `#[serde(flatten)]` so that `limit`, `model`, and
/// `max_workers` are automatically supported without duplication.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct BuiltinConfig {
    /// Number of dataset rows to evaluate. If omitted, the entire dataset is used.
    #[serde(default)]
    pub(crate) limit: Option<usize>,
    /// Dataset source to evaluate. Builtins currently support Hugging Face via `hf://...`.
    #[serde(default)]
    pub(crate) dataset: Option<String>,
    /// Which model sampler to use. If omitted, the builtin chooses a sensible default.
    #[serde(default)]
    pub(crate) model: Option<Sampler>,
    /// Maximum concurrent workers. Falls back to `QUANTILES_MAX_WORKERS` env var (default 25).
    #[serde(default)]
    pub(crate) max_workers: Option<usize>,
}

/// Extract a string field from a JSON row.
pub(crate) fn extract_text(row: &Value, key: &str) -> Option<String> {
    row.get(key)?.as_str().map(String::from)
}

/// Compute a deterministic hash for step caching.
pub(crate) fn hash_input(input: &str) -> String {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Begin a step (reusing cached output if available) or run the provided work,
/// record latency, complete the step, and emit a `latency_ms` metric.
///
/// Note that the given `work` parameter should be a future that was
/// created, but not started. This function will, if it determines that
/// the step should be run, start it, drive it to
/// completion, and time it.
///
/// Returns the deserialized result and `Some(step_id)` if work was executed,
/// or `None` if the step output was reused.
pub(crate) async fn run_timed_step<T>(
    db: &DatabaseConnection,
    metrics_store: &MetricsStore,
    run_id: i64,
    step_key: &str,
    input_hash: &str,
    work: impl std::future::Future<Output = Result<T>>,
) -> Result<(T, Option<i64>)>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    match steps::begin_step(db, run_id, step_key, input_hash).await? {
        StepDecision::Reuse { output } => {
            let parsed: T = serde_json::from_str(&output)
                .with_context(|| format!("failed to parse cached step output for {step_key}"))?;
            Ok((parsed, None))
        }
        StepDecision::Run { step_id } => {
            let start = Instant::now();
            let result = work.await?;
            // this is a good alternative to using `start.elapsed().as_millis()`, and then having
            // to figure out how to cast that to an `f64`. It also preserves sub-millisecond precision,
            // whereas as_millis() truncates to an integer number of milliseconds.
            let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

            let output = serde_json::to_string(&result)
                .with_context(|| format!("failed to serialize step output for {step_key}"))?;
            steps::complete_step(db, step_id, &output).await?;

            metrics_store
                .emit(run_id, Some(step_id), "latency_ms", latency_ms, Some("ms"))
                .await;

            Ok((result, Some(step_id)))
        }
    }
}

/// Read the `QUANTILES_MAX_WORKERS` environment variable, clamp to [1, 2000], and warn on invalid input.
pub(crate) fn get_max_workers() -> usize {
    const DEFAULT: usize = 25;
    const MIN: usize = 1;
    const MAX: usize = 2000;

    match std::env::var("QUANTILES_MAX_WORKERS") {
        Ok(val) => {
            if let Ok(n) = val.parse::<usize>() {
                if n < MIN {
                    eprintln!(
                        "Warning: QUANTILES_MAX_WORKERS={n} is below minimum {MIN}, clamping to {MIN}"
                    );
                    MIN
                } else if n > MAX {
                    eprintln!(
                        "Warning: QUANTILES_MAX_WORKERS={n} exceeds maximum {MAX}, clamping to {MAX}"
                    );
                    MAX
                } else {
                    n
                }
            } else {
                eprintln!(
                    "Warning: QUANTILES_MAX_WORKERS={val} is not a valid number, using default {DEFAULT}"
                );
                DEFAULT
            }
        }
        Err(_) => DEFAULT,
    }
}

/// Statistics computed from a collection of similarity scores.
#[derive(Debug)]
pub(crate) struct ScoreStatistics {
    pub(crate) mean: f64,
    pub(crate) std: f64,
    pub(crate) variance: f64,
    pub(crate) median: f64,
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) p99: f64,
    pub(crate) p95: f64,
}

/// Compute population statistics in a single pass.
#[expect(clippy::cast_precision_loss)]
pub(crate) fn compute_statistics(values: &[f64]) -> ScoreStatistics {
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let std = variance.sqrt();

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let median = percentile(&sorted, 0.5);
    let min = *sorted.first().unwrap_or(&0.0);
    let max = *sorted.last().unwrap_or(&0.0);
    let p99 = percentile(&sorted, 0.99);
    let p95 = percentile(&sorted, 0.95);

    ScoreStatistics {
        mean,
        std,
        variance,
        median,
        min,
        max,
        p99,
        p95,
    }
}

/// Compute a percentile from a sorted slice using linear interpolation.
#[expect(clippy::cast_precision_loss)]
#[expect(clippy::cast_sign_loss)]
#[expect(clippy::cast_possible_truncation)]
pub(crate) fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.len() == 1 {
        return sorted[0];
    }
    let rank = p * (sorted.len() - 1) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    let weight = rank - lower as f64;
    if lower == upper {
        return sorted[lower];
    }
    sorted[lower] * (1.0 - weight) + sorted[upper] * weight
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("hello", Some("hello"))]
    #[case("", Some(""))]
    fn test_extract_text(#[case] value: &str, #[case] expected: Option<&str>) {
        use serde_json::json;
        let row = json!({"field": value});
        assert_eq!(extract_text(&row, "field"), expected.map(String::from));
    }

    #[rstest]
    fn test_extract_text_missing_key() {
        use serde_json::json;
        let row = json!({ "other": "data" });
        assert_eq!(extract_text(&row, "field"), None);
    }

    #[rstest]
    fn test_extract_text_non_string_value() {
        use serde_json::json;
        let row = json!({ "field": 42 });
        assert_eq!(extract_text(&row, "field"), None);
    }

    #[rstest]
    #[case("hello")]
    #[case("")]
    #[case("a longer string with unicode ñ and symbols @#$")]
    #[case("model=gpt-4\nprompt=what is the meaning of life?")]
    fn test_hash_input_deterministic(#[case] input: &str) {
        let a = hash_input(input);
        let b = hash_input(input);
        assert_eq!(a, b, "hash should be deterministic");
        assert_eq!(a.len(), 16, "hash should be 16 hex chars");
    }

    #[rstest]
    fn test_hash_input_different_inputs() {
        let a = hash_input("foo");
        let b = hash_input("bar");
        assert_ne!(
            a, b,
            "different inputs should (almost certainly) produce different hashes"
        );
    }

    #[rstest]
    #[case(&[1.0], 0.5, 1.0)]
    #[case(&[1.0], 0.0, 1.0)]
    #[case(&[1.0], 1.0, 1.0)]
    #[case(&[1.0, 3.0], 0.5, 2.0)]
    #[case(&[1.0, 3.0], 0.0, 1.0)]
    #[case(&[1.0, 3.0], 1.0, 3.0)]
    #[case(&[0.0, 10.0, 20.0, 30.0, 40.0], 0.5, 20.0)]
    #[case(&[0.0, 10.0, 20.0, 30.0, 40.0], 0.25, 10.0)]
    #[case(&[0.0, 10.0, 20.0, 30.0, 40.0], 0.75, 30.0)]
    fn test_percentile(#[case] sorted: &[f64], #[case] p: f64, #[case] expected: f64) {
        let result = percentile(sorted, p);
        assert!(
            (result - expected).abs() < 1e-10,
            "percentile({sorted:?}, {p}) = {result}, expected {expected}"
        );
    }

    #[rstest]
    fn test_compute_statistics_single_value() {
        let stats = compute_statistics(&[42.0]);
        assert!((stats.mean - 42.0).abs() < 1e-10);
        assert!((stats.median - 42.0).abs() < 1e-10);
        assert!((stats.min - 42.0).abs() < 1e-10);
        assert!((stats.max - 42.0).abs() < 1e-10);
        assert!(stats.std < 1e-10);
        assert!(stats.variance < 1e-10);
    }

    #[rstest]
    fn test_compute_statistics_known_values() {
        let stats = compute_statistics(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert!((stats.mean - 3.0).abs() < 1e-10);
        assert!((stats.median - 3.0).abs() < 1e-10);
        assert!((stats.min - 1.0).abs() < 1e-10);
        assert!((stats.max - 5.0).abs() < 1e-10);
        assert!(stats.std > 0.0);
        assert!(stats.variance > 0.0);
        assert!(stats.p99 >= stats.p95);
        assert!(stats.p95 >= stats.median);
    }

    #[rstest]
    fn test_compute_statistics_monotonic_percentiles() {
        let stats = compute_statistics(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
        assert!(stats.min <= stats.p95);
        assert!(stats.p95 <= stats.p99);
        assert!(stats.p99 <= stats.max);
        assert!(stats.min <= stats.median);
        assert!(stats.median <= stats.max);
    }
}
