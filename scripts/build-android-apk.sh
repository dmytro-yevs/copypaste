#!/usr/bin/env bash
# DEPRECATED: This script is not wired into any CI workflow or Makefile target.
# The active release pipeline performs equivalent steps inline in .github/workflows/release.yml.
# References to android/keystore-beta.jks in this script are stale — that file does not exist.
# Do NOT use this script for production APK builds. See .github/workflows/release.yml instead.
#
# build-android-apk.sh — Full Android APK release build, in-container.
#
# Pipeline:
#   1. cargo ndk -> .so for arm64-v8a, armeabi-v7a, x86_64 into android/app/src/main/jniLibs/
#   2. UniFFI Kotlin bindings (regenerate via uniffi-bindgen against UDL)
#   3. gradle assembleRelease -> app-release-unsigned.apk
#   4. Sign with beta keystore (generate on the fly if missing)
#   5. Copy + sha256 into dist/CopyPaste-v0.2.0-beta.1-android-arm64.apk
#
# All release artefacts live in dist/ only (canonical naming convention:
# CopyPaste-v<version>-<platform>-<arch>.<ext>).
#
# Designed for: docker run --rm -v "$PWD:/workspace" copypaste-builder-android:beta
# Host execution: ABORTS unless IN_DOCKER=1 (project policy: host stays Android-tool-free).
set -euo pipefail

# Pull JAVA_HOME from image-baked profile (Debian's path differs by arch).
[[ -f /etc/profile.d/java_home.sh ]] && source /etc/profile.d/java_home.sh
export JAVA_HOME="${JAVA_HOME:-$(dirname "$(dirname "$(readlink -f "$(which javac)")")")}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

IN_DOCKER="${IN_DOCKER:-0}"
if [[ "$IN_DOCKER" != "1" ]] && [[ ! -f /.dockerenv ]]; then
  echo "!! Refusing to run on host. Project policy: Android build must run inside Docker."
  echo "   Use: docker build -f docker/Dockerfile.android -t copypaste-builder-android:beta . && \\"
  echo "        docker run --rm -v \"\$PWD:/workspace\" copypaste-builder-android:beta"
  exit 2
fi

ANDROID_DIR="$ROOT/android"
CRATE_DIR="$ROOT/crates/copypaste-android"
JNI_BASE="$ANDROID_DIR/app/src/main/jniLibs"
DIST_DIR="$ROOT/dist"

# Derive version from the workspace Cargo.toml so the artifact name tracks
# the single source of truth ([workspace.package] version = "...").
CARGO_VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"
ARTIFACT_NAME="CopyPaste-v${CARGO_VERSION}-android-arm64.apk"

echo "==[1/5] cargo ndk: building .so for arm64-v8a, armeabi-v7a =="
# NOTE: x86_64 ABI is intentionally dropped from the beta build matrix.
# Reason: bundled OpenSSL 3.x (vendored via rusqlite bundled-sqlcipher-vendored-openssl)
# contains SM3 cipher x86_64 assembly (vsm3msg1/vsm3msg2/vsm3rnds2) that NDK r26
# clang's assembler does not recognise. arm64-v8a covers all modern Android devices;
# armeabi-v7a covers older ARMs. x86_64 is only Android emulators / rare Chromebooks
# and is not required for the *-arm64.apk artifact.
mkdir -p "$JNI_BASE"
# CARGO_BUILD_JOBS=1 to avoid OOM under x86_64 emulation on Apple Silicon (~1GB Docker default).
# Build each ABI in a separate invocation so peak memory stays bounded.
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
for abi in arm64-v8a armeabi-v7a; do
  echo "  -> $abi"
  # --lib skips the uniffi-bindgen host tool (built separately in step 2),
  # which otherwise SIGKILLs under Docker-Desktop-VM memory pressure.
  cargo ndk \
    -t "$abi" \
    -o "$JNI_BASE" \
    build --release \
    -p copypaste-android \
    --lib \
    --features android-uniffi-live
done

echo "  .so files produced:"
find "$JNI_BASE" -name "*.so" -exec ls -lh {} \;

echo
echo "==[2/5] UniFFI Kotlin bindings =="
BINDGEN_OUT="$ANDROID_DIR/app/src/main/java/com/copypaste/android/generated"
mkdir -p "$BINDGEN_OUT"
# Build the uniffi-bindgen binary out of the crate, then run it against the UDL.
cargo build -p copypaste-android --bin uniffi-bindgen --release --jobs 1
"$ROOT/target/release/uniffi-bindgen" generate \
  "$CRATE_DIR/uniffi/copypaste_android.udl" \
  --language kotlin \
  --out-dir "$BINDGEN_OUT"
