# Eval Best Practices

Quantiles provides a framework that makes evals faster to build, more reliable to run, and easier to measure, so you can focusing in figuring out if your AI app is getting better over time.

This document provides a set of best practices for writing evals using Quantiles.

## Deterministic Step Inputs

A _step_ in Quantiles is a discrete action that your eval take, that is a "checkpoint" in the eval script. You generally run code inside a `step` that can fail or is expensive, like calling your model or doing some big database operation. Most often, you'll use the `step(...)` function in the Typescript/Python SDks to mark the processing of a sample in your eval.

Quantiles makes sure that, if your eval crashes, it doesn't have to start from the beginning - it can just start from the last `step` that succeeded.

Samples are also useful for observability - they have inputs and outputs, so you can get a report of what happened just before, during, and just after each step.

For Quantiles to treat samples properly, it's usually helpful to pass data to the `step` function that tells Quantiles whether a given `step` invocation is different from a previous one. Consider the below code:

```typescript
const cities = ["Tokyo", "Munich", "Nairobi", "San Francisco", "Buenos Aires", "Sydney"];
const city1 = cities[Math.floor(Math.random() * cities.length)];
const city2 = cities[Math.floor(Math.random() * cities.length)];

step("call-llm", async () => {
  return async call_llm(`what is the weather in ${city1}`);
});

step("call-llm", async () => {
  return async call_llm(`what is the weather in ${city2}`);
})
```

In this code, the two samples have the same name, but they close over different data (`city1` and `city2`, respectively). That means Quantiles can't tell the difference between them, so it will:

- Assume they're equal - which can lead to problems when comparing two evals
- Not be able to intelligently cache and recover from failed samples - which can lead to problems when, for example, `call_llm` is flaky

Initially, you won't need to worry about these things, but as you continue to run your evals over time (and we believe that you should!), you will be increasingly likely to encounter them. To solve this problem, you have two options:

1. Give each step a distinct name - this is the easiest fix, but you might not want to change your step name
2. Pass a cache key to the `step` function - this is the fix if you don't want to change your step name

```typescript
// <snip>

// The second parameter to `step(...)` tells Quantiles that this 
// call-llm step uses the 'city1' data. Quantiles will consider it
// different from any other call-llm step that uses any other
// data.
step("call-llm", {"city": city1}, async () => {
  return async call_llm(`what is the weather in ${city1}`);
});

// Because the second parameter to `step(...)` has a different
// value than that of the previous `step(...)` call, Quantiles
// considers this step as different from the previous call.
step("call-llm", {"city": city2}, async () => {
  return async call_llm(`what is the weather in ${city2}`);
})
````

## Running Samples in Loops

Many evals start by iterating over a dataset. Below is an example:

```typescript
interface DatasetRow {
  prompt: string;
  goldenOutput: string;
}

const datasetRows: DatasetRow[] = await loadDataset();
for (let i = 0; i < 100; i++) {
  const datasetRow = datasetRows[i];
  await step(`iter_step:${i}`, () => {
    doSomething(i)
  });
}
```

In this example, Quantiles will be able to differentiate each step, which is great. Over time, though, you might want to alter the dataset over which you're looping. For example:
