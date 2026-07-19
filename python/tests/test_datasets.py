"""Tests for quantiles.datasets module."""

from collections.abc import Awaitable, Callable, Mapping, Sequence
from typing import cast
from unittest.mock import AsyncMock, MagicMock, patch

import pytest
from pydantic import BaseModel

from quantiles.datasets import Dataset, _HttpCliSource, dataset
from quantiles.types import JsonValue, QuantilesError
from quantiles.workflow_context import WorkflowContext


class _SampleRow(BaseModel):
  id: int
  name: str


class _FakeSource:
  """Fake dataset source for tests."""

  def __init__(self, rows: Sequence[Mapping[str, object]]) -> None:
    self._rows: list[dict[str, JsonValue]] = [cast(dict[str, JsonValue], dict(row)) for row in rows]
    self.initialize_calls = 0

  @property
  def source_id(self) -> str:
    return "fake:source"

  async def initialize(self) -> JsonValue:
    self.initialize_calls += 1
    return cast(JsonValue, {"total_rows": len(self._rows)})

  async def load_batch(self, offset: int, batch_size: int) -> list[dict[str, JsonValue]]:
    return self._rows[offset : offset + batch_size]


def _make_mock_ctx() -> WorkflowContext:
  """Create a mock WorkflowContext that bypasses real step calls."""
  mock_client = AsyncMock()
  mock_client.base_url = "http://test:8765"

  async def fake_run_step(
    *,
    run_id: int,
    step_key: str,
    input_value: JsonValue | None = None,
    execute: Callable[[], Awaitable[JsonValue]] | None = None,
  ) -> JsonValue:
    if execute is not None:
      return await execute()
    return cast(JsonValue, [])

  mock_client.run_step = fake_run_step
  return WorkflowContext(run_id=1, workflow_name="test", client=mock_client)


def _make_cached_init_ctx(
  init_metadata: JsonValue,
) -> tuple[WorkflowContext, list[tuple[str, JsonValue | None]]]:
  """Create a mock context that reuses dataset-init and executes other steps."""
  mock_client = AsyncMock()
  mock_client.base_url = "http://test:8765"
  step_inputs: list[tuple[str, JsonValue | None]] = []

  async def fake_run_step(
    *,
    run_id: int,
    step_key: str,
    input_value: JsonValue | None = None,
    execute: Callable[[], Awaitable[JsonValue]] | None = None,
  ) -> JsonValue:
    step_inputs.append((step_key, input_value))
    if step_key == "dataset-init":
      return init_metadata
    if execute is not None:
      return await execute()
    return cast(JsonValue, [])

  mock_client.run_step = fake_run_step
  return WorkflowContext(run_id=1, workflow_name="test", client=mock_client), step_inputs


def _make_mock_aiohttp(resp: AsyncMock) -> MagicMock:
  """Build a mock aiohttp session with proper async context manager support."""
  post_cm = MagicMock()
  post_cm.__aenter__ = AsyncMock(return_value=resp)
  post_cm.__aexit__ = AsyncMock(return_value=False)

  session = MagicMock()
  session.post = MagicMock(return_value=post_cm)
  session.__aenter__ = AsyncMock(return_value=session)
  session.__aexit__ = AsyncMock(return_value=False)
  return session


