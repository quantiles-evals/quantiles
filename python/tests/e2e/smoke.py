from collections.abc import Awaitable, Callable
from typing import cast

from quantiles import emit, entrypoint, step, workflow
from quantiles.types import JsonValue
from quantiles.workflow_context import WorkflowContext


async def _smoke_handler(_input: JsonValue, ctx: WorkflowContext) -> JsonValue:
  iterations = 3
  if isinstance(_input, dict):
    raw = _input.get("iterations")
    if isinstance(raw, int):
      iterations = raw

  results: list[JsonValue] = []
  for i in range(iterations):
    _input_value: JsonValue = {"index": i}

    def _make_execute(idx: int) -> Callable[[], Awaitable[JsonValue]]:
      async def _execute() -> JsonValue:
        return await _make_step_result(idx)

      return _execute

    result = await step(
      ctx,
      step_key=f"step-{i}",
      input_value=_input_value,
      execute=_make_execute(i),
    )
    results.append(result)

  total = 0
  for r in results:
    val = cast(dict[str, JsonValue], r).get("val", 0)
    if isinstance(val, int):
      total += val

  await emit(ctx, "total", float(total))
  await emit(ctx, "iterations", float(iterations))

  return cast(JsonValue, {"ok": True, "total": total, "iterations": iterations})


async def _make_step_result(idx: int) -> JsonValue:
  return {"val": idx * 2}


smoke = workflow("e2e-smoke-py", _smoke_handler)
entrypoint(smoke)
