mod types;

use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use sea_orm::DatabaseConnection;
use serde_json::json;
use tokio::net::TcpListener;

use crate::dataset::DatasetManager;
use crate::db::steps::{self, StepDecision};
use crate::db::{self, DBUrl, SQLitePathURL};
use crate::metrics_store::MetricsStore;
use types::{
    BeginStepRequest, CompleteStepRequest, CreateRunRequest, CreateRunResponse,
    DatasetBatchRequest, DatasetBatchResponse, DatasetInitRequest, DatasetInitResponse,
    EmitMetricRequest, EmptyResponse, FailRunRequest, FailStepRequest, RunResponse,
    SetRunOutputRequest, StepDecisionResponse,
};

/// Configuration for the local qt HTTP server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Socket address the server should bind to.
    pub addr: SocketAddr,
    /// Path to the sqlite database used by API handlers.
    pub db_path: PathBuf,
}

/// Shared application state cloned into request handlers.
#[derive(Debug, Clone)]
struct AppState {
    /// Shared database connection for API handlers.
    ///
    /// Stored inside a Mutex to ensure serialization of DB access
    db: Arc<tokio::sync::Mutex<DatabaseConnection>>,
    metrics_store: MetricsStore,
}

/// Run the local qt HTTP server until it is stopped.
///
/// # Errors
///
/// Returns an error if the server cannot bind its socket or if the HTTP server
/// exits with an error.
pub async fn run_server(config: ServerConfig) -> Result<()> {
    let listener = TcpListener::bind(config.addr)
        .await
        .with_context(|| format!("failed to bind {}", config.addr))?;

    serve_listener(listener, config.db_path).await
}

/// Run the local qt HTTP server until `shutdown` resolves.
///
/// # Errors
///
/// Returns an error if the server cannot bind its socket or if the HTTP server
/// exits with an error.
pub async fn run_server_with_shutdown<F>(config: ServerConfig, shutdown: F) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind(config.addr)
        .await
        .with_context(|| format!("failed to bind {}", config.addr))?;

    serve_listener_with_shutdown(listener, config.db_path, shutdown).await
}

/// Serve the API on an already-bound listener until the process is stopped.
///
/// # Errors
///
/// Returns an error if the HTTP server exits with an error.
async fn serve_listener(listener: TcpListener, db_path: PathBuf) -> Result<()> {
    let db_url = DBUrl::SQLitePath(SQLitePathURL {
        path: db_path.clone(),
        create: false,
    });
    let db = db::open_database(db_url).await?;
    // TODO: pass the root directory in so we can derive the DB path
    // and the metrics path here, rather than deriving metrics path
    // from DB path
    let metrics_store = MetricsStore::new(db_path.parent().unwrap().join("metrics"))?;
    let app = router(AppState {
        db: Arc::new(tokio::sync::Mutex::new(db)),
        metrics_store,
    });

    axum::serve(listener, app)
        .await
        .context("qt HTTP server failed")
}

/// Serve the API on an already-bound listener until `shutdown` resolves.
///
/// # Errors
///
/// Returns an error if the HTTP server exits with an error.
async fn serve_listener_with_shutdown<F>(
    listener: TcpListener,
    db_path: PathBuf,
    shutdown: F,
) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let db_url = DBUrl::SQLitePath(SQLitePathURL {
        path: db_path.clone(),
        create: false,
    });
    let db = db::open_database(db_url).await?;
    // TODO: pass the root directory in so we can derive the DB path
    // and the metrics path here, rather than deriving metrics path
    // from DB path
    let metrics_store = MetricsStore::new(db_path.parent().unwrap().join("metrics"))?;
    let app = router(AppState {
        db: Arc::new(tokio::sync::Mutex::new(db)),
        metrics_store,
    });

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .context("qt HTTP server failed")
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/runs", post(create_run))
        // create a new run
        .route("/runs/{run_id}", get(get_run))
        // finish an eval run and mark it as successful
        .route("/runs/{run_id}/complete", post(complete_run))
        // finish an eval run and mark it as failed
        .route("/runs/{run_id}/fail", post(fail_run))
        // record the output of an eval run
        .route("/runs/{run_id}/output", post(set_run_output))
        // record one metric on an existing run
        .route("/runs/{run_id}/metrics", post(emit_metric_handler))
        // begin a new step on a run
        .route("/steps/begin", post(begin_step))
        // finish an in-progress step on an existing run, and mark it as successful
        .route("/steps/complete", post(complete_step))
        // finish an in-progress step on an existing run, and mark it as failed
        .route("/steps/fail", post(fail_step))
        // initialize a dataset source
        .route("/dataset/init", post(dataset_init))
        // fetch a batch of rows from a dataset
        .route("/datasets/batch", post(dataset_batch))
        .with_state(state)
}

async fn health() -> Json<EmptyResponse> {
    Json(EmptyResponse {})
}

async fn create_run(
    State(state): State<AppState>,
    Json(request): Json<CreateRunRequest>,
) -> Result<Json<CreateRunResponse>, ApiError> {
    let db = state.db.lock().await;
    let run_id = db::create_run(&db, &request.workflow_name, request.input.as_deref()).await?;

    Ok(Json(CreateRunResponse { run_id }))
}

