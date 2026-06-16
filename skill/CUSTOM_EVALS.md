# Custom eval code samples

These are concrete code examples for writing custom Quantiles evals using the Python and TypeScript SDKs.

## Choose an SDK

Quantiles supports built-in and three ways to write evals:

| Approach | Best for | Entry point |
|---|---|---|
| **Python SDK (`sdk/`)** | Full-featured evals with dataset loading, LLM sampling, scoring, and export | `sdk/src/quantiles/examples/<eval>/__main__.py` |
| **Python SDK (`qt-sdk-python`)** | Lightweight, `qt`-native workflows with steps, emits, and local observability | `qt-sdk-python/examples/<eval>.py` |
| **TypeScript SDK** | Frontend-integrated or Node-based evals | `frontend/src/app/documentation/page.mdx` (reference) |

If the user is inside `sdk/`, use the **legacy Python SDK**. If they are inside `qt-sdk-python/`, use the **new Python SDK**. Only use TypeScript if the eval lives in the NextJS frontend or the user explicitly asks for Node/TS.

## Python

Python is a very popular technology for building and testing AI applications. Many users will already have a lot of Python code, and in these cases, the Quantiles Python SDK will be a good choice for them, if they want to build custom evals.

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

- Workflow names must be unique within the file. `entrypoint()` uses `QUANTILES_WORKFLOW_NAME` from the CLI.
- `step()` caches by `step_key`. Use deterministic keys (e.g., sample IDs) so reruns skip completed work.
- `emit()` writes metrics to the local SQLite/Parquet store. They show up in `qt show` and `qt compare`.
- `dataset()` must be called inside the handler, not at module level, because it needs the `WorkflowContext`.

## TypeScript

If the user does not have lots of Python code already, and they do have lots of Javascript / Typescript code, it might be appropriate for them to use the Quantiles TypeScript SDK.

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
