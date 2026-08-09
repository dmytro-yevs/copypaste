#!/usr/bin/env bash

macos_bundle_executable_name() {
    local app="$1"
    local plist="${app}/Contents/Info.plist"
    local reader="${PLIST_BUDDY:-/usr/libexec/PlistBuddy}"
    local executable

    if [[ ! -f "$plist" ]]; then
        echo "ERROR: bundle metadata is missing." >&2
        return 1
    fi
    if [[ ! -x "$reader" ]]; then
        echo "ERROR: the bundle metadata reader is unavailable." >&2
        return 1
    fi
    if ! executable="$("$reader" -c "Print :CFBundleExecutable" "$plist" 2>/dev/null)"; then
        echo "ERROR: bundle metadata has no executable." >&2
        return 1
    fi
    case "$executable" in
        ""|.|..|*/*|*$'\n'*|*$'\r'*)
            echo "ERROR: bundle metadata has an invalid executable." >&2
            return 1
            ;;
    esac
    printf '%s\n' "$executable"
}

macos_bundle_executable_path() {
    local app="$1"
    local executable
    local path

    executable="$(macos_bundle_executable_name "$app")" || return 1
    path="${app}/Contents/MacOS/${executable}"
    if [[ ! -f "$path" || ! -x "$path" ]]; then
        echo "ERROR: the bundle executable is missing or not executable." >&2
        return 1
    fi
    printf '%s\n' "$path"
}
