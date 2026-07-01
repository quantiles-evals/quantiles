"""
This file demonstrates how to use the Quantiles Python SDK to implement a new benchmark for the `qt` CLI.

It uses PubMedQA as an illustrative example, implementing the benchmark from scratch even though PubMedQA is
already available as a built-in benchmark and can be run directly with `qt run pubmedqa`.

The steps shown in this example can be used to add new benchmarks or build custom evaluation workflows in Quantiles.
"""

import hashlib
import re
from collections.abc import Awaitable, Callable
from typing import TypedDict, cast

from pydantic import BaseModel, ConfigDict, ValidationError
from quantiles import (
    LLMMessage,
    ModelProvider,
    Statistics,
    call_llm,
    collect_async_iter,
    dataset,
    emit,
    entrypoint,
    map_dataset,
    step,
    workflow,
)
from quantiles.types import JsonValue
from quantiles.workflow_context import WorkflowContext

_ALLOWED_LABELS = {"yes", "no", "maybe"}


class PubmedQARow(BaseModel):
    sample_id: str
    question: str
    context: str
    gold_answer: str


class EvalResult(TypedDict):
    sample_id: str
    question: str
    gold_answer: str
    prediction: str | None
    is_correct: bool
    model_response: str
    tokens: int


class RawPubmedQARowModel(BaseModel):
    model_config = ConfigDict(extra="allow")

    question: JsonValue = None
    query: JsonValue = None
    prompt: JsonValue = None
    input: JsonValue = None

    context: JsonValue = None
    abstract: JsonValue = None
    passage: JsonValue = None
    long_answer: JsonValue = None
    evidence: JsonValue = None
    contexts: JsonValue = None

    final_decision: JsonValue = None
    finalDecision: JsonValue = None
    answer: JsonValue = None
    label: JsonValue = None
    target: JsonValue = None

    id: JsonValue = None
    qid: JsonValue = None
    question_id: JsonValue = None


def _normalize_label(value: JsonValue) -> str | None:
    if not isinstance(value, str):
        return None
    normalized = value.strip().lower()
    if normalized in _ALLOWED_LABELS:
        return normalized
    return None


def _coerce_text(value: JsonValue) -> str:
    if isinstance(value, str):
        return value.strip()
    if isinstance(value, list):
        return "\n".join(
            _coerce_text(item) for item in value if _coerce_text(item) != ""
        ).strip()
    if isinstance(value, dict):
        return "\n".join(
            _coerce_text(item) for item in value.values() if _coerce_text(item) != ""
        ).strip()
    return ""


def _extract_context(row: RawPubmedQARowModel) -> str:
    direct_keys = [
        row.context,
        row.abstract,
        row.passage,
        row.long_answer,
        row.evidence,
    ]
    for value in direct_keys:
        text = _coerce_text(value)
        if text:
            return text

    contexts = row.contexts
    if isinstance(contexts, list):
        return "\n".join(
            part for part in (_coerce_text(item) for item in contexts) if part
        )
    if isinstance(contexts, dict):
        ordered_keys = ["label", "contexts", "context", "abstract", "title"]
        values = [_coerce_text(contexts.get(key)) for key in ordered_keys]
        values.extend(
            _coerce_text(value)
            for key, value in contexts.items()
            if key not in ordered_keys
        )
        return "\n".join(part for part in values if part)

    return ""


def _transform_pubmedqa_row(raw: dict[str, JsonValue]) -> PubmedQARow:
    try:
        row = RawPubmedQARowModel.model_validate(raw)
    except ValidationError as e:
        raise ValueError(f"Row validation failed: {e}") from e

    question = _coerce_text(row.question or row.query or row.prompt or row.input)
    context = _extract_context(row)
    gold_answer = (
        _normalize_label(row.final_decision)
        or _normalize_label(row.finalDecision)
        or _normalize_label(row.answer)
        or _normalize_label(row.label)
        or _normalize_label(row.target)
    )
    if question == "" or gold_answer is None:
        raise ValueError("Missing question or gold_answer")

    sample_id = _coerce_text(row.id or row.qid or row.question_id)
    if sample_id == "":
        sample_id = hashlib.sha256(
            f"{question}|{context}|{gold_answer}".encode()
        ).hexdigest()[:16]

    return PubmedQARow(
        sample_id=sample_id,
        question=question,
        context=context,
        gold_answer=gold_answer,
    )


