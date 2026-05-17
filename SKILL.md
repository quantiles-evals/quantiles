---
name: benchmark-runner
description: Use this skill when writing, running, or analyzing benchmarks and evals using the Quantiles Python SDK, TypeScript SDK, or qt CLI.
---

# Quantiles Benchmark Runner

## When to use

Use this skill when the user asks to:

- Write a new benchmark or eval for the Quantiles platform
- Run an existing benchmark locally or against the Quantiles backend
- Analyze benchmark results, compare runs, or export metrics
- Convert an ad-hoc Python eval script into a durable Quantiles workflow
- Debug why a benchmark failed, produced unexpected metrics, or didn’t ingest into the dashboard

## Writing benchmarks

### Choose an SDK

Quantiles supports three ways to write benchmarks:

| Approach | Best for | Entry point |
|---|---|---|
| **Python SDK (`sdk/`)** | Full-featured evals with dataset loading, LLM sampling, scoring, and export | `sdk/src/quantiles/examples/<benchmark>/__main__.py` |
| **Python SDK (`qt-sdk-python`)** | Lightweight, `qt`-native workflows with steps, emits, and local observability | `qt-sdk-python/examples/<benchmark>.py` |
| **TypeScript SDK** | Frontend-integrated or Node-based evals | `frontend/src/app/documentation/page.mdx` (reference) |

If the user is inside `sdk/`, use the **legacy Python SDK**. If they are inside `qt-sdk-python/`, use the **new Python SDK**. Only use TypeScript if the eval lives in the NextJS frontend or the user explicitly asks for Node/TS.

### Legacy Python SDK pattern (`sdk/`)

A minimal benchmark has three parts:

1. **Dataset** — use `HuggingFaceDataset` or any `Dataset[T]` implementation.
2. **Eval loop** — an async generator that yields `EvalResult` objects.
3. **Export** — wrap the loop in `save_eval(...)` to write Parquet/JSONL and handle progress bars.

Example skeleton:

```python
import asyncio
from collections.abc import AsyncIterator
from quantiles.data import HuggingFaceDataset
from quantiles.evals import EvalResult
from quantiles.evals.apis import LLMProvider
from quantiles.export.save import save_eval

async def _run_my_eval(provider: LLMProvider) -> AsyncIterator[EvalResult]:
    dataset = HuggingFaceDataset("quantiles/PubMedQA", config_name="pqa_labeled", split_name="train")
    sampler = provider.get_sampler()

    async for row in await dataset.get_iter(lambda x: x):
        sampled = await sampler.sample_freeform(...)
        yield EvalResult(
            benchmark_type="my-benchmark",
            name=row["id"],
            conversation=[...],
            primary_metric="accuracy",
            metrics={"accuracy": 1.0 if correct else 0.0},
            metadata={"model_response": sampled.content},
        )

async def main() -> None:
    provider = LLMProvider.openai_from_env(model_name="gpt-4o-mini")
    await save_eval(
        eval_name="my-benchmark",
        eval_func=lambda: _run_my_eval(provider),
        expected_total=100,
        export_strategy="my-benchmark.jsonl",  # or None for Parquet
    )

if __name__ == "__main__":
    asyncio.run(main())
```

Rules:

- Always load datasets through `Dataset[T]` (see `.codex/sdk.md`). Do not hand-roll HuggingFace loaders.
- Parse rows with Pydantic `BaseModel` subclasses. Do not use `typing.Any` or bare `dict` traversal.
- Use `LLMProvider.openai_from_env()` for OpenAI. For other providers, check `quantiles.evals.apis` first.
- Set `benchmark_type` to a stable, kebab-case string. It is used for filtering in the dashboard and derived metrics.
- Put reusable benchmark logic in `sdk/src/quantiles/evals/<benchmark>/`. Put examples in `sdk/src/quantiles/examples/<benchmark>/`.

### New Python SDK pattern (`qt-sdk-python`)

Use this when the user wants `qt` observability (steps, emits, caching, `qt compare`) out of the box.

```python
from quantiles import workflow, step, emit, entrypoint, dataset, call_llm
from quantiles.types import JsonValue
from quantiles.workflow_context import WorkflowContext

async def handler(input_data: dict[str, JsonValue], ctx: WorkflowContext) -> JsonValue:
    model_name = input_data.get("model_name", "openai:gpt_5_nano")
    num_examples = int(input_data.get("num_examples", 25))

    ds = await dataset(ctx, source="huggingface://quantiles/PubMedQA", ...)

    async def _eval_row(row):
        result = await step(ctx, step_key=f"eval-{row.sample_id}", ...)
        return result

    results = ...
    await emit(ctx, "accuracy", accuracy)
    return {"accuracy": accuracy, "total_count": len(results)}

my_eval = workflow("my-eval", handler)
entrypoint(my_eval)
```

Run it with:

```bash
qt run my-eval --input '{"model_name":"openai:gpt-4o-mini","num_examples":100}' -- uv run python my_eval.py
```

Rules:

- Workflow names must be unique within the file. `entrypoint()` uses `DURA_WORKFLOW_NAME` from the CLI.
- `step()` caches by `step_key`. Use deterministic keys (e.g., sample IDs) so reruns skip completed work.
- `emit()` writes metrics to the local SQLite/Parquet store. They show up in `qt show` and `qt compare`.
- `dataset()` must be called inside the handler, not at module level, because it needs the `WorkflowContext`.

### TypeScript SDK pattern

