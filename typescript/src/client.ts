import type {
  CreateRunResponse,
  JsonValue,
  RunRecord,
  StepDecision,
} from "./types.js";
import {
  errorMessage,
  hashJson,
  responseErrorMessage,
  stableStringify,
} from "./util.js";

/**
 * Low-level HTTP client for the Quantiles local server.
 *
 * Manages the lifecycle of runs, steps, and metrics by making JSON-over-HTTP
 * calls to `http://127.0.0.1:8765` (or the value of
 * `QUANTILES_BASE_URL`).
 *
 * Most users should not use `QuantilesClient` directly. The high-level helpers
 * `workflow`, `step`, and `emit` (from {@link workflow}) wrap this
 * client and handle state injection when a process is started via `qt run`.
 */
export class QuantilesClient {
  readonly baseUrl: string;

  constructor(options: { baseUrl?: string } = {}) {
    const baseUrl =
      options.baseUrl ??
      process.env["QUANTILES_BASE_URL"] ??
      "http://127.0.0.1:8765";
    this.baseUrl = baseUrl.replace(/\/+$/, "");
  }

  /** Check that the local Quantiles server is reachable. */
  async health(): Promise<void> {
    await this.request<void>("/health", { method: "GET" });
  }

  /**
   * Create a new run for the given workflow.
   *
   * @param workflowName - Identifier used for grouping and listing runs.
   * @param input - Optional JSON-serialisable payload stored as the run input.
   * @returns A {@link QuantilesRun} handle bound to this client.
   */
  async createRun(
    workflowName: string,
    input?: JsonValue,
  ): Promise<QuantilesRun> {
    const response = await this.request<CreateRunResponse>("/runs", {
      method: "POST",
      body: JSON.stringify({
        workflow_name: workflowName,
        input: input === undefined ? null : stableStringify(input),
      }),
    });

    return new QuantilesRun(this, response.run_id);
  }

  /**
   * Fetch the run record for the given ID.
   *
   * When called without an argument inside a process started by `qt run`,
   * the run ID is read from the `QUANTILES_RUN_ID` environment variable.
   *
   * @param runId - Numeric run ID. Omit inside a `qt run` subprocess.
   */
  async getRun(runId?: number): Promise<RunRecord> {
    let reifiedRunID: number;
    if (runId !== undefined) {
      reifiedRunID = runId;
    } else {
      const envRunId = process.env["QUANTILES_RUN_ID"];
      if (envRunId === undefined) {
        throw new Error(
          "This script must be run via `qt run` ('QUANTILES_RUN_ID' not found)",
        );
      }
      try {
        reifiedRunID = parseInt(envRunId, 10);
      } catch (e: unknown) {
        throw new Error(`Invalid run ID ${runId} (${e})`);
      }
    }
    return this.request<RunRecord>(`/runs/${reifiedRunID}`, {
      method: "GET",
    });
  }

  /** Store the final JSON output for a run. */
  async setRunOutput(runId: number, output: JsonValue): Promise<void> {
    await this.request<void>(`/runs/${runId}/output`, {
      method: "POST",
      body: JSON.stringify({
        output: stableStringify(output),
      }),
    });
  }

  /** Mark a run as successfully completed. */
  async completeRun(runId: number): Promise<void> {
    await this.request<void>(`/runs/${runId}/complete`, {
      method: "POST",
    });
  }

  /** Mark a run as failed, storing an error description on the server. */
  async failRun(runId: number, error: unknown): Promise<void> {
    await this.request<void>(`/runs/${runId}/fail`, {
      method: "POST",
      body: JSON.stringify({ error: errorMessage(error) }),
    });
  }

