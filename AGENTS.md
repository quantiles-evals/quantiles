# Quantiles Agent Guide

This file is for coding assistants such as Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and similar tools. It gives agents a short, public-safe map of the Quantiles repository and the rules for making focused, reviewable changes.

## Scope

These instructions apply to the root `quantiles` repository.

If working inside a subdirectory with its own `AGENTS.md`, follow the nearest `AGENTS.md` first. Subdirectory-specific instructions override this root guide for implementation details, commands, package managers, tests, and code style.

Explicit user instructions override this file. Do not use this root file as the source of truth for implementation-specific behavior when a subdirectory has more specific guidance.

## What This Repo Is

Quantiles is the public open-source entry point for Quantiles: a local-first CLI and SDK toolchain for running AI evaluation workflows with fast, continuous feedback.

Quantiles records evaluation runs, steps, metrics, events, inputs, outputs, and comparisons locally so teams can understand what improved, what regressed, and why.

Use this mental model:

```text
evaluation workflow -> local run -> recorded steps and metrics -> inspect -> compare -> iterate
```

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

## Repository Role

This repository is the public open-source repository for Quantiles.

Root-level files provide the public orientation, contribution, security, licensing, and agent guidance for the project. Implementation-specific work belongs in the relevant subdirectory.

Subdirectories may include:

- `cli`: the Rust `qt` CLI, local server, and SQLite-backed run storage.
- `python`: the `quantiles` Python SDK.
- `typescript`: the `@quantiles/sdk` TypeScript SDK.
- `skill`: reusable agent skill instructions for running, inspecting, and comparing Quantiles evals.
- `benchmarks`: built-in or example benchmark harnesses, datasets, fixtures, and scoring logic, when present.
- `docs`: public documentation, examples, guides, and reference material, when present.

## Authoritative Files

Start with these files before making public-facing changes:

