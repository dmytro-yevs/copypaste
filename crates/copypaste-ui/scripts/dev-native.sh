#!/usr/bin/env sh
set -eu

ui_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
repo_dir=$(CDPATH= cd -- "$ui_dir/../.." && pwd)
daemon_bin="$repo_dir/target/debug/copypaste-daemon"
cli_bin="$repo_dir/target/debug/copypaste"
log_dir="${COPYPASTE_DEV_LOG_DIR:-$repo_dir/target/copypaste-dev}"
log_file="$log_dir/daemon-$(date +%Y%m%d-%H%M%S).jsonl"
bridge_env=$(mktemp "${TMPDIR:-/tmp}/copypaste-web-bridge.XXXXXX")
daemon_owned=false

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
# `tauri dev` starts Vite as a child. Passing the ephemeral bridge values into
# that one process means the browser tab at localhost:1420 and the native
# window use the same Vite server and the same daemon.
. "$bridge_env"
export VITE_COPYPASTE_WEB_BRIDGE_URL VITE_COPYPASTE_WEB_BRIDGE_TOKEN

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

cd "$ui_dir"
COPYPASTE_DAEMON_BIN="$daemon_bin" npm run tauri -- dev
