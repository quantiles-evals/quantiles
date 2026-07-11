#!/usr/bin/env bash

set -u

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root/custom-nocode-examples" || exit 1

aggregate_exit=0

for benchmark in nocode_custom medqa medmcqa mmlu-pro gpqa; do
  printf '\n===== %s =====\n' "$benchmark"
  if qt run "$benchmark"; then
    printf '===== %s: PASS =====\n' "$benchmark"
  else
    exit_code=$?
    printf '===== %s: FAIL (exit %s) =====\n' "$benchmark" "$exit_code"
    aggregate_exit=1
  fi
done

exit "$aggregate_exit"
