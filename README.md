# Quantiles

Quantiles is open-source, local-first evaluation infrastructure for applied AI systems, designed for developer and coding-agent workflows.

Use the `qt` CLI and Python SDK to create, run, analyze, and compare evaluations for models, prompts, and agents with resource-efficient local execution. Quantiles records metrics, sample-level results, execution history, and evaluation traces so you can measure system behavior, detect regressions, validate changes, and ship higher-quality, more reliable AI systems.

Quantiles centralizes its components in this monorepo so developers, researchers, and coding agents can use, inspect, modify, test, and extend the system. Its reusable skills and instruction files work with Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and other compatible agents.

## ![New](./docs/assets/new-badge.svg) What's New

**[2026.07.07]** Added `custom_nocode` evaluations, which let users configure custom evals in `quantiles.toml` without writing or maintaining custom code. See the [configuration guide](./CONFIG.md#custom_nocode-evaluations) for details.

## Why use Quantiles?

Evaluation workflows quickly outgrow one-off scripts once teams need caching, retries, dataset handling, metrics capture, and run comparison. Quantiles gives teams those primitives so they don't have to build them from scratch:

- Run evaluation workflows locally from the CLI
- Automatically record evaluation runs, steps, metrics, events, inputs, and final outputs
- Store execution history locally in open data formats
- Analyze individual samples using recorded step status, outputs, and metrics
- Inspect and compare evaluation runs directly from the same `qt` CLI
- Write standard Python with familiar Pythonic patterns
- Resume interrupted or failed runs without repeating completed work

Quantiles borrows concepts from durable workflow execution systems to make evaluation runs resilient to crashes and restarts, while adding a high-throughput execution engine, rich observability, metrics, and eval reproducibility. Use it to run custom eval code or built-in benchmarks, then inspect what changed across runs without requiring notebooks, pipelines, manual comparisons, or a hosted evaluation service.

## Quickstart

Install the CLI:

```bash
curl -fsSL https://cli.quantiles.io/install.sh | bash
```

Run the [SimpleQA Verified](https://quantiles.io/benchmark-hub/benchmark/simpleqa-verified) built-in benchmark:

```bash
qt run simpleqa-verified
```

> The command above runs [`simpleqa-verified`](https://quantiles.io/benchmark-hub/benchmark/simpleqa-verified) with a demo model that generates random text. It validates the evaluation workflow without requiring provider API keys or incurring inference costs. Do not use its results to draw conclusions about model quality.

Inspect the recorded run:

```bash
# If you've run `qt run` before, you might need to pass a different integer to `qt show`
#
# See all your runs with `qt list`.
qt show 1
```

Or output machine- and agent-readable JSON:

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
qt --version
qt run <eval_name>
qt list
qt show <run_id>
qt compare <run_id_a> <run_id_b>
```

> Note: Pass `--json` to any of these commands to output machine- and agent-friendly JSON instead of human-formatted output.

See the [CLI reference](https://quantiles.io/documentation/reference/cli) for available commands, options, and usage details.

### Configuration and customization

You can customize how the CLI executes [built-in-benchmarks](https://quantiles.io/documentation/built-in-benchmarks), [custom no-code evaluations](https://quantiles.io/documentation/custom-nocode-evaluations), and [custom code evaluations](https://quantiles.io/documentation/custom-evaluations) using a `quantiles.toml` or `.quantiles.toml` configuration file in the current working directory or a parent directory. The CLI uses this configuration each time you run the benchmark with `qt run`.

See the following resources for more details:

- [Configuration guide](./CONFIG.md) - Detailed configuration instructions and reference documentation for supported fields, validation rules, and examples.
- [Configuration examples](./cli/examples/configs) - Complete examples, including a [custom-code evaluation](./cli/examples/configs/custom_code/quantiles.toml)

#### Built-in benchmarks

[Built-in benchmarks](https://quantiles.io/documentation/built-in-benchmarks) are ready-to-run evaluations with predefined datasets, scoring methods, and metrics. Configuration is optional and can override execution settings such as the model and sample count. Use them to get started quickly or establish a repeatable baseline.

The [benchmark hub](https://quantiles.io/benchmark-hub) describes available benchmarks, their evaluation setup, and common metrics used across AI evaluation workflows.

> To request another open-source built-in benchmark, [file an issue](https://github.com/quantiles-evals/quantiles/issues) with its name, source dataset or repository, and any available reference implementation.

#### Custom evaluations

Custom evaluations measure behavior specific to your product, workflow, prompt, dataset, rubric, or release process. Quantiles provides two ways to build them:

- [`custom_nocode`](./CONFIG.md#custom_nocode-evaluations): define a custom evaluation entirely in configuration.
- [`custom_code`](https://quantiles.io/documentation/custom-evaluations): build specialized evaluation logic with [Python](https://quantiles.io/documentation/reference/python-sdk).

Prefer to use `custom_nocode` evaluations wherever possible, since they're easier for humans and agents to create and maintain. When required, fall back to `custom_code` evaluations.

#### Python SDK for `custom_code` evaluations

Use the [official Quantiles Python SDK](https://quantiles.io/documentation/reference/python-sdk) to build `custom_code` evaluations. The SDK provides Python-native APIs for resilient, efficient evaluations, including durable steps, structured inputs and outputs, and high-performance metrics emission.

The SDK integrates tightly with the `qt` CLI’s local API for running, recording, and analyzing benchmarks.

The [Python SDK source code](./python) is available in this repository, and the [Python SDK reference](https://quantiles.io/documentation/reference/python-sdk) has usage instructions and API documentation.

## Local-First and Offline by Default

Quantiles is a [local-first system that supports offline workflows](https://quantiles.io/documentation/local-first-offline) and stores evaluation metadata, outputs, and metrics on your computer by default.

The entire Quantiles toolchain, including the `qt` CLI, SDKs, on-disk data formats, and REST API, is optimized to use your local computing power instead of relying on cloud or other non-local resources.

The CLI and Python SDK support offline evaluation workflows when the required code, datasets, and models are available locally:

- Quantiles scoring and metric aggregation are computed locally.
- Run metadata, inputs, outputs, steps, and events are stored in a local [SQLite](https://sqlite.org/) database.
- Metrics are stored in local [Parquet](https://parquet.apache.org/) files.
- `qt show`, `qt list`, and `qt compare` access only local metadata and metrics stores.
- Python evaluation code runs locally on your machine.

Downloading uncached datasets and calling remote models, hosted judges, or external tools requires network access. These operations occur only when requested by the selected benchmark or evaluation configuration.

## Coding Agents

Quantiles is designed for use with coding agents such as Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, and OpenCode. The [Quantiles `llms.txt`](https://quantiles.io/llms.txt) provides a concise, public, LLM-readable overview with links to agent guides and related documentation that agents can use for additional context.

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
