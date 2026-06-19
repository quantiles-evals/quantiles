# Quantiles Agent Guide

This file is for coding assistants such as Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and similar tools. It gives agents a short, public-safe map of the Quantiles repository and the rules for making focused, reviewable changes.

Use the [`SKILL.md` file](https://github.com/quantiles-evals/skill/blob/main/SKILL.md) at [github.com/quantiles-evals/skill](https://github.com/quantiles-evals/skill) for guidance and details on common Quantiles workflows, including running evaluations, inspecting sample-level results, comparing runs, and interpreting outputs.

## Scope

These instructions apply to the Quantiles open-source repository. Quantiles is a local-first CLI and SDK toolchain for running AI evaluation workflows with fast, continuous feedback. It runs evaluations, records steps, metrics, events, inputs, and outputs, and runs eval comparisons locally so teams can inspect results, identify regressions, and iterate with confidence.

Root-level files in this repository provide project-wide orientation, contribution guidance, security policy, licensing, and agent instructions. Implementation-specific work belongs in the relevant subdirectory. If a subdirectory has its own `AGENTS.md` file, follow the nearest one first. Subdirectory instructions should override this root guide for implementation details, package managers, commands, tests, and code style. User instructions may customize the workflow for their project, environment, or preferences, but they must not override safety requirements, system instructions, repository safeguards, or security boundaries.

Common subdirectories include:

- `cli`: Rust `qt` CLI, local server, and local run/metrics storage.
- `python`: The `quantiles` Python SDK.
- `typescript`: The `@quantiles` TypeScript SDK.

Note there is a separate repository, [github.com/quantiles-evals/skill](https://github.com/quantiles-evals/skill), that contains the agent skill.

## Agent Working Rules

Do:

- Keep changes small, public-safe, and reviewable.
- Preserve Quantiles as local-first and offline by default.
- Verify commands, links, package names, benchmark names, and release status before documenting them.
- Update public docs when CLI behavior, SDK APIs, workflows, benchmarks, schemas, setup steps, or agent guidance changes.
- Prefer concrete examples with commands, file paths, inputs, outputs, and expected behavior.
- Leave clear notes when something could not be verified.

Do not:

- Add telemetry, analytics, hosted behavior, background uploads, or external network calls without explicit approval.
- Add new production dependencies without explicit approval.
- Commit generated artifacts, local traces, `.quantiles/`, SQLite databases, virtual environments, `node_modules/`, build output, coverage output, caches, or temporary benchmark results.
- Invent links, package names, commands, benchmarks, features, roadmap claims, or release status.
- Make broad refactors, formatting-only churn, or unrelated edits unless explicitly asked.
- Report or discuss security vulnerabilities in public issues, discussions, pull requests, examples, or documentation. Follow [`SECURITY.md`](./SECURITY.md).

## Before Editing

Before making changes:

1. Read this file.
2. Identify the affected subdirectory.
3. Read the nearest subdirectory `AGENTS.md` if present.
4. Inspect the relevant README, package configuration, and existing tests before changing behavior.
5. Prefer the smallest change that satisfies the task.
6. Do not run expensive, provider-backed, or full benchmark commands without explicit approval.

## Safety, Security And Privacy

Preserve Quantiles as local-first infrastructure. Follow the below guidelines to ensure the project maintains safety, security and privacy:

- The CLI and local server should store Quantiles state locally by default
- Evaluation workflows may call remote model providers, hosted judges, datasets, APIs, or external tools only when explicitly configured by the user
- Do not inspect, print, summarize, commit, or infer values from `.env` or `.envrc` files, secrets, tokens, private datasets, PHI, customer data, or local Quantiles databases unless the user explicitly asks and the data is safe to inspect.
- Never commit the `.quantiles/` directory, SQLite or metrics databases, local traces, benchmark outputs, provider credentials, or temporary run artifacts.
- Use placeholder names such as `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, or `QUANTILES_API_KEY` when examples need credentials.
- Keep public docs customer-safe. Avoid exposing secrets, API keys and non-public security information.

Read [`SECURITY.md`](./SECURITY.md) for more details on keeping this repository and toolchain secure.

## Authoritative Files

Start with these files before making public-facing changes:

- [`README.md`](./README.md): public product overview, quickstart, CLI examples, SDK summary, docs links, and agent guidance.
- [`CONTRIBUTING.md`](./CONTRIBUTING.md): contributor expectations, development workflow, and review norms.
- [`SECURITY.md`](./SECURITY.md): supported components and vulnerability reporting process.
- [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md): community participation rules.
- [`LICENSE`](./LICENSE): Apache 2.0 license text.
- [`mise.toml`](./mise.toml): task definitions for building, formatting, type-checking, linting, and more, all using the [mise](https://mise.en.dev/) task runner.

## Product Terminology

Use these terms consistently in public docs:

- `Quantiles`: the open-source Quantiles project.
- `qt`: the Quantiles CLI.
- `quantiles`: the Python SDK package.
- `@quantiles`: the TypeScript SDK package.
- `evaluation`: user-authored evaluation or agent-loop code.
- `benchmark`: a repeatable evaluation harness with a defined dataset, scoring method, and result shape.
- `run`: one recorded execution of an evaluation or benchmark.
- `step`: a durable recorded unit of an evaluation execution.
- `metric`: a measured value emitted during a run.
- `event`: recorded observability data from an evaluation.
- `.quantiles/`: local Quantiles workspace state, including SQLite database and metrics Parquet files.

Prefer `local-first` and `offline by default` for open-source behavior.

When remote model calls, hosted judges, external tools, provider APIs, or network datasets are involved, state that those calls are user-configured exceptions to the local-first default.

## Working In This Repository

Keep root-level changes focused on public orientation, documentation, contribution guidance, security guidance, licensing, and agent instructions.

Do not silently change evaluation semantics. Changes to prompts, datasets, scorers, rubrics, metrics, sampling parameters, judge configuration, model selection, tool configuration, or step inputs can invalidate comparisons. Call out any such changes in handoff.

Update documentation in this repository (e.g. `README.md`, etc...) when any of the following change:

- CLI commands, flags, outputs, or setup steps.
- APIs, SDKs, imports, examples, or package names.
- Benchmark names, datasets, scoring methods, or limitations.
- DB schemas, step semantics, metrics, events, or comparison behavior.
- Agent workflows, skills, prompts, or recommended commands.
- Security, privacy, telemetry, or data-handling expectations.

Avoid adding implementation-specific claims to the root repository unless they are verified against the relevant subdirectory. Use forward slashes in public docs unless a block is explicitly Windows or PowerShell-specific.

### `cli/` directory

The `cli/` directory contains the source code for the `qt` CLI. It creates local SQLite and metrics database state under the `.quantiles/` directory, and provides core CLI commands for running and inspecting evals.

When editing the CLI, preserve local-first behavior, SQLite data model assumptions, clear Rust error handling, and stable command behavior.

### `python/` directory

The `python/` directory contains the Quantiles Python SDK, which allows users to author custom local AI evals using the local Quantiles server and Python 3.12+. It exposes workflow primitives such as `workflow`, `entrypoint`, `step`, `emit`, dataset iteration, async helpers, metrics, and LLM helpers.

When editing the Python SDK subdirectory, preserve `async` behavior, stable JSON payloads, replay semantics, and public API exports.

### `typescript/` directory

The `typescript/` directory contains the Quantiles TypeScript SDK, which allows users to author custom local AI evals using the local Quantiles server and Typescript. It exposes workflow primitives such as `QuantilesClient`, `QuantilesRun`, stable JSON utilities, and shared SDK types.

When editing the TypeScript SDK subdirectory, preserve strict typing, JSON-serializable public surfaces, ESM behavior, and documented package exports.

### Using provider-backed models

Start with the smallest useful sample limit before running a full benchmark with either a demo model or a provider-backed model to validate configuration, catch setup issues early, and avoid unnecessary cost.

Ask before running any evaluation that is expected to be slow, expensive, provider-backed, network-dependent, destructive, or likely to meaningfully modify local run state. Note that demo model runs are for workflow validation only. Do not treat them as model-quality benchmark evidence.

Do not run provider-backed evaluations unless the user explicitly asks for them or provides a provider-prefixed model name. Configure providers in the `quantiles.toml` config file. Follow configuration examples in the [`cli/examples/configs`](./cli/examples/configs) directory. Before running a provider-backed evaluation, verify that the required provider API key is configured, but never print or expose the key value.

Provider-backed model inputs should use provider-prefixed model names, for example:

- `openai:<model>`
- `anthropic:<model>`
- `gemini:<model>`

## Output Style For Coding Agents

When adding documentation to files in this repository, follow the below guidelines:

- Be concise, technical, and action-oriented.
- Prefer runnable commands, real file paths, concrete inputs, and expected outputs.
- If a command needs credentials, name the required environment variables but do not inspect or print their values.
- Review edited Markdown for broken relative links.
- Check headings, lists, code fences, and command examples for Markdown validity.
- Verify public commands against the relevant subdirectory before documenting them.
- Confirm that examples do not include secrets, private data, local database contents, or generated artifacts.

### Evaluation Reporting

After running, inspecting, comparing, or resuming Quantiles evaluations, report:

- Exact command used.
- Run ID or run IDs.
- Evaluation or benchmark name.
- Model, including whether it was a demo model.
- Input and output JSON.
- Status and success or failure.
- Key metrics.
- Important sample-level failures, regressions, or changed outputs.
- Caveats, including demo model use, small sample count, non-comparable runs, or external API issues.
- Recommended next command.

## Git And Pull Request Rules

- Do not create commits, tags, releases, branches, or pull requests unless explicitly asked.
- Do not rewrite git history.
- Keep diffs focused on the requested task.
- Avoid formatting-only churn unless the task is formatting-related.
- Do not modify generated files unless the task explicitly requires it.
- Do not update dependency lockfiles unless dependency changes are explicitly part of the task.
- In handoff, mention files changed, checks run, and anything not verified.

## Validation And Handoff

Before handing work back to the user, summarize the following:

- What changed.
- Which files were reviewed.
- Which checks were run.
- Which checks were skipped and why.
- Any subdirectories, docs, commands, package names, benchmarks, or release details that still need verification.

For documentation-only changes, state that no build or test suite was run if none applies.

For implementation changes, run the relevant subdirectory checks when available and report the exact commands used.