  /**
   * Record a named metric for a run.
   *
   * @param runId - The run to attach the metric to.
   * @param metricName - Arbitrary metric name, e.g. `"accuracy"` or `"latency_ms"`.
   * @param metricValue - Numeric value.
   * @param unit - Optional human-readable unit (e.g. `"ms"`, `"tokens"`).
   */
  async emitMetric(
    runId: number,
    metricName: string,
    metricValue: number,
    unit?: string,
  ): Promise<void> {
    await this.request<void>(`/runs/${runId}/metrics`, {
      method: "POST",
      body: JSON.stringify({
        metric_name: metricName,
        metric_value: metricValue,
        unit: unit ?? null,
      }),
    });
  }

  /**
   * Execute a durable step inside a run.
   *
   * The server checks whether this step has already been executed with the
   * same `step_key` and input hash. If so, the cached output is returned
   * ("reuse"). Otherwise the `execute` callback is invoked, its output is
   * serialised, and the result is stored on the server ("run").
   *
   * @param runId - The parent run ID.
   * @param stepKey - A stable identifier used for caching and restartable runs.
   * @param input - JSON input to the step; hashed for cache lookup.
   * @param execute - The actual computation to execute if the step is not cached.
   * @returns The step output, either freshly computed or replayed from cache.
   */
  async runStep<TOutput extends JsonValue>(
    runId: number,
    stepKey: string,
    input: JsonValue,
    execute: () => Promise<TOutput> | TOutput,
  ): Promise<TOutput> {
    const decision = await this.request<StepDecision>("/steps/begin", {
      method: "POST",
      body: JSON.stringify({
        run_id: runId,
        step_key: stepKey,
        input_hash: hashJson(input),
      }),
    });

    if (decision.decision === "reuse") {
      return JSON.parse(decision.output) as TOutput;
    }

    try {
      const output = await execute();
      await this.request<void>("/steps/complete", {
        method: "POST",
        body: JSON.stringify({
          step_id: decision.step_id,
          output: stableStringify(output),
        }),
      });
      return output;
    } catch (error) {
      await this.request<void>("/steps/fail", {
        method: "POST",
        body: JSON.stringify({
          step_id: decision.step_id,
          error: errorMessage(error),
        }),
      });
      throw error;
    }
  }

  private async request<T>(path: string, init: RequestInit): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`, {
      ...init,
      headers: {
        "content-type": "application/json",
        ...init.headers,
      },
    });

    if (!response.ok) {
      throw new Error(await responseErrorMessage(response));
    }

    const text = await response.text();
    return (text === "" ? undefined : JSON.parse(text)) as T;
  }
}

/**
 * A convenience wrapper around a specific run.
 *
 * Created by {@link QuantilesClient.createRun}. The methods (`step`,
 * `setOutput`, `complete`, `fail`, `emit`) forward to the underlying client
 * with the run ID already bound.
 */
export class QuantilesRun {
  readonly id: number;
  readonly client: QuantilesClient;

  constructor(client: QuantilesClient, id: number) {
    this.client = client;
    this.id = id;
  }

  /** See {@link QuantilesClient.runStep}. */
  async step<TOutput extends JsonValue>(
    stepKey: string,
    input: JsonValue,
    execute: () => Promise<TOutput> | TOutput,
  ): Promise<TOutput> {
    return this.client.runStep(this.id, stepKey, input, execute);
  }

  /** See {@link QuantilesClient.setRunOutput}. */
  async setOutput(output: JsonValue): Promise<void> {
    await this.client.setRunOutput(this.id, output);
  }

  /** See {@link QuantilesClient.completeRun}. */
  async complete(): Promise<void> {
    await this.client.completeRun(this.id);
  }

  /** See {@link QuantilesClient.failRun}. */
  async fail(error: unknown): Promise<void> {
    await this.client.failRun(this.id, error);
  }

  /** See {@link QuantilesClient.emitMetric}. */
  async emit(
    metricName: string,
    metricValue: number,
    unit?: string,
  ): Promise<void> {
    return this.client.emitMetric(this.id, metricName, metricValue, unit);
  }
}
