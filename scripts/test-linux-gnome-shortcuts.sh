#!/usr/bin/env bash
# Run the private-bus GIO fallback fixture. This verifies transport only; a
# host without GNOME/Mutter cannot prove the compositor's native grab.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_file="${root_dir}/apps/copypaste_flutter/linux/runner/linux_gnome_shortcuts_test.cc"

command -v pkg-config >/dev/null 2>&1 || {
  echo "pkg-config is required for the GNOME shortcut fixture." >&2
  exit 2
}
pkg-config --exists gio-2.0 glib-2.0 || {
  echo "GIO and GLib development packages are required (gio-2.0 glib-2.0)." >&2
  exit 2
}
command -v dbus-daemon >/dev/null 2>&1 || {
  echo "dbus-daemon is required by the private GTestDBus fixture." >&2
  exit 2
}

compiler="${CXX:-c++}"
command -v "${compiler}" >/dev/null 2>&1 || {
  echo "C++17 compiler not found: ${compiler}" >&2
  exit 2
}

build_dir="$(mktemp -d "${TMPDIR:-/tmp}/copypaste-gnome-shortcuts.XXXXXX")"
cleanup() { rm -rf "${build_dir}"; }
trap cleanup EXIT

read -r -a pkg_flags <<< "$(pkg-config --cflags --libs gio-2.0 glib-2.0)"
"${compiler}" -std=c++17 -Wall -Wextra -Werror -pthread \
  -I"${root_dir}/apps/copypaste_flutter/linux/runner" \
  "${source_file}" \
  "${root_dir}/apps/copypaste_flutter/linux/runner/linux_gnome_shortcuts.cc" \
  "${pkg_flags[@]}" -o "${build_dir}/linux-gnome-shortcuts-test"

G_DEBUG="${G_DEBUG:+${G_DEBUG},}fatal-warnings" \
  "${build_dir}/linux-gnome-shortcuts-test" "$@"
