use std::collections::HashMap;
use std::net::TcpStream;
use std::process::{Command as StdCommand, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use comfy_table::{Cell, ContentArrangement, Table, presets::NOTHING};
use serde::Serialize;

use qt::builtins;
use qt::client::QuantilesClient;
use qt::db;
use qt::db::MetricPointSummary;
use qt::metrics_store::MetricsStore;
use qt::server::{self, ServerConfig};

#[expect(clippy::too_many_lines)]
pub async fn run(
    workflow_name: &str,
    input: Option<&str>,
    resume: Option<i64>,
    json: bool,
    command: &[String],
    process_start: Instant,
) -> Result<()> {
    if command.is_empty() {
        if builtins::resolve(workflow_name).is_some() {
            return run_builtin(workflow_name, input, resume, json, process_start).await;
        }
        bail!("no command provided and `{workflow_name}` is not a builtin eval");
    }

    let cwd = std::env::current_dir()?;
    let root = db::resolve_workspace_root(&cwd, true).await?;

    let base_url =
        std::env::var("QUANTILES_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8765".to_owned());
    let addr = base_url
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    let server_started_by_us = !is_server_reachable(addr);
    let server_handle = if server_started_by_us {
        let db_path = db::workspace_db_path(&root);
        Some(start_background_server(addr, json, db_path)?)
    } else {
        None
    };

    wait_for_health(&base_url)?;

    let db = db::open_workspace(&root).await?;

    let run_id = if let Some(existing_run_id) = resume {
        let run = db::get_run(&db, existing_run_id).await?;
        if run.status == db::RunStatus::Completed {
            bail!(
                "run {existing_run_id} is already completed; \
                 create a new run or resume a running/failed one"
            );
        }
        db::resume_run(&db, existing_run_id).await?;
        if !json {
            println!("Resuming eval run {existing_run_id} ({workflow_name})");
        }
        existing_run_id
    } else {
        let new_run_id = db::create_run(&db, workflow_name, input).await?;
        if !json {
            println!("Created run {new_run_id}");
        }
        new_run_id
    };

    if !json {
        println!("Executing: {}", command.join(" "));
    }

    let program = &command[0];
    let args = &command[1..];

    let command_output =
        run_child_command(program, args, run_id, workflow_name, &base_url, input, json)?;

    let duration = process_start.elapsed();

    // TODO: do not use the client here. do DB operations directly
    let client = QuantilesClient::new(&base_url);
    let run_status;
    let run_error;
    if command_output.status.success() {
        client.complete_run(run_id).await?;
        run_status = "completed";
        run_error = None;
        if !json {
            println!(
                "Run {run_id} completed successfully in {:.2}s",
                duration.as_secs_f64()
            );
        }
    } else {
        let exit_msg = match command_output.status.code() {
            Some(code) => format!("exited with status {code}"),
            None => "was terminated by signal".to_owned(),
        };
        client.fail_run(run_id, &exit_msg).await?;
        run_status = "failed";
        run_error = Some(exit_msg.clone());
        if !json {
            println!("Run {run_id} failed: {exit_msg}");
        }
    }

    if let Some((handle, shutdown)) = server_handle {
        let _ = shutdown.send(());
        let _ = handle.join();
    }

    if json {
        let response = RunJsonOutput {
            run_id,
            workflow_name,
            input,
            resumed: resume.is_some(),
            command,
            base_url: &base_url,
            server_started_by_us,
            status: run_status,
            success: command_output.status.success(),
            exit_code: command_output.status.code(),
            duration_seconds: duration.as_secs_f64(),
            stdout: command_output.stdout,
            stderr: command_output.stderr,
            error: run_error,
        };
        println!("{}", serde_json::to_string_pretty(&response)?);
    }

    Ok(())
}

async fn run_builtin(
    workflow_name: &str,
    input: Option<&str>,
    resume: Option<i64>,
    json: bool,
    process_start: Instant,
) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root = db::resolve_workspace_root(&cwd, true).await?;

    let config = qt::config::load()?;
    let config_input: Option<String> = if input.is_none() {
        config.benchmarks.get(workflow_name).and_then(|bench| {
            if bench.samples.is_none() && bench.model.is_none() && bench.max_workers.is_none() {
                return None;
            }
            let input = BuiltinConfigInput {
                limit: bench.samples,
                model: bench.model.clone(),
                max_workers: bench.max_workers,
            };
            Some(
                serde_json::to_string(&input)
                    .expect("infallible serialization of BuiltinConfigInput"),
            )
        })
    } else {
        None
    };

    let effective_input: Option<&str> = if input.is_some() {
        input
    } else {
        config_input.as_deref()
    };

    let db = db::open_workspace(&root).await?;
    let metrics_store = MetricsStore::new(db::metrics_dir(&root))?;

    let run_id = if let Some(existing_run_id) = resume {
        let run = db::get_run(&db, existing_run_id).await?;
        if run.status == db::RunStatus::Completed {
            bail!(
                "run {existing_run_id} is already completed; \
                 create a new run or resume a running/failed one"
            );
        }
        db::resume_run(&db, existing_run_id).await?;
        if !json {
            println!("Resuming eval run {existing_run_id} ({workflow_name})");
        }
        existing_run_id
    } else {
        let new_run_id = db::create_run(&db, workflow_name, effective_input).await?;
        if !json {
            println!("Created run {new_run_id}");
        }
        new_run_id
    };

    let builtin = builtins::resolve(workflow_name)
        .with_context(|| format!("builtin `{workflow_name}` not found"))?;

    let builtin_result = builtin
        .execute(builtins::BuiltinContext {
            db: &db,
            metrics_store: &metrics_store,
            run_id,
            workflow_name,
            input: effective_input,
            quiet: json,
        })
        .await;

    let duration = process_start.elapsed();

    match &builtin_result {
        Ok(()) => {
            db::complete_run(&db, &metrics_store, run_id).await?;
            if !json {
                println!(
                    "Run {run_id} completed successfully in {:.2}s",
                    duration.as_secs_f64()
                );
            }
        }
        Err(err) => {
            let msg = format!("{err:#}");
            db::fail_run(&db, &metrics_store, run_id, &msg).await?;
            if !json {
                println!("Run {run_id} failed: {msg}");
            }
        }
    }

    let aggregate = metrics_store.list_aggregate_for_run(run_id).await?;

    if json {
        let metrics_map: HashMap<String, f64> = aggregate
            .iter()
            .map(|m| (m.metric_name.clone(), m.metric_value))
            .collect();
        let output = BuiltinRunJsonOutput {
            aggregate_metrics: metrics_map,
            run_id,
        };
        println!("{}", serde_json::to_string(&output)?);
    } else {
        print_aggregate_metrics_table(&aggregate);
        println!("\nRun `qt show {run_id} --verbose` for sample-level details.");
    }

    builtin_result
}

