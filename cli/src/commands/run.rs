use std::collections::HashMap;
use std::net::TcpStream;
use std::process::{Command as StdCommand, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use comfy_table::{Cell, ContentArrangement, Table, presets::NOTHING};
use sea_orm::DatabaseConnection;
use serde::Serialize;

use qt::builtins;
use qt::client::QuantilesClient;
use qt::db;
use qt::db::MetricPointSummary;
use qt::metrics_store::MetricsStore;
use qt::server::{self, ServerConfig};

pub async fn run(
    workflow_name: &str,
    cli_input: Option<&str>,
    json: bool,
    process_start: Instant,
) -> Result<()> {
    let config = qt::config::load()?;

    let bench_config = config.benchmarks.get(workflow_name);

    match bench_config {
        Some(bench) => {
            bench.validate()?;
            match bench {
                qt::config::BenchmarkConfig::Builtin(b) => {
                    let (effective_input, _) = assemble_builtin_input(Some(b), cli_input);
                    run_builtin_workflow(
                        workflow_name,
                        effective_input.as_deref(),
                        json,
                        process_start,
                    )
                    .await
                }
                qt::config::BenchmarkConfig::CustomCode(c) => {
                    let (merged_input, overridden_keys) =
                        merge_inputs(c.input.as_ref(), cli_input)?;
                    let warning = if overridden_keys.is_empty() {
                        None
                    } else {
                        Some(format!(
                            "--input overrides config input for keys: {}",
                            overridden_keys.join(", ")
                        ))
                    };
                    let command = &c.command;

                    let cwd = std::env::current_dir()?;
                    let root = db::resolve_workspace_root(&cwd, true).await?;
                    let db = db::open_workspace(&root).await?;
                    let run_id =
                        db::create_run(&db, workflow_name, merged_input.as_deref()).await?;

                    if !json {
                        println!("Created run {run_id}");
                    }

                    execute_custom(
                        run_id,
                        workflow_name,
                        merged_input.as_deref(),
                        command,
                        json,
                        process_start,
                        warning.as_deref(),
                    )
                    .await
                }
                qt::config::BenchmarkConfig::CustomNoCode(c) => {
                    let input = assemble_custom_nocode_input(c, cli_input);

                    let cwd = std::env::current_dir()?;
                    let root = db::resolve_workspace_root(&cwd, true).await?;
                    let db = db::open_workspace(&root).await?;
                    let metrics_store = MetricsStore::new(db::metrics_dir(&root))?;
                    let run_id = db::create_run(&db, workflow_name, input.as_deref()).await?;

                    if !json {
                        println!("Created run {run_id}");
                    }

                    let builtin =
                        Box::new(qt::builtins::CustomNoCodeBuiltin::new(
                            workflow_name.to_owned(),
                        ));
                    execute_builtin(
                        &db,
                        &metrics_store,
                        run_id,
                        workflow_name,
                        builtin,
                        input.as_deref(),
                        json,
                        process_start,
                    )
                    .await
                }
            }
        }
        None => {
            if builtins::resolve(workflow_name).is_some() {
                let (effective_input, _) = assemble_builtin_input(None, cli_input);
                run_builtin_workflow(
                    workflow_name,
                    effective_input.as_deref(),
                    json,
                    process_start,
                )
                .await
            } else {
                bail!("no config section found for benchmark `{workflow_name}`");
            }
        }
    }
}

fn assemble_builtin_input(
    bench: Option<&qt::config::BuiltinBenchmarkConfig>,
    cli_input: Option<&str>,
) -> (Option<String>, Vec<String>) {
    if let Some(cli_str) = cli_input {
        return (Some(cli_str.to_owned()), Vec::new());
    }

    if let Some(bench) = bench {
        if bench.samples.is_none() && bench.model.is_none() && bench.max_workers.is_none() {
            return (None, Vec::new());
        }

        let input = BuiltinConfigInput {
            limit: bench.samples,
            model: bench.model.clone(),
            max_workers: bench.max_workers,
        };

        let json =
            serde_json::to_string(&input).expect("infallible serialization of BuiltinConfigInput");

        (Some(json), Vec::new())
    } else {
        (None, Vec::new())
    }
}

/// Config input shape for `custom_nocode` benchmarks auto-generated from
/// `quantiles.toml` `[benchmarks.*]`.
#[derive(Serialize)]
struct CustomNoCodeConfigInput<'a> {
    style: &'a str,
    dataset: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: &'a Option<qt::llm::Sampler>,
    prompt_template_file: &'a str,
    prompt_column: &'a str,
    golden_column: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_workers: Option<usize>,
}

