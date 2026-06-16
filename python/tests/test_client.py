import json
from unittest.mock import AsyncMock, MagicMock, patch

import aiohttp
import pytest

from quantiles import JsonValue, QuantilesClient, QuantilesError, QuantilesRun, StepParams


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


@pytest.fixture
def mock_session() -> AsyncMock:
  return AsyncMock(spec=aiohttp.ClientSession)


@pytest.fixture
def client(mock_session: AsyncMock) -> QuantilesClient:
  return QuantilesClient(base_url="http://test:8080", session=mock_session)


class TestQuantilesClient:
  def test_init_defaults(self) -> None:
    with patch("aiohttp.ClientSession"):
      c = QuantilesClient()
      assert c.base_url == "http://127.0.0.1:8765"

  def test_init_custom_base_url(self) -> None:
    with patch("aiohttp.ClientSession"):
      c = QuantilesClient(base_url="http://custom:1234/")
      assert c.base_url == "http://custom:1234"

  @pytest.mark.asyncio
  async def test_health(self, client: QuantilesClient, mock_session: AsyncMock) -> None:
    _setup_mock_session(mock_session, _make_response("{}"))

    await client.health()
    mock_session.request.assert_called_once_with(
      "GET", "http://test:8080/health", headers={"content-type": "application/json"}, data=None
    )

  @pytest.mark.asyncio
  async def test_create_run(self, client: QuantilesClient, mock_session: AsyncMock) -> None:
    _setup_mock_session(mock_session, _make_response('{"run_id": 42}'))

    input_value: JsonValue = {"key": "value"}
    run = await client.create_run("test-workflow", input_value)
    assert isinstance(run, QuantilesRun)
    assert run.id == 42

    _args, kwargs = mock_session.request.call_args
    assert json.loads(kwargs["data"]) == json.loads(
      json.dumps({"workflow_name": "test-workflow", "input": '{"key":"value"}'})
    )

  @pytest.mark.asyncio
  async def test_get_run_with_id(self, client: QuantilesClient, mock_session: AsyncMock) -> None:
    _setup_mock_session(
      mock_session,
      _make_response(
        '{"id": 1, "workflow_name": "wf", "status": "running", '
        '"input": null, "started_at": "2024-01-01", '
        '"finished_at": null, "error": null}'
      ),
    )

    record = await client.get_run(1)
    assert record.id == 1

  @pytest.mark.asyncio
  async def test_get_run_without_id_raises_when_env_missing(self, client: QuantilesClient) -> None:
    with pytest.raises(QuantilesError, match="QUANTILES_RUN_ID"):
      await client.get_run()

  @pytest.mark.asyncio
  async def test_complete_run(self, client: QuantilesClient, mock_session: AsyncMock) -> None:
    _setup_mock_session(mock_session, _make_response("{}"))

    await client.complete_run(42)
    _args, kwargs = mock_session.request.call_args
    assert json.loads(kwargs["data"]) == {"output": None}

  @pytest.mark.asyncio
  async def test_fail_run(self, client: QuantilesClient, mock_session: AsyncMock) -> None:
    _setup_mock_session(mock_session, _make_response("{}"))

    await client.fail_run(42, ValueError("something broke"))
    _args, kwargs = mock_session.request.call_args
    body = json.loads(kwargs["data"])
    assert "something broke" in body["error"]

  @pytest.mark.asyncio
  async def test_emit_metric(self, client: QuantilesClient, mock_session: AsyncMock) -> None:
    _setup_mock_session(mock_session, _make_response("{}"))

    await client.emit_metric(42, "accuracy", 0.95, "percent")
    _args, kwargs = mock_session.request.call_args
    body = json.loads(kwargs["data"])
    assert body == {"metric_name": "accuracy", "metric_value": 0.95, "unit": "percent"}

  @pytest.mark.asyncio
  async def test_run_step_runs_when_server_says_run(
    self, client: QuantilesClient, mock_session: AsyncMock
  ) -> None:
    _setup_mock_session(
      mock_session,
      _make_response('{"decision": "run", "step_id": 7}'),
      _make_response("{}"),
    )

    executed = False

    async def do_step() -> JsonValue:
      nonlocal executed
      executed = True
      return {"result": "ok"}

    input_value: JsonValue = {"input": 1}
    result = await client.run_step(
      run_id=1,
      step_key="my-step",
      input_value=input_value,
      execute=do_step,
    )
    assert executed
    assert result == {"result": "ok"}

  @pytest.mark.asyncio
  async def test_run_step_reuses_when_server_says_reuse(
    self, client: QuantilesClient, mock_session: AsyncMock
  ) -> None:
    _setup_mock_session(
      mock_session,
      _make_response('{"decision": "reuse", "output": "{\\"cached\\": true}"}'),
    )

    async def do_step() -> JsonValue:
      return {"should_not": "execute"}

    input_value: JsonValue = {"input": 1}
    result = await client.run_step(
      run_id=1,
      step_key="my-step",
      input_value=input_value,
      execute=do_step,
    )
    assert result == {"cached": True}

  @pytest.mark.asyncio
  async def test_run_step_fails_step_on_exception(
    self, client: QuantilesClient, mock_session: AsyncMock
  ) -> None:
    _setup_mock_session(
      mock_session,
      _make_response('{"decision": "run", "step_id": 7}'),
      _make_response("{}"),
    )

    async def do_step() -> JsonValue:
      raise ValueError("boom")

    with pytest.raises(ValueError, match="boom"):
      await client.run_step(
        run_id=1,
        step_key="my-step",
        input_value={},
        execute=do_step,
      )

    assert mock_session.request.call_count == 2
    _args, kwargs = mock_session.request.call_args
    body = json.loads(kwargs["data"])
    assert "boom" in body["error"]


class TestQuantilesRun:
  @pytest.mark.asyncio
  async def test_run_step_delegates_to_client(
    self, client: QuantilesClient, mock_session: AsyncMock
  ) -> None:
    _setup_mock_session(
      mock_session,
      _make_response('{"decision": "run", "step_id": 7}'),
      _make_response("{}"),
    )

    run = QuantilesRun(client, 99)

    async def do_step() -> JsonValue:
      return {"ok": True}

    result = await run.step(
      params=StepParams(key="test-step", execute=do_step),
    )
    assert result == {"ok": True}

  @pytest.mark.asyncio
  async def test_emit_delegates_to_client(
    self, client: QuantilesClient, mock_session: AsyncMock
  ) -> None:
    _setup_mock_session(mock_session, _make_response("{}"))

    run = QuantilesRun(client, 99)
    await run.emit("latency", 42.0, "ms")
    _args, kwargs = mock_session.request.call_args
    assert json.loads(kwargs["data"])["metric_name"] == "latency"
