# Native τ³ harness

`tau3-mock` is a built-in conformance benchmark for Quantiles' native τ³-style
agent harness. It exercises the parts that differ from prompt/response
benchmarks:

- provider-neutral structured tool calling;
- a model-driven user simulator;
- a turn-based agent/user/tool orchestrator;
- an isolated mutable environment for every task trial;
- durable trajectories and component rewards; and
- aggregate `pass_at_k` metrics using the unbiased estimator.

It intentionally uses a small bundled mock domain. It is not a substitute for
the official τ³-bench 1.0.1 airline, retail, telecom, banking-knowledge, or voice
tracks, and its scores must not be submitted to or compared with the official
leaderboard.

Run it with separate agent and user models:

```console
qt run tau3-mock --input '{
  "model": "openai:gpt-5.2",
  "user_model": "openai:gpt-5.2",
  "trials": 4,
  "max_turns": 20
}'
```

The built-in supports OpenAI, Anthropic, and Gemini model configurations because
those existing Quantiles backends expose structured tool calls through `genai`.
Remote model calls occur only when the user runs the benchmark with one of those
models. Quantiles continues to store runs, steps, trajectories, and metrics
locally.

Official domain parity requires a separately reviewable port of the versioned
task data, domain policies, databases, exact tool behavior, banking retrieval
corpus and graders, and full-duplex voice orchestration. Until that work lands,
`tau3-mock` executes the bundled conformance domain, while `tau3-airline` is
reserved and returns a not-implemented error. The general `tau3` name and other
official domain names do not resolve.
