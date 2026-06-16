from collections.abc import Awaitable, Callable
from typing import final

from .client import QuantilesClient
from .types import JsonValue


@final
class WorkflowContext:
  def __init__(self, run_id: int, workflow_name: str, client: QuantilesClient) -> None:
    self.run_id: int = run_id
    self.workflow_name: str = workflow_name
    self.client: QuantilesClient = client

  async def step(
    self,
    *,
    step_key: str,
    execute: Callable[[], Awaitable[JsonValue]],
    input_value: JsonValue | None = None,
  ) -> JsonValue:
    return await self.client.run_step(
      run_id=self.run_id,
      step_key=step_key,
      input_value=input_value,
      execute=execute,
    )

  async def emit(
    self,
    metric_name: str,
    metric_value: float,
    unit: str | None = None,
  ) -> None:
    await self.client.emit_metric(self.run_id, metric_name, metric_value, unit)
