#!/usr/bin/env bash
# Stage the source patch for a distribution KWin source-package build.
set -euo pipefail

version="${1:-}"
source_root="${2:-}"
case "$version" in 6.0|6.3) ;; *) echo "ERROR: use KWin bridge version 6.0 or 6.3" >&2; exit 1 ;; esac
[[ -d "$source_root/src" && -f "$source_root/src/CMakeLists.txt" ]] || {
  echo "ERROR: expected an unpacked KWin source tree" >&2
  exit 1
}
grep -Eq "set\\(PROJECT_VERSION[[:space:]]+\\\"${version}\\." "$source_root/CMakeLists.txt" || {
  echo "ERROR: KWin source version does not match requested bridge patch $version" >&2
  exit 1
}

root="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
target="$source_root/src"
[[ ! -e "$target/copypasteclipboardbridge.cpp" && ! -e "$target/copypasteclipboardbridge.h" ]] || {
  echo "ERROR: CopyPaste bridge files already exist in this KWin source tree" >&2
  exit 1
}

cp "$root/src/copypasteclipboardbridge.cpp" "$target/"
cp "$root/src/copypasteclipboardbridge.h" "$target/"
patch --batch --forward --fuzz=0 -p1 --directory "$source_root" --input "$root/patches/kwin-${version}.patch"
