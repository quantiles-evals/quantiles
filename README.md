<p align="center">
  <a href="https://quantiles.io">
    <img src="./docs/assets/quantiles-wordmark.svg" alt="Quantiles" width="240">
  </a>
</p>

<p align="center"><strong>Local-first AI evaluation for developers and coding agents.</strong></p>

<p align="center">
  <a href="https://github.com/quantiles-evals/quantiles/blob/main/LICENSE"><img src="https://img.shields.io/github/license/quantiles-evals/quantiles" alt="License"></a>
  <a href="https://quantiles.io/documentation"><img src="https://img.shields.io/badge/docs-quantiles.io-blue" alt="Documentation"></a>
  <a href="https://github.com/quantiles-evals/skill"><img src="https://img.shields.io/badge/agent%20skill-install-6f42c1" alt="Agent Skill"></a>
  <a href="https://huggingface.co/quantiles"><img src="https://img.shields.io/badge/Hugging%20Face-Quantiles-FFD21E" alt="Quantiles on Hugging Face"></a>
</p>

---

Quantiles is open-source, local-first evaluation infrastructure for applied AI systems, designed for developer and coding-agent workflows.

Use the `qt` CLI and Python SDK to create, run, analyze, and compare evaluations for models, prompts, and agents with resource-efficient local execution. Quantiles records metrics, sample-level results, execution history, and evaluation traces so you can measure system behavior, detect regressions, validate changes, and ship higher-quality, more reliable AI systems.

Quantiles centralizes its components in this monorepo so developers, researchers, and coding agents can use, inspect, modify, test, and extend the system. Its reusable skills and instruction files work with Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and other compatible agents.

## ![New](./docs/assets/new-badge.svg) What's New

