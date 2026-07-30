#!/usr/bin/env bash
#
# Fail the release if a shipped Mach-O loads something that will not exist on a
# user's Mac.
#
# The concrete risk is SQLCipher. libsqlite3-sys compiles it from C and picks
# the crypto backend by branch (build.rs, the `bundled-sqlcipher` block):
#
#   OPENSSL_DIR, or OPENSSL_LIB_DIR + OPENSSL_INCLUDE_DIR, set
#       -> cargo:rustc-link-lib=dylib=crypto, against whatever libcrypto the
#          build machine has. On a GitHub runner that is Homebrew's, under
#          /opt/homebrew/opt/openssl@3, which no user has.
#   none of them set, host and target both Apple
#       -> -DSQLCIPHER_CRYPTO_CC and Apple's CommonCrypto, linking only the
#          Security and CoreFoundation frameworks. Every Mac has those.
#
# The second branch is the one this pipeline wants, and it is selected by the
# *absence* of an environment variable — so a runner image that starts
# exporting OPENSSL_DIR would silently move us to the first, and nothing else
# would notice: the DMG still mounts and the app still launches on the machine
# that built it. It dies with a dyld error on every other Mac.
#
# That is what this check exists to catch. It is deliberately a whitelist of
# load paths, not a search for "openssl" — the same failure arrives via any
# Homebrew library.
#
# Usage: check-macos-linkage.sh <path> [path...]
#   Paths may be files or directories; directories are searched for Mach-O.

set -euo pipefail

if [[ $# -eq 0 ]]; then
    echo "usage: $(basename "$0") <path> [path...]" >&2
    exit 2
fi

if ! command -v otool >/dev/null 2>&1; then
    echo "ERROR: otool not found. This check only means anything on macOS." >&2
    exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
LIST="${WORK}/machos"
VIOL="${WORK}/violations"
: > "$LIST"
: > "$VIOL"

for p in "$@"; do
    if [[ -d "$p" ]]; then
        while IFS= read -r f; do
            if file -b "$f" 2>/dev/null | grep -q 'Mach-O'; then
                echo "$f" >> "$LIST"
            fi
        done < <(find "$p" -type f)
    elif [[ -f "$p" ]]; then
        if file -b "$p" 2>/dev/null | grep -q 'Mach-O'; then
            echo "$p" >> "$LIST"
        else
            echo "ERROR: $p is not a Mach-O binary" >&2
            exit 1
        fi
    else
        echo "ERROR: $p does not exist" >&2
        exit 1
    fi
done

if [[ ! -s "$LIST" ]]; then
    echo "ERROR: no Mach-O binaries found in: $*" >&2
    echo "       An empty scan must not pass for a check whose whole job is to" >&2
    echo "       inspect binaries." >&2
    exit 1
fi

# A load path is acceptable if it is part of the OS, or resolved relative to
# the bundle. /usr/local and /opt/homebrew are the failure this guards.
# /Library/Frameworks is excluded deliberately: it exists on the runner and is
# not guaranteed on a user's machine.
path_ok() {
    case "$1" in
        /usr/lib/*|/System/Library/*)                 return 0 ;;
        @executable_path/*|@loader_path/*|@rpath/*)   return 0 ;;
        *)                                            return 1 ;;
    esac
}

# An @rpath load is only as safe as the LC_RPATH entries that resolve it, so
# those are checked too — an absolute rpath into /opt/homebrew re-creates the
# bug the load-path whitelist just rejected.
rpath_ok() {
    case "$1" in
        @executable_path/*|@loader_path/*)  return 0 ;;
        /usr/lib/*|/System/Library/*)       return 0 ;;
        *)                                  return 1 ;;
    esac
}

COUNT=0
while IFS= read -r bin; do
    COUNT=$((COUNT + 1))

    # tail -n +2 drops the "<file>:" header otool prints before the load
    # commands. For a dylib the first remaining line is its own LC_ID_DYLIB,
    # which is an install name and is checked by the same rules.
    while IFS= read -r dep; do
        [[ -n "$dep" ]] || continue
        if ! path_ok "$dep"; then
            printf '%s\n    loads %s\n' "$bin" "$dep" >> "$VIOL"
        fi
    done < <(otool -L "$bin" | tail -n +2 | sed 's/^[[:space:]]*//; s/ (compatibility.*$//')

    while IFS= read -r rp; do
        [[ -n "$rp" ]] || continue
        if ! rpath_ok "$rp"; then
            printf '%s\n    LC_RPATH %s\n' "$bin" "$rp" >> "$VIOL"
        fi
    done < <(otool -l "$bin" | awk '/LC_RPATH/ { want = 1; next } want && $1 == "path" { print $2; want = 0 }')
done < "$LIST"

echo "==> Checked $COUNT Mach-O binaries"

if [[ -s "$VIOL" ]]; then
    echo "::error::a shipped binary links a path that will not exist on a user's Mac"
    echo
    cat "$VIOL"
    echo
    echo "Allowed: /usr/lib, /System/Library, and @executable_path/@loader_path/@rpath."
    echo "A /opt/homebrew or /usr/local entry means the build linked a library from"
    echo "the runner. For libcrypto specifically, see the header of this script:"
    echo "unset OPENSSL_DIR / OPENSSL_LIB_DIR / OPENSSL_INCLUDE_DIR so SQLCipher"
    echo "falls to CommonCrypto."
    exit 1
fi

echo "    every load path is a system path or bundle-relative"
