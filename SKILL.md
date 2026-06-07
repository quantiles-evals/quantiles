---
name: eval-runner
description: Use this skill when writing, running, or analyzing evals and evals using the Quantiles Python SDK, TypeScript SDK, or qt CLI.
---

# Quantiles eval runner

The `qt` CLI is the main entrypoint into all Quantiles evals. If you are asked to run any evals, or analyze any Quantiles evals, use the `qt` CLI.

## When to use this skill

Use this skill when the user asks to:

- Run a Quantiles eval
- Analyze one or more Quantiles evals that already ran
- Write a new custom eval for the Quantiles platform
- Convert an ad-hoc Python eval script into a durable Quantiles workflow

## Install

To install the CLI, run the following command:

```bash
curl -fsSL https://cli.quantiles.io/install.sh | bash
```

## Running built-in evals

The `qt` CLI has several built-in evals:

- `pubmedqa`: the PubMedQA eval to evaluate a model on standard healthcare knowledge
- `simpleqa-verified`: an update SimpleQA dataset for testing models for general knowledge
- `financebench`: a small finance-specific eval

To run any of these evals, run `qt run <eval name>`. For example, to run the `pubmedqa` eval, run the following:

```bash
qt run pubmedqa --json
```

>Warning: all commands to run built-in evals will run against a fake model that generates random data. By default, these commands should be used for initial testing, but their results should not be considered as valid.

Always pass `--json` to this command. You will receive a JSON dictionary that contains data about the run, including:

- the `run_id`, which can be used to identify the run later
- high-level aggregate metrics, which can be used to summarize how it went

## Analyzing evals

If you have run a eval with `qt run`, like in the previous section, you can get take the `run_id` that was returned, and use `qt show` to see lots more detail about it:

```bash
qt show <run_id> --json
```

Always pass `--json` to this command. You will receive a JSON dictionary that contains summary statistics about the run, along with every sample (in the `samples` key) and its sample-specific metrics, inputs and outputs.

## Comparing evals

The `qt` CLI can compare two evals. To do a comparison, you need to have the two `run_id` values for the evals you want to compare. You may already have them in your memory, but if you don't, you can find them by running:

```bash
qt list --json
```

This command will output a JSON dictionary with information about each run. After you find the two `run_id`s to compare, run this command:

```bash
qt compare <run_id 1> <run_id 2> --json
```

Always pass the `--json` flag to both the `qt list` and `qt compare ...` commands, like the others above. This `qt compare` command will output a JSON dictionary with lots of metrics and information about each run. Users will most often want you to use this information to determine which of the two runs was "better" in some way.

## Custom evals

Use the subsections in this section _only_ if the user needs to write custom evals using the Quantiles Python or TypeScript SDKs

### Writing evals

#### Choose an SDK

Quantiles supports built-in ahnd three ways to write evals:

| Approach | Best for | Entry point |
|---|---|---|
| **Python SDK (`sdk/`)** | Full-featured evals with dataset loading, LLM sampling, scoring, and export | `sdk/src/quantiles/examples/<eval>/__main__.py` |
| **Python SDK (`qt-sdk-python`)** | Lightweight, `qt`-native workflows with steps, emits, and local observability | `qt-sdk-python/examples/<eval>.py` |
| **TypeScript SDK** | Frontend-integrated or Node-based evals | `frontend/src/app/documentation/page.mdx` (reference) |

If the user is inside `sdk/`, use the **legacy Python SDK**. If they are inside `qt-sdk-python/`, use the **new Python SDK**. Only use TypeScript if the eval lives in the NextJS frontend or the user explicitly asks for Node/TS.

#### Python

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

#### TypeScript

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

### Running custom evals

Like with built-in evals, running custom evals is done with the `qt` CLI. The below commands start a local server, inject run metadata into the subprocess, and tear down when done.

Use the below for a Python custom eval:

```bash
qt run my-workflow --input '{"key":"value"}' -- uv run python my_eval.py
```

And use the below for a TypeScript custom eval:

```bash
qt run my-workflow -- bun run my_eval.ts
```

In any commands like this, the `qt run` command will automatically inject the following environment variables:

- `QUANTILES_BASE_URL` — local server URL (default `http://127.0.0.1:8765`)
- `QUANTILES_RUN_ID` — existing run ID (if resuming)
- `QUANTILES_WORKFLOW_NAME` — the workflow name passed to `qt run`
- `QUANTILES_INPUT` — JSON input from `--input`

Make sure to use the Quantiles SDKs for Python or TypeScript, as appropriate. They will automatically detect and handle these variables. The above examples use the SDKs.
