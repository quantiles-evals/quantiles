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
        json: bool,
    ) -> Result<Self> {
        let run = db::get_run(db, run_id).await?;
        let steps = db::list_steps_for_run(db, run_id).await?;
        let mut metrics = metrics_store.list_aggregate_for_run(run_id).await?;
        super::super::custom_nocode_metrics::append_requested_output_metrics(
            &mut metrics,
            run.input.as_deref(),
            &steps,
            json,
            run.finished_at.unwrap_or(run.started_at),
        )?;
        Ok(Self {
            run,
            steps,
            metrics,
        })
    }
}
