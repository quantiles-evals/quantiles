# Quantiles Python SDK

The Quantiles Python SDK provides the components for building highly customized AI evaluations that run locally through the [`qt` CLI](https://quantiles.io/documentation/reference/cli) and server.

## Installation

```bash
uv add quantiles
```

## Usage

Use the following code to build a custom evaluation with Python. To run it with `qt run`, configure it in a `quantiles.toml` file as described in the [configuration guide](https://quantiles.io/documentation/configuration).

```python
from quantiles import JsonValue, WorkflowContext, emit, entrypoint, step, workflow


async def fetch_data() -> JsonValue:
  return {"status": 200}


async def handler(_input: JsonValue, ctx: WorkflowContext) -> JsonValue:
  result = await step(
    ctx,
    step_key="fetch-data",
    input_value={"url": "https://example.com"},
    execute=fetch_data,
  )
  await emit(ctx, "latency_ms", 50, "ms")
  return result


my_workflow = workflow("demo", handler)

if __name__ == "__main__":
  entrypoint(my_workflow)
```

Evaluation code executes locally. The `qt` server coordinates workflows, deduplicates steps, and manages durable state, stored outputs, observability records, and metrics.

## Development

```bash
mise run test
mise run lint
mise run fmt-check
mise run typecheck
```
