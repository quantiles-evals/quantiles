# Quantiles Open-source

Quantiles is a local-first CLI and SDK for running durable AI evaluation workflows with fast, continuous feedback. Teams can iterate on model behavior, prompts, agent workflows, and debugging scripts while preserving the metrics and run histories needed to understand what improved, what regressed, and why.

It includes a CLI (called `qt`), SDKs, built-in benchmarks, local run history, and agent-friendly instructions for coding agents such as Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and other agentic development tools. This monorepo centralizes all the pieces of Quantiles, making it easier for engineers and coding agents to inspect, change, test, and extend the system.

## Why Quantiles

Evaluation workflows quickly outgrow one-off scripts once teams need caching, retries, dataset handling, metrics capture, and run comparison. Quantiles gives teams those primitives so they don't have to build them from scratch:

- Write standard Python, with familiar developer patterns
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

Run the [SimpleQA-verified](https://arxiv.org/abs/2509.07968) built-in benchmark:

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

### Configuration file and customization

You can customize how the CLI executes benchmarks using a `quantiles.toml` or `.quantiles.toml` configuration file in the current working directory.

For **built-in benchmarks**, configure settings like `samples`, `model`, and `max_workers`:

```toml
[benchmarks.pubmedqa]
samples = 50
model = "openai:gpt-5.4-nano"
max_workers = 100
```

For **custom evaluations**, set `type = "custom_code"` and provide the `command` to run:

```toml
[benchmarks.my-eval]
type = "custom_code"
command = ["python", "my_eval.py"]

[benchmarks.my-eval.input]
foo = "foo_val"
bar = "bar_val"
```

The CLI will execute the command with `QUANTILES_RUN_ID`, `QUANTILES_WORKFLOW_NAME`, `QUANTILES_BASE_URL`, and `QUANTILES_INPUT` environment variables injected. If the run fails, you can resume it later with `qt resume <run_id>`.

See the following resources for more details:

- [`CONFIG.md`](./CONFIG.md) - in-depth guides to configuration and reference
- [`./cli/examples/configs`](./cli/examples/configs) - complete examples, including a [custom code benchmark example](./cli/examples/configs/custom_code/quantiles.toml)

## Local-First and Offline by Default

Quantiles is built as a local-first, offline system that keeps benchmark execution, metadata, metrics, and analysis on your computer by default.

The Quantiles toolchain, including the `qt` CLI, SDKs, on-disk data formats, and REST API, is optimized to use your local computing power by default instead of relying on cloud or other non-local resources.

Both the CLI and Python SDK support offline benchmark workflows, including the following local execution and analysis features:

- Benchmark code runs locally on your machine
- Measurements are computed locally, except for remote model calls, hosted judges, external tools, and LLM-as-judge evaluations that may call remote providers (e.g. OpenAI, Anthropic, cloud providers, etc.)
- Metadata are recorded to a local, on-disk database
- Metrics and evaluation outputs are recorded to local, on-disk files
- `qt show`, `qt list` and `qt compare` commands access only local metadata and analytics databases

## Built-in Benchmarks

Built-in benchmarks are ready-to-run evaluations with predefined datasets, scoring methodologies, and metrics. Use them when you want a standardized evaluation that provides a common reference point, a repeatable baseline, or a well-defined implementation of an industry benchmark.

| Code | When to use |
| --- | --- |
| `qt run <benchmark>` | Run a built-in benchmark against the demo model to inspect sample-level inputs and outputs, scoring behavior, workflow steps, and aggregate metrics |
| `qt run <benchmark> --input '{"model":"<model_name>}'` | Run a built-in benchmark against your model |

Quantiles also provides a [benchmark hub](https://quantiles.io/benchmark-hub) for discovering built-in benchmarks, understanding their evaluation setup, and reviewing common metrics used across AI evaluation workflows.

### Add a built-in benchmark 

If there is an open-source benchmark you would like to add as a built-in benchmark, [file an issue](https://github.com/quantiles-evals/quantiles/issues).

Helpful requests include the benchmark name, source dataset or repository, license and any reference implementation.

## Custom Evaluations

A custom evaluation is a [Python](https://quantiles.io/documentation/reference/python-sdk) program that is run by the `qt` CLI and uses its [local storage](http://quantiles.io/documentation/local-first-offline) and [durable workflow engine](https://quantiles.io/documentation/workflows-and-steps) to run efficiently and reliably. Your code owns the evaluation logic like loading data, calling a model or agent, scoring outputs, computing metrics, and returning a summary. Quantiles manages [durable steps, step caching, and step resume](https://quantiles.io/documentation/workflows-and-steps), metrics, inputs, outputs, and comparisons.

Custom evaluations are configured in `quantiles.toml` with `type = "custom_code"`:

```toml
[benchmarks.my-eval]
type = "custom_code"
command = ["python", "my_eval.py"]

[benchmarks.my-eval.input]
dataset = "my_dataset.jsonl"
```

Run the evaluation with `qt run my-eval`. If it fails, resume it later with `qt resume <run_id>` — the CLI re-reads the command and stored input automatically.

Use custom evaluations when you need to measure behavior that is specific to your product, workflow, prompt, dataset, rubric, or release process.

Read more about how to build and run custom evaluations at [quantiles.io/documentation/custom-evaluations](https://quantiles.io/documentation/custom-evaluations).

### Python SDK

Use the official Quantiles Python SDK to build your custom evaluations with primitives like durable steps, structured inputs/outputs, and metrics emission, using patterns and practices native to Python. The SDK integrates tightly with the `qt` CLI’s local API for running, recording and analyzing benchmarks.

The code for the Python SDK is located in this repository at [`./python/`](./python). Read more about it at [quantiles.io/documentation/reference/python-sdk](https://quantiles.io/documentation/reference/python-sdk)

## Coding Agents

Quantiles is designed to work well with coding agents such as Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and other agentic development tools. For a concise, public, LLM-readable overview of Quantiles with links to agent guides and related documentation, see [quantiles.io/llms.txt](https://quantiles.io/llms.txt).

### `SKILL.md`

The [github.com/quantiles-evals/skill](https://github.com/quantiles-evals/skill) repository includes a [`SKILL.md`](https://github.com/quantiles-evals/skill/blob/main/SKILL.md) file that gives agents complete instructions for running evaluations, inspecting results, comparing runs, and summarizing regressions. To use the skill with your agent, install it with the below prompt:

```text
Install the Quantiles eval skill at github.com/quantiles-evals/skill
```

If you want your agent to run an eval, use the below prompt:

```text
 Use the Quantiles eval skill to run the SimpleQA Verified benchmark and summarize the results.
 ```

 ### `AGENTS.md`

 The embedded [`AGENTS.md` file](./AGENTS.md) gives agents repository-specific instructions, such as how to add features to the CLI and SDKs, ensuring that contributors can use agents of their choice to make high-quality contributions to Quantiles open source systems.

## Documentation

Full documentation is available at [quantiles.io/documentation](https://quantiles.io/documentation/).

Start here:

- [Quickstart](https://quantiles.io/documentation/quickstart)
- [Agent Overview](https://quantiles.io/documentation/evals-with-agents)
- [Python SDK](https://quantiles.io/documentation/reference/python-sdk)

## Contributing

Quantiles exists to make AI evaluation workflows more practical, repeatable, and useful for engineering teams. We welcome contributions from the community, whether you are fixing bugs, improving documentation, adding evaluations and benchmarks, or helping make Quantiles Open Source more reliable for AI engineers and researchers.

Please read our [contributing guide](./CONTRIBUTING.md) to get started.

## Security

Please do not report security vulnerabilities through public GitHub issues. Follow the reporting guidance in [SECURITY.md](./SECURITY.md).

## License

Quantiles Open-source is licensed under the [Apache License 2.0](./LICENSE). Hosted, enterprise, or managed Quantiles products may be offered under separate commercial terms.