fn print_aggregate_metrics_table(metrics: &[MetricPointSummary]) {
    if metrics.is_empty() {
        println!("\nAggregate metrics");
        println!("  No aggregate metrics found.");
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(NOTHING)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["METRIC", "VALUE", "UNIT"]);

    for metric in metrics {
        table.add_row(vec![
            Cell::new(&metric.metric_name),
            Cell::new(metric.metric_value),
            Cell::new(metric.unit.as_deref().unwrap_or("-")),
        ]);
    }

    println!("\nAggregate metrics");
    println!("{table}");
}

/// JSON payload emitted by `qt run <builtin> --json`.
#[derive(Serialize)]
struct BuiltinRunJsonOutput {
    aggregate_metrics: HashMap<String, f64>,
    run_id: i64,
}

/// Config input shape auto-generated from `quantiles.toml` `[benchmarks.*]`.
#[derive(Serialize, Default)]
struct BuiltinConfigInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<qt::llm::Sampler>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_workers: Option<usize>,
}

/// Captured result of running the user command.
struct ChildCommandOutput {
    /// Process exit status reported by the operating system.
    status: ExitStatus,
    /// Captured standard output, populated only for `--json` mode.
    stdout: String,
    /// Captured standard error, populated only for `--json` mode.
    stderr: String,
}

