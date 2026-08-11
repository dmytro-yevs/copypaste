#!/usr/bin/env bash

ANDROID_SCREENCAP_ATTEMPTS="${ANDROID_SCREENCAP_ATTEMPTS:-3}"
ANDROID_SCREENCAP_RETRY_DELAY="${ANDROID_SCREENCAP_RETRY_DELAY:-1}"
ANDROID_EMULATOR_SCREENCAP_TIMEOUT="${ANDROID_EMULATOR_SCREENCAP_TIMEOUT:-15}"
ANDROID_EMULATOR_SCREENCAP_POLLS="${ANDROID_EMULATOR_SCREENCAP_POLLS:-20}"
ANDROID_EMULATOR_SCREENCAP_POLL_DELAY="${ANDROID_EMULATOR_SCREENCAP_POLL_DELAY:-0.25}"
ANDROID_PNG_VALIDATOR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/png_evidence.py"

emulator_console_screenshot() { # <serial> <host directory>
    timeout --foreground "${ANDROID_EMULATOR_SCREENCAP_TIMEOUT}s" \
        adb -s "$1" emu screenrecord screenshot "$2"
}

append_android_capture_diagnostics() { # <log> <package>
    {
        printf '%s\n' '== adb state' "$(adb get-state 2>&1 || true)"
        printf '%s\n' '== power/display' "$(adb shell dumpsys power 2>&1 || true)" \
            "$(adb shell dumpsys display 2>&1 || true)"
        printf '%s\n' '== focus' \
            "$(adb shell dumpsys window 2>&1 | grep -E 'mCurrentFocus|mFocusedApp' | head -n 4 || true)"
        printf 'expected_package=%s\n' "$2"
    } >> "$1"
}

capture_emulator_console_png() { # <png> <serial> <log>
    local png="$1" serial="$2" log="$3" candidate="${1}.candidate"
    local validation="${1}.validation" console_log="${1}.console-log"
    local console_dir generated="" status bytes=0 valid=no cleanup_status poll

    console_dir="$(mktemp -d "${TMPDIR:-/tmp}/copypaste-emulator-screencap.XXXXXX")" || {
        printf 'console_capture_failure=host temporary directory unavailable\n' >> "$log"
        return 1
    }
    if emulator_console_screenshot "$serial" "$console_dir" > "$console_log" 2>&1; then
        status=0
    else
        status=$?
    fi
    for poll in $(seq 1 "$ANDROID_EMULATOR_SCREENCAP_POLLS"); do
        generated="$(find "$console_dir" -maxdepth 1 -type f -name 'Screenshot_*.png' -print -quit)"
        [[ -z "$generated" ]] || break
        [[ "$poll" -eq "$ANDROID_EMULATOR_SCREENCAP_POLLS" ]] \
            || sleep "$ANDROID_EMULATOR_SCREENCAP_POLL_DELAY"
    done
    if [[ -n "$generated" ]]; then
        mv "$generated" "$candidate"
        bytes="$(stat -c %s "$candidate")"
        if python3 "$ANDROID_PNG_VALIDATOR" "$candidate" 2> "$validation"; then
            valid=yes
        fi
    fi
    if rm -rf -- "$console_dir"; then cleanup_status=0; else cleanup_status=$?; fi
    printf 'transport=emulator-console serial=%s console_status=%s bytes=%s decodable_contentful_png=%s cleanup_status=%s\n' \
        "$serial" "$status" "$bytes" "$valid" "$cleanup_status" >> "$log"
    [[ ! -s "$console_log" ]] || sed 's/^/emulator console: /' "$console_log" >> "$log"
    [[ ! -s "$validation" ]] || sed 's/^/PNG decoder: /' "$validation" >> "$log"
    rm -f "$console_log" "$validation"
    if [[ "$status" -eq 0 && "$bytes" -gt 0 && "$valid" == yes && "$cleanup_status" -eq 0 ]]; then
        mv "$candidate" "$png"
        return 0
    fi
    if [[ "$status" -ne 0 ]]; then
        printf 'capture_failure=emulator console command failed\n' >> "$log"
    elif [[ -z "$generated" ]]; then
        printf 'capture_failure=emulator console produced no Screenshot_*.png\n' >> "$log"
    elif [[ "$valid" != yes ]]; then
        printf 'capture_failure=emulator console output failed PNG evidence validation\n' >> "$log"
    else
        printf 'capture_failure=emulator console temporary cleanup failed\n' >> "$log"
    fi
    [[ ! -s "$candidate" ]] || mv "$candidate" "${png}.failed"
    return 1
}