**[2026.08.08]** Added built-in benchmark support for `gpqa`, `medmcqa`, `medqa`, `mmlu-pro`, and `pubmedqa`. The new `qt add <benchmark_name>` command downloads a built-in benchmark’s configuration and prompt from the hosted registry, adds them to the local project, and makes the benchmark easy to customize. See [Built-in benchmarks](https://quantiles.io/documentation/built-in-benchmarks) for details.

**[2026.07.27]** Published the [model configuration guide](https://quantiles.io/documentation/model-configuration), covering the built-in demo model, supported model providers, credentials, request concurrency, cost and data handling, and troubleshooting.

**[2026.07.19]** Added custom configuration (`custom_nocode`) evaluations, which let users configure custom evals in `quantiles.toml` without writing or maintaining custom code. See the [custom configuration evaluation documentation](https://quantiles.io/documentation/custom-evaluations/custom-nocode-evaluations) for details.

## Why use Quantiles?

Evaluation workflows quickly outgrow one-off scripts once teams need caching, retries, dataset handling, metrics capture, and run comparison. Quantiles gives teams those primitives so they don't have to build them from scratch:

- Run evaluation workflows locally from the CLI
- Automatically record evaluation runs, steps, metrics, events, inputs, and final outputs
- Store execution history locally in open data formats
- Analyze individual samples using recorded step status, outputs, and metrics
- Inspect and compare evaluation runs directly from the same `qt` CLI
- Write standard Python with familiar Pythonic patterns
- Resume failed or interrupted evaluation runs without repeating completed work

Quantiles borrows concepts from durable workflow execution systems to make evaluation runs resilient to crashes and restarts, while adding a high-throughput execution engine, rich observability, metrics, and eval reproducibility. Use it to run custom evaluations or benchmarks from the Quantiles registry, then inspect what changed across runs without requiring notebooks, pipelines, manual comparisons, or a hosted evaluation service.

## Quickstart

Install the CLI:

```bash
curl -fsSL https://cli.quantiles.io/install.sh | bash
```

Run [SimpleQA Verified](https://quantiles.io/benchmark-hub/benchmark/simpleqa-verified) from the Quantiles benchmark registry:

```bash
qt run simpleqa-verified
```

The command above downloads the [`simpleqa-verified`](https://quantiles.io/benchmark-hub/benchmark/simpleqa-verified) definition from the hosted Quantiles benchmark registry and runs it locally with a demo model that generates random text. Fetching the benchmark definition and an uncached dataset requires network access, but no provider API key or paid model inference is required.

> The demo model validates the evaluation workflow. Do not use its results to draw conclusions about model quality.

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
# download an eval from the Quantiles hosted benchmark registry
# and save it to the local quantiles.toml configuration file
qt add <eval_name>
# run the eval that was just downloaded and saved with the previous
# `qt add` command.
qt run <eval_name> [--input <json>]
qt list
qt show <run_id>
qt compare <run_id_a> <run_id_b>
qt resume <run_id>
```

> Note: Pass `--json` to `qt add`, `qt run`, `qt list`, `qt show`, `qt compare`, or `qt resume` to request machine- and agent-friendly output.

See the [CLI reference](https://quantiles.io/documentation/reference/cli) for available commands, options, and usage details.

### Configuration and customization


You can customize how the CLI executes [built-in benchmarks](https://quantiles.io/documentation/built-in-benchmarks), [custom configuration evaluations](https://quantiles.io/documentation/custom-evaluations/custom-nocode-evaluations), and [custom code evaluations](https://quantiles.io/documentation/custom-evaluations) using a `quantiles.toml` or `.quantiles.toml` configuration file in the current working directory. When you run a benchmark or evaluation, Quantiles first checks the configuration file for a matching local definition. If none is found, it queries the hosted Quantiles benchmark registry (hosted at `api.quantiles.io`) for a built-in benchmark with that name.

See the following resources for more details:

- [Configuration documentation](https://quantiles.io/documentation/configuration) - Detailed configuration instructions and reference documentation for supported fields, validation rules, and examples.
- [Model configuration guide](https://quantiles.io/documentation/model-configuration) - Configure provider models and credentials, and troubleshoot common setup issues.
- [Custom-code configuration example](./cli/examples/configs/custom_code/quantiles.toml) - A complete Python SDK evaluation configuration.
- [Custom configuration examples](./custom-nocode-examples/quantiles.toml) - Complete dataset, prompt, model, and scoring configurations.

#### Registry benchmarks

[Built-in benchmarks](https://quantiles.io/documentation/built-in-benchmarks) are ready-to-run evaluations with predefined datasets, scoring methods, and metrics. Use them to start evaluating quickly or establish a repeatable baseline. Configuration is optional and can override execution settings such as the model and sample count.

When a built-in benchmark is run, Quantiles downloads its definition and prompt files into memory and verifies each file against its declared size and SHA-256 hash. These files are not cached on disk. Benchmark datasets are downloaded separately and may be cached locally.

To save and customize a built-in benchmark locally, add it by name:

```bash
qt add simpleqa-verified
```

This command downloads the benchmark definition and prompt template, appends the benchmark to an existing `quantiles.toml` or `.quantiles.toml`, or creates `quantiles.toml` in the current directory. The prompt template is stored beside the configuration at `<benchmark_name>-prompt/prompt.txt`. The command returns an error if the benchmark is already configured or is not present in the registry. Pass `--json` for machine-readable output.

The [Quantiles Benchmark Hub](https://quantiles.io/benchmark-hub) describes available benchmarks, their evaluation setup, and common metrics used across AI evaluation workflows.

> To request another registry benchmark, [file an issue](https://github.com/quantiles-evals/quantiles/issues) with its name, source dataset or repository, and any available reference implementation.

#### Custom evaluations

Custom evaluations measure behavior specific to your product, workflow, prompt, dataset, rubric, or release process. Quantiles provides two ways to build them:

- [Custom configuration (`custom_nocode`) evaluations](https://quantiles.io/documentation/custom-evaluations/custom-nocode-evaluations): define a custom evaluation entirely in configuration without writing or maintaining Python.
- [Custom code (`custom_code`) evaluations](https://quantiles.io/documentation/custom-evaluations): build specialized evaluation logic with [Python](https://quantiles.io/documentation/reference/python-sdk).

Prefer custom configuration evaluations wherever possible because they are easier for humans and agents to create and maintain. Use a custom code evaluation when the required behavior cannot be expressed in configuration.

##### Python SDK for `custom_code` evaluations

Use the [official Quantiles Python SDK](https://quantiles.io/documentation/reference/python-sdk) to build `custom_code` evaluations. The SDK provides Python-native APIs for resilient, efficient evaluations, including durable steps, structured inputs and outputs, and high-performance metrics emission.

The SDK integrates tightly with the `qt` CLI’s local API for running, recording, and analyzing benchmarks.

The [Python SDK source code](./python) is available in this repository, and the [Python SDK reference](https://quantiles.io/documentation/reference/python-sdk) has usage instructions and API documentation.

## Local-First Execution and Offline Workflows

Quantiles is a [local-first system that supports offline workflows](https://quantiles.io/documentation/local-first-offline) and stores evaluation metadata, outputs, and metrics on your computer by default.

Quantiles supports fully offline evaluation through both the CLI and Python SDK when all required configurations, prompts, datasets, models, and other dependencies are available locally:

- Quantiles scoring and metric aggregation are computed locally.
- Run metadata, inputs, outputs, steps, and events are stored in a local [SQLite](https://sqlite.org/) database.
- Metrics are stored in local [Parquet](https://parquet.apache.org/) files.
- `qt show`, `qt list`, and `qt compare` access only local metadata and metrics stores.
- Python evaluation code runs locally on your machine.

Network access may be required to retrieve configurations from the hosted benchmark registry, retrieve datasets, or call hosted AI models.

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
