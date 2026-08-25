#!/usr/bin/env sh
set -eu

ui_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
repo_dir=$(CDPATH= cd -- "$ui_dir/../.." && pwd)
daemon_bin="$repo_dir/target/debug/copypaste-daemon"
cli_bin="$repo_dir/target/debug/copypaste"
bridge_bin="$repo_dir/target/debug/copypaste-web-bridge"
daemon_owned=false

. "$ui_dir/scripts/web-bridge-runtime.sh"
if ! acquire_bridge_session; then
  exit 0
fi
bridge_env=$(mktemp "${TMPDIR:-/tmp}/copypaste-web-bridge.XXXXXX")

resolve_data_dir() {
  if [ -n "${COPYPASTE_DATA_DIR:-}" ]; then
    printf '%s\n' "$COPYPASTE_DATA_DIR"
    return
  fi

  native_data_dir="$HOME/Library/Application Support/com.copypaste.CopyPaste"
  if [ -f "$native_data_dir/copypaste-v2.db" ]; then
    printf '%s\n' "$native_data_dir"
    return
  fi

  printf '%s\n' "$HOME/Library/Application Support/CopyPaste"
}

COPYPASTE_DATA_DIR=$(resolve_data_dir)
export COPYPASTE_DATA_DIR

cleanup() {
  kill "${bridge_pid:-}" 2>/dev/null || true
  wait "${bridge_pid:-}" 2>/dev/null || true
  if [ "$daemon_owned" = true ]; then
    kill "${daemon_pid:-}" 2>/dev/null || true
    wait "${daemon_pid:-}" 2>/dev/null || true
  fi
  clear_bridge_runtime
  rm -f "$bridge_env"
  release_bridge_session
}
trap cleanup EXIT INT TERM

cargo build --manifest-path "$repo_dir/Cargo.toml" -p copypaste-daemon
if [ ! -x "$cli_bin" ]; then
  cargo build --manifest-path "$repo_dir/Cargo.toml" -p copypaste-cli
fi
cargo build --manifest-path "$ui_dir/src-tauri/Cargo.toml" \
  --features dev-web-bridge --bin copypaste-web-bridge

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
  if ! wait_for_daemon; then
    exit 1
  fi
fi

COPYPASTE_WEB_BRIDGE_ENV_FILE="$bridge_env" "$bridge_bin" &
bridge_pid=$!

if ! wait_for_bridge_runtime; then
  exit 1
fi

# The file is created by mktemp (0600), read once, then removed by cleanup.
. "$bridge_env"
export VITE_COPYPASTE_WEB_BRIDGE_URL VITE_COPYPASTE_WEB_BRIDGE_TOKEN
write_bridge_runtime
cd "$ui_dir"
if curl --silent --fail --max-time 1 http://localhost:1420/ >/dev/null 2>&1; then
  echo "Vite is already running on http://localhost:1420/."
  echo "The open browser tab will attach to this bridge automatically."
  wait "$bridge_pid"
else
  npm run dev:web
fi
