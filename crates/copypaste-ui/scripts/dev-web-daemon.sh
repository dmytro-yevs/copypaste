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
dev_data_dir=$(mktemp -d /tmp/cpd.XXXXXX)

# Browser preview is never allowed to inspect an installed daemon, data store,
# keychain item, or clipboard. The feature and environment switch must agree:
# either one alone would leave this launcher able to reach a real Keychain.
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
  --features copypaste-daemon/dev-ephemeral-key,copypaste-daemon/dev-fake-clipboard
cargo build --manifest-path "$ui_dir/src-tauri/Cargo.toml" \
  --features dev-web-bridge --bin copypaste-web-bridge

# This launcher always owns a fresh daemon. Reusing a responsive socket would
# make a browser screenshot session read an installed user's history.
"$daemon_bin" --foreground --data-dir "$dev_data_dir" &
daemon_pid=$!
daemon_owned=true
if ! wait_for_daemon; then
  exit 1
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
if curl --silent --fail --max-time 1 http://127.0.0.1:1420/ >/dev/null 2>&1; then
  echo "Vite is already running on http://127.0.0.1:1420/."
  echo "The open browser tab will attach to this bridge automatically."
  wait "$bridge_pid"
else
  npm run dev:web
fi
