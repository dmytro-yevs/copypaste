#!/usr/bin/env bash
# Cross-compile copypaste-daemon for Windows x86_64 via mingw-w64.
# Outputs to builds/windows-<arch>/.
#
# CAVEAT: Windows daemon currently has stub IPC; build may fail at link time.
# This script is best-effort.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ARCH="${1:-x86_64}"
OUT_BASE="$ROOT/builds"

case "$ARCH" in
  x86_64)
    RUST_TARGET="x86_64-pc-windows-gnu"
    LINKER="x86_64-w64-mingw32-gcc"
    ;;
  *)
    echo "Unknown arch: $ARCH"
    echo "Usage: $0 [x86_64]"
    exit 1
    ;;
esac

if ! command -v "$LINKER" >/dev/null 2>&1; then
  echo "!! ${LINKER} not found (mingw-w64 not installed)."
  echo "   Install (macOS): brew install mingw-w64"
  echo "   Install (Linux): apt install mingw-w64"
  exit 1
fi

if ! rustup target list --installed | grep -q "^${RUST_TARGET}$"; then
  echo "!! rust target '${RUST_TARGET}' not installed."
  echo "   Install: rustup target add ${RUST_TARGET}"
  exit 1
fi

OUT_DIR="$OUT_BASE/windows-${ARCH}"
mkdir -p "$OUT_DIR"

echo "  -> cargo build --release --target ${RUST_TARGET} -p copypaste-daemon"
echo "  -> (best-effort; Windows IPC is a stub — link errors are expected)"
cargo build --release --target "$RUST_TARGET" -p copypaste-daemon || {
  echo "!! Windows cross-compile failed (expected; daemon IPC is platform-stub)."
  echo "   See scripts/build/README.md for caveats."
  exit 1
}

cp "target/${RUST_TARGET}/release/copypaste-daemon.exe" "$OUT_DIR/" 2>/dev/null || {
  echo "!! No copypaste-daemon.exe produced."
  exit 1
}

echo "  -> wrote $OUT_DIR/copypaste-daemon.exe"
ls -la "$OUT_DIR/"
