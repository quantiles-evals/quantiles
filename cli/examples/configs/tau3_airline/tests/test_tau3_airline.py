from dataclasses import dataclass
from pathlib import Path

import pytest

import tau3_airline


def test_parse_config_uses_small_defaults_and_normalizes_models() -> None:
  config = tau3_airline.parse_config(
    {
      "agent_model": "anthropic:claude-sonnet-4-5",
      "user_model": "openai:gpt-4.1",
    }
  )

  assert config.agent_model == "anthropic/claude-sonnet-4-5"
  assert config.user_model == "openai/gpt-4.1"
  assert config.num_tasks == 1
  assert config.num_trials == 1
  assert config.task_ids is None


def test_parse_config_accepts_task_ids() -> None:
  config = tau3_airline.parse_config({"task_ids": ["0", "1"], "seed": 42})

  assert config.task_ids == ("0", "1")
  assert config.seed == 42


@pytest.mark.parametrize(
  ("input_value", "message"),
  [
    ({"num_tasks": 0}, "num_tasks must be a positive integer"),
    ({"num_trials": True}, "num_trials must be a positive integer"),
    ({"task_ids": []}, "task_ids must be a non-empty list of strings"),
    ([], "workflow input must be a JSON object"),
  ],
)
def test_parse_config_rejects_invalid_input(input_value: object, message: str) -> None:
  with pytest.raises(ValueError, match=message):
    tau3_airline.parse_config(input_value)  # type: ignore[arg-type]


def test_summarize_results_uses_official_rewards() -> None:
  summary = tau3_airline.summarize_results(
    {
      "simulations": [
        {"reward_info": {"reward": 1.0}},
        {"reward_info": {"reward": 0.0}},
        {"reward_info": None},
      ]
    }
  )

  assert summary == {
    "simulation_count": 2,
    "average_reward": 0.5,
    "success_rate": 0.5,
  }


def test_infrastructure_failures_extracts_upstream_error() -> None:
  failures = tau3_airline.infrastructure_failures(
    {
      "simulations": [
        {
          "task_id": "0",
          "trial": 0,
          "termination_reason": "infrastructure_error",
          "info": {"error": "provider denied model access"},
        },
        {
          "task_id": "1",
          "trial": 0,
          "termination_reason": "agent_stop",
        },
      ]
    }
  )

  assert failures == [
    {
      "task_id": "0",
      "trial": 0,
      "error": "provider denied model access",
    }
  ]


@dataclass
class _FakeResults:
  payload: dict[str, object]

  def model_dump(self, *, mode: str) -> dict[str, object]:
    assert mode == "json"
    return self.payload


def test_run_tau3_delegates_to_official_runner(
  monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
  captured: dict[str, object] = {}

  def fake_config(**kwargs: object) -> object:
    captured.update(kwargs)
    return captured

  def fake_run_domain(config: object) -> _FakeResults:
    assert config is captured
    return _FakeResults({"simulations": [{"reward_info": {"reward": 1.0}}]})

  monkeypatch.setattr(tau3_airline, "_load_tau3", lambda: (fake_run_domain, fake_config))

  data_dir = tmp_path / "data"
  data_dir.mkdir()
  result = tau3_airline.run_tau3(tau3_airline.Tau3AirlineConfig(data_dir=str(data_dir)))

  assert captured["domain"] == "airline"
  assert captured["task_split_name"] == "base"
  assert captured["agent"] == "llm_agent"
  assert captured["user"] == "user_simulator"
  assert result["summary"] == {
    "simulation_count": 1,
    "average_reward": 1.0,
    "success_rate": 1.0,
  }


def test_run_tau3_requires_upstream_data_checkout(tmp_path: Path) -> None:
  config = tau3_airline.Tau3AirlineConfig(data_dir=str(tmp_path / "missing"))

  with pytest.raises(FileNotFoundError, match="tau3 data directory not found"):
    tau3_airline.run_tau3(config)


def test_run_tau3_raises_for_upstream_infrastructure_failure(
  monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
  def fake_config(**kwargs: object) -> object:
    return kwargs

  def fake_run_domain(_config: object) -> _FakeResults:
    return _FakeResults(
      {
        "simulations": [
          {
            "task_id": "0",
            "trial": 0,
            "termination_reason": "infrastructure_error",
            "info": {"error": "provider denied model access"},
          }
        ]
      }
    )

  monkeypatch.setattr(tau3_airline, "_load_tau3", lambda: (fake_run_domain, fake_config))
  data_dir = tmp_path / "data"
  data_dir.mkdir()
  config = tau3_airline.Tau3AirlineConfig(data_dir=str(data_dir))

  with pytest.raises(
    RuntimeError,
    match=r"tau3 reported 1 infrastructure failure\(s\); task 0, trial 0: provider denied",
  ):
    tau3_airline.run_tau3(config)
