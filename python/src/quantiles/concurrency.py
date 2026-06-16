"""Concurrent iteration utilities for async workloads."""

import asyncio
import math
from collections.abc import AsyncIterable, AsyncIterator, Awaitable, Callable
from typing import Literal

from pydantic import BaseModel

from .datasets import Dataset


async def iter_async_with_concurrency[T, U](
  items: AsyncIterable[T],
  worker: Callable[[int, T], Awaitable[U | None]],
  *,
  max_concurrency: int | None = None,
  yield_order: Literal["input", "completion"] = "input",
  max_items: int | None = None,
) -> AsyncIterator[U]:
  """Iterate over ``items`` and call ``worker`` concurrently.

  Args:
    items: Async iterable of input items.
    worker: Async function taking ``(index, item)`` and returning a result.
    max_concurrency: Maximum number of concurrent worker calls.
    yield_order: ``"input"`` to yield in input order, ``"completion"`` to
      yield as each task finishes.
    max_items: Optional hard limit on the number of items to process.

  Yields:
    Non-``None`` results from ``worker``.
  """
  if max_concurrency is not None and max_concurrency <= 0:
    raise ValueError(f"max_concurrency must be > 0, got {max_concurrency}")
  if max_items is not None and max_items < 0:
    raise ValueError(f"max_items must be >= 0, got {max_items}")

  indexed_items = aiter(items)
  pending: set[asyncio.Task[tuple[int, U | None]]] = set()
  completed_by_index: dict[int, U | None] = {}
  next_expected_index = 0
  next_item_index = 0
  source_exhausted = False

  async def run_one(idx: int, item: T) -> tuple[int, U | None]:
    return idx, await worker(idx, item)

  async def fill_pending() -> None:
    nonlocal next_item_index, source_exhausted

    while not source_exhausted and len(pending) < (max_concurrency or math.inf):
      if max_items is not None and next_item_index >= max_items:
        source_exhausted = True
        return

      try:
        item = await anext(indexed_items)
      except StopAsyncIteration:
        source_exhausted = True
        return

      pending.add(asyncio.create_task(run_one(next_item_index, item)))
      next_item_index += 1

  try:
    await fill_pending()
    while pending:
      done, pending = await asyncio.wait(pending, return_when=asyncio.FIRST_COMPLETED)
      if yield_order == "completion":
        for task in done:
          _, result = await task
          if result is not None:
            yield result
        await fill_pending()
        continue

      for task in done:
        idx, result = await task
        completed_by_index[idx] = result
      await fill_pending()

      while next_expected_index in completed_by_index:
        next_result = completed_by_index.pop(next_expected_index)
        next_expected_index += 1
        if next_result is not None:
          yield next_result
  except Exception:
    for task in pending:
      task.cancel()
    await asyncio.gather(*pending, return_exceptions=True)
    raise


async def map_dataset[ItemT: BaseModel, U](
  dataset: Dataset[ItemT],
  fn: Callable[[ItemT], Awaitable[U]],
  *,
  max_concurrency: int | None = None,
  yield_order: Literal["input", "completion"] = "input",
  max_items: int | None = None,
) -> AsyncIterator[U]:
  """
  Apply ``fn`` to every row in ``dataset``, with concurrency capped
  to the given ``max_concurrency`` parameter, and return an ``AsyncIterator``
  representing the results of each function call.

  The order in which results are returned is specified by the given ``yield_order``
  paramter. ``"input"`` matches the order of items yielded by the given ``dataset``,
  while ``"completion"`` matches the order in which items are computed.

  Args:
    ``dataset``: Typed dataset to iterate over.
    ``fn``: Async function taking a single row and returning a result.
    ``max_concurrency``: Maximum number of concurrent calls to ``fn``.
    ``yield_order``: ``"input"`` preserves dataset order; ``"completion"`` yields
      results as they finish.
    ``max_items``: Optional limit on total rows to process.

  Yields:
    Results from ``fn`` in the requested order.
  """

  async def _worker(_idx: int, item: ItemT) -> U | None:
    return await fn(item)

  async for result in iter_async_with_concurrency(
    dataset.iter_rows(),
    _worker,
    max_concurrency=max_concurrency,
    yield_order=yield_order,
    max_items=max_items,
  ):
    yield result


async def collect_async_iter[T](iter: AsyncIterator[T]) -> list[T]:
  """
  Iterate through all elements in ``iter``, store them in an in-memory list,
  then return the list.

  Warning: if you pass an iterator that yields many elements, or does expensive
  work for each yielded element, you risk exhausting your computer's memory or
  other resources.
  """
  return [elt async for elt in iter]
