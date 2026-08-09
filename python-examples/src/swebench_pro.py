"""Run the official SWE-Bench Pro patch evaluator as a Quantiles workflow."""

import asyncio
import json
from collections.abc import Awaitable, Callable
from pathlib import Path
from typing import cast

from pydantic import BaseModel, Field
from quantiles import emit, entrypoint, step, workflow
from quantiles.types import JsonValue
from quantiles.workflow_context import WorkflowContext


class SweBenchProInput(BaseModel):
    """Inputs required by Scale's official SWE-Bench Pro evaluator."""

    repo_path: str
    evaluator_python: str
    raw_sample_path: str
    patch_path: str
    output_dir: str = ".swebench-pro-results"
    dockerhub_username: str = "jefzda"
    num_workers: int = Field(default=1, ge=1)
    use_local_docker: bool = True
    docker_platform: str | None = None
    block_network: bool = False
    redo: bool = False


def _resolve_path(value: str, base_dir: Path) -> Path:
    path = Path(value).expanduser()
    return path.resolve() if path.is_absolute() else (base_dir / path).resolve()


def _require_file(path: Path, description: str) -> None:
    if not path.is_file():
        raise ValueError(f"{description} does not exist: {path}")


def _summarize_results(results_path: Path) -> dict[str, JsonValue]:
    raw_results = json.loads(results_path.read_text())
    if not isinstance(raw_results, dict) or not all(
        isinstance(instance_id, str) and isinstance(resolved, bool)
        for instance_id, resolved in raw_results.items()
    ):
        raise ValueError(f"Unexpected SWE-Bench Pro results in {results_path}")

    results = cast(dict[str, bool], raw_results)
    total_count = len(results)
    if total_count == 0:
        raise ValueError("SWE-Bench Pro evaluator returned no results")

    resolved_count = sum(results.values())
    return cast(
        dict[str, JsonValue],
        {
            "resolution_rate": resolved_count / total_count,
            "resolved_count": resolved_count,
            "total_count": total_count,
            "results_path": str(results_path),
            "results": results,
        },
    )


async def _run_official_evaluator(config: SweBenchProInput) -> JsonValue:
    caller_dir = Path.cwd()
    repo_path = _resolve_path(config.repo_path, caller_dir)
    evaluator_python = _resolve_path(config.evaluator_python, caller_dir)
    evaluator_path = repo_path / "swe_bench_pro_eval.py"
    raw_sample_path = _resolve_path(config.raw_sample_path, caller_dir)
    patch_path = _resolve_path(config.patch_path, caller_dir)
    output_dir = _resolve_path(config.output_dir, caller_dir)
    scripts_dir = repo_path / "run_scripts"

    _require_file(evaluator_path, "Official SWE-Bench Pro evaluator")
    _require_file(evaluator_python, "SWE-Bench Pro Python interpreter")
    _require_file(raw_sample_path, "Raw SWE-Bench Pro sample file")
    _require_file(patch_path, "SWE-Bench Pro predictions file")
    if not scripts_dir.is_dir():
        raise ValueError(
            f"SWE-Bench Pro run scripts directory does not exist: {scripts_dir}"
        )

    output_dir.mkdir(parents=True, exist_ok=True)
    command = [
        str(evaluator_python),
        str(evaluator_path),
        f"--raw_sample_path={raw_sample_path}",
        f"--patch_path={patch_path}",
        f"--output_dir={output_dir}",
        f"--scripts_dir={scripts_dir}",
        f"--num_workers={config.num_workers}",
        f"--dockerhub_username={config.dockerhub_username}",
    ]
    if config.use_local_docker:
        command.append("--use_local_docker")
    if config.docker_platform is not None:
        command.append(f"--docker_platform={config.docker_platform}")
    if config.block_network:
        command.append("--block_network")
    if config.redo:
        command.append("--redo")

    process = await asyncio.create_subprocess_exec(
        *command,
        cwd=repo_path,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, stderr = await process.communicate()
    if process.returncode != 0:
        stdout_tail = stdout.decode(errors="replace")[-2000:]
        stderr_tail = stderr.decode(errors="replace")[-2000:]
        raise RuntimeError(
            "Official SWE-Bench Pro evaluator failed "
            f"with exit code {process.returncode}.\n"
            f"stdout tail:\n{stdout_tail}\n"
            f"stderr tail:\n{stderr_tail}"
        )

    results_path = output_dir / "eval_results.json"
    _require_file(results_path, "SWE-Bench Pro results file")
    return cast(JsonValue, _summarize_results(results_path))


async def handler(input_value: JsonValue, ctx: WorkflowContext) -> JsonValue:
    config = SweBenchProInput.model_validate(input_value)
    summary = await step(
        ctx,
        step_key="evaluate-patches",
        input_value=cast(JsonValue, config.model_dump(mode="json")),
        execute=cast(
            Callable[[], Awaitable[JsonValue]],
            lambda: _run_official_evaluator(config),
        ),
    )
    if not isinstance(summary, dict):
        raise ValueError("SWE-Bench Pro evaluation step returned an invalid summary")

    resolution_rate = summary.get("resolution_rate")
    resolved_count = summary.get("resolved_count")
    total_count = summary.get("total_count")
    if not isinstance(resolution_rate, int | float):
        raise ValueError("SWE-Bench Pro summary is missing resolution_rate")
    if not isinstance(resolved_count, int | float):
        raise ValueError("SWE-Bench Pro summary is missing resolved_count")
    if not isinstance(total_count, int | float):
        raise ValueError("SWE-Bench Pro summary is missing total_count")
    if any(
        isinstance(value, bool)
        for value in (resolution_rate, resolved_count, total_count)
    ):
        raise ValueError("SWE-Bench Pro evaluation summary is missing numeric metrics")

    await emit(ctx, "resolution_rate", float(resolution_rate))
    await emit(ctx, "resolved_count", float(resolved_count))
    await emit(ctx, "total_count", float(total_count))
    return cast(JsonValue, summary)


if __name__ == "__main__":
    swebench_pro = workflow("custom_swebench_pro", handler)
    entrypoint(swebench_pro)
