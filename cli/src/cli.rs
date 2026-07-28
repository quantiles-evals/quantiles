use clap::{Parser, Subcommand};

pub const VERSION: &str = match option_env!("QT_CLI_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

#[derive(Debug, Parser)]
#[command(
    name = "qt",
    // By default, clap builds in a `--version` flag that produces output with the version number
    // prefixed by the binary name. For example:
    //
    // qt v0.1.2.
    //
    // This setting disables this default output, so we can manually output _just_ the actual version
    // number, so that agents and scripts don't have to parse out the `qt...` prefix.
    disable_version_flag = true,
    arg_required_else_help = true
)]
#[command(
    about = "A full-featured local-native toolchain for running and analyzing AI evals at scale"
)]
pub struct Cli {
    /// Print the qt version.
    #[arg(long, exclusive = true)]
    pub version: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
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
