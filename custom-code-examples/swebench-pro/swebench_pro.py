"""Run the official SWE-Bench Pro patch evaluator as a Quantiles workflow."""

import asyncio
import json
import shutil
import sys
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from pathlib import Path
from typing import cast

from pydantic import BaseModel, Field
from quantiles import emit, entrypoint, step, workflow
from quantiles.types import JsonValue
from quantiles.workflow_context import WorkflowContext

_UPSTREAM_REPOSITORY = "https://github.com/scaleapi/SWE-bench_Pro-os.git"


class SweBenchProInput(BaseModel):
    """Optional overrides for Scale's official SWE-Bench Pro evaluator."""

    cache_dir: str = ".swebench-pro-cache"
    repo_path: str | None = None
    evaluator_python: str | None = None
    raw_sample_path: str | None = None
    patch_path: str | None = None
    output_dir: str = ".swebench-pro-results"
    dockerhub_username: str = "jefzda"
    num_workers: int = Field(default=1, ge=1)
    use_local_docker: bool = True
    docker_platform: str | None = None
    block_network: bool = False
    redo: bool = False


@dataclass(frozen=True)
class EvaluatorPaths:
    repo: Path
    python: Path
    raw_samples: Path
    patches: Path
    output: Path


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


def _write_first_gold_patch(source_path: Path, destination_path: Path) -> None:
    raw_patches = json.loads(source_path.read_text())
    if not isinstance(raw_patches, list) or not raw_patches:
        raise ValueError(f"No gold patches found in {source_path}")

    first_patch = raw_patches[0]
    if not isinstance(first_patch, dict):
        raise ValueError(f"Unexpected gold patch in {source_path}")
    instance_id = first_patch.get("instance_id")
    patch = first_patch.get("patch")
    prefix = first_patch.get("prefix")
    if not isinstance(instance_id, str) or not isinstance(patch, str):
        raise ValueError(f"Unexpected gold patch in {source_path}")
    if prefix is not None and not isinstance(prefix, str):
        raise ValueError(f"Unexpected gold patch prefix in {source_path}")

    destination_path.write_text(
        json.dumps(
            [{"instance_id": instance_id, "patch": patch, "prefix": prefix or "gold"}],
            indent=2,
        )
    )


