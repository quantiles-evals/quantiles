# Configuration Guide

Quantiles uses a single `quantiles.toml` or `.quantiles.toml` file in the current working directory to configure built-in benchmarks and define custom evaluations. The file specifies how evaluations are loaded and executed, including their datasets, models, prompts, scoring methods, inputs, and runtime settings.

> Only one of the two filenames can exist in the same directory. If both exist, the CLI will exit with an error.

## When to use a configuration file

Create or configure a `quantiles.toml` configuration file when you want to do any of the following:

- Override built-in benchmark defaults (e.g. model, sample limit)
- Build custom no-code evaluations (`type = "custom_nocode"` in the configuration file)
- Build custom evaluations with Python code (`type = "custom_code"` in the configuration file)
- Resume custom evaluations later with `qt resume <run_id>`

> Note: Built-in benchmarks can run without a configuration file using their default settings. For example, `qt run simpleqa-verified` runs the full SimpleQA Verified benchmark with the demo model.

## File location and name

The qt CLI looks for either quantiles.toml or .quantiles.toml in the current working directory. If neither exists, it searches ancestor directories and uses the first matching file it finds. The `qt` CLI looks for either `quantiles.toml` or `.quantiles.toml` in the current working directory. If neither exists, it searches ancestor directories and uses the first matching file it finds. To configure Quantiles for your project, we recommend adding one of these files to the project's working directory.

## Top-level structure

Every benchmark lives under its own `[benchmarks.<eval_name>]` section. The section name is the eval name that you will pass to `qt run <eval_name>`.

For example, if you want to override default parameters for the built-in PubMedQA benchmark, add the following section to your configuration file:

```toml
# Configure the model and sample limit for the built-in PubMedQA benchmark.

[benchmarks.pubmedqa]
samples = 50
model = "openai:gpt-5.6"
```

## Evaluation types

Every evaluation section has an optional `type` field. Valid values are as listed below:

