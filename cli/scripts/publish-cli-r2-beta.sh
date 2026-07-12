#!/usr/bin/env bash
set -euo pipefail

# macOS defaults to 256 open files, which is too low for linking large Rust binaries.
ulimit -n 65536 2>/dev/null || true

bucket="${QT_R2_BUCKET:-quantiles-cli}"
dist_dir="${QT_DIST_DIR:-dist}"

for command in cargo git jq rustup shasum tar wrangler; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "${command} is required to publish a beta CLI build" >&2
    exit 1
  fi
done

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "beta publishing currently builds macOS targets and must run on macOS" >&2
  exit 1
fi

crate_version="$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')"
if [[ -z "$crate_version" || "$crate_version" == "null" ]]; then
  echo "failed to read crate version from cargo metadata" >&2
  exit 1
fi

timestamp="$(date -u +%Y%m%d%H%M%S)"
commit="$(git rev-parse --short HEAD)"
version="${QT_BETA_VERSION:-${crate_version}-beta.${timestamp}.${commit}}"

if [[ ! "$version" =~ ^[0-9A-Za-z][0-9A-Za-z._-]*$ ]]; then
  echo "invalid beta version: ${version}" >&2
  exit 1
fi

target="aarch64-apple-darwin"
installed_targets="$(rustup target list --installed)"
if ! grep -qx "$target" <<< "$installed_targets"; then
  echo "Rust target ${target} is not installed. Run: rustup target add ${target}" >&2
  exit 1
fi

mkdir -p "$dist_dir"
object_prefix="releases/v${version}"

echo "Building beta target: ${target}"
cargo build --target "$target"

binary_path="target/${target}/debug/qt"
if [[ ! -x "$binary_path" ]]; then
  echo "built binary not found at ${binary_path}" >&2
  exit 1
fi

archive="${dist_dir}/qt-${version}-${target}.tar.gz"
checksum="${archive}.sha256"
tmpdir="$(mktemp -d)"
cp "$binary_path" "$tmpdir/qt"
COPYFILE_DISABLE=1 tar -czf "$archive" -C "$tmpdir" qt
rm -rf "$tmpdir"

(
  cd "$dist_dir"
  shasum -a 256 "$(basename "$archive")" > "$(basename "$checksum")"
)

wrangler r2 object put "${bucket}/${object_prefix}/$(basename "$archive")" --file "$archive" --remote --cache-control "no-store"
wrangler r2 object put "${bucket}/${object_prefix}/$(basename "$checksum")" --file "$checksum" --remote --cache-control "no-store"
echo "published ${archive} to r2://${bucket}/${object_prefix}/$(basename "$archive")"

version_file="${dist_dir}/beta-version"
printf '%s\n' "$version" > "$version_file"

wrangler r2 object put "${bucket}/channels/beta/install.sh" --file scripts/install.sh --remote --cache-control "no-store"
wrangler r2 object put "${bucket}/install-beta.sh" --file scripts/install-beta.sh --remote --cache-control "no-store"

# Publish the pointer last so the beta channel changes only after every asset is available.
wrangler r2 object put "${bucket}/channels/beta/version" --file "$version_file" --remote --cache-control "no-store"

echo "Published qt beta ${version} for: ${target}"
echo "Install with: curl -fsSL https://cli.quantiles.io/install-beta.sh | bash"
