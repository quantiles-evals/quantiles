# Quantiles Commands

This document contains a command reference for the `qt` CLI.

## `qt init`

Initialize a local Quantiles workspace in the current directory.

This creates and configures the local SQLite database (stored in `.quantiles/quantiles.sqlite` by default).

```bash
qt init
```

Example output:

```text
Initialized Quantiles workspace at ./.quantiles/quantiles.sqlite
```

## `qt run`

Run a command as a durable eval run.

`qt run` creates a eval run, starts the given command as a subprocess, and records the result. It also injects two environment variables:

- `QUANTILES_RUN_ID` — the run id so the subprocess can talk back to the API
- `QUANTILES_BASE_URL` — the base URL of the local Quantiles HTTP server

`qt` automatically starts a local HTTP server on `127.0.0.1:8765` if one is not already running, runs your command, and then tears the server back down when the command finishes.

```bash
# Simple example
qt run hello-world -- echo "hello from Quantiles"
```

You can also pass structured input to the run:

```bash
qt run eval-smoke-test \
  --input '{"model":"gpt-4.1-mini","dataset":"smoke"}' \
  -- python3 ./my_eval.py
```

The `--` separator is required so that flags meant for your command are not parsed by `qt`.

Example output:

```text
Created eval run 2
Executing: echo hello from Quantiles
hello from Quantiles
Run 2 completed successfully
```

### Resuming a run

If a run fails and you want to retry it with the same run id (so that already-
completed samples are reused), pass `--resume`:

```bash
qt run hello-world --resume 2 -- echo "hello again"
```

### TypeScript SDK integration

The Typescript SDK provides a high-level API that allows users to write evals, and provides seamless integration with the `qt run` command.

```typescript
import { workflow, step, emit, entrypoint } from "@quantiles/sdk";

const runEval = workflow("eval", async (input, ctx) => {
  console.log(`Run ${ctx.runId} (${ctx.workflowName})`);

  const result = await step("call-model", async () => {
    // expensive work here
    return { response: "..." };
  });

  emit("latency_ms", 120, "ms");
  return result;
});

entrypoint(runEval);
```

You can register multiple evals and let `qt run <name>` dispatch to the right one:

```typescript
const w1 = workflow("eval", async (input, ctx) => { ... });
const w2 = workflow("benchmark", async (input, ctx) => { ... });

entrypoint(w1, w2);
```

The callback you pass to `workflow` will receive two arguments:

- `input` - parsed from `--input` when running via `qt run`, or from a `QUANTILES_INPUT` environment variable if you run this standalone.
- `ctx` — mostly for internal use, but contains the run ID, workflow name (the string you passed as the first parameter to `workflow`, and the raw Quantiles client. All of these are passed in a typescript `interface` like this: `{ runId, workflowName, client }`

You can also provide explicit input to `step()` for cache invalidation:

```typescript
const result = await step("call-model", { model: "gpt-4" }, async () => {
  return callLLM(model);
});
```

There is also a lower-level `QuantilesClient` API available for users who want finer-grained control over execution.

See `sdk/typescript/examples/run_demo.ts` for a runnable example.

## `qt serve`

Start the local Quantiles HTTP server.

```bash
qt serve
```

Generally, you won't need to run this manually. `qt run` starts a server automatically for the duration of your command. Use `qt serve` only when you want a persistent server (e.g. for development or multiple CLI windows).

## `qt list`

Show all eval runs in reverse chronological order.

```bash
qt list
```

Example output:

```text
ID     EVAL                         STATUS     SAMPLES  CREATED                  FINISHED_AT
7      eval-smoke-test              completed  2        2026-05-02T18:30:00.000Z 2026-05-02T18:30:05.000Z
3      hello-world                  failed     1        2026-05-02T18:15:00.000Z 2026-05-02T18:15:02.000Z
```

## `qt show`

Inspect a single eval run, including its samples, metrics, and events.

```bash
qt show <run_id>
```

Example output:

```text
Run 7
  eval:    eval-smoke-test
  status:      completed
  started_at:  2026-05-02T18:30:00.000Z
  finished_at: 2026-05-02T18:30:05.000Z
  input:       {"model":"gpt-4.1-mini","dataset":"smoke"}
  error:       -

Samples
  ID     KEY        STATUS     INPUT_HASH       CREATED                  FINISHED_AT
  4      fetch-data completed  a1b2c3d4e5f6789  2026-05-02T18:30:00.500Z 2026-05-02T18:30:01.000Z
  5      call-model completed  9f8e7d6c5b4a321  2026-05-02T18:30:01.200Z 2026-05-02T18:30:04.800Z

Metrics
  NAME          VALUE  UNIT
  latency_ms    120    ms  
  tokens_used   42     -   

Events
  ID  TYPE           CREATED                  MESSAGE
  10  run.started    2026-05-02T18:30:00.000Z Started eval eval-smoke-test
  11  step.started   2026-05-02T18:30:00.500Z Started step fetch-data
  12  step.completed 2026-05-02T18:30:01.000Z Completed step 4
  ...
```

## `qt compare`

Compare two eval runs side-by-side.

```bash
qt compare <run_a> <run_b>
```

Checks samples, outputs, and metrics. Exits with code 0 if the runs are identical
and code 1 if anything differs.

Example output:

```text
Run 1: eval-smoke-test (completed)
Run 2: eval-smoke-test (completed)

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