fn assemble_custom_nocode_input(
    bench: &qt::config::CustomNoCodeBenchmarkConfig,
    cli_input: Option<&str>,
) -> Option<String> {
    if let Some(cli_str) = cli_input {
        return Some(cli_str.to_owned());
    }

    let input = CustomNoCodeConfigInput {
        style: "qa",
        dataset: &bench.dataset,
        model: &bench.model,
        prompt_template_file: &bench.qa.prompt_template_file,
        prompt_column: &bench.qa.prompt_column,
        golden_column: &bench.qa.golden_column,
        limit: bench.qa.limit,
        max_workers: bench.qa.max_workers,
    };

    let json = serde_json::to_string(&input).expect("infallible serialization");
    Some(json)
}

fn merge_inputs(
    config_input: Option<&HashMap<String, serde_json::Value>>,
    cli_input: Option<&str>,
) -> Result<(Option<String>, Vec<String>)> {
    let mut merged = match config_input {
        Some(v) => serde_json::Value::Object(v.clone().into_iter().collect()),
        None => serde_json::Value::Object(serde_json::Map::default()),
    };

    let mut overridden = Vec::new();

    if let Some(cli_str) = cli_input {
        let cli_json: serde_json::Value =
            serde_json::from_str(cli_str).with_context(|| "failed to parse --input as JSON")?;

        if let Some(cli_obj) = cli_json.as_object()
            && let Some(merged_obj) = merged.as_object_mut()
        {
            for (key, value) in cli_obj {
                if merged_obj.contains_key(key) {
                    overridden.push(key.clone());
                }
                merged_obj.insert(key.clone(), value.clone());
            }
        }
    }

    if merged.as_object().is_some_and(serde_json::Map::is_empty) {
        Ok((None, overridden))
    } else {
        Ok((Some(serde_json::to_string(&merged)?), overridden))
    }
}

async fn run_builtin_workflow(
    workflow_name: &str,
    input: Option<&str>,
    json: bool,
    process_start: Instant,
) -> Result<()> {
    let builtin = builtins::resolve(workflow_name)
        .with_context(|| format!("builtin `{workflow_name}` not found"))?;

    let cwd = std::env::current_dir()?;
    let root = db::resolve_workspace_root(&cwd, true).await?;
    let db = db::open_workspace(&root).await?;
    let metrics_store = MetricsStore::new(db::metrics_dir(&root))?;

    let run_id = db::create_run(&db, workflow_name, input).await?;
    if !json {
        println!("Created run {run_id}");
    }

    execute_builtin(
        &db,
        &metrics_store,
        run_id,
        workflow_name,
        builtin,
        input,
        json,
        process_start,
    )
    .await
}

pub async fn execute_builtin(
    db: &DatabaseConnection,
    metrics_store: &MetricsStore,
    run_id: i64,
    workflow_name: &str,
    builtin: Box<dyn builtins::BuiltinWorkflow>,
    input: Option<&str>,
    json: bool,
    process_start: Instant,
) -> Result<()> {

    let builtin_result = builtin
        .execute(builtins::BuiltinContext {
            db,
            metrics_store,
            run_id,
            workflow_name,
            input,
            quiet: json,
        })
        .await;

    let duration = process_start.elapsed();

    match &builtin_result {
        Ok(()) => {
            db::complete_run(db, metrics_store, run_id).await?;
            if !json {
                println!(
                    "Run {run_id} completed successfully in {:.2}s",
                    duration.as_secs_f64()
                );
            }
        }
        Err(err) => {
            let msg = format!("{err:#}");
            db::fail_run(db, metrics_store, run_id, &msg).await?;
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
            warning: None,
        };
        println!("{}", serde_json::to_string(&output)?);
    } else {
        print_aggregate_metrics_table(&aggregate);
        println!("\nRun `qt show {run_id} --verbose` for sample-level details.");
    }

    builtin_result
}

