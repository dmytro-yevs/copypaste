#!/usr/bin/env bash
# Package copypaste-android .so files into builds/android-<abi>/.
# Distinct from scripts/build-android.sh, which targets android/app/.../jniLibs
# for the Gradle app integration. This script is for the multi-platform
# distribution layout under builds/.
#
# Runs on host (needs cargo-ndk + ANDROID_NDK_HOME) OR in the Docker image
# docker/Dockerfile.android (which provides both). Set IN_DOCKER=1 in the
# container to silence host-install instructions.
#
# ----- Caching (Docker path) -------------------------------------------------
# When invoked via `docker compose --profile build run --rm android`, the
# compose service in docker-compose.yml already mounts four named volumes
# for cache persistence:
#
#   cargo-android-cache   -> /usr/local/cargo/registry  (cargo registry)
#   cargo-android-target  -> /workspace/target-android  (cargo target dir)
#   sccache-android       -> /sccache                   (Rust compile cache)
#   ccache-android        -> /ccache                    (C compile cache)
#
# If invoking `docker run` directly instead of compose, replicate the mounts:
#
#   docker run --rm \
#     -v "$PWD:/workspace" \
#     -v copypaste-android-target:/workspace/target-android \
#     -v copypaste-android-registry:/usr/local/cargo/registry \
#     -v copypaste-sccache:/sccache \
#     -v copypaste-ccache:/ccache \
#     -e CARGO_TARGET_DIR=/workspace/target-android \
#     copypaste-builder-android \
#     bash scripts/build-android-pkg.sh arm64-v8a
#
# First cold container = full build (~5-10 min on amd64-xlarge with the
# image's pre-baked openssl/sqlcipher); subsequent runs hit sccache+ccache
# for ~1-2 min incremental on code-only changes. See docs/release/build-perf.md.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ABI="${1:-arm64-v8a}"
OUT_BASE="$ROOT/builds"
CRATE_DIR="$ROOT/crates/copypaste-android"
IN_DOCKER="${IN_DOCKER:-0}"

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
  if [[ "$IN_DOCKER" == "1" ]]; then
    echo "   (unexpected: should be preinstalled in the Android Docker image)"
  else
    echo "   Install on host: cargo install cargo-ndk"
    echo "   Also requires: Android NDK + ANDROID_NDK_HOME env var"
    echo "   Or run in container: bash scripts/build-in-docker.sh android"
  fi
  exit 1
fi

if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
  echo "!! ANDROID_NDK_HOME not set."
  if [[ "$IN_DOCKER" == "1" ]]; then
    echo "   (unexpected: container should export ANDROID_NDK_HOME)"
  else
    echo "   Install Android NDK and export ANDROID_NDK_HOME, or use:"
    echo "     bash scripts/build-in-docker.sh android"
  fi
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
