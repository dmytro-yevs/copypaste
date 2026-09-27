#!/usr/bin/env sh
set -eu

ui_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
repo_dir=$(CDPATH= cd -- "$ui_dir/../.." && pwd)
daemon_bin="$repo_dir/target/debug/copypaste-daemon"
cli_bin="$repo_dir/target/debug/copypaste"
daemon_owned=false

. "$ui_dir/scripts/web-bridge-runtime.sh"
if ! acquire_bridge_session; then
  exit 0
fi
dev_data_dir=$(mktemp -d /tmp/cpd.XXXXXX)
log_file="$dev_data_dir/daemon.jsonl"

# Native development uses an isolated throwaway history and key. Do not accept
# a caller's installed-data path or a responsive production daemon.
COPYPASTE_DATA_DIR="$dev_data_dir"
COPYPASTE_SOCKET="$dev_data_dir/daemon.sock"
COPYPASTE_EPHEMERAL_KEY=1
export COPYPASTE_DATA_DIR COPYPASTE_SOCKET COPYPASTE_EPHEMERAL_KEY

cleanup() {
  kill "${bridge_pid:-}" 2>/dev/null || true
  wait "${bridge_pid:-}" 2>/dev/null || true
  if [ "$daemon_owned" = true ]; then
    kill "${daemon_pid:-}" 2>/dev/null || true
    wait "${daemon_pid:-}" 2>/dev/null || true
  fi
  clear_bridge_runtime
  rm -f "${bridge_env:-}"
  release_bridge_session
  rm -rf "$dev_data_dir"
}
trap cleanup EXIT INT TERM
bridge_env=$(mktemp "${TMPDIR:-/tmp}/copypaste-web-bridge.XXXXXX")

cargo build --manifest-path "$repo_dir/Cargo.toml" \
  -p copypaste-daemon -p copypaste-cli \
  --features copypaste-daemon/dev-ephemeral-key

printf 'CopyPaste development daemon log: %s\n' "$log_file"
COPYPASTE_LOG_FORMAT=json RUST_LOG="${COPYPASTE_DEV_RUST_LOG:-copypaste_daemon=debug}" \
  "$daemon_bin" --foreground --data-dir "$dev_data_dir" >"$log_file" 2>&1 &
daemon_pid=$!
daemon_owned=true
if ! wait_for_daemon; then
  exit 1
fi

COPYPASTE_WEB_BRIDGE_ENV_FILE="$bridge_env" \
  cargo run --manifest-path "$ui_dir/src-tauri/Cargo.toml" \
    --features dev-web-bridge --bin copypaste-web-bridge &
bridge_pid=$!

if ! wait_for_bridge_runtime; then
  exit 1
fi
# `tauri dev` starts Vite as a child. Passing the ephemeral bridge values into
# that one process means the browser tab at 127.0.0.1:1420 and the native
# window use the same Vite server and the same daemon.
. "$bridge_env"
export VITE_COPYPASTE_WEB_BRIDGE_URL VITE_COPYPASTE_WEB_BRIDGE_TOKEN
write_bridge_runtime

cd "$ui_dir"
COPYPASTE_DAEMON_BIN="$daemon_bin" npm run tauri -- dev
