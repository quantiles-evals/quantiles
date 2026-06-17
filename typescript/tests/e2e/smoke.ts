import { emit, entrypoint, step, workflow } from "../../src/index.js";

type SmokeInput = {
  iterations?: number;
};

const smoke = workflow<SmokeInput, { ok: boolean; total: number; iterations: number }>(
  "e2e-smoke-ts",
  async (input = {}) => {
    const iterations = input.iterations ?? 3;
    const results: Array<{ val: number }> = [];

    for (let i = 0; i < iterations; i++) {
      const result = await step(
        `step-${i}`,
        { index: i },
        async () => ({ val: i * 2 }),
      );
      results.push(result);
    }

    const total = results.reduce((sum, r) => sum + r.val, 0);

    await emit("total", total);
    await emit("iterations", iterations);

    return { ok: true, total, iterations };
  },
);

entrypoint(smoke);
