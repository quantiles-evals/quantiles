# Quantiles Open-source

Quantiles is a local-first CLI and SDK for running durable AI evaluation workflows with fast, continuous feedback. Teams can iterate on model behavior, prompts, agent workflows, and debugging scripts while preserving the metrics and run histories needed to understand what improved, what regressed, and why.

It includes a CLI, SDKs, built-in benchmarks, local run history, and agent-friendly instructions for coding agents such as Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and other agentic development tools. This monorepo centralizes all the pieces of Quantiles, making it easier for engineers and coding agents to inspect, change, test, and extend the system.

## Why Quantiles

Evaluation workflows quickly outgrow one-off scripts once teams need caching, retries, dataset handling, metrics capture, and run comparison. Quantiles gives teams those primitives without slowing down iteration.

With Quantiles, teams can rely on built-in infrastructure:

- Write standard Python or TypeScript, with familiar developer patterns
- Run workflows locally from the CLI
- Automatically record runs, steps, metrics, events, inputs, and final outputs
- Store execution history locally in open data formats
- Debug individual samples with full step-by-step traces, inputs, and outputs
- Inspect and compare runs directly from the same `qt` CLI
- Resilient execution by default with caching and restartable failed runs

Quantiles borrows concepts from durable workflow execution systems to make evaluation runs resilient to crashes and restarts, while adding a high-throughput execution engine, rich observability, metrics, and eval reproducibility. Use it to run custom eval code or built-in benchmarks, then inspect what changed across runs, all without notebooks, pipelines, manual comparisons, or cloud services.

## Quickstart

Install the CLI:

```bash
curl -fsSL https://cli.quantiles.io/install.sh | bash
```

