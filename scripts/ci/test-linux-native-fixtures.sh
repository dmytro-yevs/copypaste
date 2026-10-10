#!/usr/bin/env bash
# Exercise Linux-native boundaries on an isolated CI runner.
#
# This is deliberately separate from the Flutter foundation build: each
# command below requires a real native service or compiled native host code.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

[[ "$(uname -s)" == Linux ]] || {
  echo "ERROR: Linux native fixtures require Linux" >&2
  exit 1
}

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export CMAKE_BUILD_PARALLEL_LEVEL="${CMAKE_BUILD_PARALLEL_LEVEL:-2}"

./scripts/test-linux-clipboard-x11.sh
./scripts/test-linux-clipboard-gnome.sh
./scripts/test-linux-x11-quick-paste.sh
./scripts/test-linux-portal.sh
./scripts/test-linux-gnome-shortcuts.sh
./scripts/test-linux-packagekit.sh

fixture_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$fixture_dir"
}
trap cleanup EXIT

read -r -a glib_flags <<< "$(pkg-config --cflags --libs gio-2.0 glib-2.0)"
c++ -std=c++17 -Wall -Wextra -Werror \
  -I"$ROOT/apps/copypaste_flutter/linux/runner" \
  "$ROOT/apps/copypaste_flutter/linux/runner/linux_restart_helper_fixture.cc" \
  "$ROOT/apps/copypaste_flutter/linux/runner/linux_restart_helper.cc" \
  "${glib_flags[@]}" \
  -o "$fixture_dir/linux-restart-helper-fixture"
"$fixture_dir/linux-restart-helper-fixture" --verify

python3 "$ROOT/scripts/release/linux-clipboard-provider.py" \
  --build-helper "$fixture_dir/gtk3-clipboard-provider"
test -x "$fixture_dir/gtk3-clipboard-provider"

node "$ROOT/packaging/linux/desktop-integrations/test_runtime.mjs"
"$ROOT/packaging/linux/desktop-integrations/test_contract.sh"

# Sway exposes the wlr data-control protocol without using the runner's
# desktop. The ignored Rust test drives independent wl-clipboard clients
# against that real protocol, so a missing socket or unsupported protocol
# fails this job instead of silently reducing Wayland coverage.
sway_runtime="$fixture_dir/sway-runtime"
install -d -m 700 "$sway_runtime"
dbus-run-session -- env \
  XDG_RUNTIME_DIR="$sway_runtime" \
  WLR_BACKENDS=headless \
  WLR_RENDERER=pixman \
  WLR_LIBINPUT_NO_DEVICES=1 \
  CARGO_BUILD_JOBS="$CARGO_BUILD_JOBS" \
  bash -ceu -o pipefail '
    sway -c /dev/null >"$XDG_RUNTIME_DIR/sway.log" 2>&1 &
    sway_pid=$!
    cleanup() {
      kill "$sway_pid" 2>/dev/null || true
      wait "$sway_pid" 2>/dev/null || true
    }
    trap cleanup EXIT
    for _ in $(seq 1 200); do
      socket=$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name "wayland-*" -printf "%f\\n" | head -n 1)
      if [[ -n "$socket" ]]; then
        export WAYLAND_DISPLAY="$socket"
        cargo test --locked -p copypaste-daemon clipboard::linux::wayland::tests::data_control_interoperates_with_independent_wl_clipboard_clients -- --ignored --exact --test-threads=1 --nocapture | tee "$XDG_RUNTIME_DIR/wayland-test.log"
        grep -q "test result: ok. 1 passed" "$XDG_RUNTIME_DIR/wayland-test.log"
        exit 0
      fi
      sleep 0.025
    done
    cat "$XDG_RUNTIME_DIR/sway.log" >&2 || true
    echo "ERROR: Sway did not expose a Wayland socket" >&2
    exit 1
  '
