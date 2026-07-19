# Configuration Guide

Quantiles uses a single `quantiles.toml` or `.quantiles.toml` file in the current working directory to configure built-in benchmarks and define custom evaluations. The file specifies how evaluations are loaded and executed, including datasets, models, prompts, scoring methods, inputs, and runtime settings.

> Only one of the two filenames can exist in the same directory. If both exist, the CLI will exit with an error.

## When to use a configuration file

Create or configure a `quantiles.toml` or `.quantiles.toml` configuration file when you want to do any of the following:

- Override built-in benchmark defaults (e.g., model or sample limit).
- Build custom no-code evaluations (`type = "custom_nocode"`).
- Build custom evaluations with Python (`type = "custom_code"`).
- Resume custom evaluations later with `qt resume <run_id>`.

> Note: Built-in benchmarks can run without a configuration file using their default settings. For example, `qt run simpleqa-verified` runs the full SimpleQA Verified benchmark with the demo model.

## File location and name

The `qt` CLI looks for either `quantiles.toml` or `.quantiles.toml` in the current working directory. If neither exists, it searches ancestor directories and uses the first matching file it finds. To configure Quantiles for your project, we recommend adding one of these files to the project's working directory.

## Top-level structure

Every evaluation definition or built-in override lives under its own `[benchmarks.<eval_name>]` section. The section key is the evaluation name passed to `qt run <eval_name>`.

For example, if you want to override default parameters for the built-in PubMedQA benchmark, add the following section to your configuration file:

```toml
# Configure the model and sample limit for the built-in PubMedQA benchmark.

[benchmarks.pubmedqa]
samples = 50
model = "openai:gpt-5.6"
```

## Evaluation types

Each evaluation section supports a `type` field. If omitted, `type` defaults to `"builtin"`. Valid values are:

