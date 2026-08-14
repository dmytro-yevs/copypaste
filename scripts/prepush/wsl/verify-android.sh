#!/usr/bin/env bash
# Prove the Android toolchain end to end for the tree you name, the way
# android-emulator.yml does: NDK llvm-ar/llvm-ranlib pinned (ADR-0007), then a
# debug x86_64 APK.
set -euo pipefail
COPYPASTE_GATE_NAME="verify-android.sh"
. "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/gate-lib.sh"

tree="$(copypaste_gate_tree "${1:-}")"
cd "$tree"
copypaste_gate_banner "$tree"

export RUSTUP_TOOLCHAIN=1.96

for tool in perl make; do
    command -v "$tool" >/dev/null || { echo "$tool missing (Android OpenSSL build)"; exit 1; }
done

echo "NDK_HOME=$NDK_HOME"
eval "$(./scripts/release/android-ndk-env.sh "$NDK_HOME" | sed 's/^/export /')"
env | grep -E "^(AR|RANLIB|CC|CXX)_" | sort

(cd crates/copypaste-ui && npm run tauri -- android build --debug --apk --target x86_64)

echo "=== APKs ==="
find "$tree" -name '*.apk' -newermt '-2 hours' -printf '%p  %s bytes\n'
