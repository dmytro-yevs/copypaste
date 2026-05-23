#!/usr/bin/env bash
# Package copypaste-android .so files into builds/android-<abi>/.
# Distinct from scripts/build-android.sh, which targets android/app/.../jniLibs
# for the Gradle app integration. This script is for the multi-platform
# distribution layout under builds/.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ABI="${1:-arm64-v8a}"
OUT_BASE="$ROOT/builds"
CRATE_DIR="$ROOT/crates/copypaste-android"

abi_to_rust_target() {
  case "$1" in
    arm64-v8a)   echo "aarch64-linux-android"     ;;
    armeabi-v7a) echo "armv7-linux-androideabi"   ;;
    x86_64)      echo "x86_64-linux-android"      ;;
    x86)         echo "i686-linux-android"        ;;
    *) echo ""; return 1                          ;;
  esac
}

if ! command -v cargo-ndk >/dev/null 2>&1; then
  echo "!! cargo-ndk not installed."
  echo "   Install: cargo install cargo-ndk"
  echo "   Also requires: Android NDK + ANDROID_NDK_HOME env var"
  exit 1
fi

RUST_TARGET="$(abi_to_rust_target "$ABI")"
if [[ -z "$RUST_TARGET" ]]; then
  echo "Unknown ABI: $ABI"
  echo "Usage: $0 [arm64-v8a|armeabi-v7a|x86_64|x86]"
  exit 1
fi

if ! rustup target list --installed | grep -q "^${RUST_TARGET}$"; then
  echo "!! rust target '${RUST_TARGET}' not installed."
  echo "   Install: rustup target add ${RUST_TARGET}"
  exit 1
fi

OUT_DIR="$OUT_BASE/android-${ABI}"
mkdir -p "$OUT_DIR"

echo "  -> cargo ndk -t ${ABI} build --release -p copypaste-android"
cargo ndk -t "$ABI" -o "$OUT_DIR" \
  build --release \
  --manifest-path "$CRATE_DIR/Cargo.toml"

# cargo-ndk -o lays out as $OUT_DIR/$ABI/lib*.so; normalize to flat $OUT_DIR/.
if [[ -d "$OUT_DIR/$ABI" ]]; then
  mv "$OUT_DIR/$ABI"/*.so "$OUT_DIR/" 2>/dev/null || true
  rmdir "$OUT_DIR/$ABI" 2>/dev/null || true
fi

echo "  -> wrote $OUT_DIR/libcopypaste_android.so"
ls -la "$OUT_DIR/"
