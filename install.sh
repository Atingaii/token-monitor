#!/bin/sh
set -eu

REPO="Atingaii/token-monitor"
TAG="v1.1.0"
DEFAULT_RELEASE_BASE="https://github.com/$REPO/releases/download/$TAG"
API_ASSET_BASE="https://api.github.com/repos/$REPO/releases/assets"
BASE="${TOKEN_MONITOR_RELEASE_BASE:-$DEFAULT_RELEASE_BASE}"

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
case "$STEM" in
  token-monitor-linux-aarch64) ASSET_ID=531094676; CHECKSUM_ID=531094680 ;;
  token-monitor-linux-x86_64) ASSET_ID=531094678; CHECKSUM_ID=531094677 ;;
  token-monitor-macos-aarch64) ASSET_ID=531094694; CHECKSUM_ID=531094679 ;;
  token-monitor-macos-x86_64) ASSET_ID=531094696; CHECKSUM_ID=531094693 ;;
  *) echo "No release asset mapping for $STEM" >&2; exit 1 ;;
esac

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
  for candidate in "$HOME/.local/bin" "$HOME/bin" "$HOME/.cargo/bin"; do
    if path_has_dir "$candidate"; then
      printf '%s\n' "$candidate"
      return
    fi
  done
  printf '%s\n' "$HOME/.local/bin"
}

INSTALL_DIR="$(choose_install_dir)"
WAS_ON_PATH=0
path_has_dir "$INSTALL_DIR" && WAS_ON_PATH=1
TMP="$(mktemp -d 2>/dev/null || mktemp -d -t token-monitor)"
trap 'rm -rf "$TMP"' EXIT INT TERM
mkdir -p "$INSTALL_DIR"

resolve_github_token() {
  for name in TOKEN_MONITOR_GITHUB_TOKEN GITHUB_TOKEN GH_TOKEN; do
    eval "value=\${$name:-}"
    if [ -n "${value:-}" ]; then
      printf '%s' "$value"
      return 0
    fi
  done
  if command -v gh >/dev/null 2>&1; then
    gh auth token 2>/dev/null || true
  fi
}

download_direct() {
  url="$1"
  output="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fL --retry 2 --connect-timeout 15 "$url" -o "$output"
  elif command -v wget >/dev/null 2>&1; then
    wget -q "$url" -O "$output"
  else
    return 1
  fi
}

download_api_asset() {
  asset_id="$1"
  output="$2"
  url="$API_ASSET_BASE/$asset_id"
  token="$(resolve_github_token)"

  if command -v curl >/dev/null 2>&1; then
    if [ -n "$token" ]; then
      curl -fL --retry 2 --connect-timeout 15 \
        -H 'Accept: application/octet-stream' \
        -H 'X-GitHub-Api-Version: 2022-11-28' \
        -H 'User-Agent: token-monitor-installer/1.1.0' \
        -H "Authorization: Bearer $token" \
        "$url" -o "$output" && return 0
    else
      curl -fL --retry 2 --connect-timeout 15 \
        -H 'Accept: application/octet-stream' \
        -H 'X-GitHub-Api-Version: 2022-11-28' \
        -H 'User-Agent: token-monitor-installer/1.1.0' \
        "$url" -o "$output" && return 0
    fi
  fi
  return 1
}

download_release_file() {
  name="$1"
  asset_id="$2"
  output="$3"

  if [ -n "${TOKEN_MONITOR_RELEASE_BASE:-}" ]; then
    download_direct "$BASE/$name" "$output" && return 0
  else
    # Prefer the normal public release URL. Some networks return a misleading 404;
    # fall back to GitHub's release-asset API, optionally using the user's gh login.
    download_direct "$DEFAULT_RELEASE_BASE/$name" "$output" && return 0
    echo "Direct GitHub Release download failed; trying Releases API..." >&2
    download_api_asset "$asset_id" "$output" && return 0
  fi

  echo "Failed to download $name." >&2
  echo "If GitHub returned 403, run 'gh auth login' once and retry." >&2
  exit 1
}

download_release_file "$ASSET" "$ASSET_ID" "$TMP/$ASSET"
download_release_file "$CHECKSUM" "$CHECKSUM_ID" "$TMP/$CHECKSUM"

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

BINARY="$INSTALL_DIR/token-monitor"
"$BINARY" --version

if [ "$WAS_ON_PATH" -eq 0 ]; then
  SHELL_NAME="$(basename "${SHELL:-sh}")"
  case "$SHELL_NAME" in
    zsh) PROFILE="$HOME/.zshrc" ;;
    bash) if [ -f "$HOME/.bashrc" ]; then PROFILE="$HOME/.bashrc"; else PROFILE="$HOME/.profile"; fi ;;
    *) PROFILE="$HOME/.profile" ;;
  esac
  [ -e "$PROFILE" ] || : > "$PROFILE"
  if ! grep -F '# token-monitor' "$PROFILE" >/dev/null 2>&1; then
    printf '\nexport PATH="%s:$PATH" # token-monitor\n' "$INSTALL_DIR" >> "$PROFILE"
  fi
fi

echo
echo "Installed: $BINARY"
if "$BINARY" status >/dev/null 2>&1; then
  echo "Existing Token Monitor configuration detected on this machine."
  echo "Upgrade/migrate it with:"
  printf "  '%s' password\n" "$BINARY"
  printf "  '%s' sync --full\n" "$BINARY"
else
  echo "No local Token Monitor configuration detected on this machine."
  echo "If this is the FIRST device for a new workspace:"
  printf "  '%s' setup\n" "$BINARY"
  echo "If another device already owns the workspace, DO NOT run setup here."
  echo "Run 'token-monitor invite' on the existing device and paste its complete"
  echo "'token-monitor join ...' command on this machine."
fi

if [ "$WAS_ON_PATH" -eq 0 ]; then
  echo "A new terminal will find 'token-monitor' through your shell profile."
fi
