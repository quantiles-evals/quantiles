#!/usr/bin/env bash
set -euo pipefail

# macOS defaults to 256 open files, which is too low for linking large Rust binaries.
ulimit -n 65536 2>/dev/null || true

bucket="${QT_R2_BUCKET:-quantiles-cli}"
dist_dir="${QT_DIST_DIR:-dist}"

if ! command -v wrangler >/dev/null 2>&1; then
  echo "wrangler is required to publish to Cloudflare R2" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required to parse cargo metadata" >&2
  exit 1
fi

version="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')"

if [[ -z "$version" || "$version" == "null" ]]; then
  echo "failed to read crate version from cargo metadata" >&2
  exit 1
fi

mkdir -p "$dist_dir"

object_prefix="releases/v${version}"

echo "Building target: aarch64-apple-darwin"

cargo build --target aarch64-apple-darwin

mac_binary_path="target/aarch64-apple-darwin/debug/qt"
linux_binary_path="./qt-linux"

if [[ ! -x "$mac_binary_path" ]]; then
  echo "built binary not found at ${mac_binary_path}" >&2
  exit 1
fi

if [[ ! -x "$linux_binary_path" ]]; then
  echo "linux binary not found at ${linux_binary_path}. please build it and place it there first." >&2
  exit 1
fi

for target in "aarch64-apple-darwin" "x86_64-unknown-linux-gnu"; do
  if [[ "$target" == "aarch64-apple-darwin" ]]; then
    binary_path="$mac_binary_path"
  else
    binary_path="$linux_binary_path"
  fi

  archive="${dist_dir}/qt-${version}-${target}.tar.gz"
  checksum="${archive}.sha256"

  tmpdir="$(mktemp -d)"
  cp "$binary_path" "$tmpdir/qt"
  COPYFILE_DISABLE=1 tar -czf "$archive" -C "$tmpdir" "qt"
  rm -rf "$tmpdir"
  (
    cd "$dist_dir"
    shasum -a 256 "$(basename "$archive")" > "$(basename "$checksum")"
  )

  wrangler r2 object put "${bucket}/${object_prefix}/$(basename "$archive")" --file "$archive" --remote --cache-control "no-store"
  wrangler r2 object put "${bucket}/${object_prefix}/$(basename "$checksum")" --file "$checksum" --remote --cache-control "no-store"

  echo "published ${archive} to r2://${bucket}/${object_prefix}/$(basename "$archive")"
done

echo "All targets published successfully."
