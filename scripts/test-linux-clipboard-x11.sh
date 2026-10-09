#!/usr/bin/env bash
# Run the maintained native X11 clipboard fixture in an isolated Xvfb server.
set -euo pipefail

fixture_root=$(mktemp -d)
display_number=${COPYPASTE_X11_FIXTURE_DISPLAY:-:97}
xvfb_pid=''
cleanup() {
  if [[ -n "$xvfb_pid" ]]; then
    kill "$xvfb_pid" 2>/dev/null || true
    wait "$xvfb_pid" 2>/dev/null || true
  fi
  rm -rf "$fixture_root"
}
trap cleanup EXIT

Xvfb "$display_number" -screen 0 1280x800x24 -nolisten tcp >/tmp/copypaste-x11-fixture.log 2>&1 &
xvfb_pid=$!
for _ in $(seq 1 100); do
  if xdpyinfo -display "$display_number" >/dev/null 2>&1; then
    break
  fi
  sleep 0.02
done
xdpyinfo -display "$display_number" >/dev/null

export DISPLAY="$display_number"
export XDG_DATA_HOME="$fixture_root"
export COPYPASTE_X11_FIXTURE=1
cargo test -p copypaste-daemon live_x11_fixture -- --test-threads=1 --nocapture