- `"builtin"` (default)
- `"custom_code"` - write your custom evaluation in Python, using the [Python SDK](https://quantiles.io/documentation/reference/python-sdk)
- `"custom_nocode"` - build a custom evaluation completely from inside the configuration file, without writing or maintaining any code

### `builtin`

Built-in benchmarks run natively inside the CLI, without any custom code. Below is a list of parameters that can be customized for built-in benchmarks:

| Field | Type | Required | Description | Default |
| --- | --- | --- | --- | --- |
| `type` | string | no | The type of the benchmark. See above for valid values. | `builtin` |
| `samples` | integer | no | Number of dataset rows to evaluate. | All samples avaliable in the benchmark's dataset in the order they appear |
| `model` | string or table | no | The model to use. See [model naming](#model-naming) for details on configuring your model | `random` (the built-in random sampler) |
| `max_workers` | integer | no | Maximum concurrent workers. | The default parallelism provided by the Rust [Tokio runtime](https://tokio.rs/) |

#### `model` naming

The `model` field described above accepts a provider-prefixed string, for example:

```toml
model = "openai:gpt-5.6"
```

Supported provider prefixes are:

- `openai:`
- `anthropic:`
- `gemini:`
- `cloudflare:`

We recommend using the compound string, but if needed, you can pass a TOML table to describe your model:

```toml
model = { provider = "openai", model_id = "gpt-5.6" }
```

Most models require specific configuration based on the provider. For provider-specific details, see the `quantiles.toml` file under the provider of your choice in the [provider configuration examples](./cli/examples/configs).

### `custom_nocode` Evaluations

Custom no-code run natively inside the CLI and are configured in `quantiles.toml`. They do not require any custom Python code. Define one of these evaluations with `type = "custom_nocode"` and a `style` parameter. The `style = "exact_match"` configuration creates an eval that scores an open answer or label against a golden answer column. The `style = "multiple_choice"` configuration normalizes choices, extracts the selected label from the response, and scores it against a configured label, index, or correct-choice column. Below is an example of an `exact_match` no-code evaluation:

```toml
[benchmarks.my_custom_nocode_eval]
type = "custom_nocode"
style = { type = "exact_match", golden_column = "answer" }
dataset = { name = "quantiles/simpleqa-verified" }
model = "random"
prompt_template_file = "prompts/qa.txt"
limit = 10
```

Run it with:

```bash
qt run my_custom_nocode_eval
```

>Note: When you configure `model = "random"` with `"exact_match"`, evals will use the built-in model that generates random text, so you'll likely to get very low accuracy numbers. Similarly, when you configure `model = "random"` with `multiple_choice` evals, the built-in model will uniformly sample from one of the the configured `style.choice_labels`, so you can expect higher accuracies than with `exact_match`. In both cases, `model = "random" is intended for testing your benchmark.

The following fields are expected in `custom_nocode` configuration sections:

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | string | yes | Must be `"custom_nocode"`. |
| `style` | table | yes | Scoring style and its style-specific configuration. |
| `style.type` | string | yes | `"exact_match"` for open-answer or label exact match, or `"multiple_choice"` for choice-based benchmarks. |
| `dataset` | table | yes | Hugging Face dataset coordinates. |
| `dataset.name` | string | yes | Dataset identifier, for example `"quantiles/simpleqa-verified"`. |
| `dataset.config_name` | string | no | Hugging Face dataset configuration or subset. |
| `dataset.split` | string | no | Dataset split. When omitted, Quantiles selects a standard evaluation split. |
| `dataset.revision` | string | no | Dataset revision. |
| `model` | string or table | no | Model sampler. Defaults to the demo random sampler. See [model naming](#model-naming). |
| `prompt_template_file` | string | yes | Path to a Jinja prompt template file. The template receives the complete dataset `row` and, for multiple-choice benchmarks, normalized `choices`. |
| `style.golden_column` | string | conditional | Dataset column containing the golden answer. Required for `exact_match`. |
| `style.choices` | table | conditional | Choice source. Required for `multiple_choice`. Configure exactly one of `style.choices.column` or `style.choices.columns`. |
| `style.choices.column` | string | conditional | Dataset column containing choices as an array or label-keyed object. |
| `style.choices.columns` | array of strings | conditional | Dataset columns containing choices in their original order. |
| `style.answer` | table | conditional | Correct-answer source. Required for `multiple_choice`. Configure exactly one answer-source form. |
| `style.answer.label_column` | string | conditional | Dataset column containing the golden choice label. |
| `style.answer.index_column` | string | conditional | Dataset column containing the golden choice index. |
| `style.answer.index_base` | integer | no | Index base for `style.answer.index_column`. Defaults to `0`. |
| `style.answer.correct_choice_column` | string | conditional | Member of `style.choices.columns` known to contain the correct answer. |
| `style.choice_labels` | array of strings | conditional | Labels assigned to choices in order. Required for multiple choice; array-backed rows may use a prefix of the configured labels. |
| `style.shuffle` | table | no | Enables deterministic choice shuffling for `multiple_choice`. |
| `style.shuffle.seed_column` | string | conditional | Stable row identifier used to seed deterministic shuffling. Required when `style.shuffle` is present. |
| `limit` | integer | no | Number of dataset rows to evaluate. |
| `max_workers` | integer | no | Maximum concurrent workers. |

Each sample emits `is_correct`, `response_parsed`, and `latency_ms`. For exact-match benchmarks, every response is considered parsed; for multiple-choice benchmarks, `response_parsed` is `0` when the response cannot be parsed as a configured choice label.

Each run emits these aggregate metrics:

- `accuracy`, `correct_count`, `incorrect_count`, and `total_count`
- `parsed_response_count`, `unparsed_response_count`, and `parse_rate`
- `mean_latency_ms`, `median_latency_ms`, `p95_latency_ms`, `p99_latency_ms`, `min_latency_ms`, and `max_latency_ms`

Multiple-choice runs also emit the following aggregate metrics:

- `macro_precision`, `macro_recall`, and `macro_f1` are the arithmetic means of the corresponding per-label metrics. Every configured label has equal weight, regardless of how often it appears as the golden answer.
- `weighted_precision`, `weighted_recall`, and `weighted_f1` average the corresponding per-label metrics using each label's golden-answer support (e.g. the number of samples whose correct/"golden" answer is that label). These metrics therefore account for imbalanced label frequencies.
- Per-label `precision_label_N`, `recall_label_N`, and `f1_label_N` evaluate label `N` as the positive class against all other labels. `support_label_N` is the number of samples whose golden answer is that label, and `N` is the label's index in `style.choice_labels`.
- `confusion_matrix_G_P` is the number of samples whose golden label has index `G` and whose parsed prediction has index `P`. These cells form the parsed-label columns of the run's confusion matrix.
- `confusion_matrix_G_unparsed` is the number of samples whose golden label has index `G` but whose response could not be parsed as a configured label. These cells form the confusion matrix's additional unparsed-prediction column.

Precision, recall, and F1 use `0` when their denominator is zero. The confusion matrix uses golden labels as rows and predicted labels, plus the unparsed bucket, as columns.

Multiple-choice configuration keeps its choice and answer sources inside `style`:

```toml
[benchmarks.medqa]
type = "custom_nocode"
style = { type = "multiple_choice", choices = { column = "options" }, choice_labels = ["A", "B", "C", "D"], answer = { label_column = "answer_idx" } }
dataset = { name = "quantiles/MedQA-USMLE-4-options", config_name = "default", split = "test" }
model = "random"
prompt_template_file = "prompts/medqa.txt"
limit = 10
```

Templates access dataset fields directly. A multiple-choice template can iterate the normalized choices:

```jinja
{{ row.question }}

{% for choice in choices %}
{{ choice.label }}. {{ choice.text }}
{% endfor %}
```

See [the `quantiles.toml` sample configuration file](./custom-nocode-examples/quantiles.toml) for examples of real, published benchmarks built with `custom_nocode` sections.

### `custom_code`

Custom evaluations are external programs built with the [Quantiles Python SDK](https://quantiles.io/documentation/reference/python-sdk). Their configuration sections contain the following fields:

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | string | yes | Must be `"custom_code"`. |
| `command` | array of strings | yes | Command and arguments to execute. |
| `input` | table | no | Structured input passed to your code in the `QUANTILES_INPUT`. environment variable |

Custom code evaluations can customize the model in code. See the [PubMedQA custom code example](./python-examples/src/pubmedqa.py) for details on customizing the model in `custom_code` 
evaluations.

Below is an example of a `custom_code` evaluation in the configuration file:

```toml
[benchmarks.my-custom-eval]
type = "custom_code"
command = ["python", "eval.py"]
input = {
    dataset = "my_data.jsonl",
    max_samples = 100,
    nested = { foo = "bar" }
}
```

Run it with:

```shell
qt run my-custom-eval
```


#### The `input` table

For `custom_code` evaluations, `input` is an arbitrary TOML table that becomes a Python `dict` in your custom eval. In the example from the previous section, the following input was defined:


```toml
input = {
    dataset = "my_data.jsonl",
    max_samples = 100,
    nested = { foo = "bar" }
}
```

Given these inputs, your Python code would be passed a dictionary that looks like the following:

```python
{
  "dataset": "my_data.jsonl",
  "max_samples": 100,
  "nested": {
    "foo": "bar"
  }
}
```

#### CLI `--input` overrides

You can pass an `--input` flag to `qt run` to override or extend configured inputs at runtime. These values are persisted to the local database but not written to the configuration file. They apply only to the current `qt run` invocation, so we recommend that you define inputs that should apply to all runs in your `quantiles.toml` file. Below is an example showing how to use the `--input` flag to override the model to use:

```bash
qt run my-eval --input '{"model":"openai:gpt-5.6"}'
```

The CLI merges the `--input` JSON object into the configuration file's`input` table. If a key exists in both, the CLI value wins and a warning is printed that looks like the following:

```
Warning: --input overrides config input for keys: model
```

In `--json` mode, the warning is included in the JSON output under the `warning` key.

## Configuration validation

The CLI validates benchmark configuration sections before execution:

- `built-in` sections:
  - May not contain `command` or `input` fields.
  - May include built-in fields such as `samples`, `model`, and `max_workers`.

- `custom_code` sections:
  - Must have a non-empty `command` array.
  - May not contain built-in-only fields such as `samples` or `model`.

- `custom_nocode` sections:
  - Must include `dataset`, `style`, and `prompt_template_file`.
  - `prompt_template_file` must point to an existing file.
  - Must set `style.type` to either `exact_match` or `multiple_choice`.
  - When `style.type = "exact_match"`, a non-empty `style.golden_column` field must be present.
  - When `style.type = "multiple_choice:"`, a valid choice source, answer source, and a non-empty list of unique `choice_labels` must be present in the `style` dictionary.
  - May not contain `command`, `input`, or other unsupported fields.

Validation failures produce clear error messages before any run is created.

## `qt resume` and the configuration file

When you run `qt resume <run_id>`, the CLI looks up the stored evaluation name and input from the database, then re-reads the command from the current config file. This means:

- `qt resume` provides no `--input` flag, and on resume, you do not need to re-submit parameters originally passed in the `--input` flag.
- For `custom_code` evaluations, if you edited the `command` field in your configuration file between `qt run` and `qt resume`, the resumed run uses the _updated_ command.
- If you removed your evaluation's configuration section after a `qt run`, then run a `qt resume` for that evaluation, the run fails with an error indicating the evaluation doesn't exist.

## Complete examples

### Built-in benchmark with model override

```toml
[benchmarks.pubmedqa]
model = "openai:gpt-5.6"
```

### Built-in benchmark using the demo model with a sample limit

```toml
[benchmarks.simpleqa-verified]
samples = 10
```

### Custom no-code evaluation

```toml
[benchmarks.custom_nocode_eval]
type = "custom_nocode"
style = { type = "exact_match", golden_column = "answer" }
dataset = { name = "quantiles/custom-nocode-dataset" }
model = "openai:gpt-5.6"
prompt_template_file = "prompts/custom_nocode_prompt.txt"
```

### Custom code evaluation

```toml
[benchmarks.custom_code_eval]
type = "custom_code"
command = ["python3", "hello.py"]
input = { greeting = "world" }
```

### Custom evaluation with failure simulation

See the complete [custom-code configuration example](./cli/examples/configs/custom_code/quantiles.toml) for an annotated configuration and sample Python script.
