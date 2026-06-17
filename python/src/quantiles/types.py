import json
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import TypeVar, cast

from pydantic import BaseModel

type JsonScalar = str | int | float | bool | None
type JsonValue = JsonScalar | list[JsonValue] | dict[str, JsonValue]


def to_json_value(str: str) -> JsonValue:
  return cast(JsonValue, json.loads(str))


class RunRecord(BaseModel):
  id: int
  workflow_name: str
  status: str
  input: str | None
  started_at: str
  finished_at: str | None
  error: str | None


TInput = TypeVar("TInput")
TOutput = TypeVar("TOutput")


@dataclass(frozen=True)
class WorkflowDescriptor[TInput, TOutput]:
  name: str
  run: Callable[[TInput | None], Awaitable[TOutput]]


class QuantilesError(Exception):
  pass
