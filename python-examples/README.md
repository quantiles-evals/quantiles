# Quantiles Python SDK Examples

This directory in the Quantiles monorepo contains examples to illustrate how to build [`custom_code` evals](https://quantiles.io/documentation/custom-evaluations) on the Quantiles platform using the [Python SDK](https://quantiles.io/documentation/reference/python-sdk).

The following `custom_code` evals are available along with a [`quantiles.toml` configuration file](https://quantiles.io/documentation/configuration) to make them easily runnable. They are detailed below:

| Eval | `qt run` command | Source file | Notes |
| --- | --- | --- | --- |
| PubMedQA | `qt run custom_pubmedqa` | [`src/pubmedqa.py`](./src/pubmedqa.py) | [PubMedQA](https://pubmedqa.github.io/) is a biomedical question-answering benchmark. This file shows how a custom evaluation can be implemented, but PubMedQA is also available as a built-in Quantiles benchmark: `qt run pubmedqa`. |
| Example prompt eval | `qt run custom_prompt_eval` | [`src/prompt_eval.py`](./src/prompt_eval.py) | This is a [`custom_code`](https://quantiles.io/documentation/custom-evaluations) eval to illustrate how you might use Quantiles to evaluate the performance of a customer support chatbot with various system prompts |
