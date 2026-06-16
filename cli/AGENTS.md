# AGENTS.md

## Project Overview

`qt`, the Quantiles CLI, is a local-only Rust CLI for testing and observing AI workloads such as benchmarks, evals, and agent loops. It creates a local SQLite database under `.quantiles/quantiles.sqlite`, records eval runs and related observability data, and provides CLI commands to inspect what happened. The long-term goal is a developer tool for reliable AI/ML experiments where execution history, step outputs, metrics, and events are queryable from the start.

## Working in This Repository

- Prefer focused changes that fit the current CLI and library structure.
- Keep local-only behavior explicit; do not introduce network or cloud behavior unless the task specifically calls for it. If you do add such behavior, make sure to alert the user about it, and add comments in the code to draw attention to it.
- Preserve existing SQLite data model assumptions unless the change includes a deliberate schema migration or initialization update.
- Use idiomatic Rust and keep error handling clear. This project uses `anyhow` for application-level errors.
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

Do not run 2 or more `cargo` commands, or commands that execute `cargo` commands, concurrently in the same workspace, since `cargo` commands have to acquire a file-based mutex to run. If you are using worktrees, you can execute cargo commands in parallel, as long as they are in different worktrees.

Before handing work back, run the most relevant checks for the files changed. If you need to do a build, do not run `cargo build` with the `--release` flag, since that will cause you to do needless binary optimization work. For behavior that affects CLI commands or database writes, prefer adding or updating tests and manually exercising the command against a temporary local workspace when useful.
