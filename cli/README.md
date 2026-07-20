# Quantiles CLI

This directory contains the source code for the `qt` CLI. It is implemented in [Rust](https://rust-lang.org/) for efficient local execution, memory safety, and strong compile-time guarantees.

## Install

```bash
curl -fsSL https://cli.quantiles.io/install.sh | bash
```

## Demo

A few commands to see `qt` in action:

```bash
# 1. Run a built-in evaluation using a demo model that does
# not incur any usage charges.
#
# You can also build and run custom evaluations.
# See "Configure evaluations" below.
qt run simpleqa-verified

# 2. See a list of all your evaluation runs and their run IDs.
qt list

# 3. Inspect and analyze the results of your evaluation run.
qt show <run_id>
```

See the [CLI reference](https://quantiles.io/documentation/reference/cli) for a detailed list of `qt` commands.

> Note: Quantiles is designed for high-throughput execution and may issue many parallel requests to your LLM provider. Depending on your provider, model, and account limits, benchmark runs can hit API rate limits or concurrency quotas. Reduce request concurrency or use a model or provider with higher throughput limits. The example below shows how to adjust `max_workers` if you encounter throttling.

## Configure evaluations

The CLI supports three evaluation types:

- [Built-in benchmarks](https://quantiles.io/documentation/built-in-benchmarks) run predefined datasets and scoring methods. They work without configuration, but you can override settings such as the model, sample count, and concurrency.
- [`custom_nocode` evaluations](https://quantiles.io/documentation/custom-evaluations/custom-nocode-evaluations) define the dataset, prompt template, model, and scoring method entirely in configuration.
- [`custom_code` evaluations](https://quantiles.io/documentation/custom-evaluations) run your own Python evaluation through the Quantiles Python SDK.

Add a `quantiles.toml` or `.quantiles.toml` file to configure an evaluation. For example:

```toml
[benchmarks.pubmedqa]
dataset = "hf://quantiles/PubMedQA"
samples = 50
model = "openai:gpt-5.6"
max_workers = 100
```

See the [configuration guide](https://quantiles.io/documentation/configuration) for file location, supported fields, validation behavior, and examples. Additional runnable configurations are available in [CLI configuration examples](./examples/configs) and [custom no-code examples](../custom-nocode-examples/quantiles.toml).

## Architecture

The Quantiles CLI, `qt`, keeps execution simple: your code runs locally, while `qt` handles durability and observability.

```
+--------------------------------------+
|   Benchmark / Custom Evaluation      |
+-------------------+------------------+
                    │
                    │  HTTP / JSON
                    │
                    ▼
+--------------------------------------+
|            Quantiles Server          |
+-------------------+------------------+
                    │
                    │  SQLite / Parquet
                    │
                    ▼
+------------------------------------------------+
|                 .quantiles/                    |
|  quantiles.sqlite       metrics/*.parquet      |
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

- **Server** owns durability decisions for run state, and metrics.
- **Client** (your script) owns code execution; the server never runs your evaluation logic.
- **CLI** reads run data from SQLite and metrics from Parquet.
