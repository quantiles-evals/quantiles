import os
from functools import cache
from typing import Literal, TypedDict

from openai import AsyncOpenAI
from openai.types.chat import (
  ChatCompletionMessageParam,
  ChatCompletionSystemMessageParam,
  ChatCompletionUserMessageParam,
)

type ModelProvider = Literal["openai"]


class UserMessage(TypedDict):
  role: Literal["user"]
  content: str


class SystemMessage(TypedDict):
  role: Literal["system"]
  content: str


type LLMMessage = UserMessage | SystemMessage


def _llm_message_to_openai(msg: LLMMessage) -> ChatCompletionMessageParam:
  if msg["role"] == "system":
    return ChatCompletionSystemMessageParam(role="system", content=msg["content"])
  return ChatCompletionUserMessageParam(role="user", content=msg["content"])


class LLMResult(TypedDict):
  content: str
  tokens: int


@cache
def _get_openai_client() -> AsyncOpenAI:
  api_key = os.environ.get("OPENAI_API_KEY")
  if not api_key:
    raise RuntimeError("OPENAI_API_KEY environment variable is not set")
  return AsyncOpenAI(api_key=api_key)


async def call_llm(
  provider: ModelProvider,
  model_id: str,
  messages: list[UserMessage | SystemMessage],
  *,
  temperature: int = 1,
) -> LLMResult:
  if provider == "openai":
    client = _get_openai_client()
    response = await client.chat.completions.create(
      model=model_id,
      messages=[_llm_message_to_openai(msg) for msg in messages],
      temperature=temperature,
    )
    content = response.choices[0].message.content or ""
    usage = response.usage
    tokens = 0
    if usage is not None:
      tokens = usage.total_tokens or 0

    return {"content": content, "tokens": tokens}

  raise ValueError(f"model {model_id} is not supported")
