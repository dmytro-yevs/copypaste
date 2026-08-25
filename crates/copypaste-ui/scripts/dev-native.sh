#!/usr/bin/env sh
set -eu

ui_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
repo_dir=$(CDPATH= cd -- "$ui_dir/../.." && pwd)
daemon_bin="$repo_dir/target/debug/copypaste-daemon"
cli_bin="$repo_dir/target/debug/copypaste"
log_dir="${COPYPASTE_DEV_LOG_DIR:-$repo_dir/target/copypaste-dev}"
log_file="$log_dir/daemon-$(date +%Y%m%d-%H%M%S).jsonl"
daemon_owned=false

. "$ui_dir/scripts/web-bridge-runtime.sh"
if ! acquire_bridge_session; then
  exit 0
fi
bridge_env=$(mktemp "${TMPDIR:-/tmp}/copypaste-web-bridge.XXXXXX")

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

mkdir -p "$log_dir"
cargo build --manifest-path "$repo_dir/Cargo.toml" -p copypaste-daemon
if [ ! -x "$cli_bin" ]; then
  cargo build --manifest-path "$repo_dir/Cargo.toml" -p copypaste-cli
fi

if "$cli_bin" status >/dev/null 2>&1; then
  echo "Reusing the running CopyPaste daemon."
else
  printf 'CopyPaste daemon log: %s\n' "$log_file"
  COPYPASTE_LOG_FORMAT=json RUST_LOG="${COPYPASTE_DEV_RUST_LOG:-copypaste_daemon=debug}" \
    "$daemon_bin" --foreground >"$log_file" 2>&1 &
  daemon_pid=$!
  daemon_owned=true
  if ! wait_for_daemon; then
    exit 1
  fi
fi

COPYPASTE_WEB_BRIDGE_ENV_FILE="$bridge_env" \
  cargo run --manifest-path "$ui_dir/src-tauri/Cargo.toml" \
    --features dev-web-bridge --bin copypaste-web-bridge &
bridge_pid=$!

if ! wait_for_bridge_runtime; then
  exit 1
fi
# `tauri dev` starts Vite as a child. Passing the ephemeral bridge values into
# that one process means the browser tab at localhost:1420 and the native
# window use the same Vite server and the same daemon.
. "$bridge_env"
export VITE_COPYPASTE_WEB_BRIDGE_URL VITE_COPYPASTE_WEB_BRIDGE_TOKEN
write_bridge_runtime

cd "$ui_dir"
COPYPASTE_DAEMON_BIN="$daemon_bin" npm run tauri -- dev
