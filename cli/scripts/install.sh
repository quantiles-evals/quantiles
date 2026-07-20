#!/usr/bin/env bash
set -euo pipefail

base_url="https://cli.quantiles.io"

install_dir="${QT_INSTALL_DIR:-$HOME/.quantiles}"
bin_dir="$install_dir/bin"
exe="$bin_dir/qt"

installed_help_text() {
    echo "Quantiles is a full-featured local-native toolchain for running and analyzing AI evals at scale."
    echo ""
    echo "Run your first benchmark example without calling an external model API or incurring any usage charges:"
    echo ""
    echo "    qt run pubmedqa"
    echo ""
    echo "Run qt --help for the complete CLI command reference, or visit quantiles.io/documentation "
    echo "for full documentation."
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "required command not found: $1" >&2
    exit 1
  fi
}

need_cmd curl
need_cmd tar

os="$(uname -s)"
arch="$(uname -m)"

case "${os}:${arch}" in
  Darwin:arm64) target="aarch64-apple-darwin" ;;
  Darwin:x86_64) target="x86_64-apple-darwin" ;;
  Linux:x86_64) target="x86_64-unknown-linux-gnu" ;;
  Linux:aarch64 | Linux:arm64) target="aarch64-unknown-linux-gnu" ;;
  *)
    echo "unsupported platform: ${os}/${arch}" >&2
    exit 1
    ;;
esac

archive="qt-${target}.tar.gz"
checksum="${archive}.sha256"
url="${base_url%/}/releases/latest/${archive}"
checksum_url="${url}.sha256"

tmpdir="$(mktemp -d 2>/dev/null || mktemp -d -t qt-install)"
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT INT TERM

echo "Downloading latest qt for ${target}"
curl -fsSL "$url" -o "$tmpdir/$archive"
curl -fsSL "$checksum_url" -o "$tmpdir/$checksum"

(
  cd "$tmpdir"
  expected="$(awk '{print $1}' "$checksum")"
  if command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "$archive" | awk '{print $1}')"
  elif command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "$archive" | awk '{print $1}')"
  else
    echo "required command not found: shasum or sha256sum" >&2
    exit 1
  fi
  if [ "$actual" != "$expected" ]; then
    echo "checksum mismatch for $archive" >&2
    echo "expected: $expected" >&2
    echo "actual:   $actual" >&2
    exit 1
  fi
)

mkdir -p "$bin_dir"
tar -xzf "$tmpdir/$archive" -C "$tmpdir"
install -m 0755 "$tmpdir/qt" "$exe"

echo "Installed qt to $exe"
echo ""

# If qt is already in PATH, nothing more to do.
if command -v qt >/dev/null 2>&1; then
  installed_help_text
  exit 0
fi

