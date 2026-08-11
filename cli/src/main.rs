mod cli;
mod commands;

use std::ffi::OsStr;
use std::process::ExitCode;
use std::time::Instant;

use anyhow::Result;
use clap::Parser;

fn main() -> ExitCode {
    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return handle_parse_error(&error),
    };
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

fn handle_parse_error(error: &clap::Error) -> ExitCode {
    let exit_code = ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(1));
    if add_json_requested() {
        let message = error.to_string();
        if error.use_stderr() {
            println!("{}", serde_json::json!({ "error": message.trim_end() }));
        } else {
            println!("{}", serde_json::json!({ "output": message.trim_end() }));
        }
    } else if let Err(print_error) = error.print() {
        eprintln!("Error: failed to print command-line error: {print_error}");
    }
    exit_code
}

fn add_json_requested() -> bool {
    let mut args = std::env::args_os().skip(1);
    args.next()
        .is_some_and(|argument| argument == OsStr::new("add"))
        && args.any(|argument| argument == OsStr::new("--json"))
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
