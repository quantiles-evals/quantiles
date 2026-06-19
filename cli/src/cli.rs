use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "qt")]
#[command(
    about = "A full-featured local-native toolchain for running and analyzing AI evals at scale"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize or update a local Quantiles workspace.
    Init,
    /// Show a list of all eval runs.
    List {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Compare two evals side-by-side.
    Compare {
        run_a: i64,
        run_b: i64,
        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Run an eval.
    Run {
        workflow_name: String,
        #[arg(long)]
        input: Option<String>,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Resume a running or failed eval run.
    Resume {
        run_id: i64,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Start the local qt HTTP server. Mostly used for local development or custom setups.
    Serve {
        #[arg(long, default_value = "127.0.0.1:8765")]
        addr: String,
    },
    /// Show details of one eval run.
    Show {
        run_id: i64,
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
}
