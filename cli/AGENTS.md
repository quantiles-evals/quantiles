# AGENTS.md

## Project Overview

`qt`, the Quantiles CLI, is a local-first Rust CLI for running and analyzing AI evaluations, benchmarks, and agent loops. It stores run data in `.quantiles/quantiles.sqlite`, stores metrics as Parquet files, and provides commands for inspecting recorded evaluation runs. Dataset downloads may occur when a user selects a benchmark, while model-provider calls occur only when configured. The long-term goal is a developer tool for reliable AI/ML evaluation workflows where evaluation runs, execution history, aggregrate and sample-level outputs, and events are queryable from the start.

## Working in This Repository

- Prefer focused changes that fit the current CLI and library structure.
- Preserve local-first, offline-by-default behavior. Do not introduce implicit network or cloud behavior. When a task explicitly adds remote behavior, make it user-configured, document it clearly, and call it out to the user.
- Preserve existing SQLite data model assumptions unless the change includes a deliberate schema migration or initialization update.
- Use idiomatic Rust and keep error handling clear. This project uses [`anyhow`](https://docs.rs/anyhow) to create and propagate application-level errors.
- Avoid broad refactors while implementing narrow behavior changes.

## Validation and Testing

Use the `mise.toml` targets to do most validation, building and testing work:

```bash
mise r build
mise r test
mise r fmt
mise r lint
```

Equivalent Cargo commands are:

```bash
cargo build
cargo test
cargo fmt
cargo clippy --all-targets --all-features
```

Do not run two or more `cargo` commands, or commands that invoke `cargo`, concurrently in the same workspace because they must acquire a file-based mutex. You may run them in parallel in separate worktrees.

Before handing work back, run the most relevant checks for the files changed. If you need to do a build, do not run `cargo build` with the `--release` flag, since that will cause you to do needless binary optimization work. For behavior that affects CLI commands or database writes, prefer adding or updating tests and manually exercising the command against a temporary local workspace when useful.
