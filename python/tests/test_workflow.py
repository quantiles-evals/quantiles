import json
import os
from unittest.mock import AsyncMock, MagicMock, patch

import aiohttp
import pytest

from quantiles import JsonValue, QuantilesError, WorkflowContext
from quantiles.client import QuantilesClient
from quantiles.workflow import emit, entrypoint, step, workflow


def _make_response(text_body: str, status: int = 200) -> AsyncMock:
  response = AsyncMock()
  response.ok = status < 400
  response.status = status
  response.text = AsyncMock(return_value=text_body)
  return response


def _setup_mock_session(mock_session: AsyncMock, *responses: AsyncMock) -> None:
  side_effects = list(responses)

  def _context_manager(*args: object, **kwargs: object) -> AsyncMock:
    response = side_effects.pop(0)
    response.__aenter__ = AsyncMock(return_value=response)
    response.__aexit__ = AsyncMock(return_value=False)
    return response

  mock_session.request = MagicMock(side_effect=_context_manager)


class TestWorkflow:
  @pytest.mark.asyncio
  async def test_workflow_creates_run_and_completes(self) -> None:
    async def my_handler(input_value: JsonValue, ctx: WorkflowContext) -> JsonValue:
      return {"result": "ok"}

    wf = workflow("test-wf", my_handler)
    assert wf.name == "test-wf"

    with patch("quantiles.workflow.QuantilesClient") as MockClient:
      mock_client = AsyncMock()
      mock_client.__aenter__ = AsyncMock(return_value=mock_client)
      mock_client.__aexit__ = AsyncMock(return_value=False)
      mock_run = AsyncMock()
      mock_run.id = 42
      mock_client.create_run = AsyncMock(return_value=mock_run)
      MockClient.return_value = mock_client

      result = await wf.run(json.loads('{"input": "data"}'))

      assert result == {"result": "ok"}
      mock_client.create_run.assert_called_once_with("test-wf", json.loads('{"input": "data"}'))
      mock_client.complete_run.assert_called_once_with(42)

  @pytest.mark.asyncio
  async def test_workflow_fails_run_on_exception(self) -> None:
    async def my_handler(_input: JsonValue, _ctx: WorkflowContext) -> JsonValue:
      raise ValueError("boom")

    wf = workflow("test-wf", my_handler)

    with patch("quantiles.workflow.QuantilesClient") as MockClient:
      mock_client = AsyncMock()
      mock_client.__aenter__ = AsyncMock(return_value=mock_client)
      mock_client.__aexit__ = AsyncMock(return_value=False)
      mock_run = AsyncMock()
      mock_run.id = 42
      mock_client.create_run = AsyncMock(return_value=mock_run)
      MockClient.return_value = mock_client

      with pytest.raises(ValueError, match="boom"):
        await wf.run(None)

      mock_client.fail_run.assert_called_once()

  @pytest.mark.asyncio
  async def test_workflow_uses_env_run_id(self) -> None:
    async def my_handler(input_value: JsonValue, ctx: WorkflowContext) -> JsonValue:
      return {"received": input_value}

    wf = workflow("test-wf", my_handler)

    env = {
      "QUANTILES_RUN_ID": "99",
      "QUANTILES_INPUT": json.dumps({"env": "input"}),
    }
    with (
      patch.dict(os.environ, env, clear=False),
      patch("quantiles.workflow.QuantilesClient") as MockClient,
    ):
      mock_client = AsyncMock()
      mock_client.__aenter__ = AsyncMock(return_value=mock_client)
      mock_client.__aexit__ = AsyncMock(return_value=False)
      MockClient.return_value = mock_client

      result = await wf.run(json.loads('{"caller": "input"}'))

      assert result == {"received": {"env": "input"}}
      mock_client.create_run.assert_not_called()
      mock_client.complete_run.assert_not_called()

  @pytest.mark.asyncio
  async def test_workflow_single_arg_handler(self) -> None:
    async def my_handler(ctx: WorkflowContext) -> JsonValue:
      return {"mode": "no-input"}

    wf = workflow("test-wf", my_handler)

    with patch("quantiles.workflow.QuantilesClient") as MockClient:
      mock_client = AsyncMock()
      mock_client.__aenter__ = AsyncMock(return_value=mock_client)
      mock_client.__aexit__ = AsyncMock(return_value=False)
      mock_run = AsyncMock()
      mock_run.id = 1
      mock_client.create_run = AsyncMock(return_value=mock_run)
      MockClient.return_value = mock_client

      result = await wf.run(None)
      assert result == {"mode": "no-input"}

  @pytest.mark.asyncio
  async def test_workflow_step_via_context(self) -> None:
    async def my_handler(_input: JsonValue, ctx: WorkflowContext) -> JsonValue:
      async def do_step() -> JsonValue:
        return {"step_result": 123}

      return await ctx.step(step_key="my-step", execute=do_step)

    wf = workflow("test-wf", my_handler)

    with patch("quantiles.workflow.QuantilesClient") as MockClient:
      mock_client = AsyncMock()
      mock_client.__aenter__ = AsyncMock(return_value=mock_client)
      mock_client.__aexit__ = AsyncMock(return_value=False)
      mock_run = AsyncMock()
      mock_run.id = 42
      mock_client.create_run = AsyncMock(return_value=mock_run)
      mock_client.run_step = AsyncMock(return_value={"step_result": 123})
      MockClient.return_value = mock_client

      result = await wf.run(None)
      assert result == {"step_result": 123}
      mock_client.run_step.assert_called_once()

  @pytest.mark.asyncio
  async def test_workflow_emit_via_context(self) -> None:
    async def my_handler(_input: JsonValue, ctx: WorkflowContext) -> JsonValue:
      await ctx.emit("accuracy", 0.95, "percent")
      return {"done": True}

    wf = workflow("test-wf", my_handler)

    with patch("quantiles.workflow.QuantilesClient") as MockClient:
      mock_client = AsyncMock()
      mock_client.__aenter__ = AsyncMock(return_value=mock_client)
      mock_client.__aexit__ = AsyncMock(return_value=False)
      mock_run = AsyncMock()
      mock_run.id = 42
      mock_client.create_run = AsyncMock(return_value=mock_run)
      MockClient.return_value = mock_client

      await wf.run(None)
      mock_client.emit_metric.assert_called_once_with(42, "accuracy", 0.95, "percent")


