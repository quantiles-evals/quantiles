use anyhow::{Context, Result, bail};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};

use crate::db::entities::{event, step};
use crate::db::summaries::{StepStatus, StepSummary};
use crate::time::now_utc;

#[derive(Debug, PartialEq, Eq)]
pub enum StepDecision {
    Run { step_id: i64 },
    Reuse { output: String },
}

pub async fn begin_step(
    db: &DatabaseConnection,
    run_id: i64,
    step_key: &str,
    input_hash: &str,
) -> Result<StepDecision> {
    let tx = db.begin().await?;

    let existing = step::Entity::find()
        .filter(step::Column::RunId.eq(run_id))
        .filter(step::Column::StepKey.eq(step_key))
        .one(&tx)
        .await?;

    let decision = if let Some(step) = existing {
        if step.input_hash != input_hash {
            bail!("step `{step_key}` in run {run_id} already exists with a different input hash");
        }

        match step.status {
            StepStatus::Completed => StepDecision::Reuse {
                output: step
                    .output
                    .with_context(|| format!("completed step `{step_key}` has no stored output"))?,
            },
            StepStatus::Running => StepDecision::Run { step_id: step.id },
            StepStatus::Failed => {
                // Allow retry by resetting the step status to 'running', then emit a fresh
                // 'started' event so the retry is apparent and auditable.
                step::Entity::update_many()
                    .set(step::ActiveModel {
                        status: Set(StepStatus::Running),
                        error: Set(None),
                        finished_at: Set(None),
                        ..Default::default()
                    })
                    .filter(step::Column::Id.eq(step.id))
                    .exec(&tx)
                    .await?;

                event::Entity::insert(event::ActiveModel {
                    run_id: Set(run_id),
                    step_id: Set(Some(step.id)),
                    event_type: Set("step.started".to_owned()),
                    message: Set(Some(format!("Retried step {step_key}"))),
                    created_at: Set(now_utc()),
                    ..Default::default()
                })
                .exec(&tx)
                .await?;

                StepDecision::Run { step_id: step.id }
            }
        }
    } else {
        let result = step::Entity::insert(step::ActiveModel {
            run_id: Set(run_id),
            step_key: Set(step_key.to_owned()),
            input_hash: Set(input_hash.to_owned()),
            status: Set(StepStatus::Running),
            started_at: Set(now_utc()),
            ..Default::default()
        })
        .exec(&tx)
        .await?;

        let step_id = result.last_insert_id;

        event::Entity::insert(event::ActiveModel {
            run_id: Set(run_id),
            step_id: Set(Some(step_id)),
            event_type: Set("step.started".to_owned()),
            message: Set(Some(format!("Started step {step_key}"))),
            created_at: Set(now_utc()),
            ..Default::default()
        })
        .exec(&tx)
        .await?;

        StepDecision::Run { step_id }
    };

    tx.commit().await?;

    Ok(decision)
}