Run a [built-in benchmark](https://arxiv.org/abs/2509.07968):

```bash
qt run simpleqa-verified
```

> Important note: this `qt run` command will run the [`simpleqa-verified`](https://arxiv.org/abs/2509.07968) benchmark against a "model" that simply generates random text. This functionality is intended to quickly show you how to run evals with the `qt` tool, without requiring you to set up API keys or spend money on tokens. Do not expect to draw conclusions from the results returned from this command.

Inspect the recorded run:

```bash
qt show 1
```

Or, output machine and agent-readable JSON:

```bash
qt show 1 --json
```

For the complete command reference:

```bash
qt --help
```

## CLI

Use `qt show` to inspect a single run, `qt list` to see a list of all runs, and `qt compare` to compare behavior across runs.

Common commands:

```bash
qt run <evaluation>
qt list
qt show <run_id>
qt compare <run_id_a> <run_id_b>
```

>Note: you can pass the `--json` flag to any of the above commands, to output machine and agent-friendly JSON instead of human-formatted output.

To learn more detail about what you can do with the CLI, see [quantiles.io/documentation/reference/cli](https://quantiles.io/documentation/reference/cli).

### Customization

You can customize how the CLI executes benchmarks using a `quantiles.toml` or `.quantiles.toml` configuration file. This file can be used to control benchmark execution behavior as well as customize the models, providers, and other settings used during eval runs. See [`./cli/examples/configs`](./cli/examples/configs) for examples and more details.

## Local-First and Offline by Default

Quantiles is built as a local-first, offline system that keeps benchmark execution, metadata, metrics, and analysis on your computer by default.

The Quantiles toolchain, including the `qt` CLI, SDKs, on-disk data formats, and REST API, is optimized to use your local computing power by default instead of relying on cloud or other non-local resources.

The CLI and SDKs (TypeScript or Python) support offline benchmark workflows, including the following local execution and analysis features:

- Benchmark code runs locally on your machine
- Measurements are computed locally, except for remote model calls, hosted judges, external tools, and LLM-as-judge evaluations that may call remote providers (e.g. OpenAI, Anthropic, cloud providers, etc.)
- Metadata are recorded to a local, on-disk database
- Metrics and evaluation outputs are recorded to local, on-disk files
- `qt show`, `qt list` and `qt compare` commands access only local metadata and analytics databases

## Built-in Benchmarks

Built-in benchmarks are ready-to-run evaluation harnesses with predefined datasets, scoring methodology, and metrics. Use them when you want a standardized evaluation that provides a common reference point, a repeatable baseline, or a well-defined implementation of an industry benchmark.

| Code                                                  | When to use |
| --- | --- |
| `qt run $BENCHMARK` | Run a built-in benchmark against the demo model to inspect sample-level inputs and outputs, scoring behavior, workflow steps, and aggregate metrics |
| `qt run $BENCHMARK --input '{"model":"$MODEL_NAME"}'` | Run a built-in benchmark against your model |

## Custom Evaluations

A custom evaluation is a [Python](https://quantiles.io/documentation/reference/python-sdk) or [TypeScript](https://quantiles.io/documentation/reference/typescript-sdk) program that is run by the `qt` CLI and uses the [Quantiles API](https://quantiles.io/documentation/reference/rest-api) to execute an eval. Your code owns the evaluation logic like loading data, calling a model or agent, scoring outputs, computing metrics, and returning a summary. Quantiles manages [durable steps, step caching, and step resume](https://quantiles.io/documentation/workflows-and-steps), metrics, inputs, outputs, and comparisons.

Use custom evaluations when you need to measure behavior that is specific to your product, workflow, prompt, dataset, rubric, or release process.

Read more about how to build and run custom evaluations at [quantiles.io/documentation/custom-evaluations](https://quantiles.io/documentation/custom-evaluations).

### SDKs

Use the official Quantiles SDKs to build your custom evaluations with higher-level workflow primitives like durable steps, structured inputs/outputs, and metrics emission, using patterns and practices native to Python and TypeScript. The SDKs integrate tightly with the `qt` CLI’s local API for running, recording and analyzing benchmarks.

The Python SDK is located in this repository at [`python/`](./python). Read more about it at [quantiles.io/documentation/reference/python-sdk](https://quantiles.io/documentation/reference/python-sdk). The TypeScript SDK is located in this repository at [`typescript/`](./typescript). Read more about it at [quantiles.io/documentation/reference/typescript-sdk](https://quantiles.io/documentation/reference/typescript-sdk).

## Coding Agents

Quantiles is designed to work well with coding agents such as Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and other agentic development tools.

The [github.com/quantiles-evals/skill](https://github.com/quantiles-evals/skill) repository includes a [`SKILL.md`](https://github.com/quantiles-evals/skill/blob/main/SKILL.md) that gives agents complete instructions for running evaluations, inspecting results, comparing runs, and summarizing regressions. To use the skill with your agent, install it with the below prompt:

```text
Install the Quantiles eval skill at github.com/quantiles-evals/skill
```

If you want your agent to run an eval, use the below prompt:

```text
 Use the Quantiles eval skill to run the SimpleQA Verified benchmark and summarize the results.
 ```

## Documentation

Full documentation is available at:

[Quantiles Documentation](https://quantiles.io/documentation/)

Start here:

- [Quickstart](https://quantiles.io/documentation/quickstart)
- [Agent Overview](https://quantiles.io/documentation/evals-with-agents)
- [Python SDK](https://quantiles.io/documentation/reference/python-sdk)
- [TypeScript SDK](https://quantiles.io/documentation/reference/typescript-sdk)

## Contributing

Quantiles exists to make AI evaluation workflows more practical, repeatable, and useful for engineering teams. We welcome contributions from the community, whether you are fixing bugs, improving documentation, adding evaluations and benchmarks, or helping make Quantiles Open Source more reliable for AI engineers and researchers.

Please read our [contributing guide](./CONTRIBUTING.md) to get started.

## Security

Please do not report security vulnerabilities through public GitHub issues. Follow the reporting guidance in [SECURITY.md](./SECURITY.md).

## License

Quantiles Open-source is licensed under the [Apache License 2.0](./LICENSE). Hosted, enterprise, or managed Quantiles products may be offered under separate commercial terms.
