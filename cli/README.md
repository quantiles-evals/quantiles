# Quantiles CLI

This directory holds the source code for the `qt` CLI. It is built with [Rust](https://rust-lang.org/) to help it efficiently use the resources of the local machine, to help ensure safety, and to provide strong lints and type-system invariants for Humans and Agents to work with.

## Install

```bash
curl -fsSL https://cli.quantiles.io/install.sh | bash
```

## Demo

A few commands to see `qt` in action:

```bash
# 1. Initialize a workspace
qt init

# 2. Run an eval — Quantiles auto-starts a local server, records the run,
#    and tears the server down when the command finishes.
qt run hello-world -- echo "hello from Quantiles"

# 3. List and inspect what happened
qt list
qt show 1
```

## Architecture

The Quantiles CLI, `qt`, keeps execution simple: your code runs locally, while `qt` handles durability and observability.

```
+--------------------------------------+
|            Your Script               |
|   (TypeScript / Python / Shell)      |
+-------------------+------------------+
                    │
                    │  HTTP / JSON
                    |
                    ▼
+--------------------------------------+
|            Quantiles Server          |
+-------------------+------------------+
                    │
                    │  SQLite
                    |
                    ▼
+------------------------------------------------+
|     .quantiles/quantiles.sqlite (local DB)     |
+-------------------+----------------------------+
                    │
                    │
                    │
                    ▼
+--------------------------------------+
|                 CLI                  |
|        (list, show, compare)         |
+--------------------------------------+
```

- **Server** owns durability decisions: step caching, run state, metrics
- **Client** (your script) owns code execution: the server never runs your logic
  - Note that the CLI itself also has built-in benchmarks, which do not involve your code
- **CLI** reads the same SQLite database the server writes to

## Customization

You can customize how the CLI executes benchmarks using a `quantiles.toml` or `.quantiles.toml` configuration file. This file can be used to control benchmark execution behavior as well as customize the models, providers, and other settings used during eval runs. See [`./cli/examples/configs`](./cli/examples/configs) for examples and more details.

## Durable step caching + crash resume (TypeScript SDK)

Use the high-level `workflow()`, `step()`, and `emit()` API for automatic
caching and observability:

```typescript
import { workflow, step, emit, entrypoint } from "@quantiles/sdk";

const runEval = workflow("eval", async (input, ctx) => {
  console.log(`Run ${ctx.runId} (${ctx.workflowName})`);

  const data = await step("fetch-data", { url: "https://example.com" }, async () => {
    return { status: 200, body: "<html>...</html>" };
  });

  // Same step key + input hash means cached output, and no re-execution
  const cached = await step("fetch-data", { url: "https://example.com" }, async () => {
    return { status: 999, body: "this should not execute" };
  });

  emit("latency_ms", 120, "ms");
  emit("tokens_used", 42);

  return data;
});

entrypoint(runEval);
```

Register multiple evals and let `qt run <name>` dispatch to the right one
automatically:

```typescript
const w1 = workflow("eval", async (input, ctx) => { ... });
const w2 = workflow("benchmark", async (input, ctx) => { ... });

entrypoint(w1, w2);
```

Then, when you're ready, run your eval with this command:

```bash
qt run eval -- bun run sdk/typescript/examples/run_demo.ts
```

If the run fails partway through, resume it and only the failed samples rerun:

```bash
qt run my-eval --resume 1 -- bun run sdk/typescript/examples/run_demo.ts
```

## Step Caching

You can also call `step()` without an input hash when caching by step name alone is fine:

```typescript
const modelOutput = await step("call-model", async () => {
  return callLLM(prompt);
});
```

See [quantiles.io/documentation/workflows-and-steps](https://quantiles.io/documentation/workflows-and-steps) for more details on how steps and step caching work.

## Comparing runs

After iterating on an eval, compare two runs to see exactly what changed:

```bash
# Run A — baseline
qt run my-eval -- bun run sdk/typescript/examples/run_demo.ts

# Run B — your latest iteration
qt run my-eval -- bun run sdk/typescript/examples/run_demo.ts

# See what changed between them
qt compare 1 2
```

Example output when outputs differ:

```text
Run 1: my-eval (completed)
Run 2: my-eval (completed)

Samples
  STEP                     PRESENT  INPUT          STATUS         OUTPUT
  fetch-data               both     same           same           differs

Output differences
  step: fetch-data
    run 1: {"status":200,"body":"<html>hello</html>"}
    run 2: {"status":200,"body":"<html>world</html>"}

Metrics
  NAME                          Run 1          Run 2
  latency_ms                       50            150 *
  tokens_used                      42             89 *

Runs differ.
```

`qt compare` exits with code 1 if the runs differ, making it useful in CI scripts.

## Comparisons to other systems

| | Temporal / Trigger.dev | Quantiles |
|---|---|---|
| **Goal** | Run evals reliably | Run evals and **quickly learn what improved, what regressed, and why** |
| **Setup** | Cluster, workers, cloud infra | Single binary + SQLite |
| **Programming Model** | Deterministic constraints | Normal async code |
| **Local Dev** | Usually tied to a service/runtime | Fully offline |
| **Observability** | Logs and traces | Automatically compare runs and surface changes |
| **Iteration** | Run, then inspect | Run, then compare, then improve |

Quantiles is for the iteration loop before production orchestration.

Run your code, then instantly see what changed across runs. No notebooks, no pipelines, and no manual comparisons.

It doesn’t replace Temporal for production orchestration. It’s built for the 90% of iteration that happens before you ever think about production infrastructure.

## Command Reference

See [quantiles.io/documentation/reference/cli](https://quantiles.io/documentation/reference/cli) for a detailed list of `qt` commands.
