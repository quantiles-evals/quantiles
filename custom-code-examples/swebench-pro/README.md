# SWE-Bench Pro `custom_code` example

This example is a thin adapter around Scale's official SWE-Bench Pro evaluator. With Docker running, start the one-task gold-patch smoke test from the `custom-code-examples` directory:

```bash
qt run swebench-pro
```

The first run clones [`scaleapi/SWE-bench_Pro-os`](https://github.com/scaleapi/SWE-bench_Pro-os), creates an isolated environment for its dependencies, downloads the public dataset, extracts the first gold patch, and pulls the task's Docker image. These network-dependent setup operations can take several minutes. Later runs reuse `.swebench-pro-cache/` and `.swebench-pro-results/` in this directory.

The adapter records the official evaluator invocation as a durable Quantiles step and emits `resolution_rate`, `resolved_count`, and `total_count`. It does not call a model or require a provider API key. The upstream project currently marks local Docker evaluation as beta, and the evaluator runs benchmark code inside containers.

To grade your own predictions instead of the default gold patch, override the paths:

```bash
qt run swebench-pro --input '{
  "repo_path": "/path/to/SWE-bench_Pro-os",
  "evaluator_python": "/path/to/SWE-bench_Pro-os/.venv/bin/python",
  "raw_sample_path": "/path/to/samples.jsonl",
  "patch_path": "/path/to/predictions.json",
  "output_dir": "swebench-pro/.swebench-pro-results",
  "num_workers": 1,
  "use_local_docker": true
}'
```

Run the focused offline tests from this subdirectory:

```bash
uv run python -m unittest discover -s tests -v
uv run ruff check .
uv run ruff format . --check
```
