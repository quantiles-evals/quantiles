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

You can customize how the CLI executes built-in benchmarks, custom code evaluations, and custom no-code evals using a `quantiles.toml` or `.quantiles.toml` configuration file. See the following resources for information and examples:

- [Configuration reference](../CONFIG.md) for configuration guidance and supported options.
- [Configuration examples](./examples/configs) for complete working configurations.

### Built-in benchmarks

For built-in benchmarks, configure settings like `samples`, `model`, and `max_workers`:

```toml
[benchmarks.pubmedqa]
samples = 50
model = "openai:gpt-5.6"
max_workers = 100
```

> Note: Quantiles is designed for high-throughput execution and may issue many requests in parallel. Depending on your provider, model, and account limits, benchmark runs can quickly hit API rate limits or concurrency quotas. Consider reducing concurrency or using models/providers with higher rate limits if you encounter throttling. Example configurations illustrate how to do so.

### Custom no-code evals

For evals that do not need custom evaluation code, set `type = "custom_nocode"` in your configuration file. These evals point to a dataset, render a prompt (using the [Jinja templating language](https://jinja.palletsprojects.com/en/stable/)) for each sample, send the prompt to your model, and score the result by matching exact answers or multiple choice answers.

The [example configuration file in the `custom-nocode-examples/` directory](./custom-nocode-examples/quantiles.toml) shows runnable SimpleQA Verified, MedQA, MedMCQA, MMLU-Pro, and GPQA configurations. A sample `custom_nocode` configuration is below:

```toml
[benchmarks.my_custom_eval]
type = "custom_nocode"
# style.type can be set to "exact_match" or "multiple_choice". Depending
# on the type, there are other required fields. See the configuration documentation in
# CONFIG.md for reference:
# 
# https://github.com/quantiles-evals/quantiles/blob/main/CONFIG.md#custom_nocode
style = { type = "exact_match", golden_column = "answer" }
dataset = { name = "quantiles/simpleqa-verified" }
# when the model is set to "random", `exact_match` benchmarks generate random text,
# and multiple_choice benchmarks uniformly select from the `style.choice_labels` array
model = "random"
# this file should be a jinja template that can render the prompt from any row
# in the dataset
prompt_template_file = "prompts/my_custom_eval.txt"
limit = 10
```

With this configuration saved to a `quantiles.toml` file, you can run the eval with:

```shell
qt run my_custom_eval
```

With this configuration, each sample will automatically emit correctness, response-parsing, and latency metrics, and every eval will show the following aggregate metrics:

- Accuracy and pass/fail counts
- Response parsing success
- Mean, median, p95, p99, and minimum/maximum latency

When `style.type` is set to `multiple_choice`, the eval run will also emit the following metrics:

- Macro, weighted, and per-label precision, recall, and F1 metrics
- A confusion-matrix indexed by `style.choice_labels`, with an additional column for unparsed responses

See the following resources for more details:

- An [example `quantiles.toml` file with real, runnable evals](../custom-nocode-examples/quantiles.toml)
- [Reference documentation](../CONFIG.md#custom_nocode) for `custom_nocode` configurations

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
