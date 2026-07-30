#!/usr/bin/env bash
#
# Print the archiver and ranlib the Android cargo builds must use, as KEY=VALUE
# lines for $GITHUB_ENV.
#
# openssl-src takes AR and RANLIB from cc-rs (`Build::get_archiver` /
# `get_ranlib`) and passes them to OpenSSL's Configure, which bakes them into
# the generated Makefile. For an Android target cc-rs probes PATH for
# `llvm-ar` / `llvm-ranlib` and, not finding them, falls back to the GCC-era
# `<triple>-ar` / `<triple>-ranlib` wrappers the NDK deleted in r23.
#
# The Tauri Android build (cargo-mobile2, android/target.rs) exports TARGET_CC,
# TARGET_CXX and TARGET_AR with absolute NDK paths — and no RANLIB at all. So
# the archiver was already right and the ranlib was `aarch64-linux-android-
# ranlib`, which is what killed the fourth release dry run, in OpenSSL's
# install_dev rather than anywhere that named a toolchain.
#
# cc-rs reads, in order: `<TOOL>_<triple>`, `<TOOL>_<triple, - and . as _>`,
# `TARGET_<TOOL>`, `<TOOL>`. The second is the highest-priority spelling a
# shell can export, and it outranks cargo-mobile2's TARGET_AR — so AR is set
# here too rather than left to a contract that has already proved partial.
#
# This is a script and not eight lines of YAML because the emitted names are
# the entire fix and a wrong one fails silently: cc-rs just goes back to the
# fallback. check.sh executes this against a fake NDK and asserts the names.
#
# Usage: android-ndk-env.sh <ndk-home>

set -euo pipefail

TRIPLES=(aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android)

if [[ $# -ne 1 ]]; then
    echo "usage: $(basename "$0") <ndk-home>" >&2
    exit 2
fi

ndk="$1"
if [[ ! -d "$ndk" ]]; then
    echo "::error::$ndk is not a directory — the NDK is not installed where this job expects it"
    exit 1
fi

# The host tag comes from the NDK's own layout rather than a hardcoded
# `linux-x86_64`: sdkmanager installs exactly one prebuilt toolchain, and a
# runner image that changes architecture should fail here, loudly, not silently
# stop matching a string.
shopt -s nullglob
bins=("$ndk"/toolchains/llvm/prebuilt/*/bin)
shopt -u nullglob
if [[ ${#bins[@]} -ne 1 ]]; then
    echo "::error::expected exactly one prebuilt toolchain under $ndk/toolchains/llvm/prebuilt, found ${#bins[@]}"
    exit 1
fi

ar="${bins[0]}/llvm-ar"
ranlib="${bins[0]}/llvm-ranlib"
for tool in "$ar" "$ranlib"; do
    if [[ ! -x "$tool" ]]; then
        echo "::error::$tool is missing or not executable. Without it the Android OpenSSL"
        echo "::error::build falls back to the <triple>-ranlib wrappers no NDK has shipped"
        echo "::error::since r23, and dies in install_dev several minutes in (ADR-0007)."
        exit 1
    fi
done

for triple in "${TRIPLES[@]}"; do
    t="${triple//-/_}"
    printf 'AR_%s=%s\nRANLIB_%s=%s\n' "$t" "$ar" "$t" "$ranlib"
done
