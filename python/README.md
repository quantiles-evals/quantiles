# Quantiles Python SDK

Python SDK for the [quantiles](https://quantiles.io) local AI workload observability server.

## Installation

```bash
uv pip install quantiles
```

## Usage

To build a custom eval with Python, use the below code. To ensure this eval is runnable with `qt run`, set up a `quantiles.toml` configuration file. See [`../CONFIG.md`](../CONFIG.md) for details.

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

In local development, the SDK executes user code locally. The `qt` server deduplicates steps, triggers workflows, owns durable state, stored outputs, observability records, and metrics.

## Development

Run tests:

```bash
mise run test
```

Run linter:

```bash
mise run lint
```
