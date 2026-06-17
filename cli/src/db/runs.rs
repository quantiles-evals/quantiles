use anyhow::{Context, Result, bail};
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    TransactionTrait,
};
use serde::Deserialize;

use crate::db::entities::{event, step, workflow, workflow_run};
use crate::db::summaries::{RunStatus, WorkflowRun, WorkflowRunSummary};
use crate::metrics_store::MetricsStore;
use crate::time::now_utc;

/// List eval runs in reverse chronological order.
///
/// # Errors
///
/// Returns an error if the query cannot be prepared or read.
pub async fn list_runs(db: &DatabaseConnection) -> Result<Vec<WorkflowRunSummary>> {
    workflow_run::Entity::find()
        .select_only()
        .column(workflow_run::Column::Id)
        .column(workflow_run::Column::WorkflowName)
        .column(workflow_run::Column::Status)
        .column(workflow_run::Column::StartedAt)
        .column(workflow_run::Column::FinishedAt)
        .column_as(step::Column::Id.count(), "steps_count")
        .left_join(step::Entity)
        .group_by(workflow_run::Column::Id)
        .order_by_desc(workflow_run::Column::StartedAt)
        .order_by_desc(workflow_run::Column::Id)
        .into_model::<WorkflowRunSummary>()
        .all(db)
        .await
        .context("failed to list eval runs")
}

/// Create a new running eval run.
///
/// # Errors
///
/// Returns an error if the run or its initial event cannot be persisted.
pub async fn create_run(
    db: &DatabaseConnection,
    workflow_name: &str,
    input: Option<&str>,
) -> Result<i64> {
    let tx = db.begin().await?;

    workflow::Entity::insert(workflow::ActiveModel {
        name: Set(workflow_name.to_owned()),
        created_at: Set(now_utc()),
        ..Default::default()
    })
    .on_conflict(
        OnConflict::column(workflow::Column::Name)
            .do_nothing()
            .to_owned(),
    )
    .exec_without_returning(&tx)
    .await?;

    let workflow = workflow::Entity::find()
        .filter(workflow::Column::Name.eq(workflow_name))
        .one(&tx)
        .await?
        .context("eval must exist")?;

    let result = workflow_run::Entity::insert(workflow_run::ActiveModel {
        workflow_id: Set(Some(workflow.id)),
        workflow_name: Set(workflow_name.to_owned()),
        status: Set(RunStatus::Running),
        input: Set(input.map(String::from)),
        started_at: Set(now_utc()),
        ..Default::default()
    })
    .exec(&tx)
    .await?;

    let run_id = result.last_insert_id;

    event::Entity::insert(event::ActiveModel {
        run_id: Set(run_id),
        event_type: Set("run.started".to_owned()),
        message: Set(Some(format!("Started eval {workflow_name}"))),
        created_at: Set(now_utc()),
        ..Default::default()
    })
    .exec(&tx)
    .await?;

    tx.commit().await?;

    Ok(run_id)
}

/// Update an eval run's output without changing status or emitting an event.
///
/// # Errors
///
/// Returns an error if the run does not exist.
///
/// TODO: make this take a serde Value rather than raw string, then have this function
/// marshal, rather than callers.
pub async fn set_run_output(db: &DatabaseConnection, run_id: i64, output: &str) -> Result<()> {
    let result = workflow_run::Entity::update_many()
        .set(workflow_run::ActiveModel {
            output: Set(Some(output.to_owned())),
            ..Default::default()
        })
        .filter(workflow_run::Column::Id.eq(run_id))
        .exec(db)
        .await?;

    if result.rows_affected == 0 {
        bail!("eval run {run_id} not found");
    }

    Ok(())
}

/// Update an eval run's input without changing status or emitting an event.
///
/// # Errors
///
/// Returns an error if the run does not exist.
pub async fn set_run_input(db: &DatabaseConnection, run_id: i64, input: &str) -> Result<()> {
    let result = workflow_run::Entity::update_many()
        .set(workflow_run::ActiveModel {
            input: Set(Some(input.to_owned())),
            ..Default::default()
        })
        .filter(workflow_run::Column::Id.eq(run_id))
        .exec(db)
        .await?;

    if result.rows_affected == 0 {
        bail!("eval run {run_id} not found");
    }

    Ok(())
}

