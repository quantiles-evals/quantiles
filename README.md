# Quantiles

Quantiles is open-source, local-first evaluation infrastructure for applied AI systems, designed for developer and coding-agent workflows.

Use the `qt` CLI and Python SDK to create, run, analyze, and compare evaluations for models, prompts, and agents with resource-efficient local execution. Quantiles records metrics, sample-level results, execution history, and evaluation traces so you can measure system behavior, detect regressions, validate changes, and ship higher-quality, more reliable AI systems.

Quantiles centralizes its components in this monorepo so developers, researchers, and coding agents can use, inspect, modify, test, and extend the system. Its reusable skills and instruction files work with Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and other compatible agents.

## ![New](./docs/assets/new-badge.svg) What's New

**[2026.07.07]** Added `custom_nocode` evaluations, which let users configure custom evals in `quantiles.toml`, without writing/maintaining any code. See [documentation](./cli/README.md#custom-no-code-evals) for more details.

## Why use Quantiles?

Evaluation workflows quickly outgrow one-off scripts once teams need caching, retries, dataset handling, metrics capture, and run comparison. Quantiles gives teams those primitives so they don't have to build them from scratch:

- Run workflows locally from the CLI
- Automatically record evaluation runs, steps, metrics, events, inputs, and final outputs
- Store execution history locally in open data formats
- Debug individual samples with full step-by-step traces, inputs, and outputs
- Inspect and compare runs directly from the same `qt` CLI
- Write standard Python, with familiar, Pythonic patterns
- Resilient execution by default with caching and restartable failed runs

Quantiles borrows concepts from durable workflow execution systems to make evaluation runs resilient to crashes and restarts, while adding a high-throughput execution engine, rich observability, metrics, and eval reproducibility. Use it to run custom eval code or built-in benchmarks, then inspect what changed across runs, all without notebooks, pipelines, manual comparisons, or cloud services.

## Quickstart

Install the CLI:

```bash
curl -fsSL https://cli.quantiles.io/install.sh | bash
```

Run the [SimpleQA Verified](https://quantiles.io/benchmark-hub/benchmark/simpleqa-verified) built-in benchmark:

```bash
qt run simpleqa-verified
```

> Important note: the above `qt run` command will run the [`simpleqa-verified`](https://quantiles.io/benchmark-hub/benchmark/simpleqa-verified) benchmark against a demo "model" that simply generates random text. This functionality is intended to quickly show you how to run evals with the `qt` tool, without requiring you to set up provider API keys or spend money on tokens. Do not expect to draw conclusions about any models from the results returned from using the demo model.

Inspect the recorded run:

```bash
# If you've run `qt run` before, you might need to pass a different integer to `qt show`
# 
# See all your runs with `qt list`.
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
qt run <eval_name>
qt list
qt show <run_id>
qt compare <run_id_a> <run_id_b>
```

> Note: Pass the `--json` flag to any of the above commands, to output machine and agent-friendly JSON instead of human-formatted output.

See the [CLI reference](https://quantiles.io/documentation/reference/cli) for available commands, options, and usage details.

### Configuration file and customization

You can customize how the CLI executes [built-in-benchmarks](https://quantiles.io/documentation/built-in-benchmarks), [custom no-code evaluations](https://quantiles.io/documentation/custom-nocode-evaluations), and [custom code evaluations](https://quantiles.io/documentation/custom-evaluations) using a `quantiles.toml` or `.quantiles.toml` configuration file in the current working directory or a parent directory. The CLI uses this configuration each time you run the benchmark with `qt run`.

See the following resources for more details:

- [Configuration guide](./CONFIG.md) - Detailed configuration instructions and reference documentation.
- [Complete configuration examples](./cli/examples/configs) - Complete examples, including a [custom-code benchmark configuration](./cli/examples/configs/custom_code/quantiles.toml)

#### Built-in Benchmarks

[Built-in-benchmarks](https://quantiles.io/documentation/built-in-benchmarks) are ready-to-run evaluations with predefined datasets, scoring methodologies, and metrics. Configuration is optional and can be used to override execution settings, including the model. They're intended to get started quickly with the Quantiles stack or run a standardized evaluation for a common reference point or repeatable baseline.

Quantiles also provides a [benchmark hub](https://quantiles.io/benchmark-hub) for discovering more benchmarks, including the built-in ones, and understanding their evaluation setup and reviewing common metrics used across AI evaluation workflows.

> If there is an open-source benchmark you would like to add as a built-in benchmark, please [file an issue](https://github.com/quantiles-evals/quantiles/issues) with the benchmark name, source dataset/repository and any reference implementation if one is available.

#### Custom Evaluations

Custom evaluations are important for measuring behaviors specific to your product, workflow, prompt, dataset, rubric, or release process, beyond what the [built-in benchmarks](https://quantiles.io/documentation/built-in-benchmarks) can provide. Quantiles provides two different ways to build them:

- [`custom_nocode`](https://quantiles.io/documentation/custom-nocode-evaluations) - A custom evaluation from configuration only, without writing any custom code.
- [`custom_code`](https://quantiles.io/documentation/custom-code-evaluations) - A highly specialized, custom evaluation built with [Python](https://quantiles.io/documentation/reference/python-sdk)

Prefer to use `custom_nocode` evaluations wherever possible, since they're easier for humans and agents to create and maintain. When required, fall back to `custom_code` evaluations.

#### Python SDK for `custom_code` evaluations

Use the [official Quantiles Python SDK](https://quantiles.io/documentation/reference/python-sdk) to build your `custom_code` evaluations. This SDK provides Python-native APIs to the primitives the Quantiles platform provides for building and running resilient, efficient evaluatuations, including durable steps, structured inputs/outputs, and high-performance metrics emission.

The SDK integrates tightly with the `qt` CLI’s local API for running, recording, and analyzing benchmarks.

The [Python SDK source code](./python) is available in this repository, and the [Python SDK reference](https://quantiles.io/documentation/reference/python-sdk) has usage instructions and API documentation.

## Local-First and Offline by Default

Quantiles a [local-first, offline system](https://quantiles.io/documentation/local-first-offline) that stores benchmark execution, metadata, metrics, and analytics data on your computer by default.

The entire Quantiles toolchain, including the `qt` CLI, SDKs, on-disk data formats, and REST API, is optimized to use your local computing power instead of relying on cloud or other non-local resources.

Both the CLI and Python SDK support offline benchmark workflows, including the following local execution and analysis features:

- For `custom_code` evaluations built with Quantiles SDKs, Python code runs locally on your machine.
- Measurements are computed locally, though model calls, datasets, LLM judges, and agentic tool calls might use remote providers such as OpenAI, Anthropic, or other cloud services.
- Metadata are recorded to a local, on-disk [SQLite](https://sqlite.org/) database.
- Metrics and evaluation outputs are recorded to local, on-disk [Parquet](https://parquet.apache.org/) files.
- `qt show`, `qt list` and `qt compare` commands access only local metadata and analytics databases.

## Coding Agents

Quantiles is designed for use with coding agents such as Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, and OpenCode. The [Quantiles `llms.txt`](https://quantiles.io/llms.txt) provides a concise, public, LLM-readable overview with links to agent guides and related documentation. Your agent will read this file to learn about Quantiles, then follow its links to find the additional information it needs to complete its task.

### `SKILL.md`

The [Quantiles agent skill repository](https://github.com/quantiles-evals/skill) provides a [`SKILL.md`](https://github.com/quantiles-evals/skill/blob/main/SKILL.md) instruction file that guides coding agents through creating, running, analyzing, and comparing evaluations. Use the following agent prompt to install it:

```text
Install the Quantiles eval skill at github.com/quantiles-evals/skill
```

If you want your agent to run an eval, use the following prompt:

```text
Use the Quantiles eval skill to run the SimpleQA Verified benchmark and summarize the results.
```

### `AGENTS.md`

The embedded [`AGENTS.md` file](./AGENTS.md) gives agents repository-specific instructions, such as how to add features to the CLI and SDKs, ensuring that contributors can use agents of their choice to make high-quality contributions to the Quantiles open source components.

## Documentation

See the [Quantiles documentation](https://quantiles.io/documentation/) for comprehensive guides and reference documentation.

Start here:

- [Quickstart](https://quantiles.io/documentation/quickstart)
- [Agent Overview](https://quantiles.io/documentation/evals-with-agents)
- [Python SDK](https://quantiles.io/documentation/reference/python-sdk)

## Contributing

Quantiles exists to make AI evaluation workflows more practical, repeatable, and useful for engineering teams. We welcome contributions from the community, whether you are fixing bugs, improving documentation, adding evaluations and benchmarks, or helping make the open-source Quantiles project more reliable for AI engineers and researchers.

Please read our [contributing guide](./CONTRIBUTING.md) to get started.

## Security

Please do not report security vulnerabilities through public GitHub issues. Follow the security reporting guidance in [SECURITY.md](./SECURITY.md).

## License

Quantiles open source is licensed under the [Apache License 2.0](./LICENSE). Hosted, enterprise, or managed Quantiles products may be offered under separate commercial terms.
