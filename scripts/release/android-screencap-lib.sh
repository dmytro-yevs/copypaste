#!/usr/bin/env bash

ANDROID_SCREENCAP_ATTEMPTS="${ANDROID_SCREENCAP_ATTEMPTS:-3}"
ANDROID_SCREENCAP_RETRY_DELAY="${ANDROID_SCREENCAP_RETRY_DELAY:-1}"
ANDROID_PNG_VALIDATOR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/png_evidence.py"

capture_android_png() { # <png> <package>
    local png="$1" package="$2" log="${1%.png}-screencap.log"
    local candidate="${png}.candidate" stderr="${png}.stderr"
    local validation="${png}.validation" fallback_log="${png}.fallback"
    local pull_log="${png}.pull" cleanup_log="${png}.cleanup"
    local remote="/data/local/tmp/copypaste-screencap-$$-${RANDOM}.png"
    local attempt status bytes valid fallback_status pull_status cleanup_status

    : > "$log"
    rm -f "$png" "$candidate" "$stderr" "$validation" "$fallback_log" \
        "$pull_log" "$cleanup_log" "${png}.failed"
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

    if [[ ! -s "$png" && ! -s "${png}.failed" ]]; then
        rm -f "$candidate" "$validation"
        if adb shell screencap -p "$remote" > "$fallback_log" 2>&1; then
            fallback_status=0
        else
            fallback_status=$?
        fi
        pull_status=not-run
        if [[ "$fallback_status" -eq 0 ]]; then
            if adb pull "$remote" "$candidate" > "$pull_log" 2>&1; then
                pull_status=0
            else
                pull_status=$?
            fi
        fi
        bytes=0
        [[ -f "$candidate" ]] && bytes="$(stat -c %s "$candidate")"
        valid=no
        if [[ "$pull_status" == 0 ]] \
            && python3 "$ANDROID_PNG_VALIDATOR" "$candidate" 2> "$validation"; then
            valid=yes
        fi
        if adb shell rm -f "$remote" > "$cleanup_log" 2>&1; then
            cleanup_status=0
        else
            cleanup_status=$?
        fi
        printf 'fallback_capture_status=%s pull_status=%s bytes=%s decodable_png=%s cleanup_status=%s\n' \
            "$fallback_status" "$pull_status" "$bytes" "$valid" "$cleanup_status" >> "$log"
        [[ ! -s "$fallback_log" ]] || sed 's/^/fallback capture: /' "$fallback_log" >> "$log"
        [[ ! -s "$pull_log" ]] || sed 's/^/adb pull: /' "$pull_log" >> "$log"
        [[ ! -s "$validation" ]] || sed 's/^/PNG decoder: /' "$validation" >> "$log"
        [[ ! -s "$cleanup_log" ]] || sed 's/^/fallback cleanup: /' "$cleanup_log" >> "$log"
        if [[ "$fallback_status" -eq 0 && "$pull_status" == 0 && "$bytes" -gt 0 ]]; then
            if [[ "$valid" == yes ]]; then
                mv "$candidate" "$png"
            else
                printf 'capture_failure=non-empty pulled output failed PNG decoding; not retried\n' >> "$log"
                mv "$candidate" "${png}.failed"
            fi
        fi
    fi

    {
        printf '%s\n' '== adb state' "$(adb get-state 2>&1 || true)"
        printf '%s\n' '== power/display' "$(adb shell dumpsys power 2>&1 || true)" \
            "$(adb shell dumpsys display 2>&1 || true)"
        printf '%s\n' '== focus' \
            "$(adb shell dumpsys window 2>&1 | grep -E 'mCurrentFocus|mFocusedApp' | head -n 4 || true)"
        printf 'expected_package=%s\n' "$package"
    } >> "$log"
    rm -f "$stderr" "$validation" "$fallback_log" "$pull_log" "$cleanup_log"

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
    local temp="$1" calls=0 fallback_calls=0 pulls=0 cleanups=0 mode=retry remote_file=""
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
            if [[ "$mode" == retry ]]; then
                printf 'error: device offline\n' >&2
                return 1
            fi
            return 0
        fi
        if [[ "${1:-} ${2:-} ${3:-}" == "shell screencap -p" ]]; then
            fallback_calls=$((fallback_calls + 1))
            remote_file="$4"
            return 0
        fi
        if [[ "${1:-}" == pull ]]; then
            pulls=$((pulls + 1))
            if [[ "$mode" == fallback_failure ]]; then
                printf 'remote file vanished\n' >&2
                return 1
            fi
            if [[ "$mode" == fallback_corrupt ]]; then
                command head -c 24 "$temp/good.png" > "$3"
            else
                command cp "$temp/good.png" "$3"
            fi
            return 0
        fi
        if [[ "${1:-} ${2:-} ${3:-}" == "shell rm -f" ]]; then
            cleanups=$((cleanups + 1))
            [[ "$4" == "$remote_file" ]]
            return
        fi
        [[ "${1:-} ${2:-}" == "get-state " ]] && printf 'device\n'
        [[ "$*" == *"dumpsys power"* ]] && printf 'mWakefulness=Awake\n'
        [[ "$*" == *"dumpsys window"* ]] && printf 'mCurrentFocus=%s/.MainActivity\n' "$PKG"
        return 0
    }

    capture_android_png "$temp/retried.png" "$PKG"
    if [[ "$calls" -eq 3 && "$fallback_calls" -eq 0 && -s "$temp/retried.png" \
          && -z "$(python3 "$ANDROID_PNG_VALIDATOR" "$temp/retried.png" 2>&1)" \
          && "$(< "$temp/retried-screencap.log")" == *"error: device offline"* ]]; then
        ok "a zero-byte adb screencap is retried into a PNG"
    else
        bad "a zero-byte adb screencap is retried into a PNG" \
            "calls=$calls; $(< "$temp/retried-screencap.log")"
    fi

    calls=0 fallback_calls=0 pulls=0 cleanups=0
    mode=fallback_success
    capture_android_png "$temp/fallback.png" "$PKG"
    if [[ "$calls" -eq 3 && "$fallback_calls" -eq 1 && "$pulls" -eq 1 \
          && "$cleanups" -eq 1 && -s "$temp/fallback.png" \
          && -z "$(python3 "$ANDROID_PNG_VALIDATOR" "$temp/fallback.png" 2>&1)" \
          && "$(< "$temp/fallback-screencap.log")" == *"fallback_capture_status=0 pull_status=0"* \
          && "$(< "$temp/fallback-screencap.log")" == *"cleanup_status=0"* ]]; then
        ok "empty exec-out captures fall back to a pulled device-side PNG"
    else
        bad "empty exec-out captures fall back to a pulled device-side PNG" \
            "exec=$calls fallback=$fallback_calls pull=$pulls cleanup=$cleanups; $(< "$temp/fallback-screencap.log")"
    fi

    calls=0 fallback_calls=0 pulls=0 cleanups=0
    mode=fallback_failure
    if capture_android_png "$temp/fallback-failed.png" "$PKG"; then
        bad "a failed fallback pull fails closed"
    elif [[ "$calls" -eq 3 && "$fallback_calls" -eq 1 && "$pulls" -eq 1 \
            && "$cleanups" -eq 1 && ! -e "$temp/fallback-failed.png" \
            && "$(< "$temp/fallback-failed-screencap.log")" == *"pull_status=1"* \
            && "$(< "$temp/fallback-failed-screencap.log")" == *"remote file vanished"* \
            && "$(< "$temp/fallback-failed-screencap.log")" == *"cleanup_status=0"* ]]; then
        ok "a failed fallback pull fails closed and cleans up"
    else
        bad "a failed fallback pull fails closed and cleans up" \
            "exec=$calls fallback=$fallback_calls pull=$pulls cleanup=$cleanups; $(< "$temp/fallback-failed-screencap.log")"
    fi

    calls=0 fallback_calls=0 pulls=0 cleanups=0
    mode=fallback_corrupt
    if capture_android_png "$temp/fallback-corrupt.png" "$PKG"; then
        bad "a corrupt pulled PNG is rejected"
    elif [[ "$calls" -eq 3 && "$fallback_calls" -eq 1 && "$pulls" -eq 1 \
            && "$cleanups" -eq 1 && ! -e "$temp/fallback-corrupt.png" \
            && -s "$temp/fallback-corrupt.png.failed" \
            && "$(< "$temp/fallback-corrupt-screencap.log")" == *"not retried"* ]]; then
        ok "a corrupt pulled PNG is rejected without another capture"
    else
        bad "a corrupt pulled PNG is rejected without another capture" \
            "exec=$calls fallback=$fallback_calls pull=$pulls cleanup=$cleanups; $(< "$temp/fallback-corrupt-screencap.log")"
    fi

    calls=0 fallback_calls=0 pulls=0 cleanups=0
    mode=corrupt
    if capture_android_png "$temp/corrupt.png" "$PKG"; then
        bad "a truncated PNG screencap is rejected"
    elif [[ "$calls" -eq 1 && "$fallback_calls" -eq 0 && "$pulls" -eq 0 \
            && "$cleanups" -eq 0 && ! -e "$temp/corrupt.png" && -s "$temp/corrupt.png.failed" \
            && "$(< "$temp/corrupt-screencap.log")" == *"mWakefulness=Awake"* \
            && "$(< "$temp/corrupt-screencap.log")" == *"not retried"* \
            && "$(< "$temp/corrupt-screencap.log")" == *"mCurrentFocus=$PKG"* ]]; then
        ok "a truncated PNG screencap is rejected with device diagnostics"
    else
        bad "a truncated PNG screencap is rejected with device diagnostics" \
            "calls=$calls; $(< "$temp/corrupt-screencap.log")"
    fi
}
