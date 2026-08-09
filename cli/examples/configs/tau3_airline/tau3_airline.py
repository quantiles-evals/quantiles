"""Minimal Quantiles custom_code adapter for the official tau3 airline runner."""

import asyncio
import os
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Protocol, cast

from quantiles import JsonValue, WorkflowContext, emit, entrypoint, step, workflow


@dataclass(frozen=True)
class Tau3AirlineConfig:
  data_dir: str = ".tau3/tau2-bench/data"
  agent_model: str = "openai/gpt-4.1"
  user_model: str = "openai/gpt-4.1"
  num_tasks: int = 1
  num_trials: int = 1
  max_steps: int = 100
  max_concurrency: int = 1
  seed: int = 300
  task_ids: tuple[str, ...] | None = None

  def to_json(self) -> dict[str, JsonValue]:
    value = asdict(self)
    if self.task_ids is not None:
      value["task_ids"] = list(self.task_ids)
    return cast(dict[str, JsonValue], value)


class Tau3Results(Protocol):
  def model_dump(self, *, mode: str) -> dict[str, JsonValue]: ...


class RunDomain(Protocol):
  def __call__(self, config: object) -> Tau3Results: ...


class TextRunConfigFactory(Protocol):
  def __call__(self, **kwargs: object) -> object: ...


def _model_name(value: JsonValue, field: str, default: str) -> str:
  if value is None:
    return default
  if not isinstance(value, str) or not value.strip():
    raise ValueError(f"{field} must be a non-empty string")
  model = value.strip()
  if ":" in model and "/" not in model.split(":", 1)[0]:
    provider, name = model.split(":", 1)
    if provider and name:
      return f"{provider}/{name}"
  return model


def _string(value: JsonValue, field: str, default: str) -> str:
  if value is None:
    return default
  if not isinstance(value, str) or not value.strip():
    raise ValueError(f"{field} must be a non-empty string")
  return value.strip()


def _positive_int(value: JsonValue, field: str, default: int) -> int:
  if value is None:
    return default
  if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
    raise ValueError(f"{field} must be a positive integer")
  return value


def _task_ids(value: JsonValue) -> tuple[str, ...] | None:
  if value is None:
    return None
  if not isinstance(value, list) or not value:
    raise ValueError("task_ids must be a non-empty list of strings")
  if not all(isinstance(task_id, str) and task_id for task_id in value):
    raise ValueError("task_ids must be a non-empty list of strings")
  return tuple(cast(list[str], value))


def parse_config(input_value: JsonValue) -> Tau3AirlineConfig:
  if input_value is None:
    values: dict[str, JsonValue] = {}
  elif isinstance(input_value, dict):
    values = input_value
  else:
    raise ValueError("workflow input must be a JSON object")

  return Tau3AirlineConfig(
    data_dir=_string(values.get("data_dir"), "data_dir", ".tau3/tau2-bench/data"),
    agent_model=_model_name(values.get("agent_model"), "agent_model", "openai/gpt-4.1"),
    user_model=_model_name(values.get("user_model"), "user_model", "openai/gpt-4.1"),
    num_tasks=_positive_int(values.get("num_tasks"), "num_tasks", 1),
    num_trials=_positive_int(values.get("num_trials"), "num_trials", 1),
    max_steps=_positive_int(values.get("max_steps"), "max_steps", 100),
    max_concurrency=_positive_int(values.get("max_concurrency"), "max_concurrency", 1),
    seed=_positive_int(values.get("seed"), "seed", 300),
    task_ids=_task_ids(values.get("task_ids")),
  )


def summarize_results(upstream: dict[str, JsonValue]) -> dict[str, JsonValue]:
  raw_simulations = upstream.get("simulations", [])
  simulations = raw_simulations if isinstance(raw_simulations, list) else []
  rewards: list[float] = []

  for simulation in simulations:
    if not isinstance(simulation, dict):
      continue
    reward_info = simulation.get("reward_info")
    if not isinstance(reward_info, dict):
      continue
    reward = reward_info.get("reward")
    if isinstance(reward, int | float) and not isinstance(reward, bool):
      rewards.append(float(reward))

  count = len(rewards)
  average_reward = sum(rewards) / count if count else 0.0
  successful = sum(reward == 1.0 for reward in rewards)
  return {
    "simulation_count": count,
    "average_reward": average_reward,
    "success_rate": successful / count if count else 0.0,
  }


def _load_tau3() -> tuple[RunDomain, TextRunConfigFactory]:
  from tau2.data_model.simulation import TextRunConfig
  from tau2.run import run_domain

  return cast(RunDomain, run_domain), cast(TextRunConfigFactory, TextRunConfig)


def _configure_tau3_data(data_dir: str) -> Path:
  path = Path(data_dir).expanduser().resolve()
  if not path.is_dir():
    raise FileNotFoundError(
      f"tau3 data directory not found at {path}; follow this example's setup instructions"
    )
  os.environ["TAU2_DATA_DIR"] = str(path)
  return path


def run_tau3(config: Tau3AirlineConfig) -> dict[str, JsonValue]:
  _configure_tau3_data(config.data_dir)
  run_domain, text_run_config = _load_tau3()
  upstream_config = text_run_config(
    domain="airline",
    task_split_name="base",
    task_ids=list(config.task_ids) if config.task_ids is not None else None,
    num_tasks=config.num_tasks,
    num_trials=config.num_trials,
    agent="llm_agent",
    user="user_simulator",
    llm_agent=config.agent_model,
    llm_user=config.user_model,
    max_steps=config.max_steps,
    max_concurrency=config.max_concurrency,
    seed=config.seed,
    auto_review=False,
    verbose_logs=False,
  )
  upstream = run_domain(upstream_config).model_dump(mode="json")
  return {
    "benchmark": "tau3-airline",
    "config": config.to_json(),
    "summary": summarize_results(upstream),
    "upstream": upstream,
  }


async def handler(input_value: JsonValue, ctx: WorkflowContext) -> JsonValue:
  config = parse_config(input_value)

  async def execute() -> JsonValue:
    return await asyncio.to_thread(run_tau3, config)

  result = await step(
    ctx,
    step_key="run-official-tau3-airline",
    input_value=config.to_json(),
    execute=execute,
  )
  if not isinstance(result, dict):
    raise TypeError("tau3 step returned a non-object result")
  summary = result.get("summary")
  if not isinstance(summary, dict):
    raise TypeError("tau3 step result is missing its summary")

  for metric_name in ("average_reward", "success_rate", "simulation_count"):
    value = summary.get(metric_name)
    if isinstance(value, int | float) and not isinstance(value, bool):
      await emit(ctx, metric_name, float(value))

  return result


tau3_airline = workflow("tau3-airline", handler)

if __name__ == "__main__":
  entrypoint(tau3_airline)
