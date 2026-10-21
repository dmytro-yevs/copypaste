#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
[[ $# == 1 ]] || { echo "usage: $0 <output-executable>" >&2; exit 2; }
xcrun clang -Os -mmacosx-version-min=14.0 -fstack-protector-strong \
    -Wall -Wextra -Werror -Wno-deprecated-declarations \
    -framework Security -framework CoreFoundation \
    "$root/packaging/macos/keychain-access.c" -o "$1"