class TestFreeFunctions:
  @pytest.mark.asyncio
  async def test_step_delegates_to_context(self) -> None:
    mock_session = AsyncMock(spec=aiohttp.ClientSession)
    resp = _make_response('{"decision": "run", "step_id": 1}')
    resp2 = _make_response("{}")
    _setup_mock_session(mock_session, resp, resp2)
    client = QuantilesClient(session=mock_session)
    ctx = WorkflowContext(1, "test", client)

    executed = False

    async def execute() -> JsonValue:
      nonlocal executed
      executed = True
      return {"result": "ok"}

    result = await step(
      ctx,
      step_key="key",
      execute=execute,
    )
    assert result == {"result": "ok"}
    assert executed

  @pytest.mark.asyncio
  async def test_step_with_input(self) -> None:
    mock_session = AsyncMock(spec=aiohttp.ClientSession)
    resp = _make_response('{"decision": "run", "step_id": 1}')
    resp2 = _make_response("{}")
    _setup_mock_session(mock_session, resp, resp2)
    client = QuantilesClient(session=mock_session)
    ctx = WorkflowContext(1, "test", client)

    async def execute() -> JsonValue:
      return {"result": "ok"}

    result = await step(
      ctx,
      step_key="key",
      input_value=json.loads('{"input": 1}'),
      execute=execute,
    )
    assert result == {"result": "ok"}

  @pytest.mark.asyncio
  async def test_emit_delegates_to_context(self) -> None:
    mock_session = AsyncMock(spec=aiohttp.ClientSession)
    _setup_mock_session(mock_session, _make_response("{}"))
    client = QuantilesClient(session=mock_session)
    ctx = WorkflowContext(1, "test", client)

    await emit(ctx, "metric", 42.0, "ms")
    mock_session.request.assert_called()


class TestEntrypoint:
  def test_entrypoint_missing_env(self) -> None:
    with (
      patch.dict(os.environ, {}, clear=True),
      pytest.raises(QuantilesError, match="QUANTILES_WORKFLOW_NAME"),
    ):
      entrypoint()

  def test_entrypoint_unknown_workflow(self) -> None:
    async def _handler(ctx: WorkflowContext) -> JsonValue:
      return {"dummy": True}

    wf = workflow("known", _handler)
    with (
      patch.dict(os.environ, {"QUANTILES_WORKFLOW_NAME": "unknown"}),
      pytest.raises(QuantilesError, match="Unknown workflow"),
    ):
      entrypoint(wf)

  @pytest.mark.asyncio
  async def test_entrypoint_runs_workflow(self) -> None:
    async def my_handler(ctx: WorkflowContext) -> JsonValue:
      return {"ran": True}

    wf = workflow("my-wf", my_handler)

    with (
      patch.dict(os.environ, {"QUANTILES_WORKFLOW_NAME": "my-wf"}, clear=False),
      patch("quantiles.workflow.QuantilesClient") as MockClient,
    ):
      mock_client = AsyncMock()
      mock_client.__aenter__ = AsyncMock(return_value=mock_client)
      mock_client.__aexit__ = AsyncMock(return_value=False)
      mock_run = AsyncMock()
      mock_run.id = 1
      mock_client.create_run = AsyncMock(return_value=mock_run)
      MockClient.return_value = mock_client

      result = await wf.run(None)
      assert result == {"ran": True}
