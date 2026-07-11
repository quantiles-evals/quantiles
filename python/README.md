# Quantiles Python SDK

The Quantiles Python SDK for the [quantiles](https://quantiles.io) local AI workload observability server.

## Installation

```bash
uv add quantiles
```

## Usage

Use the following code to build a custom evaluation with Python. To run it with `qt run`, configure it in a `quantiles.toml` file as described in the [configuration guide](../CONFIG.md).

```python
import asyncio
from quantiles import workflow, step, emit, entrypoint

async def handler(input_value, ctx):
    result = await ctx.step(
        "fetch-data",
        {"url": "https://example.com"},
        lambda: {"status": 200}
    )
    await ctx.emit("latency_ms", 50, "ms")
    return result

my_workflow = workflow("demo", handler)

if __name__ == "__main__":
    entrypoint(my_workflow)
```

During local development, the SDK executes user code locally. The `qt` server initiates and coordinates workflows, deduplicates steps, and manages durable state, stored outputs, observability records, and metrics.

## Development

Run tests:

```bash
mise run test
```

Run linter:

```bash
mise run lint
```
