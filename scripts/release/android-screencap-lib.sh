#!/usr/bin/env bash

ANDROID_SCREENCAP_ATTEMPTS="${ANDROID_SCREENCAP_ATTEMPTS:-3}"
ANDROID_SCREENCAP_RETRY_DELAY="${ANDROID_SCREENCAP_RETRY_DELAY:-1}"
ANDROID_PNG_VALIDATOR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/png_evidence.py"

capture_android_png() { # <png> <package>
    local png="$1" package="$2" log="${1%.png}-screencap.log"
    local candidate="${png}.candidate" stderr="${png}.stderr"
    local validation="${png}.validation" attempt status bytes valid

    : > "$log"
    rm -f "$png" "$candidate" "$stderr" "$validation" "${png}.failed"
    if ! python3 -c 'from PIL import Image' 2>> "$log"; then
        printf 'PNG validation unavailable: install requirements-ci.txt\n' >> "$log"
        return 1
    fi

    for attempt in $(seq 1 "$ANDROID_SCREENCAP_ATTEMPTS"); do
        rm -f "$candidate" "$stderr" "$validation"
        adb shell input keyevent KEYCODE_WAKEUP >/dev/null 2>> "$log" || true
        adb shell wm dismiss-keyguard >/dev/null 2>> "$log" || true
        if adb exec-out screencap -p > "$candidate" 2> "$stderr"; then
            status=0
        else
            status=$?
        fi
        bytes=0
        [[ -f "$candidate" ]] && bytes="$(stat -c %s "$candidate")"
        valid=no
        if python3 "$ANDROID_PNG_VALIDATOR" "$candidate" 2> "$validation"; then
            valid=yes
        fi
        printf 'attempt=%s adb_status=%s bytes=%s decodable_png=%s\n' \
            "$attempt" "$status" "$bytes" "$valid" >> "$log"
        if [[ -s "$stderr" ]]; then
            sed 's/^/adb stderr: /' "$stderr" >> "$log"
        fi
        if [[ -s "$validation" ]]; then
            sed 's/^/PNG decoder: /' "$validation" >> "$log"
        fi
        if [[ "$status" -eq 0 && "$bytes" -gt 0 ]]; then
            if [[ "$valid" == yes ]]; then
                mv "$candidate" "$png"
            else
                printf 'capture_failure=non-empty adb output failed PNG decoding; not retried\n' >> "$log"
                mv "$candidate" "${png}.failed"
            fi
            break
        fi
        [[ "$attempt" -eq "$ANDROID_SCREENCAP_ATTEMPTS" ]] \
            || sleep "$ANDROID_SCREENCAP_RETRY_DELAY"
    done

    {
        printf '%s\n' '== adb state' "$(adb get-state 2>&1 || true)"
        printf '%s\n' '== power/display' "$(adb shell dumpsys power 2>&1 || true)" \
            "$(adb shell dumpsys display 2>&1 || true)"
        printf '%s\n' '== focus' \
            "$(adb shell dumpsys window 2>&1 | grep -E 'mCurrentFocus|mFocusedApp' | head -n 4 || true)"
        printf 'expected_package=%s\n' "$package"
    } >> "$log"
    rm -f "$stderr" "$validation"

    if [[ -s "$png" ]]; then
        rm -f "$candidate"
        return 0
    fi
    if [[ -s "$candidate" ]]; then
        mv "$candidate" "${png}.failed"
    else
        rm -f "$candidate"
    fi
    return 1
}

android_screencap_self_test() { # <temporary directory>
    local temp="$1" calls=0 mode=retry
    ANDROID_SCREENCAP_RETRY_DELAY=0
    python3 - "$temp/good.png" <<'PY'
import base64, pathlib, sys
pathlib.Path(sys.argv[1]).write_bytes(base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
))
PY
    adb() {
        if [[ "${1:-} ${2:-} ${3:-}" == "exec-out screencap -p" ]]; then
            calls=$((calls + 1))
            if [[ "$mode" == retry && "$calls" -eq 3 ]]; then
                command cat "$temp/good.png"
                return 0
            fi
            if [[ "$mode" == corrupt ]]; then
                command head -c 24 "$temp/good.png"
                return 0
            fi
            printf 'error: device offline\n' >&2
            return 1
        fi
        [[ "${1:-} ${2:-}" == "get-state " ]] && printf 'device\n'
        [[ "$*" == *"dumpsys power"* ]] && printf 'mWakefulness=Awake\n'
        [[ "$*" == *"dumpsys window"* ]] && printf 'mCurrentFocus=%s/.MainActivity\n' "$PKG"
        return 0
    }

    capture_android_png "$temp/retried.png" "$PKG"
    if [[ "$calls" -eq 3 && -s "$temp/retried.png" \
          && -z "$(python3 "$ANDROID_PNG_VALIDATOR" "$temp/retried.png" 2>&1)" \
          && "$(< "$temp/retried-screencap.log")" == *"error: device offline"* ]]; then
        ok "a zero-byte adb screencap is retried into a PNG"
    else
        bad "a zero-byte adb screencap is retried into a PNG" \
            "calls=$calls; $(< "$temp/retried-screencap.log")"
    fi

    calls=0
    mode=corrupt
    if capture_android_png "$temp/corrupt.png" "$PKG"; then
        bad "a truncated PNG screencap is rejected"
    elif [[ "$calls" -eq 1 && ! -e "$temp/corrupt.png" && -s "$temp/corrupt.png.failed" \
            && "$(< "$temp/corrupt-screencap.log")" == *"mWakefulness=Awake"* \
            && "$(< "$temp/corrupt-screencap.log")" == *"not retried"* \
            && "$(< "$temp/corrupt-screencap.log")" == *"mCurrentFocus=$PKG"* ]]; then
        ok "a truncated PNG screencap is rejected with device diagnostics"
    else
        bad "a truncated PNG screencap is rejected with device diagnostics" \
            "calls=$calls; $(< "$temp/corrupt-screencap.log")"
    fi
}
