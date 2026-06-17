use time::Duration;

use anyhow::Result;
use qt::db::{self, RunStatus, StepSummary, WorkflowRun};
use qt::metrics_store::MetricsStore;
use sea_orm::DatabaseConnection;
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct RunInfo {
    pub(super) id: i64,
    pub(super) workflow_name: String,
    pub(super) status: RunStatus,
    pub(super) model_name: Option<String>,
    pub(super) duration: Option<Duration>,
}

impl RunInfo {
    pub(super) fn completed_marker(&self) -> &'static str {
        match self.status {
            RunStatus::Completed => "COMPLETE",
            RunStatus::Failed => "FAILED",
            RunStatus::Canceled => "CANCELLED",
            RunStatus::Running => "RUNNING",
        }
    }
}

pub(super) struct RunData {
    pub(super) run: WorkflowRun,
    pub(super) steps: Vec<StepSummary>,
    pub(super) metrics: Vec<db::MetricPointSummary>,
}

impl RunData {
    pub(super) async fn fetch_by_id(
        db: &DatabaseConnection,
        metrics_store: &MetricsStore,
        run_id: i64,
    ) -> Result<Self> {
        let run = db::get_run(db, run_id).await?;
        let steps = db::list_steps_for_run(db, run_id).await?;
        let metrics = metrics_store.list_aggregate_for_run(run_id).await?;
        Ok(Self {
            run,
            steps,
            metrics,
        })
    }
}
