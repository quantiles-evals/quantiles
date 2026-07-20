#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd -- "$script_dir/../.." && pwd)"

bucket="${QT_R2_BUCKET:-quantiles-cli}"
dist_dir="${QT_DIST_DIR:-$repo_dir/dist}"

if ! command -v wrangler >/dev/null 2>&1; then
  echo "wrangler is required to publish to Cloudflare R2" >&2
  exit 1
fi

artifacts=()
for file in "$dist_dir"/qt-*.tar.gz "$dist_dir"/qt-*.tar.gz.sha256; do
  if [[ -f "$file" ]]; then
    artifacts+=("$file")
  fi
done

if [[ ${#artifacts[@]} -eq 0 ]]; then
  echo "no CLI release artifacts found in $dist_dir" >&2
  exit 1
fi

object_prefixes=("releases/latest")
if [[ "${GITHUB_REF_TYPE:-}" == "tag" ]]; then
  if [[ -z "${GITHUB_REF_NAME:-}" ]]; then
    echo "GITHUB_REF_NAME is required for tag releases" >&2
    exit 1
  fi
  object_prefixes+=("releases/${GITHUB_REF_NAME}")
  printf '%s\n' "$GITHUB_REF_NAME" > "$dist_dir/version.txt"
fi

for object_prefix in "${object_prefixes[@]}"; do
  for file in "${artifacts[@]}"; do
    wrangler r2 object put "${bucket}/${object_prefix}/$(basename "$file")" --file "$file" --remote --cache-control "no-store"
  done
done

if [[ "${GITHUB_REF_TYPE:-}" == "tag" ]]; then
  wrangler r2 object put "${bucket}/releases/latest/version.txt" --file "$dist_dir/version.txt" --remote --cache-control "no-store"
fi