- [`README.md`](./README.md): public product overview, quickstart, CLI examples, SDK summary, docs links, and agent guidance.
- [`CONTRIBUTING.md`](./CONTRIBUTING.md): contributor expectations, development workflow, and review norms.
- [`SECURITY.md`](./SECURITY.md): supported components and vulnerability reporting process.
- [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md): community participation rules.
- [`LICENSE`](./LICENSE): Apache 2.0 license text.
- [`mise.toml`](./mise.toml): task definitions for building, formatting, type-checking, linting, and more, all using the [mise](https://mise.en.dev/) task runner.

## Agent Instruction Boundaries

Use `AGENTS.md` files for durable repository guidance: repo layout, safety constraints, coding conventions, validation commands, review expectations, and handoff requirements.

Use `skill/SKILL.md` as the reusable Quantiles evaluation workflow: when to run the `qt` CLI, how to inspect and compare runs, how to resume failed work, and how to report evaluation results.

This repository keeps the skill source in [`skill/SKILL.md`](./skill/SKILL.md). Coding agents do not necessarily auto-load that file from this path. For Codex-compatible skill discovery, install or copy the skill into an agent skill location such as `.agents/skills/quantiles/SKILL.md`, or invoke it through the agent's supported skill-install mechanism.

When both files are relevant:

1. Explicit user instructions win.
2. The nearest applicable `AGENTS.md` controls repository-specific code, commands, package managers, tests, style, and public-safety rules.
3. `skill/SKILL.md` controls reusable Quantiles eval operations, result inspection, comparison, resume, and reporting workflow.
4. If the skill suggests a generic command but a local `AGENTS.md`, README, or package configuration provides a more specific command, use the local command and note the choice in handoff.

## Product Terminology

Use these terms consistently in public docs:

- `Quantiles`: the open-source Quantiles project.
- `qt`: the Quantiles CLI.
- `quantiles`: the Python SDK package.
- `@quantiles/sdk`: the TypeScript SDK package.
- `evaluation`: user-authored evaluation or agent-loop code.
- `benchmark`: a repeatable evaluation harness with a defined dataset, scoring method, and result shape.
- `run`: one recorded execution of an evaluation or benchmark.
- `step`: a durable recorded unit of an evaluation execution.
- `metric`: a measured value emitted during a run.
- `event`: recorded observability data from an evaluation.
- `.quantiles/`: local Quantiles workspace state, including SQLite database and metrics Parquet files.

Prefer `local-first` and `offline by default` for open-source behavior.

When remote model calls, hosted judges, external tools, provider APIs, or network datasets are involved, state that those calls are user-configured exceptions to the local-first default.

## Quantiles Evaluation Workflow

Use the `qt` CLI as the source of truth for running, listing, inspecting, comparing, and resuming Quantiles evaluations.

Prefer CLI output over manually reading `.quantiles/` files. Do not manually edit or delete `.quantiles/` files unless explicitly asked.

Although `qt init` exists to initialize the local Quantiles database, `qt run` automatically does the same. `qt init` is thus often unnecessary to run explicitly.

Use `--json` for `qt run`, `qt list`, `qt show`, and `qt compare` when producing agent summaries. Inspect selected runs with `qt show <run_id> --json`.

Common commands:

```bash
qt run <evaluation> --json
qt list --json
qt show <run_id> --json
qt compare <baseline_run_id> <candidate_run_id> --json
qt run <evaluation> --resume <run_id> --json
```

Do not silently change evaluation semantics. Changes to prompts, datasets, scorers, rubrics, sampling parameters, judge configuration, model selection, tool configuration, or step inputs can invalidate comparisons. Call out any such changes in handoff.

Start with the smallest useful sample limit before running a full benchmark. Ask before running any evaluation that is expected to be slow, expensive, provider-backed, network-dependent, destructive, or likely to modify local run state in a meaningful way.

Safe commands may include read-only inspection commands, local format checks, type checks, and unit tests. Small smoke tests are allowed only when they are local, cheap, relevant to the task, and do not call external providers.

If no real model is specified, built-in evaluations may use the demo model. Treat demo model runs as workflow validation only, not model-quality benchmark evidence.

Do not run provider-backed evaluations unless explicitly asked or given a provider-prefixed model. Before running provider-backed evaluations, verify that the required provider API key is configured without printing the key value.

Provider-backed model inputs should use provider-prefixed model names, for example:

```json
{"model":"openai:<model>"}
{"model":"anthropic:<model>"}
{"model":"gemini:<model>"}
```

Example provider-backed run:

```bash
qt run simpleqa-verified --input '{"limit":10,"model":"openai:<model>"}' --json
```

## Root Validation

The root repository may not have a build or test suite that applies to every change.

Before running checks, inspect the repository for configured tooling. Do not invent root commands.

When discovering validation commands, check files such as:

- `mise.toml`
- `justfile`
- `Makefile`
- `package.json`
- `pyproject.toml`
- `Cargo.toml`
- `README.md`
- the nearest subdirectory `AGENTS.md`

For documentation-only changes, at minimum:

- Review edited Markdown for broken relative links.
- Check headings, lists, code fences, and command examples for Markdown validity.
- Verify public commands against the relevant subdirectory before documenting them.
- Check terminology against this file.
- Confirm that examples do not include secrets, private data, local database contents, or generated artifacts.

If subdirectory checks are needed, use the commands from that subdirectory's nearest `AGENTS.md`, README, or package configuration.

## Subdirectory Guidance

### CLI

The `qt` CLI is a local-first Rust CLI for running, recording, inspecting, and comparing AI evaluation workflows.

It creates local SQLite state under `.quantiles/quantiles.sqlite` and provides commands such as:

```bash
qt run
qt list
qt show
qt compare
```

When editing the CLI subdirectory, preserve local-first behavior, SQLite data model assumptions, clear Rust error handling, and stable command behavior.

### Python SDK

The Python SDK is a Python 3.12 SDK for authoring local AI workload workflows against the Quantiles local observability server at `http://127.0.0.1:8765` by default.

It exposes workflow primitives such as `workflow`, `entrypoint`, `step`, `emit`, dataset iteration, async helpers, metrics, and LLM helpers.

When editing the Python subdirectory, preserve async behavior, stable JSON payloads, replay semantics, and public API exports.

### TypeScript SDK

The TypeScript SDK is an SDK for authoring local AI workload workflows against the Quantiles local observability server at `http://127.0.0.1:8765` by default.

It exposes workflow primitives, `QuantilesClient`, `QuantilesRun`, stable JSON utilities, and shared SDK types.

When editing the TypeScript subdirectory, preserve strict typing, JSON-serializable public surfaces, ESM behavior, and documented package exports.

### Agent Skill

The [`skill`](https://github.com/quantiles-evals/skill) repository contains reusable instructions for coding agents that use Quantiles to run, inspect, compare, and summarize evaluation workflows.

When editing the skill subdirectory, keep instructions operational, command-driven, and safe for public use.

Read [github.com/quantiles-evals/skill](https://github.com/quantiles-evals/skill) for the reusable agent skill instructions.

## Working In This Repository

Keep root-level changes focused on public orientation, documentation, contribution guidance, security guidance, licensing, and agent instructions.

Update documentation in this repository (e.g. `README.md`, etc...) when any of the following change:

- CLI commands, flags, outputs, or setup steps.
- SDK APIs, imports, examples, or package names.
- Benchmark names, datasets, scoring methods, or limitations.
- Run schemas, step semantics, metrics, events, or comparison behavior.
- Agent workflows, skills, prompts, or recommended commands.
- Security, privacy, telemetry, or data-handling expectations.

Avoid adding implementation-specific claims to the root repository unless they are verified against the relevant subdirectory.

Use forward slashes in public docs unless a block is explicitly Windows or PowerShell-specific.

## Common Tasks

### Run or inspect an evaluation

Use the Quantiles CLI as the source of truth for local runs.

Prefer this flow:

```bash
qt run <evaluation>
qt show <run_id> --json
```

When summarizing results, include the command, run ID, workflow or benchmark name, model, input JSON, status, key metrics, failures, and recommended next command.

### Compare evaluation runs

Use `qt compare` to compare a baseline run against a candidate run:

```bash
qt compare <baseline_run_id> <candidate_run_id> --json
```

Before saying one run is better, verify that the comparison is apples-to-apples.

Keep benchmark or workflow name, dataset, dataset split, sample count, scorer, metric definitions, model-vs-demo setup, workflow input, and provider settings stable unless the user intentionally changed one variable.

Treat exit code `1` from `qt compare` as a signal that runs differ, not necessarily as a command failure.

For small sample counts, describe comparison results as directional or smoke-test evidence.

### Debug a regression

When debugging a regression:

1. Identify the baseline and candidate runs.
2. Run `qt compare <baseline_run_id> <candidate_run_id> --json`.
3. Inspect failing or changed samples with `qt show <run_id> --json`.
4. Look for changed inputs, outputs, metrics, step status, model configuration, prompt version, dataset row IDs, judge configuration, and sampling parameters.
5. Summarize the highest-impact fixes for reliability, cost, and latency.

### Resume a run

Resume only failed or interrupted runs caused by operational issues such as timeouts, rate limits, process exits, or network errors.

Do not resume a completed run. Start a new run instead.

When resuming a run, use the same workflow name and input JSON. For custom evaluations, also use the same command.

Start a new run instead of resuming when the model, prompt, dataset, rubric, workflow input, or scoring logic intentionally changed.

<!-- AARON: review these -->

```bash
qt run <run_id> --resume --json
```

For custom evaluations, preserve the original command as well:

```bash
qt run <run_id> --resume --json -- <command>
```

### Handle security-related content

Read [`SECURITY.md`](./SECURITY.md).

Do not include real secrets, PHI, customer data, private datasets, access tokens, or full `.quantiles/` databases in examples.

Public vulnerability reports should be redirected to the private reporting process described in [`SECURITY.md`](./SECURITY.md).

### Work on implementation

Open the relevant subdirectory and follow its nearest `AGENTS.md`.

Do not use this root file as the source of truth for implementation-specific commands, tests, package managers, release steps, or code style when a subdirectory has more specific guidance.

## Output Style For Coding Agents

- Be concise, technical, and action-oriented.
- Prefer runnable commands, real file paths, concrete inputs, and expected outputs.
- When reporting evaluation results, include run IDs, compared runs, key metrics, regressions, failures, caveats, and recommended next steps.
- If a command needs credentials, name the required environment variables but do not inspect or print their values.

## Evaluation Reporting

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

Before handing work back, summarize:

- What changed.
- Which files were reviewed.
- Which checks were run.
- Which checks were skipped and why.
- Any subdirectories, docs, commands, package names, benchmarks, or release details that still need verification.

For documentation-only changes, state that no build or test suite was run if none applies.

For implementation changes, run the relevant subdirectory checks when available and report the exact commands used.
