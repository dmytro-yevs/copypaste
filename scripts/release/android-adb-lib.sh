#!/usr/bin/env bash
# Invoking adb so device-absolute paths survive the host shell.
#
# Git Bash and MSYS2 rewrite an argument that looks like an absolute POSIX path
# into a Windows path before a native child sees it, so `adb shell` was handed
# `C:/Program Files/Git/sdcard/Download/copypaste-export.json` and every device
# read and write addressed a host directory that does not exist. Each entry is
# an argument-prefix pattern, so bare `/sdcard` covers ordinary device-path
# arguments while `of=/sdcard` and `if=/sdcard` are each needed for `dd`'s
# operands; a blanket `*` would also stop the host destination of `adb pull`
# converting, so the pulled file would never be written.
set -uo pipefail

ANDROID_DEVICE_ROOTS=(/sdcard /data/local/tmp)

android_device_path_excl() {
    local root out=""
    for root in "${ANDROID_DEVICE_ROOTS[@]}"; do
        out+="$root;of=$root;if=$root;"
    done
    printf '%s' "${out%;}"
}

ANDROID_DEVICE_PATH_EXCL="$(android_device_path_excl)"

adb_() { MSYS2_ARG_CONV_EXCL="$ANDROID_DEVICE_PATH_EXCL" adb "$@"; }

# Only a native Windows child triggers the rewrite, so proving the exclusion
# works needs one. Every other host has nothing to defeat.
android_adb_conversion_probe() {
    local observed
    case "${OSTYPE:-}" in
        msys* | cygwin*) ;;
        *)
            note "device paths reach adb unrewritten" \
                 "this host does not rewrite POSIX arguments"
            return 0
            ;;
    esac
    observed="$(MSYS2_ARG_CONV_EXCL="$ANDROID_DEVICE_PATH_EXCL" \
        cmd //c echo of=/sdcard/Download/probe 2>/dev/null | tr -d '\r')"
    [[ "$observed" == "of=/sdcard/Download/probe" ]] \
        && ok "a device path reaches a native child unrewritten" \
        || bad "a device path reaches a native child unrewritten" "$observed"
}

android_adb_self_test() {
    local root
    for root in "${ANDROID_DEVICE_ROOTS[@]}"; do
        [[ ";$ANDROID_DEVICE_PATH_EXCL;" == *";$root;"* ]] \
            && ok "$root is held back from host path conversion" \
            || bad "$root is held back from host path conversion" "$ANDROID_DEVICE_PATH_EXCL"
        [[ ";$ANDROID_DEVICE_PATH_EXCL;" == *";of=$root;"* ]] \
            && ok "the dd output operand for $root is held back too" \
            || bad "the dd output operand for $root is held back too" "$ANDROID_DEVICE_PATH_EXCL"
    done
    android_adb_conversion_probe
}
