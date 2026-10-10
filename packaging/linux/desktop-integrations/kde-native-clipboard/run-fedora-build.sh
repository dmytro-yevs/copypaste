#!/usr/bin/env bash
# Build one immutable KWin source revision in a disposable, pinned Fedora container.
set -euo pipefail

platform="${1:-linux/amd64}"
family="${2:-all}"
case "$platform" in linux/amd64|linux/arm64) ;; *) echo "ERROR: use linux/amd64 or linux/arm64" >&2; exit 1 ;; esac
case "$family" in 6.0|6.3|all) ;; *) echo "ERROR: use KWin bridge version 6.0, 6.3, or all" >&2; exit 1 ;; esac

root="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
engine="${COPYPASTE_CONTAINER_ENGINE:-docker}"
tag="copypaste-kwin-builder:fedora40"

"$engine" build --platform "$platform" --file "$root/Dockerfile.fedora40-build" --tag "$tag" "$root"
"$engine" run --rm --platform "$platform" --volume "$root:/workspace:ro" "$tag" \
    bash /workspace/verify-build.sh "$family"
