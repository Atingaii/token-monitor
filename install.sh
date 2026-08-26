#!/bin/sh
set -eu

REPO="Atingaii/token-monitor"
BASE="${TOKEN_MONITOR_RELEASE_BASE:-https://github.com/$REPO/releases/latest/download}"
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

path_has_dir() {
  wanted="$1"
  old_ifs=$IFS
  IFS=:
  for entry in ${PATH:-}; do
    [ "$entry" = "$wanted" ] && { IFS=$old_ifs; return 0; }
  done
  IFS=$old_ifs
  return 1
}

choose_install_dir() {
  if [ -n "${TOKEN_MONITOR_INSTALL_DIR:-}" ]; then
    printf '%s\n' "$TOKEN_MONITOR_INSTALL_DIR"
    return
  fi

  # Prefer common user-owned directories that are already active in this shell.
  for candidate in "$HOME/.local/bin" "$HOME/bin" "$HOME/.cargo/bin"; do
    if path_has_dir "$candidate"; then
      printf '%s\n' "$candidate"
      return
    fi
  done

  # Reuse another writable HOME-scoped PATH directory when available.
  old_ifs=$IFS
  IFS=:
  for candidate in ${PATH:-}; do
    case "$candidate" in
      "$HOME"/*)
        if [ -d "$candidate" ] && [ -w "$candidate" ]; then
          IFS=$old_ifs
          printf '%s\n' "$candidate"
          return
        fi
        ;;
    esac
  done
  IFS=$old_ifs

  printf '%s\n' "$HOME/.local/bin"
}

INSTALL_DIR="$(choose_install_dir)"
WAS_ON_PATH=0
path_has_dir "$INSTALL_DIR" && WAS_ON_PATH=1
TMP="$(mktemp -d 2>/dev/null || mktemp -d -t token-monitor)"
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
    echo "curl or wget is required to download Token Monitor." >&2
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
if command -v install >/dev/null 2>&1; then
  install -m 0755 "$TMP/token-monitor" "$INSTALL_DIR/token-monitor"
else
  cp "$TMP/token-monitor" "$INSTALL_DIR/token-monitor"
  chmod 0755 "$INSTALL_DIR/token-monitor"
fi

# Validate the actual native binary before touching shell startup files.
if ! "$INSTALL_DIR/token-monitor" --version; then
  echo "The downloaded binary could not run on this system." >&2
  if [ "$PLATFORM" = "linux" ]; then
    echo "Please report your distribution, libc (`ldd --version`) and architecture." >&2
  fi
  exit 1
fi

if [ "$WAS_ON_PATH" -eq 0 ]; then
  SHELL_NAME="$(basename "${SHELL:-sh}")"
  case "$SHELL_NAME" in
    zsh) PROFILE="$HOME/.zshrc" ;;
    bash)
      if [ -f "$HOME/.bashrc" ]; then PROFILE="$HOME/.bashrc"; else PROFILE="$HOME/.profile"; fi
      ;;
    *) PROFILE="$HOME/.profile" ;;
  esac
  [ -e "$PROFILE" ] || : > "$PROFILE"
  LINE="export PATH=\"$INSTALL_DIR:\$PATH\" # token-monitor"
  if ! grep -F "# token-monitor" "$PROFILE" >/dev/null 2>&1; then
    printf '\n%s\n' "$LINE" >> "$PROFILE"
  fi
fi

echo
echo "Installed: $INSTALL_DIR/token-monitor"
if [ "$WAS_ON_PATH" -eq 1 ]; then
  echo "First device: token-monitor setup"
  echo "Additional device: paste the 'token-monitor join ...' command printed by an existing device"
else
  echo "Your current parent shell cannot inherit PATH changes from a curl|sh installer."
  echo "Run setup immediately with:"
  printf "  '%s/token-monitor' setup\n" "$INSTALL_DIR"
  echo "New terminals will also find 'token-monitor' through your shell profile."
fi
