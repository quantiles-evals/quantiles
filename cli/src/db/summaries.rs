use sea_orm::FromQueryResult;
use sea_orm::entity::prelude::*;

#[derive(Debug, FromQueryResult)]
pub struct WorkflowRunSummary {
    pub id: i64,
    pub workflow_name: String,
    pub status: RunStatus,
    pub started_at: time::OffsetDateTime,
    pub finished_at: Option<time::OffsetDateTime>,
    pub steps_count: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkflowRun {
    pub id: i64,
    pub workflow_name: String,
    pub model_name: Option<String>,
    pub status: RunStatus,
    pub input: Option<String>,
    pub output: Option<String>,
    pub started_at: time::OffsetDateTime,
    pub finished_at: Option<time::OffsetDateTime>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StepSummary {
    pub id: i64,
    pub step_key: String,
    pub input_hash: String,
    pub status: StepStatus,
    pub output: Option<String>,
    pub error: Option<String>,
    pub started_at: time::OffsetDateTime,
    pub finished_at: Option<time::OffsetDateTime>,
}

#[derive(Debug, serde::Serialize)]
pub struct EventSummary {
    pub id: i64,
    pub event_type: String,
    pub message: Option<String>,
    pub created_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricPointSummary {
    pub metric_name: String,
    pub metric_value: f64,
    pub unit: Option<String>,
    pub step_id: Option<i64>,
    pub created_at: time::OffsetDateTime,
}

#[derive(
    Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, serde::Serialize, serde::Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "canceled")]
    Canceled,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, serde::Serialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "failed")]
    Failed,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            RunStatus::Running => "running",
            RunStatus::Completed => "completed",
            RunStatus::Failed => "failed",
            RunStatus::Canceled => "canceled",
        };
        write!(f, "{s}")
    }
}

impl std::fmt::Display for StepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            StepStatus::Running => "running",
            StepStatus::Completed => "completed",
            StepStatus::Failed => "failed",
        };
        write!(f, "{s}")
    }
}
