"""Dataset loading with Quantiles step tracking and lazy row iteration."""

from collections.abc import AsyncIterator, Callable
from typing import Literal, Protocol, cast, final

import aiohttp
from pydantic import BaseModel, ValidationError

from .types import JsonValue, QuantilesError
from .workflow import step
from .workflow_context import WorkflowContext


class DatasetSource(Protocol):
  """Protocol defining the interface for pluggable dataset sources."""

  @property
  def source_id(self) -> str:
    """Return a stable identifier for this source, used as part of the step cache key."""
    ...

  async def initialize(self) -> JsonValue:
    """Validate the source and return metadata (e.g. available splits, total rows)."""
    ...

  async def load_batch(self, offset: int, batch_size: int) -> list[dict[str, JsonValue]]:
    """Return a slice of raw rows starting at ``offset`` up to ``batch_size``."""
    ...


class _HttpCliSource(DatasetSource):
  """Dataset source that fetches batches from the qt CLI HTTP API."""

  def __init__(
    self,
    base_url: str,
    source: str,
    config: str | None = None,
    split: str | None = None,
    revision: str | None = None,
  ) -> None:
    self._base_url = base_url.rstrip("/")
    self.source = source
    self.config = config
    self.split = split
    self.revision = revision
    self._resolved_config: str | None = None
    self._resolved_split: str | None = None

  @property
  def source_id(self) -> str:
    parts = ["hf", self.source]
    if self._resolved_config is not None:
      parts.append(self._resolved_config)
    elif self.config is not None:
      parts.append(self.config)
    if self._resolved_split is not None:
      parts.append(self._resolved_split)
    elif self.split is not None:
      parts.append(self.split)
    return ":".join(parts)

  async def initialize(self) -> JsonValue:
    payload: dict[str, JsonValue] = {"source": self.source}
    if self.config is not None:
      payload["config"] = self.config
    if self.split is not None:
      payload["split"] = self.split
    if self.revision is not None:
      payload["revision"] = self.revision

    async with (
      aiohttp.ClientSession() as session,
      session.post(
        f"{self._base_url}/dataset/init",
        json=payload,
      ) as resp,
    ):
      if resp.status >= 400:
        text = await resp.text()
        raise QuantilesError(f"dataset init failed ({resp.status}): {text}")
      data = await resp.json()

    self._resolved_config = data.get("config")
    self._resolved_split = data.get("selected_split")
    return cast(JsonValue, data)

  async def load_batch(self, offset: int, batch_size: int) -> list[dict[str, JsonValue]]:
    config = self._resolved_config or self.config
    split = self._resolved_split or self.split
    if config is None or split is None:
      raise QuantilesError("dataset source not initialized; call initialize() first")

    payload: dict[str, JsonValue] = {
      "source": self.source,
      "config": config,
      "split": split,
      "offset": offset,
      "limit": batch_size,
    }
    if self.revision is not None:
      payload["revision"] = self.revision

    async with (
      aiohttp.ClientSession() as session,
      session.post(
        f"{self._base_url}/datasets/batch",
        json=payload,
      ) as resp,
    ):
      if resp.status >= 400:
        text = await resp.text()
        raise QuantilesError(f"dataset batch failed ({resp.status}): {text}")
      data = await resp.json()

    rows = data.get("rows", [])
    return cast(list[dict[str, JsonValue]], rows)


@final
class Dataset[RowT: BaseModel]:
  """Typed dataset with lazy row loading backed by Quantiles steps."""

  def __init__(
    self,
    ctx: WorkflowContext,
    source: DatasetSource,
    row_type: type[RowT],
    batch_size: int,
    on_error: Literal["skip", "fail"],
    transform: Callable[[dict[str, JsonValue]], RowT] | None,
    max_rows: int | None,
  ) -> None:
    self._ctx = ctx
    self._source = source
    self._row_type = row_type
    self._batch_size = batch_size
    self._on_error = on_error
    self._transform = transform
    self._max_rows = max_rows
    self._buffer: list[dict[str, JsonValue]] = []
    self._offset = 0
    self._yielded = 0
    self._exhausted = False

  async def iter_rows(self) -> AsyncIterator[RowT]:
    while not self._exhausted:
      if not self._buffer:
        await self._load_next_batch()
        if not self._buffer:
          self._exhausted = True
          break

      raw = self._buffer.pop(0)
      self._offset += 1

      if self._max_rows is not None and self._yielded >= self._max_rows:
        self._exhausted = True
        break

      try:
        if self._transform is not None:
          yield self._transform(raw)
        else:
          yield self._row_type.model_validate(raw)
      except (ValidationError, ValueError) as e:
        if self._on_error == "fail":
          raise QuantilesError(f"Row validation failed at offset {self._offset}: {e}") from e
        # skip silently on "skip"

      self._yielded += 1

  async def _load_next_batch(self) -> None:
    batch_input: dict[str, JsonValue] = {
      "source": self._source.source_id,
      "offset": self._offset,
      "batch_size": self._batch_size,
    }

    async def _execute() -> JsonValue:
      # `load_batch` returns `list[dict[str, JsonValue]]`, which, per the
      # type annotation, is a `JsonValue`, because `JsonValue` has a variant
      # that is `list[JsonValue]`.
      #
      # However, `ty` doesn't unify `list[dict[str, JsonValue]]` with
      # `list[JsonValue]` when resolving the recursive type alias.
      # The cast is a workaround for this limitation. The data is guaranteed
      # by `load_batch`'s return type.
      return cast(JsonValue, await self._source.load_batch(self._offset, self._batch_size))

    batch = await step(
      self._ctx,
      step_key=f"dataset-batch-{self._offset}",
      input_value=cast(JsonValue, batch_input),
      execute=_execute,
    )
    batch_list = batch if isinstance(batch, list) else []
    self._buffer.extend(batch_list)


async def dataset[RowT: BaseModel](
  ctx: WorkflowContext,
  source: str,
  row_type: type[RowT],
  *,
  batch_size: int = 100,
  on_error: Literal["skip", "fail"] = "fail",
  transform: Callable[[dict[str, JsonValue]], RowT] | None = None,
  config: str | None = None,
  split: str | None = None,
  revision: str | None = None,
  max_rows: int | None = None,
  # **kwargs: JsonValue,
) -> Dataset[RowT]:
  ds_source: DatasetSource = _HttpCliSource(
    base_url=ctx.client.base_url,
    source=source,
    config=config,
    split=split,
    revision=revision,
  )

  ds = Dataset(
    ctx,
    ds_source,
    row_type,
    batch_size,
    on_error,
    transform,
    max_rows,
  )

  init_input: dict[str, JsonValue] = {
    "source": source,
    "batch_size": batch_size,
  }
  await step(
    ctx,
    step_key="dataset-init",
    input_value=cast(JsonValue, init_input),
    execute=ds_source.initialize,
  )

  return ds
