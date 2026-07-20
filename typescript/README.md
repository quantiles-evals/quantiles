# `@quantiles/sdk`

> **Note**: This SDK is currently unsupported and unreleased.

TypeScript client for the local Quantiles eval runner.

## Installation

```bash
bun install quantiles
```

## Usage

Use the following code to build a custom evaluation with TypeScript. To run it with `qt run`, configure it in a `quantiles.toml` file as described in the [configuration guide](https://quantiles.io/documentation/configuration).

```ts
import { QuantilesClient } from "@quantiles/sdk";

const client = new QuantilesClient();
const run = await client.createRun("eval-smoke-test", {
  model: "gpt-5.6",
});

const output = await run.step("call-model", { prompt: "hello" }, async () => {
  return { text: "model output" };
});

await run.complete();
```

During local development, the SDK executes user code locally. The `qt` server initiates and coordinates workflows, deduplicates steps, and manages durable state, stored outputs, observability records, and metrics.
