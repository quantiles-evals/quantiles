# Quantiles custom-code examples

This directory contains runnable `custom_code` benchmarks. Each benchmark has an isolated Python project, while the top-level [`quantiles.toml`](./quantiles.toml) provides a single place to run and compare them.

Run commands from this directory:

```bash
qt run t3-airline
qt run swebench-pro
```

- [`t3-airline`](./t3-airline/README.md) wraps the official τ³ airline benchmark. It requires a one-time data checkout and configured model-provider credentials, and its default run calls external models.
- [`swebench-pro`](./swebench-pro/README.md) runs a one-task gold-patch smoke test with the official SWE-Bench Pro evaluator. Its first run downloads the public harness, dataset, Python dependencies, and a Docker image; it does not call a model.

Quantiles stores both benchmarks' local run history in this directory's ignored `.quantiles/` workspace. Benchmark-specific environments, caches, and generated results stay inside their respective subdirectories and are also ignored.