echo "  Generated:"
find "$BINDGEN_OUT" -name "*.kt"

echo
echo "==[3/5] gradle assembleRelease =="
cd "$ANDROID_DIR"
# Use the Gradle wrapper (./gradlew) so the build is tied to the project's
# declared Gradle version rather than whatever `gradle` is on PATH in the image.
# Skip buildCargoNdk task — we already ran cargo-ndk in step 1.
./gradlew --no-daemon assembleRelease -x buildCargoNdk

UNSIGNED_APK="$ANDROID_DIR/app/build/outputs/apk/release/app-release-unsigned.apk"
SIGNED_APK="$ANDROID_DIR/app/build/outputs/apk/release/app-release.apk"

if [[ ! -f "$UNSIGNED_APK" ]] && [[ -f "$SIGNED_APK" ]]; then
  # Some AGP versions name the file already; copy to expected location for signing flow.
  UNSIGNED_APK="$SIGNED_APK"
fi

if [[ ! -f "$UNSIGNED_APK" ]]; then
  echo "!! release APK not found at $UNSIGNED_APK"
  ls -la "$ANDROID_DIR/app/build/outputs/apk/release/" || true
  exit 1
fi

echo
echo "==[4/5] sign (beta keystore) =="
KEYSTORE_PATH="${KEYSTORE_PATH:-$ANDROID_DIR/keystore-beta.jks}"
KEY_ALIAS="${KEY_ALIAS:-copypaste-beta}"

# SECURITY: KEYSTORE_PASS must always be set in the environment.
# Never default to a known password — doing so would silently produce a keystore
# (and signed APK) whose password is publicly known, creating a false sense of
# security. Callers must supply KEYSTORE_PASS explicitly; CI should inject it via
# a secret (e.g. ANDROID_KEYSTORE_PASS GitHub secret).
if [[ -z "${KEYSTORE_PASS:-}" ]]; then
  echo "!! KEYSTORE_PASS is not set." >&2
  echo "   Export KEYSTORE_PASS in your environment before signing." >&2
  echo "   For local beta builds, generate a keystore and set KEYSTORE_PASS to its password." >&2
  exit 1
fi

if [[ ! -f "$KEYSTORE_PATH" ]]; then
  echo "  no keystore at $KEYSTORE_PATH — generating beta keystore"
  keytool -genkeypair \
    -alias "$KEY_ALIAS" \
    -keyalg RSA \
    -keysize 2048 \
    -validity 3650 \
    -keystore "$KEYSTORE_PATH" \
    -storepass "$KEYSTORE_PASS" \
    -keypass "$KEYSTORE_PASS" \
    -dname "CN=CopyPaste Beta, O=CopyPaste, C=UA"
fi

cd "$ROOT"
ALIGNED_APK="$ANDROID_DIR/app/build/outputs/apk/release/app-release-aligned.apk"
zipalign -p -f 4 "$UNSIGNED_APK" "$ALIGNED_APK"
apksigner sign \
  --ks "$KEYSTORE_PATH" \
  --ks-pass pass:"$KEYSTORE_PASS" \
  --key-pass pass:"$KEYSTORE_PASS" \
  --ks-key-alias "$KEY_ALIAS" \
  --v1-signing-enabled true \
  --v2-signing-enabled true \
  --v3-signing-enabled true \
  --out "$SIGNED_APK" \
  "$ALIGNED_APK"
apksigner verify --print-certs "$SIGNED_APK" | head -5

echo
echo "==[5/5] dist + sha256 =="
mkdir -p "$DIST_DIR"
cp "$SIGNED_APK" "$DIST_DIR/$ARTIFACT_NAME"
( cd "$DIST_DIR" && sha256sum "$ARTIFACT_NAME" > "${ARTIFACT_NAME}.sha256" )
ls -lh "$DIST_DIR/$ARTIFACT_NAME" "$DIST_DIR/${ARTIFACT_NAME}.sha256"

echo
echo "==[verify] aapt badging =="
aapt dump badging "$DIST_DIR/$ARTIFACT_NAME" | head -20

echo
echo "DONE: $DIST_DIR/$ARTIFACT_NAME"
