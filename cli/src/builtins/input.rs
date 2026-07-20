use serde::Serialize;

use crate::llm::Sampler;

/// Normalized run input schema for all builtins.
#[derive(Serialize)]
pub(crate) struct BuiltinRunInput {
    pub(crate) model: String,
    pub(crate) samples: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_workers: Option<usize>,
}

/// Rewrite the run record input to the normalized builtin shape.
pub(crate) async fn set_builtin_run_input(
    db: &sea_orm::DatabaseConnection,
    run_id: i64,
    model: Option<&Sampler>,
    samples: usize,
    max_workers: Option<usize>,
) -> anyhow::Result<()> {
    let input = serde_json::to_string(&BuiltinRunInput {
        model: builtin_model_name(model),
        samples,
        max_workers,
    })?;
    crate::db::set_run_input(db, run_id, &input).await
}

/// Derive the display name for a builtin model.
/// The built-in `Random` sampler is always reported as `demo-builtin`.
fn builtin_model_name(model: Option<&Sampler>) -> String {
    match model {
        None | Some(Sampler::Random) => "demo-builtin".to_string(),
        Some(other) => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_input_uses_samples() {
        let input = BuiltinRunInput {
            model: "demo-builtin".to_owned(),
            samples: 10,
            max_workers: None,
        };

        assert_eq!(
            serde_json::to_value(input).unwrap(),
            serde_json::json!({"model": "demo-builtin", "samples": 10})
        );
    }
}
