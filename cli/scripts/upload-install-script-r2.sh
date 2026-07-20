#!/usr/bin/env bash
set -euo pipefail

bucket="${QT_R2_BUCKET:-quantiles-cli}"
dist_dir="${QT_DIST_DIR:-dist}"

if ! command -v wrangler >/dev/null 2>&1; then
  echo "wrangler is required to publish to Cloudflare R2" >&2
  exit 1
fi

mkdir -p "$dist_dir"
install_script="${dist_dir}/install.sh"

cp scripts/install.sh "$install_script"
chmod +x "$install_script"

wrangler r2 object put "${bucket}/install.sh" --file "$install_script" --remote --cache-control "no-store"

echo "published installer to r2://${bucket}/install.sh"
