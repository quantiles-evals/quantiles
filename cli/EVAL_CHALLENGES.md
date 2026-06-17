# Why Evals Are Hard (And What to do Instead)

Evals look simple:

```typescript
for(const data of dataset) {
  const output = await sampleModel(data);
  const score = await judge(data, output);
  await recordScore(data, output, score);
}
```

In practice, they're not. The moment you try to run real experiments, things break down quickly:

- You need to load and shuffle datasets deterministically
- You need to track which version of your prompt or model you ran
- You need to be able to recover from crashes
- You need to cache expensive samples so you don’t re-run everything
- You need to record the right metrics so you can analyze results later
- You need to compare runs to understand what actually improved

Instead of writing a loop, you end up building a system before you can even start doing what you came here to do -- figure out if your AI is good or not.

## What Most People End Up With

A typical eval looks like this:

1. Write a script to prepare data
2. Run it
3. Write a script to call the model
4. Run it
5. Write a script to score outputs
6. Run it
7. Dump everything into a database
8. Write SQL or open a dashboard to figure out what happened

Each step is separate, each run is hard to reproduce, and comparing results is manual.

## With Quantiles

Quantiles turns all those samples into a single script:

```typescript
import { workflow, step, emit } from "quantiles";
export const evalPrompt = workflow(async ({ promptVersion }) => {
  for (const prompt of dataset) {
    const output = await step("call-model", {prompt}, async () => {
      return callModel(prompt, promptVersion);
    });
    const score = await step("judge", {prompt}, async () => {
      return judge(output);
    });
    emit("score", score);
  }
});
```

... which you can then run and analyze with a few commands:

```bash
# run your eval against two prompts, to figure out
# which performs better
qt run evalPrompt --input '{"promptVersion":"v1"}'
qt run evalPrompt --input '{"promptVersion":"v2"}'
# get a deep analysis of your results
qt compare 1 2
```

You write less code, you get resilience for free, and you spend all your time _actually_ making your AI app better, not building systems.
