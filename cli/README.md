# Quantiles CLI

This directory contains the source code for the `qt` CLI. It is implemented in [Rust](https://rust-lang.org/) to use local machine resources efficiently, improve safety, and provide strong lints and type-system invariants for humans and agents.

## Install

```bash
curl -fsSL https://cli.quantiles.io/install.sh | bash
```

## Demo

A few commands to see `qt` in action:

```bash
# 1. Initialize a workspace
qt init

# 2. Run a built-in eval
#
# Note that you can also build and run your own custom evals
# with the `qt` CLI. See the following "Custom evaluations" section
# for details.
qt run pubmedqa

# 3. List and inspect what happened
qt list
qt show 1
```

> See the [CLI reference](https://quantiles.io/documentation/reference/cli) for a detailed list of `qt` commands.

### Custom evaluations

Custom evaluations are denoted in the configuration file with `type = "custom_code"`. The `command` array tells the CLI how to execute your eval, and the optional `input` table is merged with any values passed through the `--input` flag, then passed to your script as `QUANTILES_INPUT`. An example is below:

```toml
[benchmarks.my-eval]
type = "custom_code"
command = ["python", "my_eval.py"]
input = {dataset = "my_dataset.jsonl"}
```

```bash
# Run the custom evaluation
qt run my-eval

# If it fails, resume with only the run ID
qt resume <run_id>
```

See the [custom-code configuration example](./examples/configs/custom_code/quantiles.toml) for a complete working configuration.

## Configuration files and customization

You can customize how the CLI executes built-in benchmarks and custom evaluations using a `quantiles.toml` or `.quantiles.toml` configuration file. See the following resources for information and examples:

- [Configuration reference](../CONFIG.md) for configuration guidance and supported options.
- [Configuration examples](./examples/configs) for complete working configurations.

### Built-in benchmarks

For built-in benchmarks, configure settings like `dataset`, `samples`, `model`, and `max_workers`:

```toml
[benchmarks.pubmedqa]
dataset = "hf://quantiles/PubMedQA"
samples = 50
model = "openai:gpt-5.6"
max_workers = 100
```

> Note: Quantiles is designed for high-throughput execution and may issue many requests in parallel. Depending on your provider, model, and account limits, benchmark runs can quickly hit API rate limits or concurrency quotas. Consider reducing concurrency or using models/providers with higher rate limits if you encounter throttling. Example configurations illustrate how to do so.

### Custom code evals

For custom evaluations, set `type = "custom_code"` and provide the `command` to run. The optional `input` table is passed to your script as a JSON dictionary.

```toml
[benchmarks.my-eval]
type = "custom_code"
command = ["python", "my_eval.py"]
input = { foo = "foo_val" }
```

## Comparing runs

After iterating on an eval, you can compare two runs to see exactly what changed:

```bash
# Run A — baseline
qt run my-eval

# Run B — your latest iteration
qt run my-eval

# See what changed between them
qt compare 1 2
```

`qt compare` exits with code 1 if the runs differ, making it useful in CI scripts.

## Architecture

The Quantiles CLI, `qt`, keeps execution simple: your code runs locally, while `qt` handles durability and observability.

```
+--------------------------------------+
|   Benchmark / Custom Eval (Python)   |
+-------------------+------------------+
                    │
                    │  HTTP / JSON
                    │
                    ▼
+--------------------------------------+
|            Quantiles Server          |
+-------------------+------------------+
                    │
                    │  SQLite
                    │
                    ▼
+------------------------------------------------+
|     .quantiles/quantiles.sqlite (local DB)     |
+-------------------+----------------------------+
                    │
                    │
                    │
                    ▼
+--------------------------------------+
|                 CLI                  |
|        (list, show, compare)         |
+--------------------------------------+
```

- **Server** owns durability decisions: step caching, run state, metrics
- **Client** (your script) owns code execution: the server never runs your logic
  - Note that the CLI itself also has built-in benchmarks, which do not involve your code
- **CLI** reads the same SQLite database the server writes to
