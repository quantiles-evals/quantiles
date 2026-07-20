# Quantiles Python SDK Examples

This directory contains examples of building [`custom_code` evaluations](https://quantiles.io/documentation/custom-evaluations) with the Quantiles [Python SDK](https://quantiles.io/documentation/reference/python-sdk).

The accompanying [`quantiles.toml`](./quantiles.toml) makes these examples easily runnable with `qt`:

| Evaluation                         | Command                     | Source                                       | Notes                                                                                                                                                                                             |
| ---------------------------------- | --------------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| PubMedQA                           | `qt run custom_pubmedqa`    | [`src/pubmedqa.py`](./src/pubmedqa.py)       | Implements the [PubMedQA](https://pubmedqa.github.io/) biomedical question-answering benchmark as a custom evaluation. PubMedQA is also available as a built-in benchmark with `qt run pubmedqa`. |
| Customer-support prompt evaluation | `qt run custom_prompt_eval` | [`src/prompt_eval.py`](./src/prompt_eval.py) | Demonstrates a deterministic customer-support classification evaluation with recorded steps and metrics.                                                                                          |