async def _run_command(command: list[str], *, cwd: Path, description: str) -> str:
    process = await asyncio.create_subprocess_exec(
        *command,
        cwd=cwd,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, stderr = await process.communicate()
    stdout_text = stdout.decode(errors="replace")
    stderr_text = stderr.decode(errors="replace")
    if process.returncode != 0:
        raise RuntimeError(
            f"{description} failed with exit code {process.returncode}.\n"
            f"stdout tail:\n{stdout_text[-2000:]}\n"
            f"stderr tail:\n{stderr_text[-2000:]}"
        )
    return stdout_text


async def _ensure_upstream_repo(repo_path: Path, cache_dir: Path) -> None:
    evaluator_path = repo_path / "swe_bench_pro_eval.py"
    if evaluator_path.is_file():
        return
    if repo_path.exists():
        raise ValueError(
            f"SWE-Bench Pro cache is incomplete: {repo_path}. "
            "Remove that cache directory and retry."
        )

    git = shutil.which("git")
    if git is None:
        raise ValueError("git is required to download the SWE-Bench Pro evaluator")
    cache_dir.mkdir(parents=True, exist_ok=True)
    print("Downloading the official SWE-Bench Pro evaluator...", flush=True)
    await _run_command(
        [git, "clone", "--depth", "1", _UPSTREAM_REPOSITORY, str(repo_path)],
        cwd=cache_dir,
        description="SWE-Bench Pro clone",
    )


async def _ensure_upstream_environment(repo_path: Path, evaluator_python: Path) -> None:
    ready_marker = evaluator_python.parent / ".quantiles-ready"
    if evaluator_python.is_file() and ready_marker.is_file():
        return

    uv = shutil.which("uv")
    if uv is None:
        raise ValueError("uv is required to prepare the SWE-Bench Pro evaluator")
    requirements = repo_path / "requirements.txt"
    _require_file(requirements, "SWE-Bench Pro requirements file")

    if not evaluator_python.is_file():
        print("Creating an isolated SWE-Bench Pro Python environment...", flush=True)
        await _run_command(
            [uv, "venv", str(evaluator_python.parent), "--python", sys.executable],
            cwd=repo_path,
            description="SWE-Bench Pro environment creation",
        )

    print("Installing the SWE-Bench Pro evaluator dependencies...", flush=True)
    await _run_command(
        [
            uv,
            "pip",
            "install",
            "--python",
            str(evaluator_python),
            "-r",
            str(requirements),
        ],
        cwd=repo_path,
        description="SWE-Bench Pro dependency installation",
    )
    ready_marker.write_text("ready\n")


async def _ensure_gold_smoke_patch(
    repo_path: Path, evaluator_python: Path, patch_path: Path
) -> None:
    if patch_path.is_file():
        return

    all_gold_patches = patch_path.parent / "gold_patches.json"
    extractor = repo_path / "helper_code" / "extract_gold_patches.py"
    _require_file(extractor, "SWE-Bench Pro gold patch extractor")
    patch_path.parent.mkdir(parents=True, exist_ok=True)

    if not all_gold_patches.is_file():
        print(
            "Downloading the public SWE-Bench Pro dataset and extracting gold patches...",
            flush=True,
        )
        await _run_command(
            [str(evaluator_python), str(extractor), "--output", str(all_gold_patches)],
            cwd=repo_path,
            description="SWE-Bench Pro gold patch extraction",
        )

    _write_first_gold_patch(all_gold_patches, patch_path)


async def _prepare_paths(config: SweBenchProInput) -> EvaluatorPaths:
    caller_dir = Path.cwd()
    cache_dir = _resolve_path(config.cache_dir, caller_dir)
    repo_path = (
        _resolve_path(config.repo_path, caller_dir)
        if config.repo_path is not None
        else cache_dir / "SWE-bench_Pro-os"
    )
    evaluator_python = (
        _resolve_path(config.evaluator_python, caller_dir)
        if config.evaluator_python is not None
        else repo_path / ".venv" / "bin" / "python"
    )

    await _ensure_upstream_repo(repo_path, cache_dir)
    if config.evaluator_python is None:
        await _ensure_upstream_environment(repo_path, evaluator_python)
    else:
        _require_file(evaluator_python, "SWE-Bench Pro Python interpreter")

    raw_sample_path = (
        _resolve_path(config.raw_sample_path, caller_dir)
        if config.raw_sample_path is not None
        else repo_path / "helper_code" / "sweap_eval_full_v2.jsonl"
    )
    patch_path = (
        _resolve_path(config.patch_path, caller_dir)
        if config.patch_path is not None
        else cache_dir / "one_gold_patch.json"
    )
    if config.patch_path is None:
        await _ensure_gold_smoke_patch(repo_path, evaluator_python, patch_path)

    return EvaluatorPaths(
        repo=repo_path,
        python=evaluator_python,
        raw_samples=raw_sample_path,
        patches=patch_path,
        output=_resolve_path(config.output_dir, caller_dir),
    )


async def _run_official_evaluator(config: SweBenchProInput) -> JsonValue:
    paths = await _prepare_paths(config)
    evaluator_path = paths.repo / "swe_bench_pro_eval.py"
    scripts_dir = paths.repo / "run_scripts"

    _require_file(evaluator_path, "Official SWE-Bench Pro evaluator")
    _require_file(paths.python, "SWE-Bench Pro Python interpreter")
    _require_file(paths.raw_samples, "Raw SWE-Bench Pro sample file")
    _require_file(paths.patches, "SWE-Bench Pro predictions file")
    if not scripts_dir.is_dir():
        raise ValueError(
            f"SWE-Bench Pro run scripts directory does not exist: {scripts_dir}"
        )

    if config.use_local_docker:
        docker = shutil.which("docker")
        if docker is None:
            raise ValueError("Docker is required for local SWE-Bench Pro evaluation")
        await _run_command(
            [docker, "info"], cwd=paths.repo, description="Docker availability check"
        )

    paths.output.mkdir(parents=True, exist_ok=True)
    command = [
        str(paths.python),
        str(evaluator_path),
        f"--raw_sample_path={paths.raw_samples}",
        f"--patch_path={paths.patches}",
        f"--output_dir={paths.output}",
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

    print("Running the official SWE-Bench Pro evaluator...", flush=True)
    await _run_command(
        command, cwd=paths.repo, description="Official SWE-Bench Pro evaluator"
    )

    results_path = paths.output / "eval_results.json"
    _require_file(results_path, "SWE-Bench Pro results file")
    summary = _summarize_results(results_path)
    summary["repo_path"] = str(paths.repo)
    summary["patch_path"] = str(paths.patches)
    return cast(JsonValue, summary)


async def handler(input_value: JsonValue, ctx: WorkflowContext) -> JsonValue:
    config = SweBenchProInput.model_validate(input_value or {})
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
  swebench_pro = workflow("swebench-pro", handler)
    entrypoint(swebench_pro)
