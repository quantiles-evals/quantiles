mod common;
mod dataset_runner;
mod financebench;
mod input;
mod output;
mod pubmedqa;
mod similarity;
mod simpleqa_verified;

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
    /// Unique name of the built-in (e.g. "financebench").
    fn name(&self) -> &'static str;
    /// Execute the builtin eval.
    async fn execute(&self, ctx: BuiltinContext<'_>) -> Result<()>;
}

/// Try to resolve a builtin by its name.
#[must_use]
pub fn resolve(name: &str) -> Option<Box<dyn BuiltinWorkflow>> {
    if name == financebench::FinancebenchBuiltin.name() {
        Some(Box::new(financebench::FinancebenchBuiltin))
    } else if name == pubmedqa::PubmedqaBuiltin.name() {
        Some(Box::new(pubmedqa::PubmedqaBuiltin))
    } else if name == simpleqa_verified::SimpleqaVerifiedBuiltin.name() {
        Some(Box::new(simpleqa_verified::SimpleqaVerifiedBuiltin))
    } else {
        None
    }
}
