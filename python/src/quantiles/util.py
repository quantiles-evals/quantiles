import hashlib
import json
import traceback

from .types import JsonValue


def hash_json(value: JsonValue) -> str:
  return hashlib.sha256(stable_stringify(value).encode("utf-8")).hexdigest()


def stable_stringify(value: JsonValue) -> str:
  if value is None or not isinstance(value, (dict, list)):
    return json.dumps(value)

  if isinstance(value, list):
    items = ",".join(stable_stringify(item) for item in value)
    return f"[{items}]"

  entries = sorted(
    ((k, v) for k, v in value.items() if v is not None),
    key=lambda item: item[0],
  )
  pairs = ",".join(f"{json.dumps(k)}:{stable_stringify(v)}" for k, v in entries)
  return "{" + pairs + "}"


def error_message(error: BaseException | object) -> str:
  if isinstance(error, BaseException):
    return "".join(traceback.format_exception(type(error), error, error.__traceback__))

  return str(error)


async def response_error_message(response: object) -> str:
  import aiohttp

  if not isinstance(response, aiohttp.ClientResponse):
    return str(response)

  text = await response.text()
  if text == "":
    return f"HTTP {response.status}"

  try:
    body = json.loads(text)
    if isinstance(body, dict):
      return body.get("error", text)
    return text
  except json.JSONDecodeError:
    return text
