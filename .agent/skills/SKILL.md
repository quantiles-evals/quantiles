# Quantiles eval workflows

Use this skill for Quantiles AI evaluation work. The `qt` CLI is the canonical entrypoint for running benchmarks and evaluations, inspecting run results, comparing runs, resuming interrupted workflows, and executing custom Python or TypeScript eval workflows.

Prefer `qt` CLI commands over manually reading local Quantiles storage files unless the CLI output is insufficient.

Always pass `--json` when using `qt run`, `qt show`, or `qt compare` so results can be parsed reliably.

## When to use this skill

Use this skill when the user asks to:

- Run a Quantiles evaluation
- Run a built-in Quantiles benchmark
- Inspect or analyze one or more Quantiles eval runs
- Compare two Quantiles eval runs
- Resume a failed or interrupted Quantiles workflow
- Debug failed samples, metrics, scorers, or run outputs
- Write a new custom eval using the Quantiles Python SDK or TypeScript SDK
- Convert an ad-hoc Python or TypeScript eval script into a durable Quantiles workflow

## Do not use this skill for

Do not use this skill for:

- General statistics questions about quantiles, percentiles, medians, or distributions
- Non-Quantiles eval frameworks unless the user asks to convert them into Quantiles workflows
- Reporting demo sampler runs as valid model-quality benchmark results
- Manually editing Quantiles run storage unless the user explicitly asks and the CLI cannot do the task

## Core rules

Follow these rules for all Quantiles work:

1. Use the `qt` CLI as the source of truth.
2. Use `--json` for `qt run`, `qt show`, and `qt compare`.
3. Report the exact command used.
4. Report the `run_id` after every successful run.
5. Do not claim a demo sampler run measures real model quality.
6. Do not run external provider-backed evals unless the user asked for a real model run or provided a model name.
7. For external model runs, verify the required provider API key is configured without printing the key value.
8. For small sample counts, describe the run as a smoke test, not a statistically reliable benchmark.
9. Before saying one run is better than another, check whether the runs are comparable.
10. Never print, log, or expose API key values.

## Preflight checks

Before running Quantiles commands, check whether `qt` is installed:

```bash
command -v qt && qt --version
```

If `qt` is missing, do not install it unless the user asked for setup or the task cannot proceed without installation.

If installation is needed, use the project’s documented install command when available. Otherwise, the standard Quantiles CLI install command is:

```bash
curl -fsSL https://cli.quantiles.io/install.sh | bash
```

After installation, verify that `qt` is available:

```bash
command -v qt && qt --version
```

If the repository has not been initialized and the user asked to run or create Quantiles evals, initialize it:

```bash
qt init
```

Do not run `qt init` repeatedly without checking whether the repository is already initialized.

## Secrets and cost safety

Provider-backed evals may cost money. Demo-sampler evals are safe for smoke testing.

Only run a provider-backed eval when the user asks for a real model run or provides a model name. Never print API key values. Check whether the required credential is present without revealing it:

```bash
test -n "$OPENAI_API_KEY" && echo "OPENAI_API_KEY is set"
test -n "$ANTHROPIC_API_KEY" && echo "ANTHROPIC_API_KEY is set"
test -n "$GEMINI_API_KEY" && echo "GEMINI_API_KEY is set"
```

Use the provider prefix in `model` to decide which key to check:

- `openai:<model>` requires `OPENAI_API_KEY`
- `anthropic:<model>` requires `ANTHROPIC_API_KEY`
- `gemini:<model>` requires `GEMINI_API_KEY`

If the required key is missing, stop before running the provider-backed eval and report which environment variable is missing. If the selected provider uses a different key, use the provider-specific environment variable required by that model or SDK.

If the user asks for a real model run but does not specify a sample count, start with a small smoke-test limit unless the user explicitly asks for a larger run.

## Built-in eval reference

The `qt` CLI includes built-in evals such as:

| Eval                | Purpose                                                                     |
| ------------------- | --------------------------------------------------------------------------- |
| `pubmedqa`          | Evaluates model performance on standard healthcare knowledge questions      |
| `simpleqa-verified` | Evaluates general factual knowledge using an updated SimpleQA-style dataset |
| `financebench`      | Evaluates open-book financial question answering over public-company filings |

Run a built-in eval with:

```bash
qt run <eval-name> --json
```

Example:

```bash
qt run pubmedqa --json
```

For initial workflow validation, prefer a small local smoke test:

```bash
qt run simpleqa-verified --input '{"limit":10}' --json
```

When no real model is specified, built-in evals may use the default demo sampler. Demo sampler runs are useful for validating the CLI, run history, JSON output, metrics, and sample-level result shape. Do not treat demo sampler results as valid model-quality benchmark results.

## Limiting sample count

For built-in evals, pass a JSON `limit` key through `--input`.

Example:

```bash
qt run simpleqa-verified --input '{"limit":100}' --json
```

Use small limits for smoke tests and larger limits only when the user wants a more complete benchmark run.

For provider-backed model runs, use a small limit first unless the user explicitly requests a larger run.

## JSON output

Always pass `--json` to:

```bash
qt run <eval-name> --json
qt show <run_id> --json
qt compare <run_id_1> <run_id_2> --json
```

`qt run` returns structured data that usually includes:

- `run_id`
- workflow or benchmark name
- input JSON
- command used
- status
- success or failure
- exit code, if present
- duration, if present
- summary metrics, if available
- stdout, stderr, or errors, if available

Use the returned `run_id` to inspect the run later:

```bash
qt show <run_id> --json
```

Do not paste large raw JSON into the response. Summarize the important fields.

## Customizing the model

If the user wants the demo sampler, omit `model` and let the CLI use its default. Do not explicitly specify `demo-builtin` as a model.

For provider-backed runs, pass a provider-prefixed `model` value:

- `"model":"openai:<model>"`
- `"model":"anthropic:<model>"`
- `"model":"gemini:<model>"`

A typical provider-backed run looks like this:

```bash
qt run simpleqa-verified --input '{"limit":100,"model":"openai:gpt-4o-mini"}' --json
```

Before running a provider-backed eval, follow the credential checks in Secrets and cost safety. For shell command examples, use the project’s documented default model when available; otherwise, use a concrete provider-prefixed model string.

If the correct input key is unclear, inspect the CLI help or local documentation before running the command:

```bash
qt run --help
```

After running, report whether the run used the demo sampler or a real provider-backed model.

## Inspecting and analyzing evals

If an eval has already run, use its `run_id` with `qt show`:

```bash
qt show <run_id> --json
```

`qt show` returns structured information about the run. Use it to summarize:

- Run ID
- Workflow or benchmark name
- Input JSON
- Model or sampler
- Status
- Success or failure
- Exit code, if present
- Duration, if present
- Aggregate metrics
- Sample count
- Sample-specific metrics, inputs, and outputs
- Failed, errored, or low-scoring samples
- Notable stdout, stderr, or error messages

When the user asks what happened in a run, inspect both aggregate metrics and sample-level results.

## Sample-level analysis

When analyzing a run, use this process:

1. Read aggregate metrics first.
2. Identify failed, errored, abstained, hedged, or low-scoring samples.
3. Group failures by likely cause:
   - model answer wrong
   - model abstained, refused, or hedged
   - grader or scorer issue
   - prompt or input issue
   - provider API issue
   - runtime or dependency error
   - ambiguous dataset item
   - expected-answer mismatch

4. Summarize representative sample IDs.
5. Recommend a next action:
   - inspect specific samples
   - compare against another run
   - rerun with a larger sample count
   - fix the prompt
   - fix the scorer
   - fix runtime dependencies
   - run a provider-backed eval instead of a demo sampler eval

For small sample counts, call the analysis preliminary.

## Listing runs