Use this only for frontend or Node-based evals:

```typescript
import { workflow, step, emit, entrypoint } from "@quantiles/sdk";

const runEval = workflow("eval", async (input, ctx) => {
  const result = await step("call-model", input, async () => {
    return { response: "..." };
  });
  emit("latency_ms", 120, "ms");
  return result;
});

entrypoint(runEval);
```

Run it with:

```bash
qt run eval -- bun run ./my_eval.ts
```

## Running benchmarks

### Local run with `qt` CLI

The preferred way to run a benchmark is through the `qt` CLI. It starts a local server, injects run metadata into the subprocess, and tears down when done.

```bash
# Python (qt-sdk-python)
qt run my-workflow --input '{"key":"value"}' -- uv run python my_eval.py

# TypeScript
qt run my-workflow --input '{"key":"value"}' -- bun run my_eval.ts

# Arbitrary shell command
qt run my-workflow -- echo "hello"
```

Key env vars injected by `qt run`:

- `DURA_BASE_URL` — local server URL (default `http://127.0.0.1:8765`)
- `DURA_RUN_ID` — existing run ID (if resuming)
- `DURA_WORKFLOW_NAME` — the workflow name passed to `qt run`
- `DURA_INPUT` — JSON input from `--input`

### Legacy SDK direct run

If the benchmark uses the legacy `sdk/` package and does not need `qt` step tracking, run it directly:

```bash
cd sdk
export OPENAI_API_KEY=...
export OPENAI_MODEL=gpt-4o-mini
export NUM_EXAMPLES=25
export EXPORT_PATH=my-benchmark.jsonl   # optional
uv run python -m quantiles.examples.my_benchmark
```

### Ingesting into the web dashboard

To push results to the Quantiles web app (NextJS backend):

1. Create a benchmark via the API or UI to get a `benchmarkUID`.
2. Use an exporter that POSTs to `/api/eval/batch`. The SDK does not yet ship a built-in HTTP exporter, so implement a small `BenchmarkExporter` subclass or write JSONL and POST manually.
3. Each `EvalResult` must include `benchmark_type` and a stable `name` (sample ID).

The frontend ingests protobuf `BenchmarkResult` messages. See `frontend/src/app/api/eval/ingest.ts` for field validation rules.

## Analyzing results

### Local file analysis

By default, `save_eval()` writes a timestamped Parquet file:

```
{timestamp}_{eval_name}_{uuid}.parquet
```

If `export_strategy` is a path ending in `.jsonl`, it writes newline-delimited JSON instead.

Read Parquet results in Python:

```python
import pandas as pd
df = pd.read_parquet("2024-..._my-benchmark_....parquet")
print(df["metrics"].mean())
```

Read JSONL results:

```python
import pandas as pd
df = pd.read_json("my-benchmark.jsonl", lines=True)
```

### `qt` CLI analysis

When using `qt-sdk-python`, use the CLI to inspect runs:

```bash
qt list                          # show all runs
qt show <run-id>                 # detailed view of one run
qt compare <run-id-1> <run-id-2> # side-by-side diff of steps, inputs, outputs, and metrics
```

These commands read from the local SQLite/Parquet store (`.dura/dura.sqlite` and adjacent Parquet files by default).

### Dashboard analysis

If results were ingested into the web app:

- Navigate to the benchmark page in the dashboard.
- The primary metric is surfaced automatically from `EvalResult.primary_metric`.
- Derived metrics (e.g., `macro_f1` for PubMedQA) are computed in `frontend/src/lib/analytics/derived_metrics.ts`.
- Sample-level metadata is queryable through BigQuery or the local DuckDB layer, depending on the `AnalyticsStorage` backend.

## Common gotchas

- **Do not use `typing.Any` in `sdk/`**. Parse everything through Pydantic models (see `.codex/sdk.md`).
- **Dataset loading must go through `Dataset[T]`**. Do not import `datasets.load_dataset` directly in new benchmark code inside `sdk/`.
- **Step keys must be unique and deterministic**. Collisions or nondeterministic keys break caching and restart behavior in `qt-sdk-python`.
- **`benchmark_type` consistency**. All results in a run should share the same `benchmark_type`. It drives dashboard filtering and derived metric selection.
- **Environment variables**. The legacy SDK reads `OPENAI_API_KEY`, `NUM_EXAMPLES`, `EXPORT_PATH`, etc. `qt-sdk-python` reads `DURA_BASE_URL`, `DURA_WORKFLOW_NAME`, `DURA_INPUT`, etc.
- **Static checks**. After changing `sdk/`, run `make check-sdk`. After changing `qt-sdk-python/`, run `mise run all` (or `fmt`, `lint`, `typecheck`, `test`).

## Useful paths

- Legacy Python SDK examples: `sdk/src/quantiles/examples/`
- Legacy Python SDK eval core: `sdk/src/quantiles/evals/`
- Legacy Python SDK export: `sdk/src/quantiles/export/save.py`
- New Python SDK examples: `qt-sdk-python/examples/`
- New Python SDK workflow API: `qt-sdk-python/src/quantiles/workflow.py`
- TypeScript SDK ref: `frontend/src/app/documentation/page.mdx`
- Frontend eval ingest: `frontend/src/app/api/eval/ingest.ts`
- Frontend batch route: `frontend/src/app/api/eval/batch/route.ts`
- Derived metrics: `frontend/src/lib/analytics/derived_metrics.ts`
- SDK conventions: `.codex/sdk.md`
