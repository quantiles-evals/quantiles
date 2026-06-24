# Configuration Guide

Quantiles uses a single TOML configuration file in the current working directory to describe how benchmarks and custom evaluations are executed.

## When you need a config file

You need a config file when you want to do one or more of the following:

- Override built-in benchmark defaults (e.g. model, sample limit)
- Define custom evaluations (`type = "custom_code"`)
- Resume custom evaluations later with `qt resume <run_id>`

You don't, however, need any configuration when you want to run built-in benchmarks. `qt run pubmedqa`, for example, works out of the box.

## File location and name

If you want to make a configuration file, create either a `quantiles.toml` or `.quantiles.toml` file in the current working directory. You can only have one or the other -- if both exist, the CLI exits with an ambiguity error.

## Top-level structure

Every benchmark lives under its own `[benchmarks.<eval_name>]` section. The section name is the eval name that you will pass to `qt run <eval_name>`.

For example, if you want to override default parameters for the built-in PubMedQA benchmark, add the following section to your config file:

```toml
# This is configuration for the PubMedQA benchmark that is built into the `qt` CLI.
# 
# When you run `qt run pubmedqa`, the CLI will look here for how exactly to run PubMedQA.
[benchmarks.pubmedqa]
# Limit to the first 50 samples of the benchmark
samples = 50
# Customize the model to test
model = "openai:gpt-5.4-nano"
# see below for the full reference
```

Alternatively, if you want to build your own custom evaluation, using the Quantiles Python or TypeScript SDK, define that custom eval as follows:

```toml
# This is configuration for a custom code benchmark called my-eval.
# 
# When you run `qt run my-eval`, the CLI will look here for how exactly to run your custom
# benchmark.
[benchmarks.my-eval]
# This `type` parameter defaults to "builtin". for custom evals, be sure to override
# the default with "custom_code" here.
type = "custom_code"
# This is the command that the CLI will execute to run your custom eval code.
command = ["python", "my_eval.py"]
# see below for the full reference
```

## Benchmark types

Every benchmark section has a `type` field. Valid values are `"builtin"` (default when absent) and `"custom_code"`.

### `builtin`

Built-in benchmarks run natively inside the CLI, without any custom code. Below is a list of parameters that can be customized for built-in benchmarks:

| Field | Type | Required | Description |
|--|--|--|--|
| `type` | string | no | Defaults to `"builtin"`. May be omitted for built-in benchmarks. |
| `samples` | integer | no | Number of dataset rows to evaluate. |
| `model` | string or table | no | Model sampler. See [model format](#model-format). |
| `max_workers` | integer | no | Maximum concurrent workers. |

If none of these fields are customized, the built-in benchmark uses the following defaults:

- `type`: `builtin`
- `samples`: All samples available in the benchmark's dataset, in order
- `model`: The "demo" model, which outputs random values
- `max_workers`: The default parallelism provided by the Rust [Tokio runtime](https://tokio.rs/)

#### `model` naming

The above `model` field accepts a provider-prefixed string, for example

```toml
model = "openai:gpt-5.4-nano"
```

Supported provider prefixes are listed below:

- `openai:`
- `anthropic:`
- `gemini:`
- `cloudflare:`

You can pass a TOML table instead of such a prefixed string:

```toml
model = { provider = "openai", model_id = "gpt-5.4-nano" }
```

Note that models require specific configuration based on the provider. For details, see the `quantiles.toml` file under the provider of your choice in [`cli/examples/configs`](./cli/examples/configs).

### `custom_code`

Custom evaluations are external programs built with one of the Quantiles SDKs. Their config sections contain the following fields:

| Field | Type | Required | Description |
|--|--|--|--|
| `type` | string | yes | Must be `"custom_code"`. |
| `command` | array of strings | yes | Command and arguments to execute. |
| `input` | table | no | Structured input passed to the child as `QUANTILES_INPUT`. |

Note that custom code evaluations can customize the model in code. See the [PubMedQA custom code example](./python/examples/pubmedqa.py) for details on customizing the model in `custom_code` benchmarks.

#### The `input` table

For `custom_code` benchmarks, `input` is an arbitrary TOML table that becomes a JSON object in the custom eval (e.g. a `dict` in Python and a `Map` in TypeScript):

```toml
[benchmarks.my-eval]
type = "custom_code"
command = ["python", "eval.py"]
input = {
    dataset_path = "my_data.jsonl",
    max_samples = 100,
    nested = { foo = "bar" }
}
```

This produces:

```json
{
  "dataset_path": "my_data.jsonl",
  "max_samples": 100,
  "nested": {
    "foo": "bar"
  }
}
```

Quantiles passes these values through to your custom program. It does not assign special meaning to `dataset_path` or load the file automatically; your custom code is responsible for opening the file directly or using the value to construct a dataset source.

#### CLI `--input` overrides

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

- `builtin` sections may not contain `command` or `input` fields.
- `custom_code` sections must adhere to the following rules:
  - They must have a non-empty `command` array.
  - They may not contain `builtin`-only fields like `samples` or `model`.
- The `type` field must be set to `builtin` or `custom_code`.

Validation failures produce clear error messages before any run is created.

## `qt resume` and the config file

When you run `qt resume <run_id>`, the CLI looks up the stored eval name and input from the database, then re-reads the command from the current config file. This means:

- `qt resume` provides no `--input` flag, and you do not need to re-submit input parameters on resume.
- If you edited the config file between `qt run` and `qt resume`, the resumed run uses the _updated_ command.
- If the config section is removed after a `qt run`, resuming a `custom_code` benchmark fails with "no config section found".

## Complete examples

### Built-in with model override

```toml
[benchmarks.pubmedqa]
model = "openai:gpt-5.4-nano"
```

### Built-in using demo model, and restricting samples

```toml
[benchmarks.simpleqa-verified]
samples = 10
```

### Custom evaluation

```toml
[benchmarks.hello]
type = "custom_code"
command = ["python3", "hello.py"]
input = { greeting = "world" }
```

### Custom evaluation with failure simulation

See [`cli/examples/configs/custom_code/quantiles.toml`](./cli/examples/configs/custom_code/quantiles.toml) for a complete, commented example including a sample Python script.