- `"builtin"` (default): a benchmark built into the CLI.
- `"custom_code"`: a custom evaluation implemented in Python using the [Python SDK](https://quantiles.io/documentation/reference/python-sdk).
- `"custom_nocode"`: a custom evaluation defined entirely in the configuration file.

### `builtin` benchmarks

Built-in benchmarks run directly through the CLI with the demo model by default and require no custom code. You can override the defaults using the configuration parameters below.

| Field         | Type            | Required | Description                                                                                    | Default                                                                          |
| ------------- | --------------- | -------- | ---------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `type`        | string          | no       | The type of the benchmark. See above for valid values.                                         | `builtin`                                                                        |
| `samples`     | integer         | no       | Number of dataset rows to evaluate.                                                            | All samples available in the benchmark's dataset in the order they appear.       |
| `model`       | string or table | no       | The model to use. See [model naming](#naming-the-model) for details on configuring your model. | Benchmark-specific demo model.                                                   |
| `max_workers` | integer         | no       | Maximum concurrent workers.                                                                    | The default parallelism provided by the Rust [Tokio runtime](https://tokio.rs/). |

### `custom_nocode` evaluations

Custom no-code evaluations run natively inside the CLI and require no Python implementation. Define one with `type = "custom_nocode"` and a `style` parameter. Setting `style.type = "exact_match"` scores an open answer or label against a golden-answer column. Setting `style.type = "multiple_choice"` normalizes choices, extracts the selected label from the response, and scores it against a configured label, index, or correct-choice column. The following example defines an exact-match evaluation:

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

> Note: With `style.type = "exact_match"`, `model = "random"` generates random text and will typically produce very low accuracy. With `style.type = "multiple_choice"`, it samples uniformly from `style.choice_labels`, so accuracy will usually be higher. In both cases, the demo model is intended only to validate the evaluation workflow.

The following table lists the required and optional fields supported by `custom_nocode` configuration sections.

| Field                                | Type             | Required    | Description                                                                                                                                                                 |
| ------------------------------------ | ---------------- | ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `type`                               | string           | yes         | Selects the no-code custom evaluation runner. Must be `"custom_nocode"`.                                                                                                    |
| `style`                              | table            | yes         | Defines how responses are parsed and scored, along with the fields required by the selected scoring method.                                                                 |
| `style.type`                         | string           | yes         | Scoring method: `"exact_match"` compares the trimmed response with a golden answer, while `"multiple_choice"` extracts and scores a configured choice label.                |
| `dataset`                            | table            | yes         | Identifies the Hugging Face dataset to evaluate, including its optional subset, split, and revision.                                                                        |
| `dataset.name`                       | string           | yes         | Hugging Face dataset ID, such as `"quantiles/simpleqa-verified"`.                                                                                                           |
| `dataset.config_name`                | string           | no          | Dataset configuration or subset to load. If omitted, Quantiles infers the configuration.                                                                                    |
| `dataset.split`                      | string           | no          | Dataset split to evaluate. If omitted, Quantiles prefers `test`, `validation`, `eval`, or `train`, in that order, then uses the first available split.                      |
| `dataset.revision`                   | string           | no          | Dataset branch, tag, or commit to load. If omitted, Hugging Face uses the dataset's default revision.                                                                       |
| `model`                              | string or table  | no          | Model or sampler used to generate responses. If omitted, Quantiles uses its built-in random demo sampler. See [model naming](#naming-the-model) for provider-backed models. |
| `prompt_template_file`               | string           | yes         | Path to an existing Jinja prompt template. The template can access the complete dataset `row` and, for multiple-choice evaluations, normalized `choices`.                   |
| `style.golden_column`                | string           | conditional | Dataset column containing the expected answer for exact-match scoring. Required when `style.type = "exact_match"`.                                                          |
| `style.choices`                      | table            | conditional | Defines where multiple-choice options come from. Specify exactly one of `style.choices.column` or `style.choices.columns`.                                                  |
| `style.choices.column`               | string           | conditional | Dataset column containing the choices as an ordered array or an object keyed by the configured choice labels.                                                               |
| `style.choices.columns`              | array of strings | conditional | Ordered list of dataset columns whose scalar values become the choices. Each column maps to the choice label at the same position.                                          |
| `style.answer`                       | table            | conditional | Defines how Quantiles determines the correct multiple-choice option. Specify exactly one supported answer source.                                                           |
| `style.answer.label_column`          | string           | conditional | Dataset column containing the correct choice label. Values are trimmed and matched case-insensitively against `style.choice_labels`.                                        |
| `style.answer.index_column`          | string           | conditional | Dataset column containing the numeric position of the correct choice before optional shuffling.                                                                             |
| `style.answer.index_base`            | integer          | no          | Starting index used by `style.answer.index_column`, such as `0` for zero-based indexes or `1` for one-based indexes. Defaults to `0`.                                       |
| `style.answer.correct_choice_column` | string           | conditional | Name of the entry in `style.choices.columns` that always contains the correct answer. Only supported with column-backed choices.                                            |
| `style.choice_labels`                | array of strings | conditional | Unique labels assigned to choices in order, such as `["A", "B", "C", "D"]`. Required for multiple-choice evaluations.                                                       |
| `style.shuffle`                      | table            | no          | Enables deterministic per-row shuffling of choices before labels are assigned.                                                                                              |
| `style.shuffle.seed_column`          | string           | conditional | Dataset column containing a stable row-specific value used to reproduce the same shuffled order. Required when `style.shuffle` is configured.                               |
| `limit`                              | integer          | no          | Maximum number of dataset rows to evaluate. Defaults to all available rows and must be greater than zero.                                                                   |
| `max_workers`                        | integer          | no          | Maximum number of dataset rows evaluated concurrently. If omitted, Quantiles uses its configured runtime default.                                                           |

Each sample emits `is_correct`, `response_parsed`, and `latency_ms`. For exact-match benchmarks, every response is considered parsed; for multiple-choice benchmarks, `response_parsed` is `0` when the response cannot be parsed as a configured choice label.

Each run emits these aggregate metrics:

- `accuracy`, `correct_count`, `incorrect_count`, and `total_count`
- `parsed_response_count`, `unparsed_response_count`, and `parse_rate`
- `mean_latency_ms`, `median_latency_ms`, `p95_latency_ms`, `p99_latency_ms`, `min_latency_ms`, and `max_latency_ms`

Multiple-choice runs also emit the following aggregate metrics:

- `macro_precision`, `macro_recall`, and `macro_f1` are the arithmetic means of the corresponding per-label metrics. Every configured label has equal weight, regardless of how often it appears as the golden answer.
- `weighted_precision`, `weighted_recall`, and `weighted_f1` average the corresponding per-label metrics using each label's golden-answer support (e.g., the number of samples whose golden answer is that label). These metrics therefore account for imbalanced label frequencies.
- Per-label `precision_label_N`, `recall_label_N`, and `f1_label_N` evaluate label `N` as the positive class against all other labels. `support_label_N` is the number of samples whose golden answer is that label, and `N` is the label's index in `style.choice_labels`.
- `confusion_matrix_G_P` is the number of samples whose golden label has index `G` and whose parsed prediction has index `P`. These cells form the parsed-label columns of the run's confusion matrix.
- `confusion_matrix_G_unparsed` is the number of samples whose golden label has index `G` but whose response could not be parsed as a configured label. These cells form the confusion matrix's additional unparsed-prediction column.

Per-label precision, recall, F1, and support metrics and all confusion-matrix metrics are stored with the run but omitted from normal CLI output, `qt run --json`, and comparisons. Use `qt show <run_id> --json` to inspect them.

Precision, recall, and F1 are reported as `0` when their denominator is zero. The confusion matrix uses golden labels as rows and predicted labels, plus the unparsed bucket, as columns.

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

See the [sample custom_nocode configuration](./custom-nocode-examples/quantiles.toml) for a complete example.

### `custom_code` evaluation

Custom code evaluations are built with the [Quantiles Python SDK](https://quantiles.io/documentation/reference/python-sdk). Their configuration sections contain the following fields:

| Field     | Type             | Required | Description                                                                        |
| --------- | ---------------- | -------- | ---------------------------------------------------------------------------------- |
| `type`    | string           | yes      | Must be `"custom_code"`.                                                           |
| `command` | array of strings | yes      | Command and arguments to execute.                                                  |
| `input`   | table            | no       | Structured input passed to your code in the `QUANTILES_INPUT` environment variable |

Custom-code evaluations can select and configure models in Python. See the [PubMedQA custom-code example](./python-examples/src/pubmedqa.py) for an implementation.

Below is an example of a `custom_code` evaluation in the configuration file:

```toml
[benchmarks.my-custom-eval]
type = "custom_code"
command = ["python", "eval.py"]

[benchmarks.my-custom-eval.input]
dataset = "my_data.jsonl"
max_samples = 100
nested = { foo = "bar" }
```

Run it with:

```bash
qt run my-custom-eval
```

See the complete [custom-code configuration example](./cli/examples/configs/custom_code/quantiles.toml) for an annotated configuration and failure-simulation script.

#### The `input` table

For `custom_code` evaluations, `input` is an arbitrary TOML table that becomes a Python `dict`. The input table above produces the following dictionary:

```python
{
  "dataset": "my_data.jsonl",
  "max_samples": 100,
  "nested": {
    "foo": "bar"
  }
}
```

### Naming the `model`

The `model` field described above accepts a provider-prefixed string, for example:

```toml
model = "openai:gpt-5.6"
```

Supported provider prefixes are:

- `openai:`
- `anthropic:`
- `gemini:`
- `cloudflare_ai_gateway:`

We recommend using the provider-prefixed string, but you can also use a TOML table:

```toml
model = { provider = "openai", model_id = "gpt-5.6" }
```

Most models require specific configuration based on the provider. For provider-specific details, see the `quantiles.toml` file under the provider of your choice in the [provider configuration examples](./cli/examples/configs).

### CLI `--input` overrides

Pass `--input` to `qt run` to override or extend configured inputs at runtime. These values are persisted to the local database but are not written to the configuration file. They apply only to the current invocation, so define values shared by all runs in the configuration file. For example, the following command overrides the model:

```bash
qt run my-eval --input '{"model":"openai:gpt-5.6"}'
```

The CLI merges the `--input` JSON object into the configuration file's `input` table. If a key exists in both, the CLI value wins and the CLI prints a warning:

```
Warning: --input overrides config input for keys: model
```

In `--json` mode, the warning is included in the JSON output under the `warning` key.

For `custom_nocode` evaluations, `--input` may override only `model`, `limit`, and
`prompt_template_file`. The CLI merges those fields with the evaluation's complete
configuration for the current run. Any other field causes the command to fail. For
example, this runs a configured no-code evaluation with a different model and a
smaller sample limit while preserving its dataset and scoring configuration:

```bash
qt run gpqa --input '{"model":"openai:gpt-5.6","limit":10}'
```

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
  - When `style.type = "exact_match"`, a `style.golden_column` field must be present.
  - When `style.type = "multiple_choice"`, a valid choice source, answer source, and a non-empty list of unique `choice_labels` must be present in the `style` dictionary.
  - May not contain `command`, `input`, or other unsupported fields.

Validation failures produce clear error messages before any run is created.

## `qt resume` and the configuration file

When you run `qt resume <run_id>`, the CLI looks up the stored evaluation name and input, then reloads the current configuration file. This means:

- `qt resume` has no `--input` flag. Built-in and `custom_code` evaluations reuse the stored run input.
- A resumed `custom_code` evaluation uses the current `command` value. If the command changed after the original run, the resumed run executes the updated command.
- A resumed `custom_nocode` evaluation uses its current configuration. Keep that configuration unchanged when resuming; start a new run after intentional dataset, model, prompt, scoring, or runtime changes.
- If a custom evaluation's configuration section has been removed, the run cannot be resumed.
