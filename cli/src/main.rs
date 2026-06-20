mod cli;
mod commands;

use std::time::Instant;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let process_start = Instant::now();

    // TODO: allow number of total threads to be configurable, and possibly default
    // to something other than the default of 61, which is specified in the implementation
    // of new_multi_thread()
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main(process_start))
}

async fn async_main(process_start: Instant) -> Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        cli::Command::Init => commands::init().await,
        cli::Command::List { json } => commands::list(json).await,
        cli::Command::Compare { run_a, run_b, json } => commands::compare(run_a, run_b, json).await,
        cli::Command::Run {
            workflow_name,
            input,
            json,
        } => commands::run(&workflow_name, input.as_deref(), json, process_start).await,
        cli::Command::Resume { run_id, json } => {
            commands::resume(run_id, json, process_start).await
        }
        cli::Command::Serve { addr } => commands::serve(&addr).await,
        cli::Command::Show { run_id, json } => commands::show(run_id, json).await,
    }
}
