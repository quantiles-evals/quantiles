# Quantiles Python SDK Examples

This directory contains examples of building [`custom_code` evaluations](https://quantiles.io/documentation/custom-evaluations) with the Quantiles [Python SDK](https://quantiles.io/documentation/reference/python-sdk).

The accompanying [`quantiles.toml`](./quantiles.toml) makes these examples easily runnable with `qt`:

| Evaluation | Command | Source | Notes |
| --- | --- | --- | --- |
| PubMedQA | `qt run custom_pubmedqa` | [`src/pubmedqa.py`](./src/pubmedqa.py) | Implements the [PubMedQA](https://pubmedqa.github.io/) biomedical question-answering benchmark as a custom evaluation. A registry-backed definition is also available with `qt run pubmedqa`. |
| Customer-support prompt evaluation | `qt run custom_prompt_eval` | [`src/prompt_eval.py`](./src/prompt_eval.py) | Demonstrates a deterministic customer-support classification evaluation with recorded steps and metrics. |
| SWE-Bench Pro | `qt run custom_swebench_pro` | [`src/swebench_pro.py`](./src/swebench_pro.py) | Grades existing patch predictions with Scale's official SWE-Bench Pro evaluator. |

## SWE-Bench Pro

The SWE-Bench Pro example is a thin adapter around Scale's official evaluator. It grades existing patch predictions; it does not generate patches or bundle the upstream evaluation harness.

Before running it:

1. Clone [`scaleapi/SWE-bench_Pro-os`](https://github.com/scaleapi/SWE-bench_Pro-os) and install its requirements in a dedicated Python environment.
2. Install and start Docker. Local Docker evaluation is currently marked beta by the upstream project.
3. Prepare the raw sample CSV or JSONL and a predictions JSON file using the formats documented by the upstream project.

From this directory, run a small evaluation with explicit local paths:

```bash
qt run custom_swebench_pro --input '{
  "repo_path": "/path/to/SWE-bench_Pro-os",
  "evaluator_python": "/path/to/SWE-bench_Pro-os/.venv/bin/python",
  "raw_sample_path": "/path/to/swe_bench_pro_full.csv",
  "patch_path": "/path/to/predictions.json",
  "output_dir": ".swebench-pro-results",
  "num_workers": 1,
  "use_local_docker": true
}'
```

The example records the official evaluator invocation as a durable Quantiles step and emits `resolution_rate`, `resolved_count`, and `total_count`. The evaluator may download Docker images and run untrusted benchmark code inside containers. Its output directory contains generated logs, patches, and test results and must not be committed.
