# Quantiles Agent Skill

Reusable coding-agent instructions for running Quantiles evaluations from a repository.

This skill teaches agents to use the `qt` CLI as the source of truth for eval runs: initialize local state, run benchmarks and custom workflows, inspect sample-level outputs, compare run history, resume interrupted work, and summarize regressions in a reviewable format.

## Why this exists

Coding agents can make evals part of the development loop, but only if they follow the same workflow every time. This skill gives agents durable Quantiles behavior so evaluation results are reproducible, traceable, and useful in engineering review.

It is designed for Codex, Claude Code, Cursor, GitHub Copilot, Gemini CLI, OpenCode, and other coding agents that support reusable skills or instruction files.

## What the skill does

- Runs evaluations through `qt`.
- Preserves run IDs, commands, inputs, metrics, stdout, stderr, and failure context.
- Uses `qt run $BENCHMARK` to run built-in benchmarks and custom eval workflows.
- Uses `qt show $RUN_ID` to inspect run results.
- Uses `qt compare $RUN_ID_A $RUN_ID_B` to analyze changes between runs.
- Uses `--json` to inspect structured result outputs.
- Uses `--resume` when a run is interrupted or partially completed.
- Reports aggregate metrics, sample-level results, failed samples, regressions, and next steps.

## Install

Install the Quantiles CLI and skill in the target repository and initialize local eval state:

```bash
qt install skill
```

For repository-specific behavior, create an `AGENTS.md` file in the target project, or append the Quantiles instructions to the existing [`AGENTS.md`](./AGENTS.md).

### Verify the install

The easiest way to validate the setup is to run a test evaluation that does not use your own model or API keys. The demo sampler handles the run locally, so no inference costs are incurred.

```text
Use the Quantiles eval skill. Run the SimpleQA Verified benchmark and summarize the results.
```

After validating the workflow, you can begin running evaluations with your own models, benchmarks, and sample sizes.

## Expected workflow

1. Confirm the repository has a Quantiles workspace.
2. Run the requested benchmark or workflow with `qt`.
3. Capture the returned run ID.
4. Inspect the run with `qt show <run_id>`.
5. Compare against a baseline when requested with `qt compare <run_id_a> <run_id_b>`.
6. Report aggregate metrics, sample-level results, failures, and next steps.

## Local-first behavior

Quantiles stores run history and sample-level results locally in the repository workspace. Agents should treat the local Quantiles database as the source of truth and avoid inventing results, skipping inspection, or reporting metrics without a run ID.

## Related files

- [`SKILL.md`](./SKILL.md) defines reusable Quantiles behavior for coding agents.
- [`AGENTS.md`](./AGENTS.md) defines repository-specific rules in projects that use Quantiles.
- `llms.txt` helps agents discover documentation and high-signal references.
- The main Quantiles documentation explains the CLI, SDKs, benchmarks, and run comparison workflow.

## License

This skill follows the license of the Quantiles repository.
