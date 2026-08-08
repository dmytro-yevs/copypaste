#!/usr/bin/env bash
#
# Link libcopypaste_ui_lib.so for all four Android ABIs, the way the APK links
# it, and fail on anything the device's dynamic linker could not resolve.
#
# The universal APK is the only artifact that links all four, and only
# release.yml builds it, only on a pushed tag. So v2.0.0-alpha.5 failed twice in
# public on i686 defects nothing could have seen: sha2-asm's `R_386_32 cannot be
# used against local symbol`, then vendored OpenSSL's undefined
# `__atomic_load_8` (ADR-0007). Both landed at this link and neither at crate
# level — sha2-asm compiles clean and fails when its object reaches ld.lld.
#
# --no-undefined is the half that is not obvious. Without it the second defect
# links exit 0 and yields a .so carrying seven unresolved `__atomic_*` that
# would fail at dlopen on the device — measured, not assumed. It is also the
# strictness the APK link has, which is why that defect was a build failure in
# the release job rather than a crash in a user's hands.
#
# Usage: android-link-abis.sh [triple ...]   (default: all four)
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT" || exit 1

TRIPLES=(aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android)
if [[ $# -gt 0 ]]; then
    TRIPLES=("$@")
fi

# app/build.gradle.kts. The clang wrapper carries the API level into the link,
# so a number above minSdk resolves symbols the shipped .so would not find.
API=24

: "${NDK_HOME:?NDK_HOME is unset — this script cannot guess where the pinned NDK is}"
if [[ ! -d "$NDK_HOME" ]]; then
    echo "::error::$NDK_HOME is not a directory — the NDK is not installed where this job expects it"
    exit 1
fi

shopt -s nullglob
bins=("$NDK_HOME"/toolchains/llvm/prebuilt/*/bin)
shopt -u nullglob
if [[ ${#bins[@]} -ne 1 ]]; then
    echo "::error::expected exactly one prebuilt toolchain under $NDK_HOME/toolchains/llvm/prebuilt, found ${#bins[@]}"
    exit 1
fi
NDKBIN="${bins[0]}"

# AR and RANLIB for openssl-src, from the one script that owns those names.
NDK_ENV="$(./scripts/release/android-ndk-env.sh "$NDK_HOME")" || exit 1
eval "$(sed 's/^/export /' <<<"$NDK_ENV")"

# What cargo-mobile2 puts in CARGO_ENCODED_RUSTFLAGS for the APK, plus the
# strictness above. Reproducing the release link is the entire point; a linker
# flag this job does not share is a defect this job cannot see.
export RUSTFLAGS="-landroid -llog -lOpenSLES -C link-arg=-Wl,--no-undefined"

failed=()
for triple in "${TRIPLES[@]}"; do
    case "$triple" in
        armv7-linux-androideabi) cc="$NDKBIN/armv7a-linux-androideabi${API}-clang" ;;
        *)                       cc="$NDKBIN/${triple}${API}-clang" ;;
    esac
    if [[ ! -x "$cc" ]]; then
        echo "::error::$cc is missing — the NDK cannot target $triple at API $API"
        failed+=("$triple")
        continue
    fi

    t="${triple//-/_}"
    T="${t^^}"
    export "CC_${t}=$cc"
    export "CXX_${t}=${cc}++"
    export "CARGO_TARGET_${T}_LINKER=$cc"

    printf '\n=== %s ===\n' "$triple"
    if ( cd crates/copypaste-ui/src-tauri \
         && cargo build --release --target "$triple" --lib ); then
        so="${CARGO_TARGET_DIR:-target}/$triple/release/libcopypaste_ui_lib.so"
        if [[ -f "$so" ]]; then
            printf 'linked %s (%s bytes)\n' "$so" "$(wc -c <"$so")"
        else
            echo "::error::$triple built but produced no $so"
            failed+=("$triple")
        fi
    else
        echo "::error::$triple failed to link"
        failed+=("$triple")
    fi
done

if (( ${#failed[@]} )); then
    echo
    echo "::error::Android ABIs that would fail the release build: ${failed[*]}"
    exit 1
fi

printf '\nAll %d Android ABIs link.\n' "${#TRIPLES[@]}"
