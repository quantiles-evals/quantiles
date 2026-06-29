from typing import Literal, TypedDict, cast

from quantiles import (
  JsonValue,
  Statistics,
  emit,
  entrypoint,
  step,
  workflow,
)
from quantiles.workflow_context import WorkflowContext

Label = Literal["billing", "bug", "how_to"]


class EvalInput(TypedDict):
  prompt_version: str


class EvalCase(TypedDict):
  id: str
  ticket: str
  expected: Label


class CaseResult(TypedDict):
  id: str
  expected: Label
  prediction: Label
  correct: bool
  tokens_used: int


CASES: list[EvalCase] = [
  {
    "id": "double-charge",
    "ticket": "I was charged twice for my subscription renewal.",
    "expected": "billing",
  },
  {
    "id": "csv-crash",
    "ticket": "The app crashes every time I upload a CSV file.",
    "expected": "bug",
  },
  {
    "id": "invite-teammate",
    "ticket": "Can you show me how to invite another teammate?",
    "expected": "how_to",
  },
]


async def evaluate_case(
  ctx: WorkflowContext,
  item: EvalCase,
  prompt_version: str,
) -> CaseResult:
  prediction = cast(
    Label,
    await step(
      ctx,
      step_key=f"case:{item['id']}",
      input_value={
        "sample_id": item["id"],
        "ticket": item["ticket"],
        "expected": item["expected"],
        "prompt_version": prompt_version,
      },
      execute=lambda: call_model(item["ticket"]),
    ),
  )

  return {
    "id": item["id"],
    "expected": item["expected"],
    "prediction": prediction,
    "correct": prediction == item["expected"],
    "tokens_used": len(item["ticket"].split()),
  }


async def handler(input_value: EvalInput | None, ctx: WorkflowContext) -> JsonValue:
  prompt_version = (input_value or {}).get("prompt_version", "A")
  results: list[CaseResult] = []
  for item in CASES:
    results.append(await evaluate_case(ctx, item, prompt_version))

  correct = sum(1 for result in results if result["correct"])
  accuracy = Statistics.accuracy(correct, len(results))
  tokens_used = sum(result["tokens_used"] for result in results)

  await emit(ctx, "accuracy", accuracy)
  await emit(ctx, "correct_count", float(correct))
  await emit(ctx, "total_count", float(len(results)))
  await emit(ctx, "tokens_used", float(tokens_used), "tokens")

  return cast(
    JsonValue,
    # the below value is a valid JsonValue
    {
      "accuracy": accuracy,
      "correct_count": correct,
      "total_count": len(results),
      "results": results,
    },
  )


async def call_model(ticket: str) -> Label:
  text = ticket.lower()
  if "crash" in text:
    return "bug"
  if "invite" in text or "show me" in text:
    return "how_to"
  return "billing"


support_triage = workflow("support-triage", handler)
entrypoint(support_triage)
