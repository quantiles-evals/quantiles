# Quantiles OS Agent Guide

This file is for coding assistants such as Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and similar tools. It gives agents a short, public-safe map of the Quantiles OS repository and the rules for making focused, reviewable changes.

## Scope

These instructions apply to the root `quantiles-os` repository.

If working inside a subdirectory with its own `AGENTS.md`, follow the nearest `AGENTS.md` first. Subdirectory-specific instructions override this root guide for implementation details, commands, package managers, tests, and code style.

Explicit user instructions override this file. Do not use this root file as the source of truth for implementation-specific behavior when a subdirectory has more specific guidance.

## What This Repo Is

Quantiles OS is the public open-source entry point for Quantiles: a local-first CLI and SDK toolchain for running AI evaluation workflows with fast, continuous feedback.

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
- Use the nearest subdirectory `AGENTS.md` before making implementation changes.
- Leave clear notes when something could not be verified.

Do not:

- Add telemetry, analytics, hosted behavior, background uploads, or external network calls without explicit approval.
- Add new production dependencies without explicit approval.
- Commit generated artifacts, local traces, `.quantiles/`, SQLite databases, virtual environments, `node_modules/`, build output, coverage output, caches, or temporary benchmark results.
- Invent links, package names, commands, benchmarks, features, roadmap claims, or release status.
- Make broad refactors, formatting-only churn, or unrelated edits unless explicitly asked.
- Report or discuss security vulnerabilities in public issues, discussions, pull requests, examples, or documentation. Follow [`SECURITY.md`](./SECURITY.md).

## Safety And Privacy

Preserve Quantiles as local-first infrastructure. The CLI and local server should store Quantiles state locally by default.

User-authored workflows may call remote model providers, hosted judges, datasets, APIs, or external tools only when explicitly configured by the user.

Do not inspect, print, summarize, commit, or infer values from `.env`, secrets, tokens, private datasets, PHI, customer data, or local Quantiles databases unless the user explicitly asks and the data is safe to inspect.

Never commit `.quantiles/`, SQLite databases, local traces, benchmark outputs, provider credentials, or temporary run artifacts.

Use placeholder names such as `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, or `QUANTILES_API_KEY` when examples need credentials.

Keep public docs customer-safe. Avoid internal-only planning, prioritization, unreleased roadmap claims, private operational details, or non-public security information.

## Repository Role

This repository is the public open-source repository for Quantiles OS.

Root-level files provide the public orientation, contribution, security, licensing, and agent guidance for the project. Implementation-specific work belongs in the relevant subdirectory.

Subdirectories may include:

- `cli`: the Rust `qt` CLI, local server, and SQLite-backed run storage.
- `python`: the `quantiles` Python SDK.
- `typescript`: the `@quantiles/sdk` TypeScript SDK.
- `skill`: reusable agent skill instructions for running, inspecting, and comparing Quantiles evals.
- `benchmarks`: built-in or example benchmark harnesses, datasets, fixtures, and scoring logic, when present.
- `docs`: public documentation, examples, guides, and reference material, when present.

When working in a subdirectory, read its README, nearest `AGENTS.md`, and validation commands before editing.

## Authoritative Files

Start with these files before making public-facing changes:

- [`README.md`](./README.md): public product overview, quickstart, CLI examples, SDK summary, docs links, and agent guidance.
- [`CONTRIBUTING.md`](./CONTRIBUTING.md): contributor expectations, development workflow, and review norms.
- [`SECURITY.md`](./SECURITY.md): supported components and vulnerability reporting process.
- [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md): community participation rules.
- [`LICENSE`](./LICENSE): Apache 2.0 license text.
- [`CHANGELOG.md`](./CHANGELOG.md): notable public changes, when present.

For implementation details, read the relevant subdirectory and its nearest `AGENTS.md` first.

## Product Terminology

Use these terms consistently in public docs:

- `Quantiles OS`: the open-source Quantiles project.
- `qt`: the Quantiles CLI.
- `quantiles`: the Python SDK package.
- `@quantiles/sdk`: the TypeScript SDK package.
- `evaluation`: user-authored evaluation or agent-loop code.
- `benchmark`: a repeatable evaluation harness with a defined dataset, scoring method, and result shape.
- `run`: one recorded execution of an evaluation or benchmark.
- `step`: a durable recorded unit of an evaluation execution.
- `metric`: a measured value emitted during a run.
- `event`: recorded observability data from an evaluation.
- `.quantiles/`: local Quantiles workspace state.

Prefer `local-first` and `offline by default` for open-source behavior.

When remote model calls, hosted judges, external tools, provider APIs, or network datasets are involved, state that those calls are user-configured exceptions to the local-first default.

## Root Validation

The root repository may not have a build or test suite that applies to every change.

Before running checks, inspect the repository for configured tooling. Do not invent root commands.

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
qt init
qt run
qt list
qt show
qt compare
```

