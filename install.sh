#!/bin/sh
set -eu

REPO="Atingaii/token-monitor"
BASE="https://github.com/$REPO/releases/latest/download"
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) PLATFORM="macos" ;;
  Linux) PLATFORM="linux" ;;
  *) echo "Unsupported OS: $OS" >&2; exit 1 ;;
esac
case "$ARCH" in
  x86_64|amd64) ARCHIVE_ARCH="x86_64" ;;
  arm64|aarch64) ARCHIVE_ARCH="aarch64" ;;
  *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

STEM="token-monitor-${PLATFORM}-${ARCHIVE_ARCH}"
ASSET="${STEM}.tar.gz"
CHECKSUM="${STEM}.sha256"
INSTALL_DIR="${TOKEN_MONITOR_INSTALL_DIR:-$HOME/.local/bin}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT INT TERM
mkdir -p "$INSTALL_DIR"

download() {
  url="$1"
  output="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fL --retry 3 --connect-timeout 15 "$url" -o "$output"
  elif command -v wget >/dev/null 2>&1; then
    wget -q "$url" -O "$output"
  else
    echo "curl or wget is required only for downloading the prebuilt binary." >&2
    exit 1
  fi
}

download "$BASE/$ASSET" "$TMP/$ASSET"
download "$BASE/$CHECKSUM" "$TMP/$CHECKSUM"

if command -v sha256sum >/dev/null 2>&1; then
  (cd "$TMP" && sha256sum -c "$CHECKSUM")
elif command -v shasum >/dev/null 2>&1; then
  (cd "$TMP" && shasum -a 256 -c "$CHECKSUM")
else
  echo "No SHA-256 verifier found; refusing an unverified install." >&2
  exit 1
fi

tar -xzf "$TMP/$ASSET" -C "$TMP"
install -m 0755 "$TMP/token-monitor" "$INSTALL_DIR/token-monitor"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    SHELL_NAME="$(basename "${SHELL:-sh}")"
    case "$SHELL_NAME" in
      zsh) PROFILE="$HOME/.zshrc" ;;
      bash) PROFILE="$HOME/.bashrc" ;;
      *) PROFILE="$HOME/.profile" ;;
    esac
    LINE="export PATH=\"$INSTALL_DIR:\$PATH\" # token-monitor"
    if ! grep -F "# token-monitor" "$PROFILE" >/dev/null 2>&1; then
      printf '\n%s\n' "$LINE" >> "$PROFILE"
    fi
    export PATH="$INSTALL_DIR:$PATH"
    echo "Added $INSTALL_DIR to PATH in $PROFILE"
    ;;
esac

echo "Installed: $INSTALL_DIR/token-monitor"
"$INSTALL_DIR/token-monitor" --version
echo
echo "First device: token-monitor setup"
echo "Additional device: paste the 'token-monitor join ...' command printed by an existing device"
