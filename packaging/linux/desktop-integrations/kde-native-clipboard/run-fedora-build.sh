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
linux_root="$(CDPATH='' cd -- "$root/../.." && pwd)"
container_root="/workspace/linux/desktop-integrations/kde-native-clipboard"
engine="${COPYPASTE_CONTAINER_ENGINE:-docker}"

prepare_runtime_output() {
  if [[ -e "$runtime_output" || -L "$runtime_output" ]]; then
    [[ -d "$runtime_output" && ! -L "$runtime_output" ]] || {
      echo "ERROR: runtime output must be an ordinary directory" >&2
      exit 1
    }
    first_entry="$(find "$runtime_output" -mindepth 1 -maxdepth 1 -print -quit)" || {
      echo "ERROR: could not inspect runtime output" >&2
      exit 1
    }
    [[ -z "$first_entry" ]] || {
      echo "ERROR: runtime output must be empty" >&2
      exit 1
    }
  else
    mkdir -p "$runtime_output"
  fi
}

if [[ -n "$runtime_output" ]]; then
  prepare_runtime_output
fi

build_one() {
  local selected="$1" tag platform_tag
  platform_tag="${platform//\//-}"
  tag="copypaste-kwin-builder:fedora40-${selected}-${platform_tag}"
  "$engine" build --platform "$platform" --build-arg "KWIN_FAMILY=$selected" \
    --file "$root/Dockerfile.fedora40-build" --tag "$tag" "$root"
  if [[ -n "$runtime_output" ]]; then
    "$engine" run --rm --platform "$platform" --volume "$linux_root:/workspace/linux:ro" --volume "$runtime_output:/output" "$tag" \
      bash "$container_root/verify-build.sh" "$selected" /output
  else
    "$engine" run --rm --platform "$platform" --volume "$linux_root:/workspace/linux:ro" "$tag" \
      bash "$container_root/verify-build.sh" "$selected"
  fi
}

case "$family" in
  all) build_one 6.0; build_one 6.3 ;;
  6.0|6.3) build_one "$family" ;;
esac
