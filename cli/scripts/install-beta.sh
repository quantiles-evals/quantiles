#!/usr/bin/env bash
set -euo pipefail

base_url="https://cli.quantiles.io"
version_url="${base_url}/channels/beta/version"
installer_url="${base_url}/channels/beta/install.sh"

if ! command -v curl >/dev/null 2>&1; then
  echo "required command not found: curl" >&2
  exit 1
fi

if [[ "$(uname -s):$(uname -m)" != "Darwin:arm64" ]]; then
  echo "qt beta builds are currently available only for Apple Silicon Macs" >&2
  exit 1
fi

version="${QT_VERSION:-$(curl -fsSL "$version_url")}"

if [[ ! "$version" =~ ^[0-9A-Za-z][0-9A-Za-z._-]*$ ]]; then
  echo "invalid beta version returned by ${version_url}: ${version}" >&2
  exit 1
fi

echo "Installing qt beta ${version}"
curl -fsSL "$installer_url" | QT_VERSION="$version" bash