class TestHttpCliSource:
  def test_source_id_before_init(self) -> None:
    src = _HttpCliSource("http://test:8765", "huggingface://quantiles/PubMedQA")
    assert src.source_id == "hf:huggingface://quantiles/PubMedQA"

  def test_source_id_with_config_and_split(self) -> None:
    src = _HttpCliSource("http://test:8765", "hf://ds", config="cfg", split="train")
    assert src.source_id == "hf:hf://ds:cfg:train"

  @pytest.mark.asyncio
  async def test_initialize_parses_response(self) -> None:
    src = _HttpCliSource("http://test:8765", "huggingface://quantiles/PubMedQA")
    resp_json = {
      "total_rows": 1000,
      "available_splits": ["train", "test"],
      "selected_split": "test",
      "config": "pqa_labeled",
    }
    resp = AsyncMock()
    resp.status = 200
    resp.json = AsyncMock(return_value=resp_json)

    session = _make_mock_aiohttp(resp)

    with patch("aiohttp.ClientSession", return_value=session):
      result = await src.initialize()

    assert result == cast(JsonValue, resp_json)
    assert src._resolved_config == "pqa_labeled"
    assert src._resolved_split == "test"
    assert src.source_id == "hf:huggingface://quantiles/PubMedQA:pqa_labeled:test"

  def test_apply_init_metadata_parses_cached_response(self) -> None:
    src = _HttpCliSource("http://test:8765", "huggingface://quantiles/PubMedQA")

    src._apply_init_metadata(
      cast(
        JsonValue,
        {
          "total_rows": 1000,
          "available_splits": ["train", "test"],
          "selected_split": "test",
          "config": "pqa_labeled",
        },
      )
    )

    assert src._resolved_config == "pqa_labeled"
    assert src._resolved_split == "test"
    assert src.source_id == "hf:huggingface://quantiles/PubMedQA:pqa_labeled:test"

  def test_apply_init_metadata_rejects_invalid_response(self) -> None:
    src = _HttpCliSource("http://test:8765", "huggingface://quantiles/PubMedQA")

    with pytest.raises(QuantilesError, match="invalid config metadata"):
      src._apply_init_metadata(cast(JsonValue, {"config": 123, "selected_split": "test"}))

  @pytest.mark.asyncio
  async def test_initialize_raises_on_error(self) -> None:
    src = _HttpCliSource("http://test:8765", "huggingface://bad")
    resp = AsyncMock()
    resp.status = 404
    resp.text = AsyncMock(return_value="not found")

    session = _make_mock_aiohttp(resp)

    with (
      patch("aiohttp.ClientSession", return_value=session),
      pytest.raises(QuantilesError, match="dataset init failed"),
    ):
      await src.initialize()

  @pytest.mark.asyncio
  async def test_load_batch_requires_init(self) -> None:
    src = _HttpCliSource("http://test:8765", "huggingface://ds")
    with pytest.raises(QuantilesError, match="not initialized"):
      await src.load_batch(0, 10)

  @pytest.mark.asyncio
  async def test_load_batch_returns_rows(self) -> None:
    src = _HttpCliSource("http://test:8765", "huggingface://ds", config="cfg", split="test")
    resp_json = {"rows": [{"id": 1, "name": "a"}, {"id": 2, "name": "b"}]}
    resp = AsyncMock()
    resp.status = 200
    resp.json = AsyncMock(return_value=resp_json)

    session = _make_mock_aiohttp(resp)

    with patch("aiohttp.ClientSession", return_value=session):
      rows = await src.load_batch(0, 10)

    assert len(rows) == 2
    assert rows[0]["name"] == "a"


class TestDatasetHelper:
  @pytest.mark.asyncio
  async def test_hydrates_huggingface_source_from_cached_init(self) -> None:
    ctx, _step_inputs = _make_cached_init_ctx(
      cast(
        JsonValue,
        {
          "total_rows": 1000,
          "available_splits": ["train", "test"],
          "selected_split": "test",
          "config": "pqa_labeled",
        },
      )
    )
    ds = await dataset(
      ctx,
      source="huggingface://quantiles/PubMedQA",
      row_type=_SampleRow,
      batch_size=10,
    )

    assert isinstance(ds._source, _HttpCliSource)
    assert ds._source._resolved_config == "pqa_labeled"
    assert ds._source._resolved_split == "test"

  @pytest.mark.asyncio
  async def test_huggingface_init_input_includes_source_options(self) -> None:
    ctx, step_inputs = _make_cached_init_ctx(
      cast(
        JsonValue,
        {
          "total_rows": 1000,
          "available_splits": ["train", "test"],
          "selected_split": "test",
          "config": "pqa_labeled",
        },
      )
    )

    _ds = await dataset(
      ctx,
      source="huggingface://quantiles/PubMedQA",
      row_type=_SampleRow,
      batch_size=10,
      config="pqa_labeled",
      split="test",
      revision="abc123",
      max_rows=20,
    )

    assert step_inputs[0] == (
      "dataset-init",
      cast(
        JsonValue,
        {
          "source": "huggingface://quantiles/PubMedQA",
          "config": "pqa_labeled",
          "split": "test",
          "revision": "abc123",
        },
      ),
    )

  @pytest.mark.asyncio
  async def test_custom_source_initialize_runs_when_init_step_is_cached(self) -> None:
    source = _FakeSource([{"id": 1, "name": "alice"}])
    ctx, step_inputs = _make_cached_init_ctx(cast(JsonValue, {"total_rows": 1}))

    ds = await dataset(
      ctx,
      source=source,
      row_type=_SampleRow,
      batch_size=10,
    )

    assert source.initialize_calls == 1
    assert step_inputs[0] == (
      "dataset-init",
      cast(JsonValue, {"source": "fake:source"}),
    )

    collected = []
    async for row in ds.iter_rows():
      collected.append(row)

    assert len(collected) == 1
    assert collected[0].name == "alice"

  @pytest.mark.asyncio
  async def test_accepts_custom_source(self) -> None:
    rows = [{"id": 1, "name": "alice"}]
    ctx = _make_mock_ctx()
    ds = await dataset(
      ctx,
      source=_FakeSource(rows),
      row_type=_SampleRow,
      batch_size=10,
    )

    collected = []
    async for row in ds.iter_rows():
      collected.append(row)

    assert len(collected) == 1
    assert collected[0].id == 1
    assert collected[0].name == "alice"

  @pytest.mark.asyncio
  async def test_rejects_huggingface_options_for_custom_source(self) -> None:
    ctx = _make_mock_ctx()

    with pytest.raises(QuantilesError, match="only supported for Hugging Face"):
      await dataset(
        ctx,
        source=_FakeSource([]),
        row_type=_SampleRow,
        config="cfg",
      )


