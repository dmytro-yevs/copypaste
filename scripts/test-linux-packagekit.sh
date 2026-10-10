#!/usr/bin/env bash
set -euo pipefail

# Run only in a disposable Linux root: the fixture owns a private system bus.
test "$(uname -s)" = Linux
root_dir=$(cd -- "$(dirname -- "$0")/.." && pwd)
fixture="$root_dir/apps/copypaste_flutter/linux/runner/test/linux_packagekit_fixture.cc"
output_dir=$(mktemp -d)
daemon_pid=

cleanup() {
  if [[ -n "$daemon_pid" ]]; then
    kill "$daemon_pid" 2>/dev/null || true
  fi
  rm -rf "$output_dir"
}
trap cleanup EXIT

address_file="$output_dir/system-bus-address"
dbus-daemon --session --fork --print-address=1 --print-pid=1 >"$address_file"
export DBUS_SYSTEM_BUS_ADDRESS
DBUS_SYSTEM_BUS_ADDRESS=$(head -n1 "$address_file")
daemon_pid=$(tail -n1 "$address_file")

read -r -a gio_cflags <<<"$(pkg-config --cflags gio-2.0 glib-2.0)"
read -r -a gio_libs <<<"$(pkg-config --libs gio-2.0 glib-2.0)"
"${CXX:-c++}" -std=c++17 -Wall -Wextra -Werror \
  -I"$root_dir/apps/copypaste_flutter/linux/runner" "${gio_cflags[@]}" \
  "$fixture" -o "$output_dir/linux-packagekit-fixture" "${gio_libs[@]}"

# The service name must be acquired before Properties.Get reaches the helper.
"$output_dir/linux-packagekit-fixture"
