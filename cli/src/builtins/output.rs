use serde::Serialize;

/// Normalized run output schema for all builtins.
#[derive(Serialize)]
pub(crate) struct BuiltinRunOutput {
    pub(crate) samples: usize,
}

/// Rewrite the run record output to the normalized builtin shape.
pub(crate) async fn set_builtin_run_output(
    db: &sea_orm::DatabaseConnection,
    run_id: i64,
    samples: usize,
) -> anyhow::Result<()> {
    let output = serde_json::to_string(&BuiltinRunOutput { samples })?;
    crate::db::set_run_output(db, run_id, &output).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_output_uses_samples() {
        let output = BuiltinRunOutput { samples: 10 };

        assert_eq!(
            serde_json::to_value(output).unwrap(),
            serde_json::json!({"samples": 10})
        );
    }
}
