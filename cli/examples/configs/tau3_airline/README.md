# τ³ Airline `custom_code` example

This example wraps the official [`tau2` `v1.0.1` tag](https://github.com/sierra-research/tau2-bench/tree/v1.0.1) airline runner in one durable Quantiles step. The adapter environment installs that immutable Git tag because τ³ is not published as a `tau2==1.0.1` package on PyPI. τ³ remains responsible for loading tasks, simulating the user, providing airline tools and state, and calculating rewards; the adapter records the official result and emits `average_reward`, `success_rate`, and `simulation_count`.

The adapter intentionally starts with one task and one trial. It accepts these input fields:

- `data_dir`: the `data/` directory in a local checkout of the pinned τ³ release.
- `agent_model` and `user_model`: provider-prefixed model names such as `openai:gpt-4.1`; the adapter converts `:` to the LiteLLM `/` convention.
- `num_tasks`, `num_trials`, `max_steps`, `max_concurrency`, and `seed`: positive integers.
- `task_ids`: an optional non-empty list of τ³ task IDs.

## Run

Install `uv`, make Python 3.12 available to it, and run the following one-time setup from this directory:

```bash
git clone --branch v1.0.1 --depth 1 https://github.com/sierra-research/tau2-bench.git .tau3/tau2-bench
```

The adapter installs the τ³ Python modules from the same immutable tag and reads benchmark tasks and databases from this ignored checkout. This is necessary because the Git-installed Python wheel does not contain the upstream `data/` tree. The adapter also constrains the environment to Python 3.12 because the pinned τ³ release imports the removed `audioop` standard-library module on Python 3.13, including in text mode.

Configure the API keys required by the two selected models, then run:

```bash
qt run tau3-airline
```

For example, both default models require `OPENAI_API_KEY`. A run calls external model providers and may incur charges. Quantiles remains local-first for its run metadata, step output, and metrics, while the configured τ³ models are an explicit networked exception.

Override the defaults with `--input`:

```bash
qt run tau3-airline --input '{"agent_model":"openai:gpt-4.1","user_model":"openai:gpt-4.1","task_ids":["0"],"num_trials":1}'
```

Inspect or compare the resulting runs:

```bash
qt show <run_id>
qt compare <run_id-a> <run_id-b>
```

This minimal adapter stores the complete upstream result inside one Quantiles step. It does not create one step per simulation, stream message or tool-call events, provide an external-agent bridge, or isolate the Python process from the host.

## Test

The focused tests mock the τ³ runner and do not call a model provider:

```bash
uv run pytest
uv run ruff check .
uv run ruff format . --check
```
