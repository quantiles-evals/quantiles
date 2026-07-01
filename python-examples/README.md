# Quantiles Python SDK Examples

This directory in the Quantiles monorepo contains examples to illustrate how to build [`custom_code` evals](https://quantiles.io/documentation/custom-evaluations) on the Quantiles platform using the [Python SDK](https://quantiles.io/documentation/reference/python-sdk).

The following `custom_code` evals are available along with a [`quantiles.toml` configuration file](https://quantiles.io/documentation/configuration) to make them easily runnable. They are detailed below:

| Eval | `qt run` command | Source file | Notes |
| --- | --- | --- | --- |
| PubMedQA | `qt run custom_pubmedqa` | [`src/pubmedqa.py`](./src/pubmedqa.py) | This is a [real benchmark](https://pubmedqa.github.io/) for evaluating models on biomedical research questions. The code for this custom eval is provided for illustrative purposes only. If you need to run PubMedQA in production, we recommend using the [built-in benchmark](https://quantiles.io/documentation/built-in-benchmarks) with this command: `qt run pubmedqa` |
| Example prompt eval | `qt run custom_prompt_eval` | [`src/prompt_eval.py`](./src/prompt_eval.py) | This is a `custom_code` eval to illustrate how you might use Quantiles to evaluate the performance of a customer support chatbot with various system prompts |
