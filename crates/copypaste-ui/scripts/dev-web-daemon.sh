#!/usr/bin/env sh
set -eu

ui_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
repo_dir=$(CDPATH= cd -- "$ui_dir/../.." && pwd)
daemon_bin="$repo_dir/target/debug/copypaste-daemon"
cli_bin="$repo_dir/target/debug/copypaste"
bridge_env=$(mktemp "${TMPDIR:-/tmp}/copypaste-web-bridge.XXXXXX")
daemon_owned=false

cleanup() {
  kill "${bridge_pid:-}" 2>/dev/null || true
  wait "${bridge_pid:-}" 2>/dev/null || true
  if [ "$daemon_owned" = true ]; then
    kill "${daemon_pid:-}" 2>/dev/null || true
    wait "${daemon_pid:-}" 2>/dev/null || true
  fi
  rm -f "$bridge_env"
}
trap cleanup EXIT INT TERM

cargo build --manifest-path "$repo_dir/Cargo.toml" -p copypaste-daemon
if [ ! -x "$cli_bin" ]; then
  cargo build --manifest-path "$repo_dir/Cargo.toml" -p copypaste-cli
fi

# Native CopyPaste and the browser bridge use the same daemon socket.  Reuse a
# responsive daemon when one is already running; only this script's own child
# is stopped on exit.  A stale socket simply fails `status` and is recovered by
# starting a new daemon below.
if "$cli_bin" status >/dev/null 2>&1; then
  echo "Reusing the running CopyPaste daemon for the browser bridge."
else
  "$daemon_bin" --foreground &
  daemon_pid=$!
  daemon_owned=true
  attempt=0
  while ! "$cli_bin" status >/dev/null 2>&1 && [ "$attempt" -lt 50 ]; do
    attempt=$((attempt + 1))
    sleep 0.1
  done
  if ! "$cli_bin" status >/dev/null 2>&1; then
    echo "CopyPaste daemon did not become ready." >&2
    exit 1
  fi
fi

COPYPASTE_WEB_BRIDGE_ENV_FILE="$bridge_env" \
  cargo run --manifest-path "$ui_dir/src-tauri/Cargo.toml" \
    --features dev-web-bridge --bin copypaste-web-bridge &
bridge_pid=$!

attempt=0
while [ ! -s "$bridge_env" ] && [ "$attempt" -lt 100 ]; do
  attempt=$((attempt + 1))
  sleep 0.1
done

if [ ! -s "$bridge_env" ]; then
  echo "CopyPaste browser bridge did not start." >&2
  exit 1
fi

# The file is created by mktemp (0600), read once, then removed by cleanup.
. "$bridge_env"
export VITE_COPYPASTE_WEB_BRIDGE_URL VITE_COPYPASTE_WEB_BRIDGE_TOKEN
cd "$ui_dir"
if curl --silent --fail --max-time 1 http://localhost:1420/ >/dev/null 2>&1; then
  echo "Vite is already running on http://localhost:1420/."
  echo "Restart it with 'npm run dev:native' so the browser tab receives this bridge."
  wait "$bridge_pid"
else
  npm run dev
fi