If the user does not provide a run ID, list recent runs:

```bash
qt list
```

Use `qt list` to find:

- the most recent run
- two runs to compare
- failed or interrupted runs
- runs for a specific benchmark or workflow

If there are many runs, narrow by benchmark name, workflow name, timestamp, status, or model when available.

ASK AARON SHOULD LIST HAVE JSON??
`qt list` currently returns human-readable output and does not support `--json`. Use it only to identify candidate run IDs, then use `qt show <run_id> --json` for structured inspection.

## Comparing evals

The `qt` CLI can compare two eval runs. To compare runs, first identify the two `run_id` values.

If the user does not provide run IDs, find recent runs:

```bash
qt list
```

Then compare the two runs:

```bash
qt compare <run_id_1> <run_id_2> --json
```

Before saying one run is better, check whether the comparison is apples-to-apples.

Check:

- Same benchmark or workflow
- Same dataset or dataset version
- Same sample count
- Same split or sample selection, if available
- Same scorer or grader
- Same metric definitions
- Same model-vs-demo sampler setup
- Same input JSON except for the intended changed variable
- Same provider settings, temperature, seed, or decoding parameters, if available

If benchmark names differ, tell the user that the comparison may not be apples-to-apples because the runs may measure different model functionality and behaviors.

If sample counts are small, say the comparison is directional and should be confirmed with a larger run.

When comparing, report:

- Run IDs
- What changed between the runs
- Metrics that improved
- Metrics that regressed
- Error or failure differences
- Sample-level differences, if available
- Whether the result is meaningful, directional, or only a smoke-test comparison
- Recommended next command

## Resuming interrupted runs

If a run failed or was interrupted, find the run:

```bash
qt list
```

Inspect it:

```bash
qt show <run_id> --json
```

If the installed CLI supports resume, resume the run instead of starting from scratch.

Common resume pattern:

```bash
qt run <workflow-name> --resume <run_id> --json
```

For custom evals, preserve the workflow name, input JSON, and command unless the user explicitly wants a new run:

```bash
qt run <workflow-name> --resume <run_id> --input '<same-input-json>' --json -- <command>
```

If the installed CLI uses a different resume syntax, check:

```bash
qt run --help
```

Then follow the installed CLI’s resume instructions.

When resuming, report:

- Original run ID
- Resume command
- Whether completed steps were reused
- Which steps reran, if available
- Final status
- Any remaining errors

## Deleting evals

It is not currently possible to delete Quantiles eval runs through the CLI.

If the user asks to delete or remove eval runs, explain that deletion is not available. Offer to hide, filter, or exclude that group from displayed results instead, such as hiding failed runs or filtering the run list by status.

Do not manually delete or modify Quantiles local storage files unless the user explicitly asks and understands that this may corrupt run history.

## Custom evals

Use this section only when the user asks to write, modify, or convert a custom Quantiles eval.

Like built-in evals, custom evals are run with the `qt` CLI. The `qt run` command starts a local Quantiles runtime, injects run metadata into the subprocess, records results, and tears down when done.

Prefer the SDK and patterns already used by the repository.

Before writing code, inspect the project:

```bash
find . -maxdepth 3 -type f \( -name "pyproject.toml" -o -name "requirements.txt" -o -name "uv.lock" -o -name "package.json" -o -name "bun.lockb" -o -name "pnpm-lock.yaml" \)
find . -maxdepth 4 -type f | grep -Ei 'quantiles|eval|benchmark'
```

Do not invent SDK API details when local examples are available. Follow the local examples and dependency versions.

## Choose an SDK

Quantiles supports multiple ways to write evals. Prefer the SDK and pattern already used by the repository.

