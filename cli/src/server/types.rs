use crate::db::RunStatus;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request body for `POST /runs`.
#[derive(Debug, Deserialize)]
pub struct CreateRunRequest {
    pub workflow_name: String,
    pub input: Option<String>,
}

/// Response body for `POST /runs`.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRunResponse {
    pub run_id: i64,
}

/// Response body for `GET /runs/{run_id}`.
#[derive(Debug, Serialize, Deserialize)]
pub struct RunResponse {
    pub id: i64,
    pub workflow_name: String,
    pub status: RunStatus,
    pub input: Option<String>,
    pub output: Option<String>,
    pub started_at: time::OffsetDateTime,
    pub finished_at: Option<time::OffsetDateTime>,
    pub error: Option<String>,
}

/// Request body for `POST /runs/{run_id}/output`.
#[derive(Debug, Deserialize)]
pub struct SetRunOutputRequest {
    pub output: String,
}

/// Request body for `POST /steps/begin`.
#[derive(Debug, Deserialize)]
pub struct BeginStepRequest {
    pub run_id: i64,
    pub step_key: String,
    pub input_hash: String,
}

/// Response body for `POST /steps/begin`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum StepDecisionResponse {
    Run { step_id: i64 },
    Reuse { output: String },
}

/// Request body for `POST /steps/complete`.
#[derive(Debug, Deserialize)]
pub struct CompleteStepRequest {
    pub step_id: i64,
    pub output: String,
}

/// Request body for `POST /steps/fail`.
#[derive(Debug, Deserialize)]
pub struct FailStepRequest {
    pub step_id: i64,
    pub error: String,
}

/// Request body for `POST /runs/{run_id}/fail`.
#[derive(Debug, Deserialize)]
pub struct FailRunRequest {
    pub error: String,
}

/// Request body for `POST /runs/{run_id}/metrics`.
#[derive(Debug, Deserialize)]
pub struct EmitMetricRequest {
    pub metric_name: String,
    pub metric_value: f64,
    pub unit: Option<String>,
}

/// Request body for `POST /dataset/init`.
#[derive(Debug, Deserialize)]
pub struct DatasetInitRequest {
    pub source: String,
    pub config: Option<String>,
    pub split: Option<String>,
    pub revision: Option<String>,
}

/// Response body for `POST /dataset/init`.
#[derive(Debug, Serialize)]
pub struct DatasetInitResponse {
    pub total_rows: Option<usize>,
    pub available_splits: Vec<String>,
    pub selected_split: String,
    pub config: String,
}

/// Request body for `POST /datasets/batch`.
#[derive(Debug, Deserialize)]
pub struct DatasetBatchRequest {
    pub source: String,
    pub config: String,
    pub split: String,
    pub offset: usize,
    pub limit: usize,
    pub revision: Option<String>,
}

/// Response body for `POST /datasets/batch`.
#[derive(Debug, Serialize)]
pub struct DatasetBatchResponse {
    pub rows: Vec<Value>,
}

/// Generic empty JSON response used for endpoints that return no body.
#[derive(Debug, Serialize, Deserialize)]
pub struct EmptyResponse {}
