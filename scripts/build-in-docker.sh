#!/usr/bin/env bash
# Wrapper for container-based cross-builds.
#
# Usage:
#   bash scripts/build-in-docker.sh android      # Android arm64-v8a
#   bash scripts/build-in-docker.sh windows      # Windows x86_64 (best-effort)
#   bash scripts/build-in-docker.sh linux        # Linux musl (sanity only — frozen runtime)
#   bash scripts/build-in-docker.sh all          # All three (non-macOS)
#
# macOS: docker cannot run Apple SDK. Use host:
#   bash scripts/build-all.sh macos
#
# Outputs are written to ./builds/<platform>-<arch>/ via bind mount.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if ! command -v docker >/dev/null 2>&1; then
  echo "!! docker not installed."
  echo "   macOS: brew install --cask docker"
  echo "   Linux: see https://docs.docker.com/engine/install/"
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "!! Docker daemon not running. Start Docker Desktop / dockerd and retry."
  exit 1
fi

PLATFORM="${1:-all}"

run_one() {
  local svc="$1"
  echo ""
  echo "=========================================================="
  echo "==> Docker build: $svc"
  echo "=========================================================="
  docker compose --profile build run --rm "$svc"
}

case "$PLATFORM" in
  android|windows|linux)
    run_one "$PLATFORM"
    ;;
  all)
    rc=0
    for p in android windows linux; do
      run_one "$p" || { echo "(skipped: $p failed — continuing)"; rc=1; }
    done
    exit "$rc"
    ;;
  -h|--help|help)
    grep '^#' "$0" | sed 's/^# \{0,1\}//'
    exit 0
    ;;
  *)
    echo "Unknown platform: $PLATFORM"
    echo "Usage: $0 [android|windows|linux|all]"
    echo ""
    echo "macOS builds run on host: bash scripts/build-all.sh macos"
    exit 1
    ;;
esac

echo ""
echo "Outputs in: $ROOT/builds/"
ls -la "$ROOT/builds/" 2>/dev/null || echo "(no builds/ dir yet)"