async fn get_run(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
) -> Result<Json<RunResponse>, ApiError> {
    let db = state.db.lock().await;
    let run = db::get_run(&db, run_id).await?;

    Ok(Json(RunResponse {
        id: run.id,
        workflow_name: run.workflow_name,
        status: run.status,
        input: run.input,
        output: run.output,
        started_at: run.started_at,
        finished_at: run.finished_at,
        error: run.error,
    }))
}

async fn complete_run(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let db = state.db.lock().await;
    db::complete_run(&db, &state.metrics_store, run_id).await?;

    Ok(Json(EmptyResponse {}))
}

async fn set_run_output(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
    Json(request): Json<SetRunOutputRequest>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let db = state.db.lock().await;
    db::set_run_output(&db, run_id, &request.output).await?;

    Ok(Json(EmptyResponse {}))
}

async fn fail_run(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
    Json(request): Json<FailRunRequest>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let db = state.db.lock().await;
    db::fail_run(&db, &state.metrics_store, run_id, &request.error).await?;

    Ok(Json(EmptyResponse {}))
}

async fn emit_metric_handler(
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
    Json(request): Json<EmitMetricRequest>,
) -> Result<Json<EmptyResponse>, ApiError> {
    state
        .metrics_store
        .emit(
            run_id,
            None, // step_id not tracked at the API level yet
            &request.metric_name,
            request.metric_value,
            request.unit.as_deref(),
        )
        .await;

    Ok(Json(EmptyResponse {}))
}

async fn begin_step(
    State(state): State<AppState>,
    Json(request): Json<BeginStepRequest>,
) -> Result<Json<StepDecisionResponse>, ApiError> {
    let db = state.db.lock().await;
    let decision =
        steps::begin_step(&db, request.run_id, &request.step_key, &request.input_hash).await?;

    Ok(Json(match decision {
        StepDecision::Run { step_id } => StepDecisionResponse::Run { step_id },
        StepDecision::Reuse { output } => StepDecisionResponse::Reuse { output },
    }))
}

async fn complete_step(
    State(state): State<AppState>,
    Json(request): Json<CompleteStepRequest>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let db = state.db.lock().await;
    steps::complete_step(&db, request.step_id, &request.output).await?;

    Ok(Json(EmptyResponse {}))
}

async fn fail_step(
    State(state): State<AppState>,
    Json(request): Json<FailStepRequest>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let db = state.db.lock().await;
    steps::fail_step(&db, request.step_id, &request.error).await?;

    Ok(Json(EmptyResponse {}))
}

async fn dataset_init(
    Json(request): Json<DatasetInitRequest>,
) -> Result<Json<DatasetInitResponse>, ApiError> {
    let manager = DatasetManager::new()?;
    let dataset_id = parse_hf_source(&request.source)?;
    let info = manager
        .init(
            dataset_id,
            request.config.as_deref(),
            request.split.as_deref(),
            request.revision.as_deref(),
        )
        .await?;

    Ok(Json(DatasetInitResponse {
        total_rows: info.total_rows,
        available_splits: info.available_splits,
        selected_split: info.selected_split,
        config: info.config,
    }))
}

async fn dataset_batch(
    Json(request): Json<DatasetBatchRequest>,
) -> Result<Json<DatasetBatchResponse>, ApiError> {
    let manager = DatasetManager::new()?;
    let dataset_id = parse_hf_source(&request.source)?;
    let rows = manager
        .batch(
            dataset_id,
            &request.config,
            &request.split,
            request.offset,
            request.limit,
            request.revision.as_deref(),
        )
        .await?;

    Ok(Json(DatasetBatchResponse { rows }))
}

fn parse_hf_source(source: &str) -> Result<&str, ApiError> {
    source
        .strip_prefix("huggingface://")
        .or_else(|| source.strip_prefix("hf://"))
        .ok_or_else(|| {
            ApiError(anyhow::anyhow!(
                "unsupported dataset source `{source}`; expected `huggingface://...` or `hf://...`"
            ))
        })
}

struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("{:#}", self.0) })),
        )
            .into_response()
    }
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    use crate::client::QuantilesClient;
    use crate::db::{init_workspace, workspace_db_path};

    #[tokio::test]
    async fn set_run_output_endpoint_persists_output() -> Result<(), Box<dyn std::error::Error>> {
        let tmpdir = tempfile::tempdir()?;
        let root = tmpdir.path();
        init_workspace(root).await?;
        let db_path = workspace_db_path(root);

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = super::serve_listener_with_shutdown(listener, db_path, async {
                let _ = shutdown_rx.await;
            })
            .await;
        });

        let base_url = format!("http://{addr}");
        let client = QuantilesClient::new(&base_url);

        for _ in 0..50 {
            if client.health().await.is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }

        let run_id = client.create_run("server-test", None).await?;
        client.set_run_output(run_id, "server-output").await?;

        let run = reqwest::get(format!("{base_url}/runs/{run_id}"))
            .await?
            .json::<super::types::RunResponse>()
            .await?;
        assert_eq!(run.output.as_deref(), Some("server-output"));

        let _ = shutdown_tx.send(());
        Ok(())
    }
}
