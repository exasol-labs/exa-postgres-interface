#!/bin/sh
# One-step installer for exa-postgres-interface.
#
# Usage:
#   curl -fsSL https://github.com/exasol-labs/exa-postgres-interface/releases/latest/download/install.sh | sh
#
# Environment overrides:
#   INSTALL_VERSION   release tag to install (default: latest, e.g. v0.2.0)
#   INSTALL_PREFIX    install directory (default: /opt/exa-postgres-interface
#                     when running as root, $HOME/.local/exa-postgres-interface
#                     otherwise)
#   INSTALL_REPO      GitHub owner/repo (default: exasol-labs/exa-postgres-interface)
#   INSTALL_NO_LAUNCH set to 1 to install without running the interactive
#                     bootstrap

set -eu

REPO="${INSTALL_REPO:-exasol-labs/exa-postgres-interface}"
VERSION="${INSTALL_VERSION:-latest}"
PREFIX="${INSTALL_PREFIX:-}"
NO_LAUNCH="${INSTALL_NO_LAUNCH:-0}"

log() { printf '==> %s\n' "$*"; }
err() { printf 'error: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || err "$1 is required but not installed"; }

need curl
need tar
need sha256sum
need uname
need mktemp

os="$(uname -s)"
[ "$os" = "Linux" ] || err "unsupported OS: $os (only Linux is currently published)"

arch="$(uname -m)"
case "$arch" in
  x86_64|amd64) arch_tag="linux-x86_64" ;;
  *) err "unsupported architecture: $arch (only linux-x86_64 is currently published)" ;;
esac

if [ "$VERSION" = "latest" ]; then
  log "Resolving latest release tag"
  redirect="$(curl -fsSLI -o /dev/null -w '%{url_effective}' \
    "https://github.com/$REPO/releases/latest")"
  VERSION="${redirect##*/}"
  case "$VERSION" in
    v*) : ;;
    *) err "could not resolve latest version (got: '$VERSION')" ;;
  esac
fi

if [ -z "$PREFIX" ]; then
  if [ "$(id -u)" -eq 0 ]; then
    PREFIX="/opt/exa-postgres-interface"
  else
    PREFIX="$HOME/.local/exa-postgres-interface"
  fi
fi

asset_base="exa-postgres-interface-${VERSION}-${arch_tag}"
tarball_url="https://github.com/$REPO/releases/download/$VERSION/$asset_base.tar.gz"
sha_url="$tarball_url.sha256"

log "Installing $REPO $VERSION ($arch_tag) into $PREFIX"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

log "Downloading $asset_base.tar.gz"
curl -fsSL -o "$tmp/$asset_base.tar.gz" "$tarball_url" \
  || err "failed to download $tarball_url"
curl -fsSL -o "$tmp/$asset_base.tar.gz.sha256" "$sha_url" \
  || err "failed to download $sha_url"

log "Verifying SHA256"
(cd "$tmp" && sha256sum -c "$asset_base.tar.gz.sha256" >/dev/null) \
  || err "SHA256 verification failed"

log "Unpacking to $PREFIX"
mkdir -p "$PREFIX"
tar -xzf "$tmp/$asset_base.tar.gz" -C "$tmp"
cp -R "$tmp/$asset_base/." "$PREFIX/"

binary="$PREFIX/bin/exa-postgres-interface"
config="$PREFIX/config/local.toml"

if [ "$(id -u)" -eq 0 ] && [ -d /usr/local/bin ]; then
  ln -sf "$binary" /usr/local/bin/exa-postgres-interface
  log "Linked /usr/local/bin/exa-postgres-interface -> $binary"
fi

log "Installed to $PREFIX"

if [ "$NO_LAUNCH" = "1" ]; then
  log "Skipping interactive launch (INSTALL_NO_LAUNCH=1)"
  log "Run manually: $binary --config $config"
  exit 0
fi

# When invoked as `curl ... | sh`, stdin is the script body, so the gateway's
# interactive bootstrap can't read keystrokes. Reattach the controlling TTY.
if [ -t 0 ]; then
  log "Launching interactive setup (config: $config)"
  exec "$binary" --config "$config"
elif [ -r /dev/tty ]; then
  log "Launching interactive setup (config: $config)"
  exec "$binary" --config "$config" </dev/tty
else
  log "No TTY available; skipping interactive launch."
  log "Run manually: $binary --config $config"
fi
