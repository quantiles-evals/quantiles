# Quantiles CLI

This directory contains the source code for the `qt` CLI. It is implemented in [Rust](https://rust-lang.org/) to use local machine resources efficiently, improve safety, and provide strong lints and type-system invariants for humans and agents.

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
# You can also build and run your own custom evaluations.
# See the following "No-code custom" and "Custom code" sections.
qt run simpleqa-verified

# 2. See a list of all your evaluation runs and their run IDs.
qt list

# 3. Inspect and analyze the results of your evaluation run.
qt show <run_id>
```

> See the [CLI reference](https://quantiles.io/documentation/reference/cli) for a detailed list of `qt` commands.

> Note: Quantiles is designed for high-throughput execution and may issue many parallel requests to your LLM provider. Depending on your provider, model, and account limits, benchmark runs can quickly hit API rate limits or concurrency quotas. Consider reducing concurrency or using models/providers with higher rate limits if you encounter throttling. Example configurations below illustrate how to do so.

## Configuration files and customization

You can customize how the CLI executes built-in benchmarks, custom code evaluations, and custom no-code evaluations using a `quantiles.toml` or `.quantiles.toml` configuration file. See the following resources for information and examples:

- [Configuration reference](../CONFIG.md) for configuration guidance and supported options.
- [Configuration examples](./examples/configs) for complete working configurations.

## Built-in benchmarks

For built-in benchmarks, configure `samples`, `model`, and `max_workers`:

```toml
[benchmarks.pubmedqa]
samples = 50
model = "openai:gpt-5.6"
max_workers = 100
```

## Custom no-code evaluations

No-code custom evaluations are configured entirely in `quantiles.toml` and require no Python implementation. These evaluations load samples from a dataset, render a prompt for each sample using the [Jinja templating language](https://jinja.palletsprojects.com/en/stable/), send each prompt to the configured model, and score responses using exact-match or multiple-choice scoring.

See the [no-code custom evaluation examples](./custom-nocode-examples/quantiles.toml) for runnable configurations for SimpleQA-Verified, MMLU-Pro, and more. The following example shows the `custom_nocode` configuration structure for SimpleQA Verified:

```toml
[benchmarks.my_custom_eval]
type = "custom_nocode"

# style.type can be set to "exact_match" or "multiple_choice".
# Depending on the type, there are other required fields.
# See the configuration documentation innCONFIG.md for reference:
# https://github.com/quantiles-evals/quantiles/blob/main/CONFIG.md#custom_nocode
style = { type = "exact_match", golden_column = "answer" }

dataset = { name = "quantiles/simpleqa-verified" }

# when the model is set to "random",
#`exact_match` benchmarks generate random text
# and multiple_choice benchmarks uniformly select
# from the `style.choice_labels` array
model = "random"

# the prompt_template_file should be a jinja template
# that can render the prompt from any row in the dataset
prompt_template_file = "prompts/my_custom_eval.txt"

limit = 10
```

After adding the configuration to `quantiles.toml`, run the evaluation using:

```shell
qt run my_custom_eval
```

For each sample, this configuration automatically records correctness, response-parsing, and latency metrics. Each evaluation run also reports the following aggregate metrics:

- Accuracy and pass/fail counts
- Response parsing success
- Mean, median, p95, p99, and minimum/maximum latency

When `style.type` is set to `multiple_choice`, the evaluation run also reports the following aggregate metrics:

- Macro, weighted, and per-label precision, recall, and F1 metrics
- A confusion-matrix indexed by `style.choice_labels`, with an additional column for unparsed responses

Additional examples and configuration details are available in:

- [Runnable no-code evaluation examples](../custom-nocode-examples/quantiles.toml)
- [Custom no-code evaluation configuration reference](../CONFIG.md#custom_nocode)

## Custom code evaluations

A `custom_code` evaluation is a [Python-based evaluation](https://quantiles.io/documentation/reference/python-sdk) that you implement for behavior specific to your product, workflow, prompts, datasets, scoring rubrics, or release process. Unlike no-code custom evaluations, it supports fully customizable evaluation logic but requires you to write and maintain the Python implementation.

```toml
# Custom-code evaluation
[benchmarks.customcode_eval]
type = "custom_code"

# The `command` array tells the CLI how
# to execute your eval code.
command = ["python", "my_eval.py"]

# The optional `input` table is merged with
# any values passed through the `--input`
# flag on the command line, then passed
# to your script as JSON in the `QUANTILES_INPUT`
# environment variable. The Quantiles Python
# SDK transparently parses this value for you.
input = {dataset = "my_dataset.jsonl"}
```

After adding the configuration to `quantiles.toml`, run the evaluation using:

```bash
qt run customcode_eval
```

See the [custom-code configuration example](./examples/configs/custom_code/quantiles.toml) for a complete working configuration.

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
