# `@quantiles/sdk`

TypeScript client for the local Quantiles eval runner. Note that this SDK is currently unsupported and unreleased.

## Installation

```bash
bun install quantiles
```

## Usage

To build a custom eval with TypeScript, use the below code. To ensure this eval is runnable with `qt run`, set up a `quantiles.toml` configuration file. See [`../CONFIG.md`](../CONFIG.md) for details.

```ts
import { QuantilesClient } from "@quantiles/sdk";

const client = new QuantilesClient();
const run = await client.createRun("eval-smoke-test", {
  model: "gpt-4.1-mini",
});

const output = await run.step("call-model", { prompt: "hello" }, async () => {
  return { text: "model output" };
});

await run.complete();
```

In local development, the SDK executes user code locally. The `qt` server deduplicates steps, triggers workflows, owns durable state, stored outputs, observability records, and metrics.
