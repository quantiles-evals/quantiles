/**
 * Quantiles TypeScript SDK
 *
 * High-level, durable workflow primitives for the `qt` CLI.
 *
 * Quick start:
 *
 * ```ts
 * import { workflow, step, emit, entrypoint } from "@quantiles/sdk";
 *
 * const hello = workflow("hello", async (name: string) => {
 *   const result = await step("greet", async () => `Hello, ${name}!`);
 *   emit("len", result.length);
 *   return result;
 * });
 *
 * entrypoint(hello);
 * ```
 */

export { QuantilesClient, QuantilesRun } from "./client.js";
export type {
  JsonValue,
  QuantilesClientOptions,
  RunRecord,
  WorkflowContext,
  WorkflowDescriptor,
} from "./types.js";
export { hashJson, stableStringify } from "./util.js";
export type { StepExecutor } from "./workflow.js";
export { emit, entrypoint, step, workflow } from "./workflow.js";
