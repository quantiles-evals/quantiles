use serde::Serialize;

/// Normalized run output schema for all builtins.
#[derive(Serialize)]
pub(crate) struct BuiltinRunOutput {
    pub(crate) samples_completed: usize,
}

/// Rewrite the run record output to the normalized builtin shape.
pub(crate) async fn set_builtin_run_output(
    db: &sea_orm::DatabaseConnection,
    run_id: i64,
    samples_completed: usize,
) -> anyhow::Result<()> {
    let output = serde_json::to_string(&BuiltinRunOutput { samples_completed })?;
    crate::db::set_run_output(db, run_id, &output).await
}
