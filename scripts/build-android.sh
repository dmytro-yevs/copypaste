#!/usr/bin/env bash
# DEPRECATED: This script is not wired into CI or any Makefile target.
# Use `make android-so` or `make android-docker` for the canonical build path.
# Profile updated to --profile release-size for consistency with the CI pipeline
# (ci-android-build.yml, release.yml, build-android-pkg.sh).

set -euo pipefail

# Build copypaste-android .so for arm64-v8a using cargo-ndk
# Prerequisites: cargo install cargo-ndk, Android NDK installed, ANDROID_NDK_HOME set

ANDROID_DIR="$(cd "$(dirname "$0")/.." && pwd)/android"
CRATE_DIR="$(cd "$(dirname "$0")/.." && pwd)/crates/copypaste-android"
JNI_DIR="$ANDROID_DIR/app/src/main/jniLibs/arm64-v8a"

echo "Building copypaste-android for arm64-v8a..."
mkdir -p "$JNI_DIR"

cargo ndk \
  -t arm64-v8a \
  -o "$JNI_DIR" \
  build --profile release-size \
  --manifest-path "$CRATE_DIR/Cargo.toml"

echo "Built: $JNI_DIR/libcopypaste_android.so"

# Generate UniFFI Kotlin bindings
cargo run --manifest-path "$CRATE_DIR/Cargo.toml" \
  --features uniffi/cli \
  --bin uniffi-bindgen \
  generate \
  "$CRATE_DIR/uniffi/copypaste_android.udl" \
  --language kotlin \
  --out-dir "$ANDROID_DIR/app/src/main/java/com/copypaste/android/generated/"

echo "Generated Kotlin bindings"
