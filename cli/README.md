# Quantiles CLI

This directory contains the source code for the `qt` CLI. It is implemented in [Rust](https://rust-lang.org/) for efficient local execution, memory safety, and strong compile-time guarantees.

## Install

```bash
curl -fsSL https://cli.quantiles.io/install.sh | bash
```

## Demo

A few commands to see `qt` in action:

```bash
# 1. Run a built-in benchmark using a demo model that does
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

Running a built-in benchmark, such as `simpleqa-verified` above, requires an internet connection so that `qt` can retrieve its configuration from `https://api.quantiles.io`.

See the [CLI reference](https://quantiles.io/documentation/reference/cli) for a detailed list of `qt` commands.

> Note: Quantiles is designed for high-throughput execution and may issue many parallel requests to your LLM provider. Depending on your provider, model, and account limits, benchmark runs can hit API rate limits or concurrency quotas. Reduce request concurrency or use a model or provider with higher throughput limits. The example below shows how to adjust `max_workers` if you encounter throttling.

## Configure evaluations

The CLI supports three evaluation types:

- [Built-in benchmarks](https://quantiles.io/documentation/built-in-benchmarks) are ready-to-run evaluations, with optional configuration for custom settings such as a hosted AI model.
- [`custom_nocode` evaluations](https://quantiles.io/documentation/custom-evaluations/custom-nocode-evaluations) define the dataset, prompt template, model, and scoring method entirely in configuration.
- [`custom_code` evaluations](https://quantiles.io/documentation/custom-evaluations) run your own Python evaluation through the Quantiles Python SDK.

Add a `quantiles.toml` or `.quantiles.toml` config file to configure an evaluation. When you run a benchmark, Quantiles first checks this file for a matching configuration. If none is found, it queries the remote Quantiles benchmark registry at `https://api.quantiles.io` for a built-in benchmark with that name.

The following example configures the built-in PubMedQA benchmark to use an OpenAI model and limit the number of samples:

```toml
# Define a local configuration for the PubMedQA benchmark.
[benchmarks.pubmedqa]

# Use the configurable no-code evaluation framework.
type = "custom-nocode"

# Use the same dataset as the built-in PubMedQA benchmark.
dataset = "hf://quantiles/PubMedQA"

# Run the evaluation on 50 samples.
# Omit this field to evaluate the full dataset.
samples = 50

# Replace the default demo model with a hosted OpenAI model.
model = "openai:gpt-5.6-luna"
```

For additional guidance, see:

- [Configuration guide](https://quantiles.io/documentation/configuration) for file location, supported fields, validation behavior, and examples.
- [Model configuration guide](https://quantiles.io/documentation/model-configuration) for guidance on setting up hosted AI models, managing credentials, and troubleshooting configuration issues.
- [CLI configuration examples](./examples/configs) and [custom no-code examples](../custom-nocode-examples/quantiles.toml) for additional runnable examples.

## Architecture

ASK AARON IF THIS NEEDS TO CHANGE

The Quantiles CLI, `qt` runs code locally, while `qt` handles durability and observability.

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
