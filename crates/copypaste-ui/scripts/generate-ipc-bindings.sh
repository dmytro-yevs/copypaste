#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ui_dir=$(dirname "$script_dir")
workspace_dir=$(CDPATH= cd -- "$ui_dir/../.." && pwd)
checked_in=${COPYPASTE_IPC_BINDINGS_CHECKED_IN:-"$ui_dir/src/generated/ipc.ts"}

compare_bindings() {
  git -c core.autocrlf=false diff --no-index --ignore-cr-at-eol -- "$1" "$2"
}

if [ "${1:-}" = "--self-test" ]; then
  fixture_dir=$(mktemp -d "${TMPDIR:-/tmp}/copypaste-ipc-fixtures.XXXXXX")
  trap 'rm -rf "$fixture_dir"' EXIT HUP INT TERM

  printf 'export type Greeting = "hello";\n' > "$fixture_dir/lf.ts"
  printf 'export type Greeting = "hello";\r\n' > "$fixture_dir/crlf.ts"
  printf 'export type Greeting = "goodbye";\n' > "$fixture_dir/changed.ts"
  printf 'export type Greeting = "hello"; \n' > "$fixture_dir/trailing-space.ts"
  cp "$checked_in" "$fixture_dir/stale-capture.ts"
  sed 's/export type CaptureRung =/export type StaleCaptureRung =/' \
    "$fixture_dir/stale-capture.ts" > "$fixture_dir/changed-capture.ts"

  compare_bindings "$fixture_dir/lf.ts" "$fixture_dir/crlf.ts" >/dev/null
  if compare_bindings "$fixture_dir/lf.ts" "$fixture_dir/changed.ts" >/dev/null; then
    echo "content change was not detected" >&2
    exit 1
  fi
  if compare_bindings "$fixture_dir/lf.ts" "$fixture_dir/trailing-space.ts" >/dev/null; then
    echo "trailing-space change was not detected" >&2
    exit 1
  fi
  if compare_bindings "$fixture_dir/stale-capture.ts" "$fixture_dir/changed-capture.ts" >/dev/null; then
    echo "capture binding fixture was not changed" >&2
    exit 1
  fi
  if COPYPASTE_IPC_BINDINGS_CHECKED_IN="$fixture_dir/changed-capture.ts" \
    "$0" --check >/dev/null 2>&1; then
    echo "--check accepted a stale capture binding" >&2
    exit 1
  fi

  echo "IPC binding check self-test passed"
  exit 0
fi

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
  compare_bindings "$checked_in" "$generated"
elif [ "$#" -eq 0 ]; then
  mkdir -p "$(dirname "$checked_in")"
  cp "$generated" "$checked_in"
else
  echo "usage: $0 [--check|--self-test]" >&2
  exit 2
fi