/// Machine-readable result emitted by `qt run --json`.
#[derive(Serialize)]
struct RunJsonOutput<'a> {
    /// Workflow run identifier.
    run_id: i64,
    /// Name of the eval that was run.
    workflow_name: &'a str,
    /// Structured run input, if one was provided.
    input: Option<&'a str>,
    /// Whether this command resumed an existing run.
    resumed: bool,
    /// User command and arguments that were executed.
    command: &'a [String],
    /// Local qt API base URL injected into the child process.
    base_url: &'a str,
    /// Whether `qt run` started the local server for this command.
    server_started_by_us: bool,
    /// Final eval run status recorded in the database.
    ///
    /// TODO: make this an enum
    status: &'a str,
    /// Whether the child process exited successfully.
    success: bool,
    /// Child process exit code, when available.
    exit_code: Option<i32>,
    /// Wall-clock duration of the child process in seconds.
    duration_seconds: f64,
    /// Captured child standard output.
    stdout: String,
    /// Captured child standard error.
    stderr: String,
    /// Failure reason recorded for the run, if any.
    error: Option<String>,
}

/// Execute the user command with qt environment variables injected.
///
/// In JSON mode, standard output and standard error are captured so `qt run`
/// can emit a single machine-readable JSON object. Otherwise, the child process
/// inherits the terminal streams and output is shown live.
///
/// # Errors
///
/// Returns an error if the command cannot be spawned or waited on.
fn run_child_command(
    program: &str,
    args: &[String],
    run_id: i64,
    workflow_name: &str,
    base_url: &str,
    input: Option<&str>,
    json: bool,
) -> Result<ChildCommandOutput> {
    let mut command = StdCommand::new(program);
    command
        .args(args)
        .env("QUANTILES_RUN_ID", run_id.to_string())
        .env("QUANTILES_WORKFLOW_NAME", workflow_name)
        .env("QUANTILES_BASE_URL", base_url)
        .env("QUANTILES_INPUT", input.unwrap_or("{}"));

    if json {
        let output = command
            .output()
            .with_context(|| format!("failed to spawn command `{program}`"))?;
        Ok(ChildCommandOutput {
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    } else {
        let mut child = command
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("failed to spawn command `{program}`"))?;

        let status = child.wait().context("failed to wait for command exit")?;
        Ok(ChildCommandOutput {
            status,
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

fn is_server_reachable(addr: &str) -> bool {
    TcpStream::connect(addr).is_ok()
}

fn wait_for_health(base_url: &str) -> Result<()> {
    let addr = base_url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let deadline = Instant::now() + Duration::from_secs(5);

    while Instant::now() < deadline {
        if TcpStream::connect(addr).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    bail!("qt server at {base_url} did not become reachable within 5 seconds")
}

/// Start the local qt server in a background thread for the lifetime of a run.
///
/// Returns the server thread handle and a shutdown sender. When `quiet` is true,
/// startup and failure messages are suppressed so `qt run --json` can keep
/// standard output machine-readable.
///
/// # Errors
///
/// Returns an error if the address cannot be parsed.
fn start_background_server(
    addr: &str,
    quiet: bool,
    db_path: std::path::PathBuf,
) -> Result<(thread::JoinHandle<()>, mpsc::Sender<()>)> {
    let addr = addr
        .parse()
        .with_context(|| format!("invalid server address `{addr}`"))?;
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

    let handle = thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(err) => {
                eprintln!("failed to start qt background runtime: {err:#}");
                return;
            }
        };

        if !quiet {
            println!("Serving Quantiles HTTP API on http://{addr}");
        }
        let result = runtime.block_on(server::run_server_with_shutdown(
            ServerConfig { addr, db_path },
            async move {
                let _ = tokio::task::spawn_blocking(move || shutdown_rx.recv()).await;
            },
        ));

        if let Err(err) = result
            && !quiet
        {
            eprintln!("qt background server failed: {err:#}");
        }
    });

    Ok((handle, shutdown_tx))
}
