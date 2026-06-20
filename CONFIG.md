# Configuration Guide

Quantiles uses a single TOML configuration file in the current working directory to describe how benchmarks and custom evaluations are executed. The CLI looks for `quantiles.toml` or `.quantiles.toml` in the current working directory. If both exist, the CLI exits with an ambiguity error.

## When you need a config file

You don't need a config file to run built-in benchmarks. `qt run pubmedqa` works out of the box. You do, however, need a config file when you want to do one or more of the following:

- Override built-in benchmark defaults (e.g. model, sample limit)
- Define custom evaluations (`type = "custom_code"`)
- Resume custom evaluations later with `qt resume <run_id>`

## File location and name

The CLI looks for a configuration file in the current working directory, in this order:

1. `./quantiles.toml`
2. `./.quantiles.toml`

Only one may exist in a given directory.

## Top-level structure

Every benchmark lives under the `[benchmarks.<name>]` section. The section name is the workflow name you pass to `qt run <name>`.

```toml
[benchmarks.pubmedqa]
# builtin fields...

[benchmarks.my-eval]
type = "custom_code"
# custom_code fields...
```

## Benchmark types

Every benchmark section has a `type` field. Valid values are `"builtin"` (default when absent) and `"custom_code"`.

### `builtin`

Built-in benchmarks run natively inside the CLI. Their config sections may contain:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | no | Defaults to `"builtin"`. May be omitted for built-in benchmarks. |
| `samples` | integer | no | Number of dataset rows to evaluate. |
| `model` | string or table | no | Model sampler. See [model format](#model-format). |
| `max_workers` | integer | no | Maximum concurrent workers. |

If none of these fields are present, the built-in uses its own defaults and no config JSON is generated.

### `custom_code`

Custom evaluations are external programs run as child processes and generally built with one of the Quantiles SDKs. Their config sections contain the following fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | yes | Must be `"custom_code"`. |
| `command` | array of strings | yes | Command and arguments to execute. |
| `input` | table | no | Structured input passed to the child as `QUANTILES_INPUT`. |

The CLI injects these environment variables into the child process:

- `QUANTILES_RUN_ID` — the run ID
- `QUANTILES_WORKFLOW_NAME` — the benchmark name
- `QUANTILES_BASE_URL` — local API base URL
- `QUANTILES_INPUT` — JSON string from the `input` table

## Model format

The `model` field accepts either a provider-prefixed string or a table.

### String form

```toml
model = "openai:gpt-5.4-nano"
```

Supported provider prefixes are listed below:

- `openai:`
- `anthropic:`
- `gemini:`
- `cloudflare:`

### Table form

```toml
model = { provider = "cloudflare", model_id = "@cf/..." }
```

## Input tables

For `custom_code` benchmarks, `input` is an arbitrary TOML table that becomes a JSON object in the custom eval (e.g. a `dict` in Python and a `Map` in TypeScript):

```toml
[benchmarks.my-eval]
type = "custom_code"
command = ["python", "eval.py"]

[benchmarks.my-eval.input]
dataset = "my_data.jsonl"
max_samples = 100

[benchmarks.my-eval.input.nested]
foo = "bar"
```

This produces:

```json
{"dataset":"my_data.jsonl","max_samples":100,"nested":{"foo":"bar"}}
```

An empty input table (`[benchmarks.my-eval.input]` with no keys) deserializes as an empty JSON object.

## CLI `--input` overrides

You can override or extend config input at runtime:

```bash
qt run my-eval --input '{"max_samples":50}'
```

The CLI merges the `--input` JSON object into the config `input` table. If a key exists in both, the CLI value wins and a warning is printed:

```
Warning: --input overrides config input for keys: max_samples
```

In `--json` mode, the warning is included in the JSON output under the `warning` key.

## Config validation

The CLI validates benchmark configs before execution:

- `builtin` sections may **not** contain `command` or `input` fields.
- `custom_code` sections **must** contain a non-empty `command` array.
- `custom_code` sections may **not** contain builtin-only fields like `samples` or `model`.
- Unknown `type` values are rejected.

Validation failures produce clear error messages before any run is created.

## Resuming and the config file

When you run `qt resume <run_id>`, the CLI looks up the stored workflow name and input from the database, then re-reads the command from the current config file. This means:

- `qt resume` provides no `--input` flag, and you do not need to re-submit input parameters on resume.
- If you edited the config file between `qt run` and `qt resume`, the resumed run uses the _updated_ command.
- If the config section has been removed, resuming a `custom_code` benchmark fails with "no config section found".

## Complete examples

### Built-in with model override

```toml
[benchmarks.pubmedqa]
samples = 50
model = "openai:gpt-5.4-nano"
max_workers = 100
```

### Built-in with demo model (no fields needed)

```toml
[benchmarks.simpleqa-verified]
samples = 10
```

### Custom evaluation

```toml
[benchmarks.hello]
type = "custom_code"
command = ["python3", "hello.py"]

[benchmarks.hello.input]
greeting = "world"
```

### Custom evaluation with failure simulation

See [`cli/examples/configs/custom_code/quantiles.toml`](./cli/examples/configs/custom_code/quantiles.toml) for a complete, commented example including a sample Python script.