# Helper to abbreviate $HOME as ~
tildify() {
  if [[ "$1" == "$HOME"/* ]]; then
# shellcheck disable=SC2088
    echo "~/${1#"$HOME"/}"
  else
    echo "$1"
  fi
}

refresh_command=""
tilde_bin_dir="$(tildify "$bin_dir")"
quoted_install_dir="${install_dir//\"/\\\"}"

if [[ "$quoted_install_dir" == "\"$HOME/"* ]]; then
  quoted_install_dir="${quoted_install_dir/\"$HOME\//\"\$HOME/}"
fi

# Check if the export block is already present in the given file.
config_has_export() {
  local file="$1"
  [[ -f "$file" ]] && grep -q "# quantiles" "$file" 2>/dev/null && grep -q "$bin_dir" "$file" 2>/dev/null
}

case "$(basename "$SHELL")" in
  fish)
    commands=(
      "set --export QT_INSTALL_DIR $quoted_install_dir"
      "set --export PATH $bin_dir \\$PATH"
    )
    fish_config="$HOME/.config/fish/config.fish"
    tilde_fish_config="$(tildify "$fish_config")"

    if [[ -w "$fish_config" ]] && ! config_has_export "$fish_config"; then
      {
        echo ""
        echo "# quantiles"
        for command in "${commands[@]}"; do
          echo "$command"
        done
      } >> "$fish_config"
      echo "Added \"$tilde_bin_dir\" to \$PATH in \"$tilde_fish_config\""
      refresh_command="source $tilde_fish_config"
    else
      if config_has_export "$fish_config"; then
        echo "qt PATH export already present in \"$tilde_fish_config\""
        refresh_command="source $tilde_fish_config"
      else
        echo "Manually add the directory to $tilde_fish_config (or similar):"
        for command in "${commands[@]}"; do
          echo "  $command"
        done
      fi
    fi
    ;;
  zsh)
    commands=(
      "export QT_INSTALL_DIR=$quoted_install_dir"
      "export PATH=\"$bin_dir:\$PATH\""
    )
    zsh_config="$HOME/.zshrc"
    tilde_zsh_config="$(tildify "$zsh_config")"

    if [[ -w "$zsh_config" ]] && ! config_has_export "$zsh_config"; then
      {
        echo ""
        echo "# quantiles"
        for command in "${commands[@]}"; do
          echo "$command"
        done
      } >> "$zsh_config"
      echo "Added \"$tilde_bin_dir\" to \$PATH in \"$tilde_zsh_config\""
      refresh_command="exec $SHELL"
    else
      if config_has_export "$zsh_config"; then
        echo "qt PATH export already present in \"$tilde_zsh_config\""
        refresh_command="exec $SHELL"
      else
        echo "Manually add the directory to $tilde_zsh_config (or similar):"
        for command in "${commands[@]}"; do
          echo "  $command"
        done
      fi
    fi
    ;;
  bash)
    commands=(
      "export QT_INSTALL_DIR=$quoted_install_dir"
      "export PATH=\"$bin_dir:\$PATH\""
    )
    bash_configs=(
      "$HOME/.bash_profile"
      "$HOME/.bashrc"
    )
    if [[ "${XDG_CONFIG_HOME:-}" ]]; then
      bash_configs+=(
        "$XDG_CONFIG_HOME/.bash_profile"
        "$XDG_CONFIG_HOME/.bashrc"
        "$XDG_CONFIG_HOME/bash_profile"
        "$XDG_CONFIG_HOME/bashrc"
      )
    fi

    set_manually=true
    bash_refresh_command=""
    for bash_config in "${bash_configs[@]}"; do
      tilde_bash_config="$(tildify "$bash_config")"
      if [[ -w "$bash_config" ]] && ! config_has_export "$bash_config"; then
        {
          echo ""
          echo "# quantiles"
          for command in "${commands[@]}"; do
            echo "$command"
          done
        } >> "$bash_config"
        echo "Added \"$tilde_bin_dir\" to \$PATH in \"$tilde_bash_config\""
        refresh_command="source $tilde_bash_config"
        set_manually=false
        break
      elif config_has_export "$bash_config"; then
        bash_refresh_command="source $tilde_bash_config"
      fi
    done

    if [[ "$set_manually" == true ]]; then
      if [[ -n "$bash_refresh_command" ]]; then
        echo "qt PATH export already present in a bash config file."
        refresh_command="$bash_refresh_command"
      else
        echo "Manually add the directory to ~/.bashrc (or similar):"
        for command in "${commands[@]}"; do
          echo "  $command"
        done
      fi
    fi
    ;;
  *)
    echo "Manually add the directory to ~/.bashrc (or similar):"
    echo "  export QT_INSTALL_DIR=$quoted_install_dir"
    echo "  export PATH=\"$bin_dir:\$PATH\""
    ;;
esac

if [[ "$refresh_command" ]]; then
  echo "The 'qt' CLI has been added to your PATH. Run this command to apply the changes:"
  echo "  $refresh_command"
  echo ""
fi

installed_help_text
