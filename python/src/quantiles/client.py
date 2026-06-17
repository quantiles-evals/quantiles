import inspect
import json
import os
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import TypeVar, overload

import aiohttp
from pydantic import BaseModel

from ._rpc_types import (
  BeginStepRequest,
  CompleteRunRequest,
  CompleteStepRequest,
  CreateRunRequest,
  CreateRunResponse,
  EmitMetricRequest,
  FailRunRequest,
  FailStepRequest,
  RunResponse,
  SetRunOutputRequest,
  StepDecisionReuse,
  step_decision_adapter,
)
from .types import JsonValue, QuantilesError, RunRecord, to_json_value
from .util import error_message, hash_json, response_error_message, stable_stringify

T = TypeVar("T", bound=BaseModel)


class QuantilesApiError(QuantilesError):
  pass


class QuantilesClient:
  def __init__(
    self,
    *,
    base_url: str | None = None,
    session: aiohttp.ClientSession | None = None,
  ) -> None:
    resolved = base_url or os.environ.get("QUANTILES_BASE_URL") or "http://127.0.0.1:8765"
    self.base_url: str = resolved.rstrip("/")
    self._session: aiohttp.ClientSession = session or aiohttp.ClientSession()

  async def _get_session(self) -> aiohttp.ClientSession:
    return self._session

  async def __aenter__(self) -> "QuantilesClient":
    return self

  async def __aexit__(
    self,
    exc_type: type[BaseException] | None,
    exc_val: BaseException | None,
    exc_tb: object,
  ) -> None:
    await self._session.close()

  @overload
  async def _request(
    self,
    method: str,
    path: str,
    *,
    request_model: BaseModel | None = None,
    response_model: type[T],
  ) -> T: ...

  @overload
  async def _request(
    self,
    method: str,
    path: str,
    *,
    request_model: BaseModel | None = None,
    response_model: None = None,
  ) -> JsonValue | None: ...

  async def _request(
    self,
    method: str,
    path: str,
    *,
    request_model: BaseModel | None = None,
    response_model: type[T] | None = None,
  ) -> T | JsonValue | None:
    session = await self._get_session()
    url = f"{self.base_url}{path}"
    headers: dict[str, str] = {"content-type": "application/json"}
    data = request_model.model_dump_json() if request_model is not None else None

    async with session.request(method, url, headers=headers, data=data) as response:
      if not response.ok:
        raise QuantilesApiError(await response_error_message(response))
      text = await response.text()
      if text == "":
        return None
      if response_model is not None:
        return response_model.model_validate(json.loads(text))
      return to_json_value(text)

  async def health(self) -> None:
    # we don't care about the response as long as it's a 200
    _ = await self._request("GET", "/health", request_model=None, response_model=None)

  async def create_run(
    self,
    workflow_name: str,
    input_value: JsonValue | None = None,
  ) -> "QuantilesRun":
    request = CreateRunRequest(
      workflow_name=workflow_name,
      input=stable_stringify(input_value) if input_value is not None else None,
    )
    response = await self._request(
      "POST",
      "/runs",
      request_model=request,
      response_model=CreateRunResponse,
    )
    return QuantilesRun(client=self, id=response.run_id)

  async def get_run(self, run_id: int | None = None) -> RunRecord:
    if run_id is not None:
      reified_run_id = run_id
    else:
      env_run_id = os.environ.get("QUANTILES_RUN_ID")
      if env_run_id is None:
        raise QuantilesError("This script must be run via `qt run` ('QUANTILES_RUN_ID' not found)")
      try:
        reified_run_id = int(env_run_id)
      except ValueError as e:
        raise QuantilesError(f"Invalid run ID {run_id} ({e})") from e

    response = await self._request(
      "GET",
      f"/runs/{reified_run_id}",
      request_model=None,
      response_model=RunResponse,
    )
    return RunRecord(
      id=response.id,
      workflow_name=response.workflow_name,
      status=response.status,
      input=response.input,
      started_at=response.started_at,
      finished_at=response.finished_at,
      error=response.error,
    )

  async def complete_run(self, run_id: int) -> None:
    _ = await self._request(
      "POST",
      f"/runs/{run_id}/complete",
      request_model=CompleteRunRequest(output=None),
      response_model=None,
    )

  async def set_run_output(self, run_id: int, output: JsonValue) -> None:
    _ = await self._request(
      "POST",
      f"/runs/{run_id}/output",
      request_model=SetRunOutputRequest(output=stable_stringify(output)),
      response_model=None,
    )

  async def fail_run(self, run_id: int, error: BaseException | object) -> None:
    _ = await self._request(
      "POST",
      f"/runs/{run_id}/fail",
      request_model=FailRunRequest(error=error_message(error)),
      response_model=None,
    )

  async def emit_metric(
    self,
    run_id: int,
    metric_name: str,
    metric_value: float,
    unit: str | None = None,
  ) -> None:
    _ = await self._request(
      "POST",
      f"/runs/{run_id}/metrics",
      request_model=EmitMetricRequest(
        metric_name=metric_name,
        metric_value=metric_value,
        unit=unit,
      ),
      response_model=None,
    )

  async def run_step(
    self,
    *,
    run_id: int,
    step_key: str,
    input_value: JsonValue | None,
    execute: Callable[[], Awaitable[JsonValue]],
  ) -> JsonValue:

    raw = await self._request(
      "POST",
      "/steps/begin",
      request_model=BeginStepRequest(
        run_id=run_id,
        step_key=step_key,
        input_hash=hash_json(input_value),
      ),
    )
    if raw is None or not isinstance(raw, dict):
      raise QuantilesApiError("Invalid response from /steps/begin")
    decision = step_decision_adapter.validate_python(raw)

    if isinstance(decision, StepDecisionReuse):
      return to_json_value(decision.output)

    try:
      output = execute()
      if inspect.isawaitable(output):
        output = await output
      _ = await self._request(
        "POST",
        "/steps/complete",
        request_model=CompleteStepRequest(
          step_id=decision.step_id,
          output=stable_stringify(output),
        ),
        response_model=None,
      )
      return output
    except Exception as error:
      _ = await self._request(
        "POST",
        "/steps/fail",
        request_model=FailStepRequest(
          step_id=decision.step_id,
          error=error_message(error),
        ),
        response_model=None,
      )
      raise


@dataclass(frozen=True)
class StepParams:
  key: str
  execute: Callable[[], Awaitable[JsonValue]]
  input_value: JsonValue | None = None


@dataclass(frozen=True)
class QuantilesRun:
  client: QuantilesClient
  id: int

  async def step(
    self,
    params: StepParams,
  ) -> JsonValue:
    return await self.client.run_step(
      run_id=self.id,
      step_key=params.key,
      input_value=params.input_value,
      execute=params.execute,
    )

  async def complete(self) -> None:
    await self.client.complete_run(self.id)

  async def fail(self, error: BaseException | object) -> None:
    await self.client.fail_run(self.id, error)

  async def emit(
    self,
    metric_name: str,
    metric_value: float,
    unit: str | None = None,
  ) -> None:
    await self.client.emit_metric(self.id, metric_name, metric_value, unit)
