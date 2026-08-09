use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct Scores {
    #[serde(rename = "reward")]
    pub(crate) overall: f64,
    #[serde(rename = "database_reward")]
    pub(crate) database: f64,
    #[serde(rename = "communicate_reward")]
    pub(crate) communication: f64,
}

pub(crate) fn score(
    final_state: &Value,
    expected_state: &Value,
    transcript: &str,
    required: &[String],
) -> Scores {
    let database_reward = f64::from(value_contains(final_state, expected_state));
    let transcript = transcript.to_lowercase();
    let communicate_reward = f64::from(
        required
            .iter()
            .all(|needle| transcript.contains(&needle.to_lowercase())),
    );
    Scores {
        overall: database_reward * communicate_reward,
        database: database_reward,
        communication: communicate_reward,
    }
}

fn value_contains(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => expected.iter().all(|(key, value)| {
            actual
                .get(key)
                .is_some_and(|actual| value_contains(actual, value))
        }),
        (Value::Array(actual), Value::Array(expected)) => expected
            .iter()
            .all(|value| actual.iter().any(|actual| value_contains(actual, value))),
        _ => actual == expected,
    }
}

#[expect(clippy::cast_precision_loss)]
pub(crate) fn pass_at_k(rewards_by_task: &[Vec<f64>], k: usize) -> Option<f64> {
    if k == 0
        || rewards_by_task.is_empty()
        || rewards_by_task.iter().any(|rewards| rewards.len() < k)
    {
        return None;
    }
    let total = rewards_by_task
        .iter()
        .map(|rewards| {
            let n = rewards.len();
            let failures = rewards.iter().filter(|reward| **reward < 1.0).count();
            if failures < k {
                1.0
            } else {
                1.0 - combination_ratio(failures, n, k)
            }
        })
        .sum::<f64>();
    Some(total / rewards_by_task.len() as f64)
}

#[expect(clippy::cast_precision_loss)]
fn combination_ratio(failures: usize, trials: usize, k: usize) -> f64 {
    (0..k).fold(1.0, |ratio, i| {
        ratio * (failures - i) as f64 / (trials - i) as f64
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scores_database_and_communication_requirements() {
        let reward = score(
            &json!({"email": "new@example.com", "untouched": true}),
            &json!({"email": "new@example.com"}),
            "Agent: Your email is now new@example.com",
            &["new@example.com".to_owned()],
        );
        assert!((reward.overall - 1.0).abs() < f64::EPSILON);
        assert!((reward.database - 1.0).abs() < f64::EPSILON);
        assert!((reward.communication - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn computes_unbiased_pass_at_k() {
        let rewards = vec![vec![1.0, 0.0, 0.0, 0.0], vec![1.0, 1.0, 0.0, 0.0]];
        assert_eq!(pass_at_k(&rewards, 1), Some(0.375));
        assert_eq!(pass_at_k(&rewards, 4), Some(1.0));
        assert_eq!(pass_at_k(&rewards, 5), None);
    }
}
