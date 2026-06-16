# Quantiles Eval Protocol

This document describes the API calls that a successful eval should make to record its execution in Quantiles. It covers the REST API contract and the corresponding TypeScript SDK calls.

An eval, also called a workflow in the protocol, is a process that does the following:
1. Creates a run (or reuses one assigned by the CLI)
2. Runs samples (durable, memoized units of work)
3. Emits metrics
4. Saves its final output
5. Completes (or fails)

The exact calls differ slightly depending on whether the eval is started by `qt run` (CLI-driven) or started directly by calling `.run()` on a workflow object (programmatic).

---

## Execution Modes

### CLI-driven run (`qt run my-eval -- ...`)

The CLI creates the run, starts the server, and spawns the eval process with these environment variables:

- `QUANTILES_RUN_ID` — the ID of the run
- `QUANTILES_BASE_URL` — the local server URL
- `QUANTILES_WORKFLOW_NAME` — the eval name
- `QUANTILES_INPUT` — the JSON input (or `{}`)

In this mode, the eval process **must not** call `POST /runs` to create a new run — the run already exists. It **must** save its output via `POST /runs/{run_id}/output`, but should let the CLI own the final `complete` or `fail` call.

### Programmatic run (`workflow.run()`)

There is no `QUANTILES_RUN_ID` environment variable. The SDK creates the run itself, then completes or fails the run directly.

---

## Complete Protocol for a Success

### 1. Create a run (programmatic only)

**REST API (not used in CLI-driven mode):**

```http
POST /runs
Content-Type: application/json

{
  "eval_name": "my-eval",
  "input": "{\"dataset\":\"smoke\"}"
}
```

**Response:**

```json
{
  "run_id": 1
}
```

**TypeScript SDK:**

```typescript
const run = await client.createRun("my-eval", { dataset: "smoke" });
// run.id === 1
```

---

### 2. Run a step

Before executing expensive work, call `POST /steps/begin`. If the step was already completed with the same `input_hash`, the server returns a cache hit (`decision: "reuse"`). Otherwise it returns `decision: "run"` and assigns a `step_id`.

**REST API (begin):**

```http
POST /steps/begin
Content-Type: application/json

{
  "run_id": 1,
  "step_key": "fetch-data",
  "input_hash": "abc123..."
}
```

**Response (run):**

```json
{
  "decision": "run",
  "step_id": 3
}
```

**Response (reuse):**

```json
{
  "decision": "reuse",
  "output": "{\"status\":200}"
}
```

If the decision is `"run"`, execute the step, then report completion or failure.

**REST API (complete):**

```http
POST /steps/complete
Content-Type: application/json

{
  "step_id": 3,
  "output": "{\"status\":200}"
}
```

**REST API (fail):**

```http
POST /steps/fail
Content-Type: application/json

{
  "step_id": 3,
  "error": "network timeout"
}
```

**TypeScript SDK:**

```typescript
const result = await step(
  "fetch-data",
  { url: "https://example.com" },
  async () => {
    return { status: 200 };
  },
);
```

---

### 3. Emit a metric

Metrics are associated with a run. They can be emitted from the eval or from individual samples.

**REST API:**

```http
POST /runs/1/metrics
Content-Type: application/json

{
  "metric_name": "latency_ms",
  "metric_value": 50,
  "unit": "ms"
}
```

**TypeScript SDK:**

```typescript
await emit("latency_ms", 50, "ms");
```

---

### 4. Save the eval output

At the end of the eval, save the return value. This is a **separate** call from completing the run.

**CLI-driven mode:** Always call this. The CLI will complete the run later based on the process exit code.

**Programmatic mode:** Skip this — `completeRun` handles both output and status.

**REST API:**

```http
POST /runs/1/output
Content-Type: application/json

{
  "output": "{\"accuracy\":0.85,\"correct\":17,\"total\":20}"
}
```

**TypeScript SDK:**

```typescript
await client.setRunOutput(1, { accuracy: 0.85, correct: 17, total: 20 });
```

---

### 5. Complete the run (programmatic only)

Marks the run as completed, optionally including the output. Emits a `run.completed` event.

**CLI-driven mode:** Do not call this. The CLI calls it when the child process exits successfully.

**Programmatic mode:** Call this (it already includes the output).

**REST API:**

```http
POST /runs/1/complete
```

**TypeScript SDK:**

```typescript
await client.completeRun(1, { accuracy: 0.85, correct: 17, total: 20 });
```

---

### 6. Fail the run (on error)

**CLI-driven mode:** Throw an error or exit non-zero. The CLI will call `POST /runs/{run_id}/fail` automatically.

**Programmatic mode:** Call `failRun`.

**REST API:**

```http
POST /runs/1/fail
Content-Type: application/json

{
  "error": "network failed"
}
```

**TypeScript SDK:**

```typescript
await client.failRun(1, new Error("network failed"));
```

---

## TypeScript Eval Example (CLI-driven)

This is the typical pattern used with `qt run`:

```typescript
import { emit, entrypoint, step, workflow } from "@quantiles/sdk";

const eval = workflow("support-triage", async (input) => {
  const results = await step("classify", input, async () => {
    return await callModel(input);
  });

  emit("accuracy", results.accuracy);
  emit("latency_ms", results.latency, "ms");

  return results;
});

entrypoint(eval);
```

The SDK handles all API calls automatically. Because `QUANTILES_RUN_ID` is set, it:
1. Reuses the existing run ID
2. Creates/executes/reuses samples
3. Emits metrics
4. Calls `setRunOutput(runId, results)` at the end

Then the process exits with code 0, and the CLI completes the run.

---

## TypeScript Eval Example (Programmatic)

Useful for testing or running evals from other scripts:

```typescript
import { QuantilesClient, workflow, step, emit } from "@quantiles/sdk";

const eval = workflow("support-triage", async (input) => {
  const results = await step("classify", input, async () => {
    return await callModel(input);
  });

  emit("accuracy", results.accuracy);
  return results;
});

const client = new QuantilesClient({ baseUrl: "http://127.0.0.1:8765" });
const run = await client.createRun("support-triage", { dataset: "smoke" });
const output = await eval.run({ dataset: "smoke" });

console.log("Run completed:", run.id);
console.log("Output:", output);
```

Because `QUANTILES_RUN_ID` is not set, the eval creates its own run and calls `completeRun(runId, output)` automatically.

---

## Endpoints Summary

| Method | Path | Body | Purpose |
|---|---|---|---|
| `POST` | `/runs` | `{ workflow_name, input }` | Create a new run (programmatic) |
| `GET`  | `/runs/{run_id}` | — | Fetch run metadata |
| `POST` | `/runs/{run_id}/output` | `{ output }` | Save eval return value |
| `POST` | `/runs/{run_id}/complete` | `{ output }` | Mark run completed (programmatic) |
| `POST` | `/runs/{run_id}/fail` | `{ error }` | Mark run failed |
| `POST` | `/runs/{run_id}/metrics` | `{ metric_name, metric_value, unit }` | Record a metric |
| `POST` | `/steps/begin` | `{ run_id, step_key, input_hash }` | Reserve/reuse a step |
| `POST` | `/steps/complete` | `{ step_id, output }` | Mark step completed |
| `POST` | `/steps/fail` | `{ step_id, error }` | Mark step failed |
| `GET`  | `/health` | — | Server health check |
