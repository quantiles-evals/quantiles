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

Running a built-in benchmark directly from the hosted registry, such as `simpleqa-verified` above, requires an internet connection so that `qt` can retrieve its configuration from `https://api.quantiles.io`.

See the [CLI reference](https://quantiles.io/documentation/reference/cli) for a detailed list of `qt` commands.

> Note: Quantiles is designed for high-throughput execution and may issue many parallel requests to your model provider. Depending on your provider, model, and account limits, benchmark runs can hit API rate limits or concurrency quotas. Reduce `max_workers` or use a model or provider with higher throughput limits if you encounter throttling.

## Configure evaluations

The CLI supports built-in benchmarks and two custom evaluation approaches:

- [Built-in benchmarks](https://quantiles.io/documentation/built-in-benchmarks) are ready-to-run evaluations retrieved by name from the hosted Quantiles benchmark registry and executed locally.
- [Custom configuration evaluations](https://quantiles.io/documentation/custom-evaluations/custom-nocode-evaluations) define the dataset, prompt template, model, and scoring method entirely in configuration using `type = "custom_nocode"`. Supported scoring styles include exact match, multiple choice, and text similarity.
- [Custom code evaluations](https://quantiles.io/documentation/custom-evaluations) run your own Python evaluation through the Quantiles Python SDK using `type = "custom_code"`.

Add a `quantiles.toml` or `.quantiles.toml` config file to configure an evaluation. When you run a evaluation, Quantiles first checks this file for a matching configuration. If none is found, it queries the hosted Quantiles benchmark registry at for a built-in benchmark with that name.

The following `quantiles.toml` example configures PubMedQA to use an OpenAI model and evaluate a limited number of samples. Use `qt add <benchmark_name>` to add any built-in benchmark for local customization:

````toml
# Define a local configuration for the PubMedQA benchmark.
[benchmarks.pubmedqa]

# Type must be specified.
type = "custom_nocode"

# Use the same dataset as the built-in PubMedQA benchmark.
dataset = "hf://quantiles/PubMedQA"

# Run 50 samples of the benchmark.
# Omit this field to evaluate the full dataset.
samples = 50

# Replace the default demo model with a hosted OpenAI model.
model = "openai:gpt-5.6-luna"
```

For additional guidance, see:

- [Configuration guide](https://quantiles.io/documentation/configuration) for file location, supported fields, validation behavior, and examples.
- [Model configuration guide](https://quantiles.io/documentation/model-configuration) for guidance on setting up hosted AI models, managing credentials, and troubleshooting configuration issues.
- [Custom-code example](./examples/configs/custom_code/quantiles.toml) and [custom configuration examples](../custom-nocode-examples/quantiles.toml) for additional runnable examples.

ASK AARON IS THIS SECTION IS RIGHT/NEED

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
| Built-in and   |  | custom_code child |
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

- **Built-in and `custom_nocode` evaluations** execute inside the local CLI process.
- **`custom_code` evaluations** execute as user-configured child processes and record workflow data through the local API server.
- **CLI inspection commands** read run data from SQLite and metrics from Parquet.
````
