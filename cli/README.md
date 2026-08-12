# Quantiles CLI

This directory contains the source code for the `qt` CLI. It is implemented in [Rust](https://rust-lang.org/) for efficient local execution, memory safety, and strong compile-time guarantees.

## Install

```bash
curl -fsSL https://cli.quantiles.io/install.sh | bash
```

## Demo

A few commands to see `qt` in action:

```bash
# 1. Download a benchmark from the Quantiles hosted benchmark registry
# and run it locally using a demo model that does not incur any usage
# charges.
#
# You can also build and run custom evaluations.
# See "Configure evaluations" below.
qt run simpleqa-verified

# 2. See a list of all your evaluation runs and their run IDs.
qt list

# 3. Inspect and analyze the results of your evaluation run.
qt show <run_id>
```

Running a built-in benchmark directly from the hosted registry, such as `simpleqa-verified` above, requires an internet connection so that `qt` can retrieve its configuration from the Quantiles hosted benchmark registry (hosted at `api.quantiles.io`).

See the [CLI reference](https://quantiles.io/documentation/reference/cli) for a detailed list of `qt` commands.

> Note: Quantiles is designed for high-throughput execution and may issue many parallel requests to your model provider. Depending on your provider, model, and account limits, benchmark runs can hit API rate limits or concurrency quotas. Reduce `max_workers` or use a model or provider with higher throughput limits if you encounter throttling.

## Configure evaluations

The CLI supports built-in benchmarks and two locally configured evaluation types:

- [Built-in benchmarks](https://quantiles.io/documentation/built-in-benchmarks) are ready-to-run evaluations retrieved by name from the hosted Quantiles benchmark registry and executed locally using the native `custom_nocode` runtime.
- [Custom configuration (`custom_nocode`) evaluations](https://quantiles.io/documentation/custom-evaluations/custom-nocode-evaluations) define the dataset, prompt template, model, and scoring method entirely in configuration. Supported scoring styles include exact match, multiple choice, and text similarity.
- [Custom code (`custom_code`) evaluations](https://quantiles.io/documentation/custom-evaluations) run your own Python evaluation through the Quantiles Python SDK.

Add a `quantiles.toml` or `.quantiles.toml` file to configure a custom evaluation. For example:

```toml
[benchmarks.support-triage]
type = "custom_code"
command = ["uv", "run", "eval.py"]

[benchmarks.support-triage.input]
model = "openai:gpt-5.6"
```

Custom configuration similarity evaluations support Levenshtein distance and cosine similarity. The following configuration uses Levenshtein distance:

```toml
[benchmarks.simpleqa-levenshtein]
type = "custom_nocode"
dataset = { name = "quantiles/simpleqa-verified" }
prompt_template_file = "prompts/qa.txt"
style = { type = "similarity", golden_column = "answer", metric = "levenshtein" }
```

Cosine similarity requires an explicit embedding model. Use `"fastembed"` for the local FastEmbed-powered model built into the CLI:

```toml
[benchmarks.simpleqa-cosine]
type = "custom_nocode"
dataset = { name = "quantiles/simpleqa-verified" }
prompt_template_file = "prompts/qa.txt"
style = { type = "similarity", golden_column = "answer", metric = { type = "cosine", embedding_model = "fastembed" } }
```

For additional guidance, see:

- [Configuration documentation](https://quantiles.io/documentation/configuration) for file location, supported fields, validation behavior, and examples.
- [Model configuration guide](https://quantiles.io/documentation/model-configuration) for guidance on setting up provider models, managing credentials, and troubleshooting configuration issues.
- [Custom-code example](./examples/configs/custom_code/quantiles.toml) and [custom configuration examples](../custom-nocode-examples/quantiles.toml) for additional runnable examples.

### Hosted benchmark registry

Use `qt add <benchmark_name>` to download a built-in benchmark from the registry and save it in the local configuration. If `quantiles.toml` or `.quantiles.toml` exists in the current directory, the command appends the benchmark section without rewriting the existing content. Otherwise, it creates `quantiles.toml`. The downloaded prompt template is stored beside the configuration at `<benchmark_name>-prompt/prompt.txt` and referenced by the added configuration.

```bash
qt add simpleqa-verified
qt add simpleqa-verified --json
```

The command fails without modifying the configuration if the benchmark is already configured or the registry does not contain it. If you pass `--json`, both successful and unsuccessful output is machine-readable JSON. Resolving and downloading the benchmark requires network access.

When you run `qt run <eval_name>`, the CLI first looks in the local configuration file for an evaluation called `eval_name`. If one is found, the CLI runs it immediately. If none is found, `qt` looks in the Quantiles hosted benchmark registry for a benchmark of the same name. If a match is found, the CLI downloads the benchmark definition and runs it.

> To override the location of the hosted benchmark registry, use the `--remote-url` flag or the `QUANTILES_REMOTE_URL` environment variable.

When `qt` uses the hosted benchmark registry, downloaded definitions and prompt templates are verified and kept in memory for the run. They are not cached on disk.

Benchmarks run directly from the hosted registry do not use local benchmark configuration sections. Apply supported run-specific overrides with `--input` instead:

```bash
qt run simpleqa-verified --input '{"model":"openai:gpt-5.6","limit":50}'
```

Resolving the benchmark and downloading an uncached dataset requires network access. Provider-backed models also require their provider credentials, and the provider may charge you for usage.

For runs started from the hosted benchmark registry, the CLI persists the registry endpoint, immutable benchmark version, and manifest hash. If a run needs to be resumed later with `qt resume`, the CLI re-downloads that exact benchmark version and rejects it if the manifest hash changed. Because the registry guarantees that published versions are immutable, this preserves the original benchmark definition. Resuming a run that started from the registry requires internet access.

When `--input` overrides a registry benchmark's `prompt_template_file` with a local file, `qt` also persists the file's SHA-256 hash. On resume, `qt` reads the local prompt into memory and rejects the resume before changing the run status if its contents no longer match the stored hash.

## Architecture

The Quantiles CLI, `qt`, resolves configuration, orchestrates evaluation runs, and stores results locally:

```
+------------------------------+
|            qt CLI            |
+---------------+--------------+
                |
      +---------+---------+
      |                   |
      v                   v
+----------------+  +-------------------+
| Registry and   |  | custom_code child |
| custom_nocode  |  | process + local   |
| runtime        |  | API server        |
+--------+-------+  +---------+---------+
         |                    |
         +---------+----------+
                   v
+---------------------------------------+
|              .quantiles/              |
|  quantiles.sqlite   metrics/*.parquet |
+---------------------------------------+
```

- **Registry benchmarks and `custom_nocode` evaluations** execute inside the local CLI process.
- **`custom_code` evaluations** execute as user-configured child processes and record workflow data through the local API server.
- **CLI inspection commands** read run data from SQLite and metrics from Parquet.
