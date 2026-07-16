# AGENTS.md

## Scope

These instructions apply to the Quantiles Python SDK. For changes outside the SDK, follow the nearest applicable `AGENTS.md` file.

## Project Overview

`quantiles`, the Quantiles Python SDK, is a Python 3.12 SDK for authoring and running local AI evaluation, benchmarks, and agent-loop workflows against the Quantiles server. It exposes workflow primitives such as `workflow`, `entrypoint`, `step`, and `emit`, along with typed dataset iteration, async concurrency helpers, statistical metrics, and an LLM helper. The SDK connects to `http://127.0.0.1:8765` by default and records runs, step outputs, metrics, and dataset batches through the CLI/server API.

## Working in This Repository

- Prefer focused changes that fit the current SDK layout under `src/quantiles`.
- Use idiomatic async Python. Avoid blocking calls in workflow, dataset, client, and concurrency code unless they are deliberately isolated.
- Keep JSON values compatible with the recursive `JsonValue` type and use Pydantic models where the existing code expects typed validation.
- Follow the repository style: Python 3.12 syntax, 2-space indentation, double quotes, type annotations, and concrete types instead of `typing.Any`.
- Avoid broad refactors while implementing narrow behavior changes.

## Non-Negotiable Invariants

- Preserve Quantiles as local-first SDK infrastructure. Do not add new implicit telemetry, hosted services, background uploads, or external network calls unless the task explicitly requires them.
- Default tests should be deterministic and offline. Mock OpenAI, model providers, and external HTTP services unless a test is explicitly marked as integration or e2e.
- Preserve run and step replay semantics. Changes to step keys, input hashing, JSON normalization, dataset batch identity, or cache reuse must include focused regression tests.
- Keep public APIs intentional and stable. User-facing symbols exported from `src/quantiles/__init__.py` should have tests and, when applicable, documentation updates.
- Do not commit local Quantiles state or generated artifacts such as `.quantiles/`, SQLite databases, coverage output, caches, or temporary benchmark results.

## Dependency and API Changes

- Avoid adding runtime dependencies unless they are clearly necessary for the SDK surface being changed.
- Prefer existing internal helpers, client abstractions, and Pydantic models before introducing new patterns.
- If a change affects the public SDK API, update exports, tests, examples, and relevant documentation together.
- Avoid `typing.Any` in public APIs unless no narrower accurate type exists. For JSON-like payloads, prefer the existing `JsonValue` type or a narrower typed model.

## Validation and Testing

Use the `mise.toml` targets to do most validation, formatting, linting, type-checking, and testing work:

```bash
mise run test
mise run lint
mise run fmt-check
mise run typecheck
```

To apply formatting and automatic Ruff fixes, use:

```bash
mise run fmt
```

The equivalent direct commands are:

```bash
uv run python -m pytest
uv run ruff check .
uv run ruff format . --check
uv run ty check
```

Run the most relevant checks for the files changed. For behavior that affects workflow execution, step reuse, HTTP client payloads, dataset loading, or emitted metrics, add or update focused tests under `tests/`. Use `mise run e2e` only when the Quantiles CLI/server dependency is available and the change needs end-to-end coverage.

## Agent Handoff

Before handing work back, summarize:

- What changed.
- Which tests or checks were run.
- Any checks that were skipped and why.
- Any behavior, compatibility, or migration risks.
