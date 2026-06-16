export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

/** Options accepted by {@link QuantilesClient}. */
export interface QuantilesClientOptions {
  baseUrl?: string;
}

/**
 * A run record as returned by the local Quantiles server.
 *
 * Timestamps are ISO-8601 strings. `input` and `output` are JSON-encoded
 * strings (or `null`).
 */
export interface RunRecord {
  id: number;
  workflow_name: string;
  // TODO: make a discriminated union
  status: string;
  input: string | null;
  output: string | null;
  started_at: string;
  finished_at: string | null;
  error: string | null;
}

/** Response shape when creating a new run via POST /runs. */
export interface CreateRunResponse {
  run_id: number;
}

/**
 * Server decision returned when beginning a step.
 *
 * - `"run"` means the server wants the client to execute the step and report
 *   back with the output.
 * - `"reuse"` means an identical step key + input hash has already been
 *   executed in this run, so the server returns the previously stored output.
 */
export type StepDecision =
  | { decision: "run"; step_id: number }
  | { decision: "reuse"; output: string };

/**
 * Context object passed to workflow handlers.
 *
 * When using the high-level API, `workflow()` automatically builds this context
 * from environment variables injected by `qt run`.
 */
export interface WorkflowContext {
  runId: number;
  workflowName: string;
  // TODO: change this to a plain static import
  client: import("./client.js").QuantilesClient;
}

/**
 * Descriptor returned by {@link workflow}. Holds the workflow name and a
 * `run()` function that creates a new run and invokes the handler.
 *
 * @typeParam TInput - Type of the optional input payload.
 * @typeParam TOutput - Type of the workflow result (must be JSON-serialisable).
 */
export interface WorkflowDescriptor<
  TInput = unknown,
  TOutput extends JsonValue = JsonValue,
> {
  name: string;
  run: (input?: TInput) => Promise<TOutput>;
}
