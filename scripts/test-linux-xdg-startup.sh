#!/usr/bin/env bash
# Compile and run the XDG startup/URI fixture only against temporary XDG homes.
set -euo pipefail

[[ "$(uname -s)" == Linux ]] || {
  echo "ERROR: Linux XDG startup fixture requires Linux." >&2
  exit 2
}

root_dir=$(cd -- "$(dirname -- "$0")/.." && pwd)
runner_dir="$root_dir/apps/copypaste_flutter/linux/runner"
output_dir=$(mktemp -d)
trap 'rm -rf "$output_dir"' EXIT

read -r -a gio_flags <<<"$(pkg-config --cflags --libs gio-unix-2.0 gio-2.0 glib-2.0)"
"${CXX:-c++}" -std=c++17 -Wall -Wextra -Werror \
  -I"$runner_dir" \
  "$runner_dir/linux_xdg_startup_test.cc" \
  "$runner_dir/linux_xdg_startup.cc" \
  "${gio_flags[@]}" -o "$output_dir/linux-xdg-startup-test"
"$output_dir/linux-xdg-startup-test"