| Approach                      | Best for                                                                                      | Typical entry point                                              |
| ----------------------------- | --------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| Python SDK in `sdk/`          | Full-featured evals with dataset loading, LLM sampling, scoring, and export                   | `sdk/src/quantiles/examples/<eval>/__main__.py`                  |
| Python SDK in `python/`       | Lightweight, qt-native workflows with durable steps, emitted metrics, and local observability | `python/examples/<eval>.py`                                      |
| TypeScript SDK                | Frontend-integrated, Node-based, or TypeScript-first evals                                    | Existing project TypeScript eval files or documentation examples |

If the user is working inside `sdk/`, follow the existing legacy Python SDK patterns.

If the user is working inside `python/`, follow the newer qt-native Python SDK patterns.

Use TypeScript when the repository is TypeScript-first, the eval lives in a Node or Next.js workflow, or the user explicitly asks for TypeScript.

## Python custom eval guidance

Use Python when:

- The repository is Python-first
- The eval needs dataset loading, scoring, model calls, or analysis in Python
- The user already has an ad-hoc Python eval script
- The project uses `uv`, `pyproject.toml`, or Python Quantiles examples

A Python Quantiles eval should generally include:

- A workflow name
- An async handler
- Input JSON parsing
- Deterministic sample IDs
- Durable `step()` calls for per-sample work
- `emit()` calls for metrics
- A returned JSON summary
- `entrypoint()` registration

Important rules:

- Workflow names must be stable and unique within the file.
- `entrypoint()` uses `QUANTILES_WORKFLOW_NAME` from the CLI.
- `step()` caches by `step_key`.
- Use deterministic step keys, usually based on sample IDs.
- Include all cache-invalidating values in step inputs.
- `emit()` writes metrics to the local Quantiles run store so they appear in `qt show` and `qt compare`.
- Do not call dataset-loading helpers at module import time if they require runtime context.
- If using `dataset()`, call it inside the handler when it needs `WorkflowContext`.
- Keep provider credentials in environment variables, not source code.

Run a Python custom eval with:

```bash
qt run <workflow-name> --input '{"key":"value"}' --json -- uv run python <eval-file>.py
```

Example:

```bash
qt run my-eval --input '{"limit":25}' --json -- uv run python my_eval.py
```

If the project does not use `uv`, use the project’s existing Python execution command.

## TypeScript custom eval guidance

Use TypeScript when:

- The repository is TypeScript-first
- The eval is part of a Node, Bun, or Next.js workflow
- The user explicitly asks for TypeScript
- Existing Quantiles examples in the repository are written in TypeScript

A TypeScript Quantiles eval should generally include:

- A workflow name
- Input JSON parsing
- Durable per-sample `step()` calls
- `emit()` calls for metrics
- A returned JSON summary
- `entrypoint()` registration

Run a TypeScript custom eval with:

```bash
qt run <workflow-name> --input '{"key":"value"}' --json -- bun run <eval-file>.ts
```

Example:

```bash
qt run my-eval --input '{"limit":25}' --json -- bun run my_eval.ts
```

If the project uses `npm`, `pnpm`, `yarn`, or `tsx` instead of Bun, use the project’s existing TypeScript execution command.

## Running custom evals

Run Python custom evals with:

```bash
qt run my-workflow --input '{"key":"value"}' --json -- uv run python my_eval.py
```

Run TypeScript custom evals with:

```bash
qt run my-workflow --input '{"key":"value"}' --json -- bun run my_eval.ts
```

Place `--json` before the custom command separator `--`.

Correct:

```bash
qt run my-eval --input '{"limit":10}' --json -- uv run python my_eval.py
```

Incorrect:

```bash
qt run my-eval --input '{"limit":10}' -- uv run python my_eval.py --json
```

## Custom eval environment variables

When `qt run` executes a custom eval command, Quantiles injects runtime metadata into the subprocess.

Common environment variables include:

- `QUANTILES_BASE_URL` — local server URL, often `http://127.0.0.1:8765`
- `QUANTILES_RUN_ID` — existing run ID, if resuming
- `QUANTILES_WORKFLOW_NAME` — workflow name passed to `qt run`
- `QUANTILES_INPUT` — JSON input from `--input`