/// Mark an eval run as completed.
///
/// # Errors
///
/// Returns an error if the run does not exist or the status update/event
/// cannot be persisted.
pub async fn complete_run(
    db: &DatabaseConnection,
    metrics_store: &MetricsStore,
    run_id: i64,
) -> Result<()> {
    let tx = db.begin().await?;
    let result = workflow_run::Entity::update_many()
        .set(workflow_run::ActiveModel {
            status: Set(RunStatus::Completed),
            finished_at: Set(Some(now_utc())),
            error: Set(None),
            ..Default::default()
        })
        .filter(workflow_run::Column::Id.eq(run_id))
        .exec(&tx)
        .await?;

    if result.rows_affected == 0 {
        bail!("eval run {run_id} not found");
    }

    event::Entity::insert(event::ActiveModel {
        run_id: Set(run_id),
        event_type: Set("run.completed".to_owned()),
        message: Set(Some(format!("Completed eval run {run_id}"))),
        created_at: Set(now_utc()),
        ..Default::default()
    })
    .exec(&tx)
    .await?;

    tx.commit().await?;
    // TODO: flush metrics prior to committing metadata
    metrics_store.flush(run_id).await?;

    Ok(())
}

/// Mark an eval run as failed.
///
/// # Errors
///
/// Returns an error if the run does not exist or the status update/event
/// cannot be persisted.
pub async fn fail_run(
    db: &DatabaseConnection,
    metrics_store: &MetricsStore,
    run_id: i64,
    error: &str,
) -> Result<()> {
    let tx = db.begin().await?;

    let result = workflow_run::Entity::update_many()
        .set(workflow_run::ActiveModel {
            status: Set(RunStatus::Failed),
            finished_at: Set(Some(now_utc())),
            error: Set(Some(error.to_owned())),
            ..Default::default()
        })
        .filter(workflow_run::Column::Id.eq(run_id))
        .exec(&tx)
        .await?;

    if result.rows_affected == 0 {
        bail!("eval run {run_id} not found");
    }

    // Cascade: mark any still-running samples as failed so the DB stays
    // consistent.
    step::Entity::update_many()
        .set(step::ActiveModel {
            status: Set(crate::db::summaries::StepStatus::Failed),
            error: Set(Some("run failed".to_owned())),
            finished_at: Set(Some(now_utc())),
            ..Default::default()
        })
        .filter(step::Column::RunId.eq(run_id))
        .filter(step::Column::Status.eq(crate::db::summaries::StepStatus::Running))
        .exec(&tx)
        .await?;

    event::Entity::insert(event::ActiveModel {
        run_id: Set(run_id),
        event_type: Set("run.failed".to_owned()),
        message: Set(Some(error.to_owned())),
        created_at: Set(now_utc()),
        ..Default::default()
    })
    .exec(&tx)
    .await?;

    tx.commit().await?;
    // TODO: flush metrics prior to committing metadata
    metrics_store.flush(run_id).await?;

    Ok(())
}

