#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
read -r -a flags <<< "$(pkg-config --cflags --libs gtk+-3.0)"
"${CXX:-c++}" -std=c++17 -Wall -Wextra -Werror \
  -I "$root/apps/copypaste_flutter/linux/runner" \
  "$root/apps/copypaste_flutter/linux/runner/linux_quick_paste_window.cc" \
  "$root/scripts/fixtures/linux-quick-paste-window.cc" \
  "${flags[@]}" -o "$work/geometry"
"$work/geometry"
