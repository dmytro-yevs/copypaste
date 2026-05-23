#!/usr/bin/env bash
# generate-android-bindings.sh — Generate Kotlin UniFFI bindings for copypaste-android.
#
# Usage:
#   ./scripts/generate-android-bindings.sh
#
# Prerequisites:
#   - Rust toolchain installed (cargo, rustup)
#
# The script:
#   1. Builds the copypaste-android cdylib and the uniffi-bindgen binary (debug mode).
#   2. Runs uniffi-bindgen against the UDL file to produce Kotlin sources.
#   3. Writes the generated sources to:
#      android/app/src/main/java/com/copypaste/generated/
#
# To regenerate bindings after editing the UDL or Rust API, simply re-run this script.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

CRATE_DIR="${REPO_ROOT}/crates/copypaste-android"
UDL_FILE="${CRATE_DIR}/uniffi/copypaste_android.udl"
OUT_DIR="${REPO_ROOT}/android/app/src/main/java/com/copypaste/generated"
BINDGEN="${REPO_ROOT}/target/debug/uniffi-bindgen"

echo "==> Building copypaste-android (debug)..."
cargo build -p copypaste-android 2>&1

echo "==> Building uniffi-bindgen binary..."
cargo build -p copypaste-android --bin uniffi-bindgen 2>&1

echo "==> Creating output directory: ${OUT_DIR}"
mkdir -p "${OUT_DIR}"

echo "==> Running uniffi-bindgen generate..."
# Must run from within the crate directory so uniffi can locate Cargo.toml.
(cd "${CRATE_DIR}" && \
    "${BINDGEN}" generate "${UDL_FILE}" \
        --language kotlin \
        --out-dir "${OUT_DIR}")

echo ""
echo "Done. Kotlin bindings written to:"
echo "  ${OUT_DIR}"
echo ""
echo "Generated files:"
find "${OUT_DIR}" -name "*.kt"
