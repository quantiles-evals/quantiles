"""A minimal custom eval script for the Quantiles CLI example."""

import json
import os
import sys

run_id = os.environ.get("QUANTILES_RUN_ID", "unknown")
workflow = os.environ.get("QUANTILES_WORKFLOW_NAME", "unknown")
base_url = os.environ.get("QUANTILES_BASE_URL", "unknown")
input_json = os.environ.get("QUANTILES_INPUT", "{}")

try:
    parsed = json.loads(input_json)
except json.JSONDecodeError:
    parsed = {}

print(
    {
        "run_id": run_id,
        "eval_name": workflow,
        "base_url": base_url,
        "parsed_input": parsed,
    }
)

if parsed.get("should_fail"):
    print("Simulated failure triggered by input.", file=sys.stderr)
    sys.exit(1)
