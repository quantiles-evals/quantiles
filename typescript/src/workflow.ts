import { AsyncLocalStorage } from "node:async_hooks";
import { QuantilesClient } from "./client.js";
import type {
  JsonValue,
  WorkflowContext,
  WorkflowDescriptor,
} from "./types.js";

const workflowStore = new AsyncLocalStorage<WorkflowContext>();

export type StepExecutor<TOutput> = () => Promise<TOutput> | TOutput;

/**
 * Create a durable workflow definition.
 *
 * The returned {@link WorkflowDescriptor} can be passed to {@link entrypoint}
 * so that `qt run <name>` knows how to dispatch execution.
 *
 * When the descriptor's `run()` method is called, the function:
 * 1. Creates (or reuses) a {@link QuantilesRun} via the local Quantiles server.
 * 2. Injects a {@link WorkflowContext} into the handler.
 * 3. Stores the handler's return value as the run output.
 * 4. Marks the run as completed or failed.
 *
 * Overload signatures let TypeScript preserve the correct input/output types
 * depending on whether the handler accepts an explicit `input` argument.
 *
 * @param name - The workflow name. Must match the name used with
 *   `qt run <name>`.
 * @param handler - Async function that receives the workflow context (and
 *   optionally the input value), and returns a JSON-serialisable result.
 *
 * @example
 * ```ts
 * const hello = workflow("hello", async (name: string, ctx) => {
 *   const result = await step("greet", name, async () => `Hello, ${name}!`);
 *   emit("greeting_length", result.length);
 *   return result;
 * });
 * ```
 */
export function workflow<TOutput extends JsonValue>(
  name: string,
  handler: (ctx: WorkflowContext) => Promise<TOutput> | TOutput,
): WorkflowDescriptor<undefined, TOutput>;

export function workflow<TInput extends JsonValue, TOutput extends JsonValue>(
  name: string,
  handler: (input: TInput, ctx: WorkflowContext) => Promise<TOutput> | TOutput,
): WorkflowDescriptor<TInput, TOutput>;

export function workflow<TInput extends JsonValue, TOutput extends JsonValue>(
  name: string,
  handler:
    | ((input: TInput, ctx: WorkflowContext) => Promise<TOutput> | TOutput)
    | ((ctx: WorkflowContext) => Promise<TOutput> | TOutput),
): WorkflowDescriptor<TInput | undefined, TOutput> {
  const run = async (callerInput?: TInput): Promise<TOutput> => {
    const baseUrl =
      process.env["QUANTILES_BASE_URL"] ?? "http://127.0.0.1:8765";
    const client = new QuantilesClient({ baseUrl });

    let runId: number;
    const envRunId = process.env["QUANTILES_RUN_ID"];

    if (envRunId !== undefined) {
      runId = Number(envRunId);
    } else {
      const run = await client.createRun(
        name,
        callerInput as unknown as JsonValue,
      );
      runId = run.id;
    }

    await client.health();

    const ctx: WorkflowContext = { runId, workflowName: name, client };

    let input: TInput | undefined;
    if (envRunId !== undefined) {
      const envInput = process.env["QUANTILES_INPUT"];
      input =
        envInput !== undefined ? (JSON.parse(envInput) as TInput) : undefined;
    } else {
      input = callerInput;
    }

    return workflowStore.run(ctx, async () => {
      try {
        const result = await (
          handler as (
            input: TInput | undefined,
            ctx: WorkflowContext,
          ) => Promise<TOutput> | TOutput
        )(input, ctx);
        if (envRunId === undefined) {
          // if a run ID isn't given, we're executing programmatically,
          // not via the CLI, so manually complete the run
          await client.setRunOutput(runId, result);
          await client.completeRun(runId);
        } else {
          // otherwise, we're executing in the CLI, so don't manually
          // complete the run and let the CLI do it.
          await client.setRunOutput(runId, result);
        }
        return result;
      } catch (error) {
        if (envRunId === undefined) {
          await client.failRun(runId, error);
        }
        throw error;
      }
    });
  };

  return { name, run };
}

/**
 * Execute a named step inside a workflow handler.
 *
 * The Quantiles server caches step results keyed by the `stepKey` and a hash of
 * the input. This makes reruns fast and failures recoverable: if a workflow
 * crashes halfway through, re-running it replays cached steps and only
 * re-executes the ones that failed or are new.
 *
 * Must be called inside a function passed to {@link workflow}. Calling it
 * outside a workflow handler throws an error.
 *
 * @param stepKey - A stable identifier unique within the run. Collisions or
 *   dynamic (non-deterministic) keys break caching.
 * @param execute - The computation to execute if the step is not already
 *   cached.
 */
