import asyncio
import inspect
import json
import os
import sys
from collections.abc import Awaitable, Callable

from .client import QuantilesClient
from .types import JsonValue, QuantilesError, WorkflowDescriptor
from .workflow_context import WorkflowContext


def workflow(
  name: str,
  handler: Callable[..., Awaitable[JsonValue]],
) -> WorkflowDescriptor[JsonValue, JsonValue]:
  sig = inspect.signature(handler)
  params = [
    p
    for p in sig.parameters.values()
    if p.default is inspect.Parameter.empty
    and p.kind in (p.POSITIONAL_OR_KEYWORD, p.POSITIONAL_ONLY)
  ]
  num_params = len(params)

  async def run(caller_input: JsonValue = None) -> JsonValue:
    base_url = os.environ.get("QUANTILES_BASE_URL", "http://127.0.0.1:8765")
    async with QuantilesClient(base_url=base_url) as client:
      run_id: int
      env_run_id = os.environ.get("QUANTILES_RUN_ID")

      if env_run_id is not None:
        run_id = int(env_run_id)
      else:
        run = await client.create_run(name, caller_input)
        run_id = run.id

      await client.health()

      ctx = WorkflowContext(run_id, name, client)

      input_value: JsonValue
      if env_run_id is not None:
        env_input = os.environ.get("QUANTILES_INPUT")
        input_value = json.loads(env_input) if env_input is not None else None
      else:
        input_value = caller_input

      try:
        if num_params == 1:
          result = await handler(ctx)
        else:
          result = await handler(input_value, ctx)

        await client.set_run_output(run_id, result)

        if env_run_id is None:
          await client.complete_run(run_id)

        return result
      except Exception as error:
        if env_run_id is None:
          await client.fail_run(run_id, error)
        raise

  return WorkflowDescriptor(name, run)


async def step(
  ctx: WorkflowContext,
  *,
  step_key: str,
  execute: Callable[[], Awaitable[JsonValue]],
  input_value: JsonValue | None = None,
) -> JsonValue:
  return await ctx.step(
    step_key=step_key,
    input_value=input_value,
    execute=execute,
  )


async def emit(
  ctx: WorkflowContext,
  metric_name: str,
  metric_value: float,
  unit: str | None = None,
) -> None:
  await ctx.emit(metric_name, metric_value, unit)


def entrypoint(*workflows: WorkflowDescriptor[JsonValue, JsonValue]) -> None:
  name = os.environ.get("QUANTILES_WORKFLOW_NAME")
  if not name:
    raise QuantilesError("Run via `qt run <workflow>` (QUANTILES_WORKFLOW_NAME not set)")

  wf = next((w for w in workflows if w.name == name), None)
  if wf is None:
    raise QuantilesError(f"Unknown workflow: {name}")

  async def _run() -> None:
    try:
      env_input = os.environ.get("QUANTILES_INPUT")
      input_value = json.loads(env_input) if env_input is not None else None
      await wf.run(input_value)
    except Exception as error:
      sys.stderr.write(f"{error}\n")
      sys.exit(1)

  asyncio.run(_run())