class TestDatasetIterator:
  @pytest.mark.asyncio
  async def test_iter_rows_basic(self) -> None:
    rows = [{"id": 1, "name": "alice"}, {"id": 2, "name": "bob"}]
    ctx = _make_mock_ctx()
    ds = Dataset(
      ctx,
      source=_FakeSource(rows),
      row_type=_SampleRow,
      batch_size=10,
      on_error="fail",
      transform=None,
      max_rows=None,
    )

    collected = []
    async for row in ds.iter_rows():
      collected.append(row)

    assert len(collected) == 2
    assert collected[0].id == 1
    assert collected[0].name == "alice"
    assert collected[1].id == 2

  @pytest.mark.asyncio
  async def test_iter_rows_respects_max_rows(self) -> None:
    rows = [
      {"id": 1, "name": "a"},
      {"id": 2, "name": "b"},
      {"id": 3, "name": "c"},
      {"id": 4, "name": "d"},
    ]
    ctx = _make_mock_ctx()
    ds = Dataset(
      ctx,
      source=_FakeSource(rows),
      row_type=_SampleRow,
      batch_size=10,
      on_error="fail",
      transform=None,
      max_rows=2,
    )

    collected = []
    async for row in ds.iter_rows():
      collected.append(row)

    assert len(collected) == 2

  @pytest.mark.asyncio
  async def test_iter_rows_skip_on_error(self) -> None:
    rows = [{"id": 1, "name": "ok"}, {"id": "bad", "name": 123}, {"id": 3, "name": "fine"}]
    ctx = _make_mock_ctx()
    ds = Dataset(
      ctx,
      source=_FakeSource(rows),
      row_type=_SampleRow,
      batch_size=10,
      on_error="skip",
      transform=None,
      max_rows=None,
    )

    collected = []
    async for row in ds.iter_rows():
      collected.append(row)

    assert len(collected) == 2
    assert collected[0].id == 1
    assert collected[1].id == 3

  @pytest.mark.asyncio
  async def test_iter_rows_fail_on_error(self) -> None:
    rows = [{"id": 1, "name": "ok"}, {"id": "bad", "name": 123}]
    ctx = _make_mock_ctx()
    ds = Dataset(
      ctx,
      source=_FakeSource(rows),
      row_type=_SampleRow,
      batch_size=10,
      on_error="fail",
      transform=None,
      max_rows=None,
    )

    with pytest.raises(QuantilesError, match="Row validation failed"):
      async for _row in ds.iter_rows():
        pass

  @pytest.mark.asyncio
  async def test_iter_rows_with_transform(self) -> None:
    rows = [{"raw_id": 42, "raw_name": "alice"}]
    ctx = _make_mock_ctx()

    def transform(raw: dict[str, JsonValue]) -> _SampleRow:
      return _SampleRow(id=cast(int, raw["raw_id"]), name=cast(str, raw["raw_name"]))

    ds = Dataset(
      ctx,
      source=_FakeSource(rows),
      row_type=_SampleRow,
      batch_size=10,
      on_error="fail",
      transform=transform,
      max_rows=None,
    )

    collected = []
    async for row in ds.iter_rows():
      collected.append(row)

    assert len(collected) == 1
    assert collected[0].id == 42
    assert collected[0].name == "alice"
