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

## Datasets

Use `quantiles.toml` input for configuration, then construct dataset loading behavior in code. For example, pass `dataset_path = "my_data.jsonl"` in config, read `input_value["dataset_path"]` in the workflow handler, and either open that file directly or pass it into a custom `DatasetSource`.

Hugging Face datasets can be loaded by passing a URI string to `dataset(...)`:

```python
ds = await dataset(
    ctx,
    source="huggingface://quantiles/PubMedQA",
    row_type=Row,
    config="pqa_labeled",
    split="train",
)
```

For non-Hugging Face public or private sources, implement `DatasetSource` and pass an instance as `source`. Custom sources run inside the Python workflow process, while batch loading is still recorded through Quantiles steps.

## Development

Run tests:

```bash
mise run test
```

Run linter:

```bash
mise run lint
```
