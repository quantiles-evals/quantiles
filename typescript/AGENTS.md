# AGENTS.md

>**Note**: This SDK is currently unreleased and unsupported.

## Scope

These instructions apply to the Quantiles TypeScript SDK. For changes outside the SDK, follow the nearest applicable `AGENTS.md` file.

## Project Overview

`@quantiles/sdk`, the Quantiles TypeScript SDK, is an ESM TypeScript SDK for authoring and running local AI workload workflows against the Quantiles local observability server. It exposes workflow primitives such as `workflow`, `entrypoint`, `step`, `emit`, a low-level `QuantilesClient`, `QuantilesRun`, stable JSON utilities, and shared SDK types. The SDK talks to the local Quantiles server by default at `http://127.0.0.1:8765`, records runs, step outputs, and metrics through the CLI/server API, and is meant to make TypeScript eval and agent-loop workloads observable from the start.

## Working in This Repository

- Prefer focused changes that fit the current SDK layout under `src`.
- Use idiomatic TypeScript with strict compiler settings. Keep types concrete and prefer `unknown` plus narrowing when handling untyped values.
- Keep JSON values compatible with the recursive `JsonValue` type and preserve JSON-serializable public surfaces.
- Follow the repository style: ESM imports with explicit `.js` extensions, type-only imports/exports where appropriate, and Biome formatting.
- Avoid blocking or process-global behavior in workflow and client code unless the task explicitly requires it.
- Treat Bun as the repository tool runner unless the package explicitly supports Bun-only runtime behavior. Do not introduce `Bun.*` APIs or runtime-specific globals into SDK source unless the task explicitly requires it.
- Avoid broad refactors while implementing narrow behavior changes.

## Non-Negotiable Invariants

- Preserve Quantiles as local-first SDK infrastructure. Do not add new implicit telemetry, hosted services, background uploads, or external network calls unless the task explicitly requires them.
- Default checks and tests should be deterministic and offline. Mock model providers and external HTTP services unless a test is explicitly marked as integration or e2e.
- Preserve run and step replay semantics. Changes to step keys, input hashing, JSON normalization, or cache reuse must include focused regression coverage.
- Keep public APIs intentional and stable.
- Do not commit local Quantiles state or generated artifacts such as `.quantiles/`, SQLite databases, `dist/`, `node_modules/`, coverage output, caches, or temporary benchmark results. Do not add, remove, or regenerate lockfiles unless a dependency change requires it.

## Dependency and API Changes

- Avoid adding runtime dependencies unless they are clearly necessary for the SDK surface being changed.
- Prefer existing internal helpers, client abstractions, and TypeScript types before introducing new patterns.
- If a change affects the public SDK API, package exports, or package metadata, update `src/index.ts`, `package.json`, examples, and relevant documentation together.
- Avoid `any` in public APIs unless no narrower accurate type exists. For JSON-like payloads, prefer the existing `JsonValue` type or a narrower typed model.

## Validation and Testing

Use the `mise.toml` targets to do most validation, formatting, linting, and type-checking work:

```bash
mise run check
mise run typecheck
mise run lint
```

To apply formatting, use:

```bash
mise run fmt
```

The equivalent direct commands are:

```bash
bun run check
bunx tsc --noEmit
bunx biome ci
bun run fmt
```

Run the most relevant checks for the files changed. For behavior that affects workflow execution, step reuse, HTTP client payloads, stable JSON hashing, or emitted metrics, add or update focused coverage when a test harness is available. Use `mise run e2e` only when the Quantiles CLI/server dependency is available and the change needs end-to-end coverage.

## Agent Handoff

Before handing work back, summarize:

- What changed.
- Which tests or checks were run.
- Any checks that were skipped and why.
- Any behavior, compatibility, or migration risks.
