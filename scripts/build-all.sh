#!/usr/bin/env bash
# Multi-platform build orchestrator.
# Usage:
#   bash scripts/build-all.sh             # all platforms (skips missing toolchains)
#   bash scripts/build-all.sh macos       # only macOS (arm64 + x86_64 + universal)
#   bash scripts/build-all.sh android     # only Android (arm64-v8a + armeabi-v7a)
#
#   bash scripts/build-all.sh --docker [android|linux|all]
#     Delegates non-macOS builds to Docker containers (no host pollution).
#     macOS still runs on host (Apple SDK cannot run in Linux container).
#
# Windows: FROZEN 2026-05-23 (ADR-012). The `windows` arm is accepted and
# emits a notice but does not invoke any build steps.
#
# Note: uses plain if-cascade for bash 3.2 compatibility (macOS default shell).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

USE_DOCKER=false
PLATFORM=""
for arg in "$@"; do
  case "$arg" in
    --docker) USE_DOCKER=true ;;
    *) PLATFORM="$arg" ;;
  esac
done
PLATFORM="${PLATFORM:-all}"

if [[ "$USE_DOCKER" == "true" ]]; then
  case "$PLATFORM" in
    android|linux|all)
      echo "==> Delegating $PLATFORM build to Docker"
      exec bash scripts/build-in-docker.sh "$PLATFORM"
      ;;
    windows)
      echo "Windows is frozen as of 2026-05-23 (ADR-012). Skipping Docker build."
      exit 0
      ;;
    macos)
      echo "!! --docker not applicable to macOS (Apple SDK is host-only)."
      echo "   Falling through to host build."
      ;;
    *)
      echo "Unknown platform with --docker: $PLATFORM"
      exit 1
      ;;
  esac
fi

mkdir -p "$ROOT/builds"

# Track outcomes for final summary.
RESULTS=()

run_step() {
  local label="$1"; shift
  echo ""
  echo "==> $label"
  if "$@"; then
    RESULTS+=("  OK    $label")
  else
    RESULTS+=("  SKIP  $label (toolchain missing or build failed; see output above)")
  fi
}

case "$PLATFORM" in
  all|macos|android) ;;
  windows)
    echo "Windows is frozen as of 2026-05-23 (ADR-012). No build steps to run."
    exit 0
    ;;
  *)
    echo "Unknown platform: $PLATFORM"
    echo "Usage: $0 [all|macos|android]     (windows is frozen — ADR-012)"
    exit 1
    ;;
esac

if [[ "$PLATFORM" == "all" || "$PLATFORM" == "macos" ]]; then
  run_step "macOS arm64"      bash scripts/build-macos.sh arm64
  if [[ "$PLATFORM" == "all" || "$PLATFORM" == "macos" ]]; then
    run_step "macOS x86_64"   bash scripts/build-macos.sh x86_64
    run_step "macOS universal" bash scripts/build-macos.sh universal
  fi
fi

if [[ "$PLATFORM" == "all" || "$PLATFORM" == "android" ]]; then
  run_step "Android arm64-v8a"    bash scripts/build-android-pkg.sh arm64-v8a
  run_step "Android armeabi-v7a"  bash scripts/build-android-pkg.sh armeabi-v7a
fi

echo ""
echo "============================================================"
echo "Build summary:"
for r in "${RESULTS[@]}"; do
  echo "$r"
done
echo "============================================================"
echo ""
echo "Build outputs in: $ROOT/builds/"
if command -v find >/dev/null 2>&1; then
  find "$ROOT/builds" -mindepth 1 -maxdepth 2 -print | sort
else
  ls -la "$ROOT/builds/"
fi
