#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_file="${project_root}/apps/copypaste_flutter/linux/runner/linux_portal_test.cc"

if ! command -v pkg-config >/dev/null 2>&1; then
  echo "pkg-config is required to build the Linux portal fixture." >&2
  exit 2
fi

if ! pkg-config --exists gtk+-3.0 gio-2.0; then
  echo "GTK 3 and GIO development packages are required (gtk+-3.0 gio-2.0)." >&2
  exit 2
fi

if ! command -v dbus-daemon >/dev/null 2>&1; then
  echo "dbus-daemon is required by the private GTestDBus fixture." >&2
  exit 2
fi

compiler="${CXX:-c++}"
if ! command -v "${compiler}" >/dev/null 2>&1; then
  echo "C++17 compiler not found: ${compiler}" >&2
  exit 2
fi

build_dir="$(mktemp -d "${TMPDIR:-/tmp}/copypaste-linux-portal.XXXXXX")"
cleanup() {
  rm -rf "${build_dir}"
}
trap cleanup EXIT

read -r -a pkg_config_flags <<< "$(pkg-config --cflags --libs gtk+-3.0 gio-2.0)"
"${compiler}" -std=c++17 -pthread -Wall -Wextra -Werror "${source_file}" \
  "${pkg_config_flags[@]}" -o "${build_dir}/linux_portal_test"

G_DEBUG="${G_DEBUG:+${G_DEBUG},}fatal-warnings" \
  "${build_dir}/linux_portal_test" "$@"
