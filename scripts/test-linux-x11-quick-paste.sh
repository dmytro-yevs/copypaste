#!/usr/bin/env bash
# Run the native X11 Quick Paste regression fixture in an isolated Xvfb server.
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runner_dir="${project_root}/apps/copypaste_flutter/linux/runner"
source_file="${runner_dir}/linux_x11_quick_paste_test.cc"
implementation_file="${runner_dir}/linux_x11_quick_paste.cc"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "ERROR: the X11 Quick Paste fixture requires Linux." >&2
  exit 2
fi

for command in pkg-config Xvfb xdpyinfo; do
  if ! command -v "${command}" >/dev/null 2>&1; then
    echo "ERROR: ${command} is required by the X11 Quick Paste fixture." >&2
    exit 2
  fi
done

if ! pkg-config --exists gtk+-3.0 x11 xtst; then
  echo "ERROR: GTK 3, X11, and XTEST development packages are required." >&2
  exit 2
fi

compiler="${CXX:-c++}"
if ! command -v "${compiler}" >/dev/null 2>&1; then
  echo "ERROR: C++17 compiler not found: ${compiler}" >&2
  exit 2
fi

build_dir="$(mktemp -d "${TMPDIR:-/tmp}/copypaste-x11-quick-paste.XXXXXX")"
xvfb_pid=""
cleanup() {
  if [[ -n "${xvfb_pid}" ]]; then
    kill "${xvfb_pid}" 2>/dev/null || true
    wait "${xvfb_pid}" 2>/dev/null || true
  fi
  rm -rf "${build_dir}"
}
trap cleanup EXIT

Xvfb -displayfd 3 -screen 0 1280x800x24 -nolisten tcp \
  3>"${build_dir}/display" >"${build_dir}/xvfb.log" 2>&1 &
xvfb_pid=$!
for _ in $(seq 1 100); do
  if [[ -s "${build_dir}/display" ]]; then
    display_number="$(cat "${build_dir}/display")"
    [[ "$display_number" =~ ^[0-9]+$ ]] || break
    export DISPLAY=":${display_number}"
    if xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then break; fi
  fi
  kill -0 "$xvfb_pid" 2>/dev/null || break
  sleep 0.01
done

if [[ -z "${DISPLAY:-}" ]] || ! xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then
  cat "${build_dir}/xvfb.log" >&2 || true
  echo "ERROR: unable to start a private Xvfb server." >&2
  exit 1
fi

read -r -a pkg_config_flags <<< "$(pkg-config --cflags --libs gtk+-3.0 x11 xtst)"
"${compiler}" -std=c++17 -Wall -Wextra -Werror -I"${runner_dir}" \
  "${source_file}" "${implementation_file}" "${pkg_config_flags[@]}" \
  -o "${build_dir}/linux_x11_quick_paste_test"

GDK_BACKEND=x11 G_DEBUG="${G_DEBUG:+${G_DEBUG},}fatal-warnings" \
  "${build_dir}/linux_x11_quick_paste_test" "$@"
