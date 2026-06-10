# Quantiles Open-source

Open-source, local-first eval infrastructure for AI systems.

Quantiles gives AI engineers a repeatable workflow for running evaluations, recording results, inspecting sample-level outputs, comparing runs, and debugging regressions from inside a repository. It includes a CLI, SDKs, built-in benchmarks, local run history, and agent-friendly instructions for coding agents such as Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and other agentic development tools.

## Why Quantiles

AI systems change quickly. Models, prompts, tools, retrieval logic, and agent workflows can all affect behavior. Quantiles helps teams measure those changes before they reach production by keeping evaluations close to the development environment.

With Quantiles, you can:

- Run built-in and custom evaluations from the command line
- Inspect recorded runs and sample-level outputs
- Compare runs to identify regressions
- Resume interrupted workflows
- Record metrics, artifacts, errors, and execution history locally
- Give coding agents a repeatable workflow for evaluating changes
- Keep evaluation evidence close to the repository

## Quickstart

Install the CLI:

```bash
curl -fsSL https://cli.quantiles.io/install.sh | sh
```

Initialize Quantiles in your project:

```bash
qt init
```

Run a built-in benchmark:

```bash
qt run simpleqa-verified
```

Inspect the recorded run:

```bash
qt show 1
```

Compare two runs:

```bash
qt compare 1 2
```

For the complete command reference:

```bash
qt --help
```

## Core Workflow

Quantiles is built around a simple evaluation loop:

```text
define eval
run eval
record results
inspect samples
compare runs
debug regressions
repeat
```

The `qt` CLI records local run history so results can be inspected, compared, and summarized without relying on ad hoc logs, spreadsheets, or one-off scripts.

## CLI

Common commands:

```bash
qt init
qt run <workflow>
qt list
qt show <run_id>
qt compare <run_id_a> <run_id_b>
```

Example:

```bash
qt run simpleqa-verified
qt show 1
qt compare 1 2
```

Use `qt show` to inspect a single run and `qt compare` to compare behavior across runs.

## Local-First by Default

Quantiles is designed for the early evaluation loop inside a repository.

By default, evaluation state is recorded locally so teams can iterate quickly, inspect results, and preserve evidence while developing AI systems. Local state can include:

- Run metadata
- Step records
- Metrics
- Artifacts
- Inputs
- Outputs
- Errors
- Runtime metadata

This makes Quantiles useful before an evaluation workflow needs production orchestration, hosted observability, or centralized experiment tracking.

## Built-in Benchmarks

Quantiles supports built-in benchmarks as ready-to-run evaluation workflows with defined datasets, scoring, metrics, and result shapes.

Built-in benchmarks are useful when you want a repeatable baseline, a standard reference point, or a quick way to validate that the evaluation workflow is working.

Example:

```bash
qt run simpleqa-verified
```

Benchmark documentation should include:

- What the benchmark evaluates
- Input and output shape
- Scoring method
- Metrics
- Provenance
- Known limitations
- Comparability notes

## Custom Evaluations

Use custom evaluations when you need to test your own model, prompt, retrieval flow, tool call, or agent workflow.

A custom eval can record:

- Inputs
- Outputs
- Intermediate steps
- Metrics
- Artifacts
- Errors
- Runtime metadata

Custom evaluations can be run locally and compared using the same `qt` workflow as built-in benchmarks.

## SDKs

Quantiles includes SDKs for defining custom evaluations in code.

SDK surfaces include:

- Python workflows
- TypeScript workflows
- Durable steps
- Metrics
- Artifacts
- Run metadata
- Local execution history

See the SDK documentation for examples.

## Coding Agents

Quantiles is designed to work well with coding agents such as Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and other agentic development tools.

The repository includes agent-facing instructions and reusable skill files:

- [`SKILL.md`](https://github.com/quantiles-evals/skill/blob/main/SKILL.md)
- [`AGENT.md`](https://github.com/quantiles-evals/skill/blob/main/AGENT.md)

`SKILL.md` gives agents durable Quantiles behavior for running evaluations, inspecting results, comparing runs, and summarizing regressions. To use the skill in another repository, copy the `quantiles` skill directory into that repository’s agent-supported skills directory.

`AGENTS.md` provides repository-specific instructions for agents working on Quantiles itself.

Example agent prompt:

```text
Use the Quantiles eval skill. Run the SimpleQA Verified benchmark with the built-in sampler, inspect the sample-level results, compare the latest run to the previous run if available, and summarize any regressions with evidence from the run output.
```

## Repository Structure

```text
quantiles/
├─ README.md
├─ LICENSE
├─ CHANGELOG.md
├─ CONTRIBUTING.md
├─ CODE_OF_CONDUCT.md
├─ SECURITY.md
├─ SUPPORT.md
├─ AGENTS.md
├─ llms.txt
│
├─ .agents/
│  └─ skills/
│     └─ quantiles/
│        └─ SKILL.md
│
├─ .github/
├─ packages/
├─ benchmarks/
├─ examples/
├─ docs/
├─ tests/
└─ scripts/
```

Important directories:

| Path | Purpose |
|---|---|
| `packages/` | CLI, core runtime, and SDK packages |
| `benchmarks/` | Built-in benchmark implementations and templates |
| `examples/` | Runnable examples for common evaluation workflows |
| `.skill/` | Reusable agent skills for using Quantiles |
| `docs/` | Documentation for CLI, SDKs, benchmarks, agents, and reference material |
| `tests/` | CLI, SDK, runtime, storage, and benchmark tests |
| `.github/` | GitHub Actions, issue templates, and pull request templates |

## Documentation

Start here:

- Quickstart: `docs/quickstart.md`
- CLI reference: `docs/cli/`
- SDKs: `docs/sdks/`
- Benchmarks: `docs/benchmarks/`
- Agents: `docs/agents/`
- Examples: `docs/examples/`
- Reference: `docs/reference/`

Full documentation is available at:

```text
https://quantiles.io/documentation
```

## Examples

The `examples/` directory includes workflows for:

- Running a first benchmark
- Using the built-in sampler
- Inspecting sample-level outputs
- Comparing runs
- Resuming interrupted runs
- Writing a custom Python eval
- Writing a custom TypeScript eval
- Using Quantiles with coding agents

## Contributing

Contributions are welcome.

Good contributions include:

- Bug fixes
- Documentation improvements
- Benchmark integrations
- SDK examples
- CLI improvements
- Tests and fixtures
- Agent workflow examples

Before opening a pull request, read:

[CONTRIBUTING.md](./CONTRIBUTING.md)

When changing benchmark behavior, scoring, CLI output, run schemas, or SDK APIs, update the relevant documentation and changelog.

## Security

Please do not report security vulnerabilities through public GitHub issues.

See:

[SECURITY.md](./SECURITY.md)

## Support

For questions, usage help, and community support, see:

[SUPPORT.md](./SUPPORT.md)

## License

Quantiles Open-source is licensed under the [Apache License 2.0](./LICENSE). Hosted, enterprise, or managed Quantiles products may be offered under separate commercial terms.