When editing the CLI subdirectory, preserve local-first behavior, SQLite data model assumptions, clear Rust error handling, and stable command behavior.

Use the CLI subdirectory's own `AGENTS.md` and validation targets as the source of truth.

### Python SDK

The Python SDK is a Python 3.12 SDK for authoring local AI workload workflows against the Quantiles local observability server at `http://127.0.0.1:8765` by default.

It exposes workflow primitives such as `workflow`, `entrypoint`, `step`, `emit`, dataset iteration, async helpers, metrics, and LLM helpers.

When editing the Python subdirectory, preserve async behavior, stable JSON payloads, replay semantics, and public API exports.

Use the Python subdirectory's own `AGENTS.md` and validation targets as the source of truth.

### TypeScript SDK

The TypeScript SDK is an ESM SDK for authoring local AI workload workflows against the Quantiles local observability server at `http://127.0.0.1:8765` by default.

It exposes workflow primitives, `QuantilesClient`, `QuantilesRun`, stable JSON utilities, and shared SDK types.

When editing the TypeScript subdirectory, preserve strict typing, JSON-serializable public surfaces, ESM behavior, and documented package exports.

Use the TypeScript subdirectory's own `AGENTS.md` and validation targets as the source of truth.

### Agent Skill

The [`skill`](./skill/) subdirectory contains reusable instructions for coding agents that use Quantiles to run, inspect, compare, and summarize evaluation workflows.

When editing the skill subdirectory, keep instructions operational, command-driven, and safe for public use. Do not treat runs that use the demo model as model-quality benchmark evidence.

Read [`skill/SKILL.md`](./skill/SKILL.md) for the reusable agent skill instructions.

### Benchmarks (should we have this?)

The `benchmarks` subdirectory may contain built-in or example benchmark harnesses, datasets, fixtures, scoring logic, and benchmark documentation.

When editing benchmark content:

- Preserve dataset provenance, scoring behavior, and benchmark limitations.
- Record benchmark source, version, commit, dataset revision, scoring configuration, and any local patches when relevant.
- Do not present demo sampler results as model-quality benchmark evidence.
- Avoid adding large datasets, generated outputs, or cached results unless explicitly required and appropriate for the repository.

## Working In This Repository

Keep root-level changes focused on public orientation, documentation, contribution guidance, security guidance, licensing, and agent instructions.

Update docs when any of the following change:

- CLI commands, flags, outputs, or setup steps.
- SDK APIs, imports, examples, or package names.
- Benchmark names, datasets, scoring methods, or limitations.
- Run schemas, step semantics, metrics, events, or comparison behavior.
- Agent workflows, skills, prompts, or recommended commands.
- Security, privacy, telemetry, or data-handling expectations.

Avoid adding implementation-specific claims to the root repository unless they are verified against the relevant subdirectory.

Use forward slashes in public docs unless a block is explicitly Windows or PowerShell-specific.

## Common Tasks

### Handle security-related content

Read [`SECURITY.md`](./SECURITY.md).

Do not include real secrets, PHI, customer data, private datasets, access tokens, or full `.quantiles/` databases in examples.

Public vulnerability reports should be redirected to the private reporting process described in [`SECURITY.md`](./SECURITY.md).

### Work on implementation

Open the relevant subdirectory and follow its nearest `AGENTS.md`.

Do not use this root file as the source of truth for implementation-specific commands, tests, package managers, release steps, or code style when a subdirectory has more specific guidance.

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