/// Reset an eval run to `running` so it can be resumed.
///
/// # Errors
///
/// Returns an error if the run does not exist or the update cannot be
/// persisted.
pub async fn resume_run(db: &DatabaseConnection, run_id: i64) -> Result<()> {
    let tx = db.begin().await?;

    let result = workflow_run::Entity::update_many()
        .set(workflow_run::ActiveModel {
            status: Set(RunStatus::Running),
            finished_at: Set(None),
            error: Set(None),
            ..Default::default()
        })
        .filter(workflow_run::Column::Id.eq(run_id))
        .exec(&tx)
        .await?;

    if result.rows_affected == 0 {
        bail!("eval run {run_id} not found");
    }

    event::Entity::insert(event::ActiveModel {
        run_id: Set(run_id),
        event_type: Set("run.resumed".to_owned()),
        message: Set(Some(format!("Resumed eval run {run_id}"))),
        created_at: Set(now_utc()),
        ..Default::default()
    })
    .exec(&tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

/// Fetch an eval run by ID.
///
/// # Errors
///
/// Returns an error if the run does not exist or cannot be read.
pub async fn get_run(db: &DatabaseConnection, run_id: i64) -> Result<WorkflowRun> {
    #[derive(Deserialize)]
    struct ModelNameExtractor {
        model: String,
    }
    let run = workflow_run::Entity::find_by_id(run_id)
        .one(db)
        .await?
        .with_context(|| format!("eval run {run_id} not found"))?;
    // TODO: parse 'model' and 'model_name' from input
    let model_name_extractor: Option<ModelNameExtractor> = match run.input {
        Some(ref input) => serde_json::from_str(input.clone().as_str()).ok(),
        None => None,
    };

    Ok(WorkflowRun {
        id: run.id,
        workflow_name: run.workflow_name,
        model_name: model_name_extractor.map(|m| m.model),
        status: run.status,
        input: run.input,
        output: run.output,
        started_at: run.started_at,
        finished_at: run.finished_at,
        error: run.error,
    })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use sea_orm::DatabaseConnection;

    use super::*;
    use crate::db::{RunStatus, init_workspace, list_events_for_run, metrics_dir, open_workspace};

    #[tokio::test]
    async fn complete_run_marks_run_completed() -> Result<()> {
        let (db, store, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;

        complete_run(&db, &store, run_id).await?;

        let run = get_run(&db, run_id).await?;
        assert_eq!(run.status, RunStatus::Completed);
        assert!(run.finished_at.is_some());
        assert!(run.error.is_none());

        let events = list_events_for_run(&db, run_id).await?;
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "run.completed"),
            "expected run.completed event"
        );

        Ok(())
    }

    #[tokio::test]
    async fn fail_run_marks_run_failed() -> Result<()> {
        let (db, store, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;

        fail_run(&db, &store, run_id, "eval failed").await?;

        let run = get_run(&db, run_id).await?;
        assert_eq!(run.status, RunStatus::Failed);
        assert!(run.finished_at.is_some());
        assert_eq!(run.error.as_deref(), Some("eval failed"));

        let events = list_events_for_run(&db, run_id).await?;
        assert!(
            events.iter().any(|event| event.event_type == "run.failed"),
            "expected run.failed event"
        );

        Ok(())
    }

    #[tokio::test]
    async fn set_run_output_persists_output() -> Result<()> {
        let (db, _store, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;

        set_run_output(&db, run_id, "hello world").await?;

        let run = get_run(&db, run_id).await?;
        assert_eq!(run.output.as_deref(), Some("hello world"));

        Ok(())
    }

    #[tokio::test]
    async fn set_run_output_leaves_other_fields_untouched() -> Result<()> {
        let (db, _store, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;

        set_run_output(&db, run_id, "hello world").await?;

        let run = get_run(&db, run_id).await?;
        assert_eq!(run.status, RunStatus::Running);
        assert!(run.finished_at.is_none());
        assert!(run.error.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn set_run_output_does_not_emit_event() -> Result<()> {
        let (db, _store, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;

        let before = list_events_for_run(&db, run_id).await?;
        set_run_output(&db, run_id, "hello world").await?;
        let after = list_events_for_run(&db, run_id).await?;

        assert_eq!(
            before.len(),
            after.len(),
            "set_run_output should not create events"
        );

        Ok(())
    }

    #[tokio::test]
    async fn set_run_output_errors_for_missing_run() -> Result<()> {
        let (db, _store, _tmpdir) = test_db().await?;

        let err = set_run_output(&db, 9999, "hello world").await.unwrap_err();
        assert!(
            err.to_string().contains("not found"),
            "unexpected error: {err:#}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn complete_run_preserves_existing_output() -> Result<()> {
        let (db, store, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;

        set_run_output(&db, run_id, "pre-computed output").await?;
        complete_run(&db, &store, run_id).await?;

        let run = get_run(&db, run_id).await?;
        assert_eq!(run.status, RunStatus::Completed);
        assert_eq!(run.output.as_deref(), Some("pre-computed output"));

        Ok(())
    }

    #[tokio::test]
    async fn resume_run_resets_to_running() -> Result<()> {
        let (db, store, _tmpdir) = test_db().await?;
        let run_id = create_run(&db, "demo", None).await?;

        set_run_output(&db, run_id, "some output").await?;
        complete_run(&db, &store, run_id).await?;

        resume_run(&db, run_id).await?;

        let run = get_run(&db, run_id).await?;
        assert_eq!(run.status, RunStatus::Running);
        assert!(run.finished_at.is_none());
        assert!(run.error.is_none());
        // output should survive a resume
        assert_eq!(run.output.as_deref(), Some("some output"));

        let events = list_events_for_run(&db, run_id).await?;
        assert!(
            events.iter().any(|event| event.event_type == "run.resumed"),
            "expected run.resumed event"
        );

        Ok(())
    }

    async fn test_db() -> Result<(DatabaseConnection, MetricsStore, tempfile::TempDir)> {
        let tmpdir = tempfile::tempdir()?;
        let root = tmpdir.path();
        init_workspace(root).await?;
        let db = open_workspace(root).await?;
        let store = MetricsStore::new(metrics_dir(root))?;
        Ok((db, store, tmpdir))
    }
}
