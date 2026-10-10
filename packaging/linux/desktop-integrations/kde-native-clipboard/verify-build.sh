#!/usr/bin/env bash
# Build a patched, immutable KWin checkout in an isolated Linux builder.
set -euo pipefail

requested="${1:-all}"
runtime_output="${2:-}"
root="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
work="$(mktemp -d)"
cleanup() {
  rm -rf "$work"
}
trap cleanup EXIT

build_one() {
  local family="$1" tag expected source build actual runtime_id license
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
  if [[ -n "$runtime_output" ]]; then
    [[ "$requested" != all ]] || { echo "ERROR: export one KWin family per runtime output" >&2; exit 1; }
    [[ ! -e "$runtime_output" ]] || { echo "ERROR: runtime output must not exist" >&2; exit 1; }
    mkdir -p "$runtime_output"
    DESTDIR="$runtime_output" cmake --install "$build" --prefix /usr
    license="$source/LICENSES/GPL-2.0-or-later.txt"
    [[ -f "$license" ]] || { echo "ERROR: immutable KWin source license is missing" >&2; exit 1; }
    runtime_id="kwin-${family}-fedora40"
    python3 "$root/../../compositor-runtime/emit_runtime_receipt.py" \
      --runtime-dir "$runtime_output" --output "$runtime_output/runtime-receipt.json" \
      --runtime-id "$runtime_id" --desktop KDE --source-revision "$expected" \
      --patch "$root/patches/kwin-${family}.patch" --glibc-floor 2.39 \
      --dependency plasma-workspace --dependency kglobalaccel \
      --private-entrypoint usr/bin/kwin_wayland --license-file "$license"
  fi
}

case "$requested" in
  all) build_one 6.0; build_one 6.3 ;;
  6.0|6.3) build_one "$requested" ;;
  *) echo "ERROR: use KWin bridge version 6.0, 6.3, or all" >&2; exit 1 ;;
esac
