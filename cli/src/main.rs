mod cli;
mod commands;

use std::process::ExitCode;
use std::time::Instant;

use anyhow::Result;
use clap::Parser;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    let json_errors = matches!(
        &cli.command,
        Some(cli::Command::Add { json: true, .. } | cli::Command::Resume { json: true, .. })
    );

    match try_main(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if json_errors => {
            println!("{}", serde_json::json!({ "error": format!("{error:#}") }));
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("Error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn try_main(cli: cli::Cli) -> Result<()> {
    // `fastembed`'s dependency tree enables another Rustls crypto provider alongside AWS-LC.
    // Install AWS-LC explicitly, to avoid Rustls-related panics when multiple providers are
    // enabled.
    connectrpc::rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("failed to install the AWS-LC Rustls crypto provider"))?;

    let process_start = Instant::now();

    // TODO: allow number of total threads to be configurable, and possibly default
    // to something other than the default of 61, which is specified in the implementation
    // of new_multi_thread()
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main(cli, process_start))
}

async fn async_main(cli: cli::Cli, process_start: Instant) -> Result<()> {
    if cli.version {
        println!("{}", cli::VERSION);
        return Ok(());
    }

    match cli.command.expect("clap requires a subcommand") {
        cli::Command::Add {
            benchmark_name,
            remote_url,
            json,
        } => commands::add(&benchmark_name, remote_url.as_deref(), json).await,
        cli::Command::Init => commands::init().await,
        cli::Command::List { json } => commands::list(json).await,
        cli::Command::Compare { run_a, run_b, json } => commands::compare(run_a, run_b, json).await,
        cli::Command::Run {
            workflow_name,
            input,
            remote_url,
            json,
        } => {
            commands::run(
                &workflow_name,
                input.as_deref(),
                remote_url.as_deref(),
                json,
                process_start,
            )
            .await
        }
        cli::Command::Resume { run_id, json } => {
            commands::resume(run_id, json, process_start).await
        }
        cli::Command::Serve { addr } => commands::serve(&addr).await,
        cli::Command::Show { run_id, json } => commands::show(run_id, json).await,
    }
}
