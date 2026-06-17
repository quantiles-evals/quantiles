use anyhow::{Context, Result};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};

use crate::db::entities::event;
use crate::db::summaries::EventSummary;

/// List events for an eval run.
///
/// # Errors
///
/// Returns an error if the query cannot be prepared or read.
pub async fn list_events_for_run(
    db: &DatabaseConnection,
    run_id: i64,
) -> Result<Vec<EventSummary>> {
    let events = event::Entity::find()
        .filter(event::Column::RunId.eq(run_id))
        .order_by_asc(event::Column::CreatedAt)
        .order_by_asc(event::Column::Id)
        .all(db)
        .await
        .context("failed to list eval run events")?;

    Ok(events
        .into_iter()
        .map(|e| EventSummary {
            id: e.id,
            event_type: e.event_type,
            message: e.message,
            created_at: e.created_at,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use crate::db::{create_run, init_workspace, metrics_dir, open_workspace};
    use crate::metrics_store::MetricsStore;

    #[tokio::test]
    async fn emit_metric_stores_metric_point() -> anyhow::Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let root = tmpdir.path();
        init_workspace(root).await?;
        let db = open_workspace(root).await?;
        let store = MetricsStore::new(metrics_dir(root))?;

        let run_id = create_run(&db, "demo", None).await?;

        store
            .emit(run_id, None, "latency_ms", 120.5, Some("ms"))
            .await;
        store.emit(run_id, None, "tokens_used", 42.0, None).await;
        store.flush(run_id).await?;

        let metrics = store.list_for_run(run_id).await?;
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[0].metric_name, "latency_ms");
        assert!((metrics[0].metric_value - 120.5).abs() < f64::EPSILON);
        assert_eq!(metrics[0].unit.as_deref(), Some("ms"));
        assert_eq!(metrics[1].metric_name, "tokens_used");
        assert!((metrics[1].metric_value - 42.0).abs() < f64::EPSILON);
        assert_eq!(metrics[1].unit.as_deref(), None);

        Ok(())
    }
}