def _extract_label_from_response(content: str) -> str | None:
    lowered = content.strip().lower()
    if lowered in _ALLOWED_LABELS:
        return lowered

    match = re.search(r"\b(yes|no|maybe)\b", lowered)
    if match is None:
        return None
    return match.group(1)


def _build_messages(question: str, context: str) -> list[LLMMessage]:
    system_prompt = "You are grading a biomedical QA item. Reply with exactly one label: yes, no, or maybe."
    user_prompt = (
        f"Question:\n{question}\n\n"
        f"Context:\n{context or '(no context provided)'}\n\n"
        "Answer with exactly one token: yes, no, or maybe."
    )
    return [
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": user_prompt},
    ]


async def _measure_row(
    row: PubmedQARow, model_provider: ModelProvider, model_id: str
) -> EvalResult:
    messages = _build_messages(row.question, row.context)
    llm_result = await call_llm(model_provider, model_id, messages)
    prediction = _extract_label_from_response(llm_result["content"])
    is_correct = prediction == row.gold_answer

    return {
        "sample_id": row.sample_id,
        "question": row.question,
        "gold_answer": row.gold_answer,
        "prediction": prediction,
        "is_correct": is_correct,
        "model_response": llm_result["content"],
        "tokens": llm_result["tokens"],
    }


async def _pubmedqa_handler(
    input_data: dict[str, JsonValue],
    ctx: WorkflowContext,
) -> JsonValue:
    model_name = input_data.get("model_name", "openai:gpt_5_nano")
    num_examples = int(cast(int, input_data.get("num_examples", 25)))

    if ":" not in str(model_name):
        raise ValueError(
            f"model_name must be in 'provider:model_id' format, got: {model_name}"
        )
    provider, raw_model_id = str(model_name).split(":", 1)
    if provider != "openai":
        raise ValueError(f"Unsupported provider: {provider}")
    model_provider: ModelProvider = "openai"
    model_id = raw_model_id.replace("_", "-")

    ds = await dataset(
        ctx,
        source="huggingface://quantiles/PubMedQA",
        row_type=PubmedQARow,
        batch_size=25,
        config="pqa_labeled",
        split="train",
        max_rows=num_examples,
        on_error="skip",
        transform=_transform_pubmedqa_row,
    )

    async def _eval_row(row: PubmedQARow) -> EvalResult:
        result_raw = await step(
            ctx,
            step_key=f"eval-{row.sample_id}",
            input_value=cast(
                JsonValue,
                {
                    "sample_id": row.sample_id,
                    "question": row.question,
                    "context": row.context,
                    "gold_answer": row.gold_answer,
                    "model_id": model_id,
                },
            ),
            execute=cast(
                Callable[[], Awaitable[JsonValue]],
                lambda r=row: _measure_row(r, model_provider, model_id),
            ),
        )
        return cast(EvalResult, result_raw)

    results: list[EvalResult] = await collect_async_iter(
        map_dataset(ds, _eval_row),
    )

    correct_count = sum(1 for r in results if r["is_correct"])
    total = len(results)
    accuracy = Statistics.accuracy(correct_count, total)
    total_tokens = sum(r["tokens"] for r in results)

    await emit(ctx, "accuracy", accuracy)
    await emit(ctx, "correct_count", float(correct_count))
    await emit(ctx, "total_count", float(total))
    await emit(ctx, "total_tokens", float(total_tokens))

    return cast(
        JsonValue,
        {
            "accuracy": accuracy,
            "correct_count": correct_count,
            "total_count": total,
            "total_tokens": total_tokens,
            "num_rows": num_examples,
        },
    )


if __name__ == "__main__":
    pubmedqa_eval = workflow("custom_pubmedqa", _pubmedqa_handler)
    entrypoint(pubmedqa_eval)