capture_android_png() { # <png> <package>
    local png="$1" package="$2" log="${1%.png}-screencap.log"
    local candidate="${png}.candidate" stderr="${png}.stderr"
    local validation="${png}.validation" fallback_log="${png}.fallback"
    local pull_log="${png}.pull" cleanup_log="${png}.cleanup"
    local remote="/data/local/tmp/copypaste-screencap-$$-${RANDOM}.png"
    local attempt status bytes valid fallback_status pull_status cleanup_status serial

    : > "$log"
    rm -f "$png" "$candidate" "$stderr" "$validation" "$fallback_log" \
        "$pull_log" "$cleanup_log" "${png}.failed"
    if ! python3 -c 'from PIL import Image' 2>> "$log"; then
        printf 'PNG validation unavailable: install requirements-ci.txt\n' >> "$log"
        return 1
    fi

    serial="$(adb get-serialno 2>> "$log" | tr -d '\r')"
    if [[ "$serial" == emulator-* ]]; then
        capture_emulator_console_png "$png" "$serial" "$log" || status=$?
        status="${status:-0}"
        append_android_capture_diagnostics "$log" "$package"
        return "$status"
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
        printf 'transport=adb-exec-out attempt=%s adb_status=%s bytes=%s decodable_contentful_png=%s\n' \
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
                printf 'capture_failure=non-empty adb output failed PNG evidence validation; not retried\n' >> "$log"
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
        printf 'transport=adb-device-file fallback_capture_status=%s pull_status=%s bytes=%s decodable_contentful_png=%s cleanup_status=%s\n' \
            "$fallback_status" "$pull_status" "$bytes" "$valid" "$cleanup_status" >> "$log"
        [[ ! -s "$fallback_log" ]] || sed 's/^/fallback capture: /' "$fallback_log" >> "$log"
        [[ ! -s "$pull_log" ]] || sed 's/^/adb pull: /' "$pull_log" >> "$log"
        [[ ! -s "$validation" ]] || sed 's/^/PNG decoder: /' "$validation" >> "$log"
        [[ ! -s "$cleanup_log" ]] || sed 's/^/fallback cleanup: /' "$cleanup_log" >> "$log"
        if [[ "$cleanup_status" -ne 0 ]]; then
            printf 'capture_failure=fallback cleanup failed; pulled output preserved; not retried\n' >> "$log"
            [[ ! -s "$candidate" ]] || mv "$candidate" "${png}.failed"
        elif [[ "$fallback_status" -eq 0 && "$pull_status" == 0 && "$bytes" -gt 0 ]]; then
            if [[ "$valid" == yes ]]; then
                mv "$candidate" "$png"
            else
                printf 'capture_failure=non-empty pulled output failed PNG evidence validation; not retried\n' >> "$log"
                mv "$candidate" "${png}.failed"
            fi
        fi
    fi

    append_android_capture_diagnostics "$log" "$package"
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
    local temp="$1" calls=0 fallback_calls=0 pulls=0 cleanups=0 console_calls=0
    local mode=retry remote_file=""
    ANDROID_SCREENCAP_RETRY_DELAY=0
    ANDROID_EMULATOR_SCREENCAP_POLL_DELAY=0
    python3 - "$temp/good.png" "$temp/black.png" <<'PY'
import sys
from PIL import Image

good = Image.new("RGB", (2, 1), "black")
good.putpixel((1, 0), (255, 255, 255))
good.save(sys.argv[1])
Image.new("RGB", (8, 8), "black").save(sys.argv[2])
PY
    adb() {
        if [[ "${1:-}" == "get-serialno" ]]; then
            [[ "$mode" == console_* ]] && printf 'emulator-5554\n' || printf 'device-123\n'
            return 0
        fi
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
            if [[ "$mode" == black ]]; then
                command cat "$temp/black.png"
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
            if [[ "$mode" == direct_empty ]]; then
                printf 'device-side screencap failed\n' >&2
                return 1
            fi
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
            if [[ "$mode" == fallback_cleanup_failure ]]; then
                printf 'rm: cannot remove remote screenshot: Permission denied\n' >&2
                return 1
            fi
            [[ "$4" == "$remote_file" ]]
            return
        fi
        [[ "${1:-} ${2:-}" == "get-state " ]] && printf 'device\n'
        [[ "$*" == *"dumpsys power"* ]] && printf 'mWakefulness=Awake\n'
        [[ "$*" == *"dumpsys window"* ]] && printf 'mCurrentFocus=%s/.MainActivity\n' "$PKG"
        return 0
    }
    emulator_console_screenshot() {
        console_calls=$((console_calls + 1))
        if [[ "$mode" == console_success ]]; then
            command cp "$temp/good.png" "$2/Screenshot_fixture.png"
            return 0
        fi
        if [[ "$mode" == console_black ]]; then
            command cp "$temp/black.png" "$2/Screenshot_fixture.png"
            return 0
        fi
        if [[ "$mode" == console_empty ]]; then
            return 0
        fi
        printf 'emulator console unavailable\n' >&2
        return 1
    }

    mode=console_success
    capture_android_png "$temp/console.png" "$PKG"
    if [[ "$console_calls" -eq 1 && "$calls" -eq 0 && "$fallback_calls" -eq 0 \
          && -s "$temp/console.png" && ! -e "$temp/console.png.failed" \
          && "$(< "$temp/console-screencap.log")" == *"transport=emulator-console"* ]]; then
        ok "an emulator uses the host console screenshot transport"
    else
        bad "an emulator uses the host console screenshot transport" \
            "console=$console_calls exec=$calls fallback=$fallback_calls; $(< "$temp/console-screencap.log")"
    fi

    calls=0 fallback_calls=0 pulls=0 cleanups=0 console_calls=0
    mode=console_black
    if capture_android_png "$temp/console-black.png" "$PKG"; then
        bad "a decodable black emulator frame is rejected"
    elif [[ "$console_calls" -eq 1 && "$calls" -eq 0 && "$fallback_calls" -eq 0 \
            && ! -e "$temp/console-black.png" && -s "$temp/console-black.png.failed" \
            && "$(< "$temp/console-black-screencap.log")" == *"contentless: uniform RGB frame"* ]]; then
        ok "a decodable black emulator frame is rejected"
    else
        bad "a decodable black emulator frame is rejected" \
            "console=$console_calls exec=$calls fallback=$fallback_calls; $(< "$temp/console-black-screencap.log")"
    fi

    calls=0 fallback_calls=0 pulls=0 cleanups=0 console_calls=0
    mode=console_empty
    if capture_android_png "$temp/console-empty.png" "$PKG"; then
        bad "an empty emulator-console capture is rejected"
    elif [[ "$console_calls" -eq 1 && "$calls" -eq 0 && "$fallback_calls" -eq 0 \
            && ! -e "$temp/console-empty.png" && ! -e "$temp/console-empty.png.failed" \
            && "$(< "$temp/console-empty-screencap.log")" == *"console_status=0 bytes=0"* \
            && "$(< "$temp/console-empty-screencap.log")" == *"produced no Screenshot_*.png"* \
            && "$(< "$temp/console-empty-screencap.log")" == *"mCurrentFocus=$PKG"* ]]; then
        ok "an empty emulator-console capture is rejected with state diagnostics"
    else
        bad "an empty emulator-console capture is rejected with state diagnostics" \
            "console=$console_calls exec=$calls fallback=$fallback_calls; $(< "$temp/console-empty-screencap.log")"
    fi

    calls=0 fallback_calls=0 pulls=0 cleanups=0 console_calls=0
    mode=retry
    capture_android_png "$temp/retried.png" "$PKG"
    if [[ "$calls" -eq 3 && "$fallback_calls" -eq 0 && -s "$temp/retried.png" \
          && -z "$(python3 "$ANDROID_PNG_VALIDATOR" "$temp/retried.png" 2>&1)" \
          && "$(< "$temp/retried-screencap.log")" == *"error: device offline"* ]]; then
        ok "a zero-byte adb screencap is retried into a PNG"
    else
        bad "a zero-byte adb screencap is retried into a PNG" \
            "calls=$calls; $(< "$temp/retried-screencap.log")"
    fi

    calls=0 fallback_calls=0 pulls=0 cleanups=0 console_calls=0
    mode=black
    if capture_android_png "$temp/black-direct.png" "$PKG"; then
        bad "a decodable black real-device frame is rejected"
    elif [[ "$calls" -eq 1 && "$fallback_calls" -eq 0 && "$pulls" -eq 0 \
            && ! -e "$temp/black-direct.png" && -s "$temp/black-direct.png.failed" \
            && "$(< "$temp/black-direct-screencap.log")" == *"contentless: uniform RGB frame"* ]]; then
        ok "a decodable black real-device frame is rejected"
    else
        bad "a decodable black real-device frame is rejected" \
            "calls=$calls fallback=$fallback_calls; $(< "$temp/black-direct-screencap.log")"
    fi

    calls=0 fallback_calls=0 pulls=0 cleanups=0 console_calls=0
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
    mode=direct_empty
    if capture_android_png "$temp/direct-empty.png" "$PKG"; then
        bad "zero-byte direct captures fail closed when device capture fails"
    elif [[ "$calls" -eq 3 && "$fallback_calls" -eq 1 && "$pulls" -eq 0 \
            && "$cleanups" -eq 1 && ! -e "$temp/direct-empty.png" \
            && ! -e "$temp/direct-empty.png.failed" \
            && "$(< "$temp/direct-empty-screencap.log")" == *"adb_status=0 bytes=0"* \
            && "$(< "$temp/direct-empty-screencap.log")" == *"fallback_capture_status=1 pull_status=not-run"* \
            && "$(< "$temp/direct-empty-screencap.log")" == *"device-side screencap failed"* \
            && "$(< "$temp/direct-empty-screencap.log")" == *"mCurrentFocus=$PKG"* ]]; then
        ok "zero-byte direct captures fail closed with transport and focus diagnostics"
    else
        bad "zero-byte direct captures fail closed with transport and focus diagnostics" \
            "exec=$calls fallback=$fallback_calls pull=$pulls cleanup=$cleanups; $(< "$temp/direct-empty-screencap.log")"
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
    mode=fallback_cleanup_failure
    if capture_android_png "$temp/fallback-cleanup-failed.png" "$PKG"; then
        bad "failed fallback cleanup prevents screenshot publication"
    elif [[ "$calls" -eq 3 && "$fallback_calls" -eq 1 && "$pulls" -eq 1 \
            && "$cleanups" -eq 1 && ! -e "$temp/fallback-cleanup-failed.png" \
            && -s "$temp/fallback-cleanup-failed.png.failed" \
            && -z "$(python3 "$ANDROID_PNG_VALIDATOR" "$temp/fallback-cleanup-failed.png.failed" 2>&1)" \
            && "$(< "$temp/fallback-cleanup-failed-screencap.log")" == *"cleanup_status=1"* \
            && "$(< "$temp/fallback-cleanup-failed-screencap.log")" == *"Permission denied"* \
            && "$(< "$temp/fallback-cleanup-failed-screencap.log")" == *"cleanup failed; pulled output preserved; not retried"* ]]; then
        ok "failed fallback cleanup preserves evidence and fails closed"
    else
        bad "failed fallback cleanup preserves evidence and fails closed" \
            "exec=$calls fallback=$fallback_calls pull=$pulls cleanup=$cleanups; $(< "$temp/fallback-cleanup-failed-screencap.log")"
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
