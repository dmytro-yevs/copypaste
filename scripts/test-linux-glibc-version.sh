#!/usr/bin/env bash
# Verify the Linux marketplace runtime metric against the actual GNU libc.
set -euo pipefail

test "$(uname -s)" = Linux
root_dir=$(cd -- "$(dirname -- "$0")/.." && pwd)
runner_dir="$root_dir/apps/copypaste_flutter/linux/runner"
output_dir=$(mktemp -d)
trap 'rm -rf "$output_dir"' EXIT

read -r -a glib_flags <<<"$(pkg-config --cflags --libs glib-2.0)"
"${CXX:-c++}" -std=c++17 -Wall -Wextra -Werror \
  -I"$runner_dir" \
  "$runner_dir/linux_glibc_version_test.cc" \
  "$runner_dir/linux_glibc_version.cc" \
  "${glib_flags[@]}" -o "$output_dir/linux-glibc-version-test"
"$output_dir/linux-glibc-version-test"
