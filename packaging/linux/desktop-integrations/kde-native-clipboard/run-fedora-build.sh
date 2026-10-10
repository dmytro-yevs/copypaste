#!/usr/bin/env bash
# Build one immutable KWin source revision in a disposable, pinned Fedora container.
set -euo pipefail

platform="${1:-linux/amd64}"
family="${2:-all}"
runtime_output="${3:-}"
case "$platform" in linux/amd64|linux/arm64) ;; *) echo "ERROR: use linux/amd64 or linux/arm64" >&2; exit 1 ;; esac
case "$family" in 6.0|6.3|all) ;; *) echo "ERROR: use KWin bridge version 6.0, 6.3, or all" >&2; exit 1 ;; esac
[[ -z "$runtime_output" || "$family" != all ]] || { echo "ERROR: runtime export requires one KWin family" >&2; exit 1; }

root="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
engine="${COPYPASTE_CONTAINER_ENGINE:-docker}"

build_one() {
  local selected="$1" tag platform_tag
  platform_tag="${platform//\//-}"
  tag="copypaste-kwin-builder:fedora40-${selected}-${platform_tag}"
  "$engine" build --platform "$platform" --build-arg "KWIN_FAMILY=$selected" \
    --file "$root/Dockerfile.fedora40-build" --tag "$tag" "$root"
  if [[ -n "$runtime_output" ]]; then
    [[ ! -e "$runtime_output" ]] || { echo "ERROR: runtime output must not exist" >&2; exit 1; }
    mkdir -p "$runtime_output"
    "$engine" run --rm --platform "$platform" --volume "$root:/workspace:ro" --volume "$runtime_output:/output" "$tag" \
      bash /workspace/verify-build.sh "$selected" /output
  else
    "$engine" run --rm --platform "$platform" --volume "$root:/workspace:ro" "$tag" \
      bash /workspace/verify-build.sh "$selected"
  fi
}

case "$family" in
  all) build_one 6.0; build_one 6.3 ;;
  6.0|6.3) build_one "$family" ;;
esac
