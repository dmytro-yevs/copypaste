#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ui_dir=$(dirname "$script_dir")
workspace_dir=$(CDPATH= cd -- "$ui_dir/../.." && pwd)
checked_in="$ui_dir/src/generated/ipc.ts"
output_dir=$(mktemp -d "${TMPDIR:-/tmp}/copypaste-ipc.XXXXXX")
trap 'rm -rf "$output_dir"' EXIT HUP INT TERM

cargo +1.96 run \
  --manifest-path "$workspace_dir/Cargo.toml" \
  --locked \
  -p copypaste-ui \
  --features typescript \
  --example generate-typescript \
  -- "$output_dir"

generated="$output_dir/ipc.ts"
if [ "${1:-}" = "--check" ]; then
  diff -u "$checked_in" "$generated"
elif [ "$#" -eq 0 ]; then
  mkdir -p "$(dirname "$checked_in")"
  cp "$generated" "$checked_in"
else
  echo "usage: $0 [--check]" >&2
  exit 2
fi