pub async fn complete_step(db: &DatabaseConnection, step_id: i64, output: &str) -> Result<()> {
    let tx = db.begin().await?;
    let run_id = step_run_id(&tx, step_id).await?;

    step::Entity::update_many()
        .set(step::ActiveModel {
            status: Set(StepStatus::Completed),
            output: Set(Some(output.to_owned())),
            error: Set(None),
            finished_at: Set(Some(now_utc())),
            ..Default::default()
        })
        .filter(step::Column::Id.eq(step_id))
        .exec(&tx)
        .await?;

    event::Entity::insert(event::ActiveModel {
        run_id: Set(run_id),
        step_id: Set(Some(step_id)),
        event_type: Set("step.completed".to_owned()),
        message: Set(Some(format!("Completed step {step_id}"))),
        created_at: Set(now_utc()),
        ..Default::default()
    })
    .exec(&tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

pub async fn fail_step(db: &DatabaseConnection, step_id: i64, error: &str) -> Result<()> {
    let tx = db.begin().await?;
    let run_id = step_run_id(&tx, step_id).await?;

    step::Entity::update_many()
        .set(step::ActiveModel {
            status: Set(StepStatus::Failed),
            error: Set(Some(error.to_owned())),
            finished_at: Set(Some(now_utc())),
            ..Default::default()
        })
        .filter(step::Column::Id.eq(step_id))
        .exec(&tx)
        .await?;

    event::Entity::insert(event::ActiveModel {
        run_id: Set(run_id),
        step_id: Set(Some(step_id)),
        event_type: Set("step.failed".to_owned()),
        message: Set(Some(error.to_owned())),
        created_at: Set(now_utc()),
        ..Default::default()
    })
    .exec(&tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

/// List samples for an eval run.
///
/// # Errors
///
/// Returns an error if the query cannot be prepared or read.
pub async fn list_steps_for_run(db: &DatabaseConnection, run_id: i64) -> Result<Vec<StepSummary>> {
    let steps = step::Entity::find()
        .filter(step::Column::RunId.eq(run_id))
        .order_by_asc(step::Column::StartedAt)
        .order_by_asc(step::Column::Id)
        .all(db)
        .await
        .context("failed to list eval run samples")?;

    Ok(steps
        .into_iter()
        .map(|s| StepSummary {
            id: s.id,
            step_key: s.step_key,
            input_hash: s.input_hash,
            status: s.status,
            output: s.output,
            error: s.error,
            started_at: s.started_at,
            finished_at: s.finished_at,
        })
        .collect())
}

async fn step_run_id<C>(db: &C, step_id: i64) -> Result<i64>
where
    C: ConnectionTrait,
{
    let step = step::Entity::find_by_id(step_id)
        .one(db)
        .await?
        .with_context(|| format!("step {step_id} not found"))?;
    Ok(step.run_id)
}

#[cfg(test)]
mod tests {
    use sea_orm::DatabaseConnection;

    use super::*;
    use crate::db::{
        StepStatus, create_run, init_workspace, list_events_for_run, list_steps_for_run,
        open_workspace,
    };

    #[tokio::test]
    async fn begin_step_reserves_new_step() -> Result<()> {
        let (db, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;

        let decision = begin_step(&db, run_id, "fetch", "hash-1").await?;

        assert!(matches!(decision, StepDecision::Run { step_id: 1 }));

        let steps = list_steps_for_run(&db, run_id).await?;
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].step_key, "fetch");
        assert_eq!(steps[0].input_hash, "hash-1");
        assert_eq!(steps[0].status, StepStatus::Running);

        Ok(())
    }

    #[tokio::test]
    async fn begin_step_reuses_completed_step_output() -> Result<()> {
        let (db, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;
        let StepDecision::Run { step_id } = begin_step(&db, run_id, "fetch", "hash-1").await?
        else {
            bail!("expected new step to run");
        };

        complete_step(&db, step_id, "{\"ok\":true}").await?;

        let decision = begin_step(&db, run_id, "fetch", "hash-1").await?;

        assert_eq!(
            decision,
            StepDecision::Reuse {
                output: "{\"ok\":true}".to_owned()
            }
        );

        Ok(())
    }

    #[tokio::test]
    async fn begin_step_rejects_changed_input_hash() -> Result<()> {
        let (db, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;
        let StepDecision::Run { step_id } = begin_step(&db, run_id, "fetch", "hash-1").await?
        else {
            bail!("expected new step to run");
        };

        complete_step(&db, step_id, "{\"ok\":true}").await?;

        let err = begin_step(&db, run_id, "fetch", "hash-2")
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("different input hash"),
            "unexpected error: {err:#}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn begin_step_is_idempotent_for_running_step() -> Result<()> {
        let (db, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;

        let first = begin_step(&db, run_id, "fetch", "hash-1").await?;
        let StepDecision::Run { step_id: first_id } = first else {
            bail!("expected new step to run");
        };

        // A retry while the step is still running should return the same step_id.
        let second = begin_step(&db, run_id, "fetch", "hash-1").await?;
        let StepDecision::Run { step_id: second_id } = second else {
            bail!("expected retry to also return Run");
        };

        assert_eq!(first_id, second_id, "retry should return the same step_id");

        Ok(())
    }

    #[tokio::test]
    async fn fail_step_marks_step_failed() -> Result<()> {
        let (db, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;
        let StepDecision::Run { step_id } = begin_step(&db, run_id, "fetch", "hash-1").await?
        else {
            bail!("expected new step to run");
        };

        fail_step(&db, step_id, "network failed").await?;

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

    #[tokio::test]
    async fn complete_step_persists_output_and_is_reused() -> Result<()> {
        let (db, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;
        let StepDecision::Run { step_id } = begin_step(&db, run_id, "fetch", "hash-1").await?
        else {
            bail!("expected new step to run");
        };

        complete_step(&db, step_id, "model-result-42").await?;

        let steps = list_steps_for_run(&db, run_id).await?;
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].output.as_deref(), Some("model-result-42"));

        // Verify reuse returns the same output
        let decision = begin_step(&db, run_id, "fetch", "hash-1").await?;
        assert_eq!(
            decision,
            StepDecision::Reuse {
                output: "model-result-42".to_owned()
            }
        );

        Ok(())
    }

    async fn test_db() -> Result<(DatabaseConnection, tempfile::TempDir)> {
        let tmpdir = tempfile::tempdir()?;
        let root = tmpdir.path();
        init_workspace(root).await?;
        let db = open_workspace(root).await?;
        Ok((db, tmpdir))
    }
}