Use the Quantiles SDKs for Python or TypeScript when available. The SDKs should automatically detect and handle these variables.

Do not manually parse these variables unless the SDK is unavailable or the user asks for a lower-level integration.

## Converting ad-hoc eval scripts

When converting an ad-hoc eval script into a Quantiles workflow:

1. Preserve the existing dataset loading logic.
2. Preserve the existing model, prompt, scorer, and metric behavior unless the user asks for changes.
3. Wrap the eval in a Quantiles workflow.
4. Move per-sample model calls or scoring into durable steps.
5. Use deterministic step keys based on sample IDs.
6. Emit aggregate metrics with `emit()`.
7. Return a JSON summary.
8. Run with a small sample limit first.
9. Inspect the result with `qt show <run_id> --json`.
10. Compare against the original ad-hoc result if available.

First preserve behavior, then make it durable and observable.

## Reporting template

After running, inspecting, comparing, or resuming Quantiles evals, report results using this structure:

```text
Command used:
<exact command>

Run ID:
<run_id>

Workflow or benchmark:
<workflow_or_benchmark_name>

Model or sampler:
<model_or_sampler>

Input:
<input_json>

Status:
<status>

Summary metrics:
<key metrics>

Sample-level findings:
<representative failures, errors, or patterns>

Caveats:
<whether this was a smoke test, demo sampler run, small sample, or non-comparable comparison>

Recommended next command:
<qt show / qt compare / qt run command>
```

Keep the response concise unless the user asks for detailed sample-level analysis.

## Troubleshooting

Use these checks when something fails.

### `qt` command not found

Check installation and PATH:

```bash
command -v qt
echo "$PATH"
```

If missing, install the Quantiles CLI only when setup is required:

```bash
curl -fsSL https://cli.quantiles.io/install.sh | bash
```

Then verify:

```bash
command -v qt && qt --version
```

### Repository is not initialized

Run:

```bash
qt init
```

Then rerun the eval.

### Built-in eval ran but results look random

Check whether the run used the demo sampler. Demo sampler output is expected to be random or fake and should only be used for workflow validation.

### Provider-backed eval fails immediately

Check the relevant provider environment variable from Secrets and cost safety, and verify that `model` uses the correct provider prefix.

### JSON parsing fails

Confirm `--json` was passed before the custom command separator `--`.

Correct:

```bash
qt run my-eval --input '{"limit":10}' --json -- uv run python my_eval.py
```

Incorrect:

```bash
qt run my-eval --input '{"limit":10}' -- uv run python my_eval.py --json
```

### Custom eval does not connect to Quantiles

Check that it is run through `qt run` and that the SDK can see the Quantiles environment variables.

Run a minimal custom eval smoke test:

```bash
qt run <workflow-name> --input '{"limit":1}' --json -- <project-command>
```

### Resume does not work

Inspect the failed run first:

```bash
qt show <run_id> --json
```

Then check the installed CLI syntax:

```bash
qt run --help
```

Use the resume syntax supported by the installed CLI.

### Comparison is confusing

Use `qt show` on both runs:

```bash
qt show <run_id_1> --json
qt show <run_id_2> --json
```

Check benchmark name, input JSON, sample count, model, sampler, scorer, and metric definitions before interpreting the comparison.

## Skill quality checks

When editing this skill, test it against should-trigger and should-not-trigger prompts.

Should trigger:

```text
Run simpleqa-verified with 10 samples and tell me the run ID.
Compare these two Quantiles runs.
Inspect this Quantiles run and summarize the failed samples.
Resume this failed Quantiles workflow.
Convert this Python eval script into a Quantiles workflow.
```

Should not trigger:

```text
Explain quantiles and percentiles.
How do I compute the 95th percentile in NumPy?
What is the median of this distribution?
Compare MMLU and GPQA conceptually.
Write a pytest unit test for this function.
```
