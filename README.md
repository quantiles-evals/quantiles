# Quantiles Open-source

Quantiles is a local-first CLI and SDK for running AI evaluation workflows with fast, continuous feedback. Teams can iterate on model behavior, prompts, agent workflows, and debugging scripts while preserving the metrics and run histories needed to understand what improved, what regressed, and why.

It includes a CLI, SDKs, built-in benchmarks, local run history, and agent-friendly instructions for coding agents such as Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and other agentic development tools.

## Why Quantiles

Evaluation workflows quickly outgrow one-off scripts once teams need caching, retries, dataset handling, metrics capture, and run comparison. Quantiles gives teams those primitives without slowing down iteration.

With Quantiles, teams can rely on built-in infrastructure for:

- Write standard Python or TypeScript, with familiar developer patterns
- Run workflows locally from the CLI
- Automatically record runs, steps, metrics, events, inputs, and final outputs
- Store execution history locally in open data formats
- Debug individual samples with full step-by-step traces, inputs, and outputs
- Inspect and compare runs directly from the same `qt` CLI
- Resilient execution by default with step caching and restartable failed runs

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

>Important note: this `qt run` command will run the [`simpleqa-verified`](https://arxiv.org/abs/2509.07968) benchmark against a "model" that simply generates random text. This functionality is intended to quickly show you how to run evals with the `qt` tool, without requiring you to set up API keys or spend money on tokens. Do not expect to draw conclusions from the results returned from this command.

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

## CLI

Common commands:

```bash
qt init
qt run <evaluation>
qt list
qt show <run_id>
qt compare <run_id_a> <run_id_b>
```

Example:

```bash
qt run simpleqa-verified
qt show 1 --json
qt compare 1 2
```

Use `qt show` to inspect a single run and `qt compare` to compare behavior across runs.

## Local-First and Offline by Default

Quantiles is built as a local-first, offline system that keeps benchmark execution, metadata, metrics, and analysis on your computer by default. 

The Quantiles toolchain, including the qt CLI, SDKs, on-disk data formats, and REST API, is optimized to use the local computing power by default instead of relying on cloud or other non-local resources.

The CLI and SDKs (TypeScript or Python) support offline benchmark workflows, including the following local execution and analysis features:

- Benchmark code runs locally on your machine
- Measurements are computed locally, except for remote model calls, hosted judges, external tools, and LLM-as-judge evaluations that may call remote providers (e.g. OpenAI, Anthropic, cloud providers, etc.)
- Metadata is recorded to a local, on-disk database
- Metrics and evaluation outputs are recorded to local, on-disk files
- `qt show` and `qt compare` commands access only local metadata and analytics databases

## Built-in Benchmarks

Built-in benchmarks are ready-to-run evaluation harnesses with predefined datasets, scoring methodology, and metrics. Use them when you want a standardized evaluation that provides a common reference point, a repeatable baseline, or a well-defined implementation of an industry benchmark.

| Code                                                        | When to use                                                                                             |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `qt run $BENCHMARK`                                        | Use the demo sampler to inspect benchmark samples, scoring behavior, workflow steps, and metric outputs |
| `qt run $BENCHMARK --input '{"model":"$MODEL_NAME"}'`      | Run your model against a built-in benchmark                                                             |

## Custom Evaluations

A custom evaluation is a normal Python or TypeScript program that runs through the qt CLI as a tracked Quantiles workflow. Your code owns the evaluation logic: loading data, calling a model or agent, scoring outputs, computing metrics, and returning a summary. Quantiles records the run, durable steps, emitted metrics, events, inputs, outputs, and comparisons.

Use custom evaluations when you need to measure behavior that is specific to your product, workflow, prompt, dataset, rubric, or release process.

## SDKs

Use the official Quantiles SDKs to build with higher-level workflow primitives like durable steps, structured inputs/outputs, and metrics emission, using patterns and practices native to Python and TypeScript. SDKs integrate tightly with the `qt` CLI’s local API for running, recording and analyzing benchmarks.

- Link python sdk
- Link Typescript sdk

## Coding Agents

Quantiles is designed to work well with coding agents such as Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and other agentic development tools.

The repository includes agent-facing instructions and reusable skill files:

- [`SKILL.md`](https://github.com/quantiles-evals/skill/blob/main/SKILL.md)

`SKILL.md` gives agents durable Quantiles behavior for running evaluations, inspecting results, comparing runs, and summarizing regressions. To use the skill in another repository, copy the `quantiles` skill directory into that repository’s agent-supported skills directory.

Example agent prompt:

```text
Use the Quantiles eval skill. Run the SimpleQA Verified benchmark and summarize the results.
```

## Documentation


Full documentation is available at:

[`Quantiles Documentation`](https://quantiles.io/documentation/)

Start here:

- [`Quickstart`](https://quantiles.io/documentation/quickstart)
- [`Agent Overview`](https://quantiles.io/documentation/evals-with-agents)
- [`Python SDK`](https://quantiles.io/documentation/reference/python-sdk)
- [`TypeScript SDK`](https://quantiles.io/documentation/reference/typescript-sdk)

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
