import { emit, entrypoint, step, workflow } from "../src/index";

const demo = workflow("demo", async (input, ctx) => {
  console.log(`Run ${ctx.runId} (${ctx.workflowName})`);
  console.log(`Input: ${JSON.stringify(input)}`);

  const result = await step(
    "fetch-data",
    { url: "https://example.com" },
    async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
      return { status: 200, body: "<html>...</html>" };
    },
  );

  // Same step key + input hash reuses the cached output.
  const result2 = await step(
    "fetch-data",
    { url: "https://example.com" },
    async () => {
      return { status: 999, body: "this should not execute" };
    },
  );

  console.log(`First call:  ${JSON.stringify(result)}`);
  console.log(`Second call: ${JSON.stringify(result2)}`);

  emit("latency_ms", 50, "ms");
  emit("tokens_used", 42);
  emit("result1_returned_status", result.status);
  emit("result2_returned_status", result2.status);

  console.log("Done!");
  return result;
});

entrypoint(demo);
