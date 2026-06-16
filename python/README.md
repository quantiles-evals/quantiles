# Quantiles Python SDK

Python SDK for the [quantiles](https://quantiles.io) local AI workload observability server.

## Installation

```bash
uv pip install -e ".[dev]"
```

## Usage

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

## Development

Run tests:

```bash
mise run test
```

Run linter:

```bash
mise run lint
```
