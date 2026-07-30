#!/usr/bin/env bash
#
# Cross-compile `copypaste-ffi` for Android and generate the Kotlin bindings.
#
# ─────────────────────────────────────────────────────────────────────────────
# THIS SCRIPT HAS NEVER BEEN RUN END TO END.
#
# The machine it was written on cannot reach `dl.google.com`, so there is no
# Android SDK and no NDK on it and the four `cargo build --target` lines below
# have never executed. What *has* been verified there, on x86_64 Linux:
#
#   * `cargo build -p copypaste-ffi --release` produces a cdylib;
#   * `uniffi-bindgen generate --library …` produces
#     `com/copypaste/ffi/copypaste_ffi.kt` from it;
#   * `CARGO_PROFILE_RELEASE_STRIP=none` is *required* for that second step —
#     see the note below. That one is a real trap and was found by hitting it.
#
# Treat the Android-target half as a recipe, not as a tested path.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
APP="$ROOT/apps/android/app"
PROFILE="${PROFILE:-release}"

# The four ABIs `app/build.gradle.kts` lists, and the Rust targets that make
# them. `cargo-ndk` maps between the two and sets the linker; doing it by hand
# means a per-target `CARGO_TARGET_*_LINKER` and an `ar` for each, which is
# exactly the kind of thing CLAUDE.md rule 1 says not to hand-roll.
#
#   cargo install cargo-ndk
#   rustup target add aarch64-linux-android armv7-linux-androideabi \
#       x86_64-linux-android i686-linux-android
#
ABIS=(arm64-v8a armeabi-v7a x86_64 x86)

command -v cargo-ndk >/dev/null 2>&1 || {
    echo "cargo-ndk is not installed. Run: cargo install cargo-ndk" >&2
    exit 1
}
: "${ANDROID_NDK_HOME:?ANDROID_NDK_HOME must point at an installed NDK}"

echo "==> Building libcopypaste_ffi.so for: ${ABIS[*]}"
(
    cd "$ROOT"
    # `-o` puts one `libcopypaste_ffi.so` per ABI directory under jniLibs,
    # which is the layout `sourceSets["main"].jniLibs.srcDirs` expects.
    cargo-ndk ndk \
        $(printf -- '-t %s ' "${ABIS[@]}") \
        -o "$APP/src/main/jniLibs" \
        build -p copypaste-ffi "--$PROFILE"
)

# ---------------------------------------------------------------- bindings
#
# The generator reads the exported surface out of a compiled library — that is
# what proc-macro mode means, and it is why there is no `.udl` to keep in step.
#
# It reads it out of the *symbol table*, and the workspace's release profile
# sets `strip = "symbols"`. A stripped library therefore fails with
# "No UniFFI metadata found in …", which reads like a build-configuration
# problem and is not one. `CARGO_PROFILE_RELEASE_STRIP=none` overrides the
# profile from the environment, so the workspace `Cargo.toml` does not have to
# change and the shipped `.so` files above keep their stripping.
#
# The host build below is only ever a source of metadata. It is never packaged.
echo "==> Generating Kotlin bindings"
(
    cd "$ROOT"
    CARGO_PROFILE_RELEASE_STRIP=none cargo build -p copypaste-ffi --release
    cargo run -p copypaste-ffi --features bindgen --bin uniffi-bindgen -- \
        generate \
        --library "target/release/libcopypaste_ffi.so" \
        --language kotlin \
        --out-dir "$APP/src/main/java"
)

echo "==> Done."
echo "    natives : $APP/src/main/jniLibs/<abi>/libcopypaste_ffi.so"
echo "    bindings: $APP/src/main/java/com/copypaste/ffi/copypaste_ffi.kt"
