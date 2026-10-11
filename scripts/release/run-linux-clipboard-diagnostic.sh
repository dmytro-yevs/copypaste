#!/usr/bin/env bash
# Fixture-only observer wrapper. It never makes a qualification claim.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUTPUT=""
diagnostic_seconds=12
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --output) OUTPUT="$2"; shift 2 ;;
    --seconds) diagnostic_seconds="$2"; shift 2 ;;
    --) shift; break ;;
    *) echo "ERROR: unknown clipboard diagnostic argument: $1" >&2; exit 2 ;;
  esac
done
[[ -n "$OUTPUT" && "$diagnostic_seconds" =~ ^[1-9][0-9]*$ && "$diagnostic_seconds" -le 30 && "$#" -gt 0 ]] || exit 2
mkdir -p "$(dirname "$OUTPUT")"
helper="$(mktemp)"
trap 'rm -f "$helper"' EXIT
pkg-config --exists x11
cc -std=c11 -O2 -Wall -Wextra -Werror "$ROOT/scripts/release/linux-x11-clipboard-observer.c" -o "$helper" $(pkg-config --cflags --libs x11) -lXres
export COPYPASTE_DIAGNOSTIC_X11_HELPER="$helper"
export COPYPASTE_DIAGNOSTIC_KWIN_OBSERVER="$ROOT/scripts/release/linux-kwin-clipboard-observer.py"
export COPYPASTE_DIAGNOSTIC_KWIN_OUTPUT="$OUTPUT"
exec "$@"
