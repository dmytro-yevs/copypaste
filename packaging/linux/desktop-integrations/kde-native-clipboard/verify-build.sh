#!/usr/bin/env bash
# Build a patched, immutable KWin checkout in an isolated Linux builder.
set -euo pipefail

requested="${1:-all}"
root="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
work="$(mktemp -d)"
cleanup() {
  rm -rf "$work"
}
trap cleanup EXIT

build_one() {
  local family="$1" tag expected source build actual
  case "$family" in
    6.0)
      tag=v6.0.0
      expected=1ddcb4e288c4f7dcecdc94efccd655b7e3666d30
      ;;
    6.3)
      tag=v6.3.0
      expected=3e19ea5a1bd69fa619aa5fd3c1b285e5b9168b5b
      ;;
    *) echo "ERROR: use KWin bridge version 6.0, 6.3, or all" >&2; exit 1 ;;
  esac
  source="$work/kwin-$family"
  build="$work/build-$family"
  git -c protocol.version=2 clone --filter=blob:none --depth=1 --branch "$tag" \
    https://invent.kde.org/plasma/kwin.git "$source"
  actual="$(git -C "$source" rev-parse HEAD)"
  [[ "$actual" == "$expected" ]] || {
    echo "ERROR: $tag resolved to $actual, expected $expected" >&2
    exit 1
  }
  "$root/apply-to-kwin-source.sh" "$family" "$source"
  cmake -S "$source" -B "$build" -G Ninja -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF
  cmake --build "$build" --parallel "${COPYPASTE_KWIN_BUILD_JOBS:-2}"
}

case "$requested" in
  all) build_one 6.0; build_one 6.3 ;;
  6.0|6.3) build_one "$requested" ;;
  *) echo "ERROR: use KWin bridge version 6.0, 6.3, or all" >&2; exit 1 ;;
esac