pub async fn execute_custom(
    run_id: i64,
    workflow_name: &str,
    input: Option<&str>,
    command: &[String],
    json: bool,
    process_start: Instant,
    warning: Option<&str>,
) -> Result<()> {
    let base_url =
        std::env::var("QUANTILES_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8765".to_owned());
    let addr = base_url
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    let server_started_by_us = !is_server_reachable(addr);
    let server_handle = if server_started_by_us {
        let cwd = std::env::current_dir()?;
        let root = db::resolve_workspace_root(&cwd, true).await?;
        let db_path = db::workspace_db_path(&root);
        Some(start_background_server(addr, json, db_path)?)
    } else {
        None
    };

    wait_for_health(&base_url)?;

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
            warning,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
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
    /// Warning about overridden config input keys.
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<&'a str>,
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    /// When only config input is present and no `--input` CLI flag is given, the result
    /// should be the exact config object with no overridden keys.
    #[test]
    fn merge_inputs_config_only() {
        let mut config = HashMap::new();
        config.insert("foo".to_owned(), json!("bar"));
        let (result, overridden) = super::merge_inputs(Some(&config), None).unwrap();
        assert_eq!(result, Some(r#"{"foo":"bar"}"#.to_owned()));
        assert!(overridden.is_empty());
    }

    /// When only a `--input` CLI flag is given with no config input, the result should be
    /// the parsed CLI JSON object with no overridden keys.
    #[test]
    fn merge_inputs_cli_only() {
        let (result, overridden) = super::merge_inputs(None, Some(r#"{"x":1}"#)).unwrap();
        assert_eq!(result, Some(r#"{"x":1}"#.to_owned()));
        assert!(overridden.is_empty());
    }

    /// When both config and CLI inputs specify the same key, the CLI value should win and
    /// the key should be recorded in the overridden list for the warning.
    #[test]
    fn merge_inputs_both_with_overlap() {
        let mut config = HashMap::new();
        config.insert("foo".to_owned(), json!("old"));
        config.insert("bar".to_owned(), json!("baz"));

        let (result, overridden) =
            super::merge_inputs(Some(&config), Some(r#"{"foo":"new"}"#)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(result.as_ref().unwrap()).unwrap();
        assert_eq!(parsed["foo"], "new");
        assert_eq!(parsed["bar"], "baz");
        assert_eq!(overridden, vec!["foo"]);
    }

    /// Passing a non-JSON string to `--input` should produce a clear error so the user
    /// knows their CLI argument is malformed.
    #[test]
    fn merge_inputs_cli_invalid_json() {
        let err = super::merge_inputs(None, Some("not json")).unwrap_err();
        assert!(err.to_string().contains("failed to parse --input as JSON"));
    }

    /// When neither config nor CLI provides any input keys, the merged result should be
    /// `None` so the run stores no input JSON.
    #[test]
    fn merge_inputs_both_empty() {
        let (result, overridden) = super::merge_inputs(None, None).unwrap();
        assert!(result.is_none());
        assert!(overridden.is_empty());
    }

    /// A `--input` CLI flag should take precedence over any config fields, returning the
    /// raw CLI string directly without assembling a config-based JSON object.
    #[test]
    fn assemble_builtin_input_with_cli_override() {
        let bench = qt::config::BuiltinBenchmarkConfig {
            type_: "builtin".to_owned(),
            samples: Some(10),
            model: None,
            max_workers: None,
        };
        let (input, _) = super::assemble_builtin_input(Some(&bench), Some(r#"{"model":"x"}"#));
        assert_eq!(input, Some(r#"{"model":"x"}"#.to_owned()));
    }

    /// When no `--input` is given but the config has builtin fields, they should be
    /// assembled into a `BuiltinConfigInput` JSON object with the correct key names.
    #[test]
    fn assemble_builtin_input_from_config() {
        let bench = qt::config::BuiltinBenchmarkConfig {
            type_: "builtin".to_owned(),
            samples: Some(5),
            model: Some(qt::llm::Sampler::Random {}),
            max_workers: Some(8),
        };
        let (input, _) = super::assemble_builtin_input(Some(&bench), None);
        let parsed: serde_json::Value = serde_json::from_str(&input.unwrap()).unwrap();
        assert_eq!(parsed["limit"], 5);
        assert_eq!(parsed["model"], "random");
        assert_eq!(parsed["max_workers"], 8);
    }

    /// When the builtin config section exists but has no runtime-relevant fields, the
    /// input should be `None` rather than an empty JSON object.
    #[test]
    fn assemble_builtin_input_none_when_no_config_fields() {
        let bench = qt::config::BuiltinBenchmarkConfig {
            type_: "builtin".to_owned(),
            samples: None,
            model: None,
            max_workers: None,
        };
        let (input, _) = super::assemble_builtin_input(Some(&bench), None);
        assert!(input.is_none());
    }

    /// When there is no config section at all and no CLI `--input`, builtin runs should
    /// proceed with no input JSON stored in the database.
    #[test]
    fn assemble_builtin_input_none_when_no_bench() {
        let (input, _) = super::assemble_builtin_input(None, None);
        assert!(input.is_none());
    }
}
