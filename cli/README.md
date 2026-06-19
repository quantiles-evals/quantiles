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

# 2. Run an eval — Quantiles auto-starts a local server, records the run, and tears the server
# down when the command finishes.
qt run my-eval -- bun run sdk/typescript/examples/run_demo.ts

# 3. List and inspect what happened
qt list
qt show 1
```

>See [quantiles.io/documentation/reference/cli](https://quantiles.io/documentation/reference/cli) for a detailed list of `qt` commands.

### Comparing runs

After iterating on an eval, you can compare two runs to see exactly what changed:

```bash
# Run A — baseline
qt run my-eval -- bun run sdk/typescript/examples/run_demo.ts

# Run B — your latest iteration
qt run my-eval -- bun run sdk/typescript/examples/run_demo.ts

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

## Customization

You can customize how the CLI executes benchmarks using a `quantiles.toml` or `.quantiles.toml` configuration file. This file can be used to control benchmark execution behavior as well as customize the models, providers, and other settings used during eval runs. See [`./cli/examples/configs`](./cli/examples/configs) for examples and more details.

>Note: Quantiles is designed for high-throughput execution and may issue many requests in parallel. Depending on your provider, model, and account limits, benchmark runs can quickly hit API rate limits or concurrency quotas. Consider reducing concurrency or using models/providers with higher rate limits if you encounter throttling.
