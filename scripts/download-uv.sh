#!/usr/bin/env bash
# Download the uv binary for the current (or specified) platform.
# Usage:
#   ./scripts/download-uv.sh              # auto-detect platform
#   ./scripts/download-uv.sh windows      # explicit
#   ./scripts/download-uv.sh linux
#   ./scripts/download-uv.sh macos

set -euo pipefail

UV_VERSION="0.6.6"
DEST_DIR="src-tauri/resources"

mkdir -p "$DEST_DIR"

# Determine platform
PLATFORM="${1:-}"
if [ -z "$PLATFORM" ]; then
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*|Windows_NT) PLATFORM="windows" ;;
    Darwin)                          PLATFORM="macos" ;;
    Linux)                           PLATFORM="linux" ;;
    *)
      echo "ERROR: Unknown platform $(uname -s). Pass windows/linux/macos as argument."
      exit 1
      ;;
  esac
fi

# Determine architecture
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|AMD64|amd64) ARCH="x86_64" ;;
  aarch64|arm64)      ARCH="aarch64" ;;
  *)
    echo "ERROR: Unsupported architecture $ARCH"
    exit 1
    ;;
esac

# Build download URL
BASE_URL="https://github.com/astral-sh/uv/releases/download/${UV_VERSION}"

case "$PLATFORM" in
  windows)
    ARCHIVE="uv-${ARCH}-pc-windows-msvc.zip"
    BINARY="uv.exe"
    ;;
  linux)
    ARCHIVE="uv-${ARCH}-unknown-linux-gnu.tar.gz"
    BINARY="uv"
    ;;
  macos)
    ARCHIVE="uv-${ARCH}-apple-darwin.tar.gz"
    BINARY="uv"
    ;;
  *)
    echo "ERROR: Unknown platform '$PLATFORM'. Use windows/linux/macos."
    exit 1
    ;;
esac

URL="${BASE_URL}/${ARCHIVE}"
TEMP_DIR="$(mktemp -d)"

echo "Downloading uv ${UV_VERSION} for ${PLATFORM}/${ARCH}..."
echo "  URL: ${URL}"

curl -fSL "$URL" -o "${TEMP_DIR}/${ARCHIVE}"

echo "Extracting..."
case "$ARCHIVE" in
  *.zip)
    unzip -o -j "${TEMP_DIR}/${ARCHIVE}" "*/${BINARY}" -d "${TEMP_DIR}/extracted" 2>/dev/null \
      || unzip -o "${TEMP_DIR}/${ARCHIVE}" -d "${TEMP_DIR}/extracted"
    # Find the binary — it might be in a subdirectory
    UV_BIN="$(find "${TEMP_DIR}/extracted" -name "$BINARY" -type f | head -1)"
    ;;
  *.tar.gz)
    mkdir -p "${TEMP_DIR}/extracted"
    tar xzf "${TEMP_DIR}/${ARCHIVE}" -C "${TEMP_DIR}/extracted"
    UV_BIN="$(find "${TEMP_DIR}/extracted" -name "$BINARY" -type f | head -1)"
    ;;
esac

if [ -z "$UV_BIN" ]; then
  echo "ERROR: Could not find $BINARY in archive"
  rm -rf "$TEMP_DIR"
  exit 1
fi

cp "$UV_BIN" "${DEST_DIR}/${BINARY}"
chmod +x "${DEST_DIR}/${BINARY}"
rm -rf "$TEMP_DIR"

echo "Done! uv binary at ${DEST_DIR}/${BINARY}"
"${DEST_DIR}/${BINARY}" --version
