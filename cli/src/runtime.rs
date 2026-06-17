use anyhow::Result;
use sea_orm::DatabaseConnection;

use crate::db::steps::{self, StepDecision};

/// Run a durable step or return its previously stored output.
///
/// # Errors
///
/// Returns an error if step reservation fails, a prior step with the same key
/// has incompatible state, the execution closure fails, or the step result
/// cannot be persisted.
pub async fn run_step<F>(
    db: &DatabaseConnection,
    run_id: i64,
    step_key: &str,
    input_hash: &str,
    execute: F,
) -> Result<String>
where
    F: FnOnce() -> Result<String>,
{
    match steps::begin_step(db, run_id, step_key, input_hash).await? {
        StepDecision::Reuse { output } => Ok(output),
        StepDecision::Run { step_id } => execute_step(db, step_id, execute).await,
    }
}

async fn execute_step<F>(db: &DatabaseConnection, step_id: i64, execute: F) -> Result<String>
where
    F: FnOnce() -> Result<String>,
{
    let result = execute();

    let output = match result {
        Ok(output) => output,
        Err(err) => {
            let message = format!("{err:#}");
            steps::fail_step(db, step_id, &message).await?;
            return Err(err);
        }
    };

    steps::complete_step(db, step_id, &output).await?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use anyhow::{Result, bail};
    use sea_orm::DatabaseConnection;

    use super::run_step;
    use crate::db::{
        StepStatus, create_run, init_workspace, list_events_for_run, list_steps_for_run,
        open_workspace,
    };

    #[tokio::test]
    async fn run_step_executes_and_completes_new_step() -> Result<()> {
        let (db, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;

        let output = run_step(&db, run_id, "fetch", "hash-1", || {
            Ok("{\"ok\":true}".to_owned())
        })
        .await?;

        assert_eq!(output, "{\"ok\":true}");

        let steps = list_steps_for_run(&db, run_id).await?;
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].status, StepStatus::Completed);

        let events = list_events_for_run(&db, run_id).await?;
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "step.completed"),
            "expected step.completed event"
        );

        Ok(())
    }

    #[tokio::test]
    async fn run_step_reuses_completed_output_without_executing() -> Result<()> {
        let (db, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;

        let first = run_step(&db, run_id, "fetch", "hash-1", || {
            Ok("first-output".to_owned())
        })
        .await?;
        let executed = Cell::new(false);
        let second = run_step(&db, run_id, "fetch", "hash-1", || {
            executed.set(true);
            Ok("second-output".to_owned())
        })
        .await?;

        assert_eq!(first, "first-output");
        assert_eq!(second, "first-output");
        assert!(!executed.get(), "reused step should not execute closure");

        Ok(())
    }

    #[tokio::test]
    async fn run_step_marks_failed_when_execute_errors() -> Result<()> {
        let (db, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;

        let err = run_step(&db, run_id, "fetch", "hash-1", || bail!("network failed"))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("network failed"));

        let steps = list_steps_for_run(&db, run_id).await?;
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].status, StepStatus::Failed);

        let events = list_events_for_run(&db, run_id).await?;
        assert!(
            events.iter().any(|event| event.event_type == "step.failed"),
            "expected step.failed event"
        );

        Ok(())
    }

    async fn test_db() -> Result<(DatabaseConnection, tempfile::TempDir)> {
        let tmpdir = tempfile::tempdir()?;
        init_workspace(tmpdir.path()).await?;
        let db = open_workspace(tmpdir.path()).await?;
        Ok((db, tmpdir))
    }
}
