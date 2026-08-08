mod common;
mod custom_nocode;
mod dataset_runner;
mod output;

pub use custom_nocode::CustomNoCodeBuiltin;
pub use custom_nocode::metrics::{
    compute_output_metrics as compute_custom_nocode_output_metrics,
    output_metrics_requested as custom_nocode_output_metrics_requested,
};

use anyhow::Result;
use async_trait::async_trait;
use sea_orm::DatabaseConnection;

use crate::metrics_store::MetricsStore;

/// Context provided to every builtin eval execution.
pub struct BuiltinContext<'a> {
    pub db: &'a DatabaseConnection,
    pub metrics_store: &'a MetricsStore,
    pub run_id: i64,
    pub workflow_name: &'a str,
    pub input: Option<&'a str>,
    /// Suppress progress bars and other interactive output.
    pub quiet: bool,
}

/// Trait for builtin evals that run natively inside the CLI.
#[async_trait]
pub trait BuiltinWorkflow: Send + Sync {
    /// Unique name of the native workflow.
    fn name(&self) -> String;
    /// Execute the builtin eval and persist its metrics/output.
    async fn execute(&self, ctx: BuiltinContext<'_>) -> Result<()>;
}
