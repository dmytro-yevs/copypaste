#!/usr/bin/env bash
# Assert the exact v2 D-Bus reply signature without a KWin dependency.
set -euo pipefail

root="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
work="$(mktemp -d)"
cleanup() {
  rm -rf "$work"
}
trap cleanup EXIT
: "${COPYPASTE_QT_PREFIX:?set COPYPASTE_QT_PREFIX to a Qt 6 SDK prefix}"
cmake -S "$root/test-wire" -B "$work/build" -DCMAKE_PREFIX_PATH="$COPYPASTE_QT_PREFIX"
cmake --build "$work/build"
"$work/build/copypaste-kwin-clipboard-wire-fixture"
