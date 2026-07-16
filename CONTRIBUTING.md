# Contributing to Quantiles

Thank you for your interest in contributing to Quantiles open source!

The Quantiles open-source project provides local-first evaluation infrastructure for AI systems. We welcome contributions that improve reliability, reproducibility, developer ergonomics, agent workflows, documentation, benchmarks, and custom evaluations.

## Before You Start

Before opening a pull request, please:

1. Search existing issues and pull requests to avoid duplicate work.
2. Open an issue for large changes, new benchmark integrations, schema changes, or API changes.
3. Keep changes focused and reviewable.
4. Update tests and documentation when behavior changes.
5. If you use one, make sure your coding agent follows the instructions in `AGENTS.md`.

## Code and Documentation Guidelines

- Keep each pull request focused on one clear problem or improvement.
- Add or update tests for new behavior, changed behavior, bug fixes, and regressions.
- Update documentation whenever CLI behavior, SDK APIs, workflows, benchmarks, schemas, or setup steps change.
- Use clear examples, commands, inputs, and expected outputs so changes are reproducible.
- Avoid unrelated refactors, formatting-only churn, generated files, or broad changes that make review harder.

## Development Setup

Clone the repository:

```bash
git clone https://github.com/quantiles-evals/quantiles.git
cd quantiles
```

For the [CLI directory](./cli), ensure Rust is installed and use its `mise.toml` tasks to build, check, lint, and test the code. For the [Python SDK directory](./python), use [`uv`](https://docs.astral.sh/uv/) through the package's `mise.toml` tasks. The [TypeScript SDK directory](./typescript) contains an unreleased and unsupported SDK.

## Security

Please do not report security vulnerabilities through public GitHub issues. Follow the security reporting guidance in [SECURITY.md](./SECURITY.md).

## Code of Conduct

By participating in this project, you agree to follow our [Code of Conduct](./CODE_OF_CONDUCT.md).

## License

By contributing, you agree that your contributions will be licensed under the [Apache License 2.0](./LICENSE).
