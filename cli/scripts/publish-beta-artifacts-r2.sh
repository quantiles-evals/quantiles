#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd -- "$script_dir/../.." && pwd)"

bucket="${QT_R2_BUCKET:-quantiles-cli}"
dist_dir="${QT_DIST_DIR:-$repo_dir/dist}"
commit="${GITHUB_SHA:-}"

if ! command -v wrangler >/dev/null 2>&1; then
  echo "wrangler is required to publish to Cloudflare R2" >&2
  exit 1
fi

if [[ -z "$commit" ]]; then
  echo "GITHUB_SHA is required to publish beta artifacts" >&2
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

for object_prefix in "channel/beta/${commit}" "channel/beta/latest"; do
  for file in "${artifacts[@]}"; do
    wrangler r2 object put "${bucket}/${object_prefix}/$(basename "$file")" --file "$file" --remote --cache-control "no-store"
  done
done

printf '%s\n' "$commit" > "$dist_dir/version.txt"
wrangler r2 object put "${bucket}/channel/beta/latest/version.txt" --file "$dist_dir/version.txt" --remote --cache-control "no-store"
