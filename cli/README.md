# Quantiles CLI

This directory holds the source code for the `qt` CLI. It is built with [Rust](https://rust-lang.org/) to help it efficiently use the resources of the local machine, to help ensure safety, and to provide strong lints and type-system invariants for humans and agents to work with.

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
# with the `qt` CLI. See the below "Custom evaluations" section
# for details.
qt run pubmedqa

# 3. List and inspect what happened
qt list
qt show 1
```

>See [quantiles.io/documentation/reference/cli](https://quantiles.io/documentation/reference/cli) for a detailed list of `qt` commands.

### Custom evaluations

Custom evaluations are denoted in the configuration file with `type = "custom_code"`. The `command` array tells the CLI how to execute your eval, and the optional `input` table is merged with any values passed in the `qt run --input` flag, then passed to your script as `QUANTILES_INPUT`. An example is below

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

See [`examples/configs/custom_code/quantiles.toml`](./examples/configs/custom_code/quantiles.toml) for a complete working example.

## Configuration files and customization

You can customize how the CLI executes benchmarks using a `quantiles.toml` or `.quantiles.toml` configuration file.

For **built-in benchmarks**, configure settings like `samples`, `model`, and `max_workers`:

```toml
[benchmarks.pubmedqa]
samples = 50
model = "openai:gpt-5.4-nano"
max_workers = 100
```

For **custom evaluations**, set `type = "custom_code"` and provide the `command` to run. The optional `input` table is passed to your script as `QUANTILES_INPUT`.

```toml
[benchmarks.my-eval]
type = "custom_code"
command = ["python", "my_eval.py"]

[benchmarks.my-eval.input]
foo = "foo_val"
```

See [`./examples/configs`](./examples/configs) for complete working examples.

>Note: Quantiles is designed for high-throughput execution and may issue many requests in parallel. Depending on your provider, model, and account limits, benchmark runs can quickly hit API rate limits or concurrency quotas. Consider reducing concurrency or using models/providers with higher rate limits if you encounter throttling. Example configurations illustrate how to do so.


### Comparing runs

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
|            Your Script               |
|   (TypeScript / Python / Shell)      |
+-------------------+------------------+
                    │
                    │  HTTP / JSON
                    |
                    ▼
+--------------------------------------+
|            Quantiles Server          |
+-------------------+------------------+
                    │
                    │  SQLite
                    |
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