export function step<TOutput extends JsonValue>(
  stepKey: string,
  execute: StepExecutor<TOutput>,
): Promise<TOutput>;

/**
 * Execute a named step inside a workflow handler, with an explicit input value.
 *
 * The Quantiles server caches step results keyed by the `stepKey` and a hash of
 * the input. This makes reruns fast and failures recoverable: if a workflow
 * crashes halfway through, re-running it replays cached steps and only
 * re-executes the ones that failed or are new.
 *
 * Must be called inside a function passed to {@link workflow}. Calling it
 * outside a workflow handler throws an error.
 *
 * @param stepKey - A stable identifier unique within the run. Collisions or
 *   dynamic (non-deterministic) keys break caching.
 * @param input - Optional JSON-serialisable payload. Hashed and stored with
 *   the step for cache invalidation.
 * @param execute - The computation to execute if the step is not already
 *   cached.
 *
 * @example
 * ```ts
 * const result = await step("fetch-user", { id: 42 }, async () => {
 *   return await db.user.findById(42);
 * });
 * ```
 */
export function step<TOutput extends JsonValue>(
  stepKey: string,
  input: JsonValue,
  execute: StepExecutor<TOutput>,
): Promise<TOutput>;

export async function step<TOutput extends JsonValue>(
  stepKey: string,
  arg2: JsonValue | StepExecutor<TOutput>,
  arg3?: StepExecutor<TOutput>,
): Promise<TOutput> {
  const ctx = workflowStore.getStore();
  if (ctx === undefined) {
    throw new Error("step() must be called inside a workflow() handler");
  }

  let stepInput: JsonValue;
  let execute: StepExecutor<TOutput>;

  if (typeof arg2 === "function") {
    stepInput = {};
    execute = arg2;
  } else {
    if (arg3 === undefined) {
      throw new Error("step() requires an execute function");
    }
    stepInput = arg2;
    execute = arg3;
  }

  return ctx.client.runStep(ctx.runId, stepKey, stepInput, execute);
}

/**
 * Emit a named metric for the current run.
 *
 * Metrics are stored by the local Quantiles server and appear in `qt list`,
 * `qt show`, and `qt compare` output.
 *
 * Must be called inside a function passed to {@link workflow}. Calling it
 * outside a workflow handler throws an error.
 *
 * @param metricName - Arbitrary metric name, e.g. `"latency_ms"` or
 *   `"accuracy"`.
 * @param metricValue - Numeric value.
 * @param unit - Optional display unit (e.g. `"ms"`, `"tokens"`).
 */
export async function emit(
  metricName: string,
  metricValue: number,
  unit?: string,
): Promise<void> {
  const ctx = workflowStore.getStore();
  if (ctx === undefined) {
    throw new Error("emit() must be called inside a workflow() handler");
  }

  return ctx.client.emitMetric(ctx.runId, metricName, metricValue, unit);
}

/**
 * Wire one or more {@link WorkflowDescriptor}s to the `qt run` CLI.
 *
 * Reads `QUANTILES_WORKFLOW_NAME` from the environment (injected by `qt run`),
 * looks up the matching descriptor, and executes its `run()` method. If no
 * matching workflow is found or the process is not started via the CLI, the
 * function throws.
 *
 * This call should be the last line of your script:
 *
 * ```ts
 * entrypoint(myWorkflow);
 * ```
 *
 * @param workflows - One or more descriptors returned by {@link workflow}.
 */
export function entrypoint(
  ...workflows: Array<{ name: string; run: () => Promise<JsonValue> }>
): void {
  const name = process.env["QUANTILES_WORKFLOW_NAME"];
  if (name === undefined || name === "") {
    throw new Error(
      "Run via `qt run <workflow>` (QUANTILES_WORKFLOW_NAME not set)",
    );
  }

  const wf = workflows.find((w) => w.name === name);
  if (wf === undefined) {
    throw new Error(`Unknown workflow: ${name}`);
  }

  wf.run().catch((error: unknown) => {
    process.stderr.write(`${String(error)}\n`);
    process.exit(1);
  });
}
