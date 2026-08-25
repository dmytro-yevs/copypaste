#!/usr/bin/env bash

# shellcheck source=scripts/release/android-adb-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-adb-lib.sh"

ANDROID_SCREENCAP_ATTEMPTS="${ANDROID_SCREENCAP_ATTEMPTS:-3}"
ANDROID_SCREENCAP_RETRY_DELAY="${ANDROID_SCREENCAP_RETRY_DELAY:-1}"
ANDROID_EMULATOR_SCREENCAP_TIMEOUT="${ANDROID_EMULATOR_SCREENCAP_TIMEOUT:-15}"
ANDROID_EMULATOR_SCREENCAP_POLLS="${ANDROID_EMULATOR_SCREENCAP_POLLS:-20}"
ANDROID_EMULATOR_SCREENCAP_POLL_DELAY="${ANDROID_EMULATOR_SCREENCAP_POLL_DELAY:-0.25}"
ANDROID_PNG_VALIDATOR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/png_evidence.py"

android_file_size() { # <file>
    wc -c < "$1" | tr -d '[:space:]'
}

run_with_android_screencap_timeout() {
    timeout --foreground "${ANDROID_EMULATOR_SCREENCAP_TIMEOUT}s" "$@"
}

# The exclusion is exported rather than applied through [adb_], because the
# timeout wrapper is a real process and cannot run a shell function — and it is
# that process, not this shell, which spawns adb and rewrites its arguments.
bounded_adb() {
    MSYS2_ARG_CONV_EXCL="$ANDROID_DEVICE_PATH_EXCL" run_with_android_screencap_timeout adb "$@"
}

targeted_adb() { # <serial> <adb args...>
    local serial="$1"
    shift
    [[ -n "$serial" ]] || return 2
    bounded_adb -s "$serial" "$@"
}

android_serial_candidate() {
    if [[ -n "${ANDROID_SERIAL:-}" ]]; then
        printf '%s\n' "$ANDROID_SERIAL"
    elif [[ "${EMULATOR_PORT:-}" =~ ^[0-9]+$ ]]; then
        printf 'emulator-%s\n' "$EMULATOR_PORT"
    else
        return 1
    fi
}

verified_android_serial() { # <candidate> <log>
    local candidate="$1" log="$2" actual="" status
    : > "$log"
    if [[ -z "$candidate" ]]; then
        printf 'route=trusted-serial status=unresolved screenshot not attempted\n' >> "$log"
        return 1
    fi
    if actual="$(targeted_adb "$candidate" get-serialno 2>> "$log")"; then
        status=0
    else
        status=$?
    fi
    actual="$(printf '%s' "$actual" | tr -d '\r\n')"
    printf 'route=trusted-serial status=%s timeout=%ss requested=%s actual=%s\n' \
        "$status" "$ANDROID_EMULATOR_SCREENCAP_TIMEOUT" "$candidate" "${actual:-unresolved}" >> "$log"
    [[ "$status" -eq 0 && "$actual" == "$candidate" ]] || return 1
    printf '%s\n' "$candidate"
}

emulator_console_screenshot() { # <serial> <host directory>
    targeted_adb "$1" emu screenrecord screenshot "$2"
}

append_bounded_adb_diagnostic() { # <log> <label> <serial> <filter> <adb args...>
    local log="$1" label="$2" serial="$3" filter="$4" output status
    shift 4
    if output="$(targeted_adb "$serial" "$@" 2>&1)"; then
        status=0
    else
        status=$?
    fi
    printf '== %s adb_status=%s timeout=%ss serial=%s\n' \
        "$label" "$status" "$ANDROID_EMULATOR_SCREENCAP_TIMEOUT" "${serial:-unresolved}" >> "$log"
    if [[ "$status" -eq 124 ]]; then
        printf 'adb diagnostic timed out after %ss\n' "$ANDROID_EMULATOR_SCREENCAP_TIMEOUT" >> "$log"
    fi
    if [[ -n "$output" && "$filter" == focus ]]; then
        printf '%s\n' "$output" | grep -E 'mCurrentFocus|mFocusedApp' | head -n 4 >> "$log" || true
    elif [[ -n "$output" ]]; then
        printf '%s\n' "$output" >> "$log"
    fi
}

append_android_capture_diagnostics() { # <log> <package> <serial>
    append_bounded_adb_diagnostic "$1" "adb state" "$3" all get-state
    append_bounded_adb_diagnostic "$1" "power" "$3" all shell dumpsys power
    append_bounded_adb_diagnostic "$1" "display" "$3" all shell dumpsys display
    append_bounded_adb_diagnostic "$1" "focus" "$3" focus shell dumpsys window
    printf 'expected_package=%s\n' "$2" >> "$1"
}

capture_emulator_console_png() { # <png> <serial> <log>
    local png="$1" serial="$2" log="$3" candidate="${1}.candidate"
    local validation="${1}.validation" console_log="${1}.console-log"
    local console_dir generated status bytes valid cleanup_status poll attempt reason

    for attempt in $(seq 1 "$ANDROID_SCREENCAP_ATTEMPTS"); do
        generated=""; status=0; bytes=0; valid=no; reason=""
        rm -f "$candidate" "$console_log" "$validation"
        console_dir="$(mktemp -d "${TMPDIR:-/tmp}/copypaste-emulator-screencap.XXXXXX")" || {
            printf 'capture_failure=emulator console host temporary directory unavailable\n' >> "$log"
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
            bytes="$(android_file_size "$candidate")"
            if [[ "$bytes" -gt 0 ]] \
                && python3 "$ANDROID_PNG_VALIDATOR" "$candidate" 2> "$validation"; then
                valid=yes
            fi
        fi
        if rm -rf -- "$console_dir"; then cleanup_status=0; else cleanup_status=$?; fi
        printf 'transport=emulator-console attempt=%s serial=%s console_status=%s bytes=%s decodable_contentful_png=%s cleanup_status=%s\n' \
            "$attempt" "$serial" "$status" "$bytes" "$valid" "$cleanup_status" >> "$log"
        [[ ! -s "$console_log" ]] || sed 's/^/emulator console: /' "$console_log" >> "$log"
        [[ ! -s "$validation" ]] || sed 's/^/PNG decoder: /' "$validation" >> "$log"
        rm -f "$console_log" "$validation"
        if [[ "$cleanup_status" -ne 0 ]]; then
            reason="emulator console temporary cleanup failed"
            break
        fi
        if [[ "$bytes" -gt 0 && "$valid" != yes ]]; then
            reason="emulator console output failed PNG evidence validation; not retried"
            break
        fi
        if [[ "$status" -eq 0 && "$bytes" -gt 0 && "$valid" == yes ]]; then
            mv "$candidate" "$png"
            return 0
        fi
        if [[ "$status" -ne 0 ]]; then
            reason="emulator console command failed"
        elif [[ -z "$generated" ]]; then
            reason="emulator console produced no Screenshot_*.png"
        else
            reason="emulator console produced a zero-byte Screenshot_*.png"
        fi
        if [[ "$attempt" -lt "$ANDROID_SCREENCAP_ATTEMPTS" ]]; then
            printf 'capture_retry=%s next_attempt=%s\n' "$reason" "$((attempt + 1))" >> "$log"
            sleep "$ANDROID_SCREENCAP_RETRY_DELAY"
        fi
    done
    if [[ "$attempt" -eq "$ANDROID_SCREENCAP_ATTEMPTS" \
          && "$reason" != *"not retried"* && "$reason" != *"cleanup failed"* ]]; then
        reason="${reason}; transport retries exhausted after ${ANDROID_SCREENCAP_ATTEMPTS} attempts"
    fi
    printf 'capture_failure=%s\n' "$reason" >> "$log"
    [[ ! -s "$candidate" ]] || mv "$candidate" "${png}.failed"
    return 1
}

capture_android_png() { # <png> <package> <trusted serial>
    local png="$1" package="$2" requested_serial="$3" log="${1%.png}-screencap.log"
    local candidate="${png}.candidate" stderr="${png}.stderr"
    local validation="${png}.validation" fallback_log="${png}.fallback"
    local pull_log="${png}.pull" cleanup_log="${png}.cleanup"
    local route_log="${png}.route"
    local remote="/data/local/tmp/copypaste-screencap-$$-${RANDOM}.png"
    local attempt status bytes valid fallback_status pull_status cleanup_status serial route_status

    : > "$log"
    rm -f "$png" "$candidate" "$stderr" "$validation" "$fallback_log" \
        "$pull_log" "$cleanup_log" "$route_log" "${png}.failed"
    if ! python3 -c 'from PIL import Image' 2>> "$log"; then
        printf 'PNG validation unavailable: install requirements-ci.txt\n' >> "$log"
        return 1
    fi

    if serial="$(verified_android_serial "$requested_serial" "$route_log")"; then
        route_status=0
    else
        route_status=$?
    fi
    command cat "$route_log" >> "$log"
    rm -f "$route_log"
    if [[ "$route_status" -ne 0 || -z "$serial" ]]; then
        printf 'capture_failure=trusted adb route could not be proven; screenshot not attempted\n' >> "$log"
        return 1
    fi
    if [[ "$serial" == emulator-* ]]; then
        capture_emulator_console_png "$png" "$serial" "$log" || status=$?
        status="${status:-0}"
        append_android_capture_diagnostics "$log" "$package" "$serial"
        return "$status"
    fi

    for attempt in $(seq 1 "$ANDROID_SCREENCAP_ATTEMPTS"); do
        rm -f "$candidate" "$stderr" "$validation"
        targeted_adb "$serial" shell input keyevent KEYCODE_WAKEUP >/dev/null 2>> "$log" || true
        targeted_adb "$serial" shell wm dismiss-keyguard >/dev/null 2>> "$log" || true
        if targeted_adb "$serial" exec-out screencap -p > "$candidate" 2> "$stderr"; then
            status=0
        else
            status=$?
        fi
        bytes=0
        [[ -f "$candidate" ]] && bytes="$(android_file_size "$candidate")"
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
        if targeted_adb "$serial" shell screencap -p "$remote" > "$fallback_log" 2>&1; then
            fallback_status=0
        else
            fallback_status=$?
        fi
        pull_status=not-run
        if [[ "$fallback_status" -eq 0 ]]; then
            if targeted_adb "$serial" pull "$remote" "$candidate" > "$pull_log" 2>&1; then
                pull_status=0
            else
                pull_status=$?
            fi
        fi
        bytes=0
        [[ -f "$candidate" ]] && bytes="$(android_file_size "$candidate")"
        valid=no
        if [[ "$pull_status" == 0 ]] \
            && python3 "$ANDROID_PNG_VALIDATOR" "$candidate" 2> "$validation"; then
            valid=yes
        fi
        if targeted_adb "$serial" shell rm -f "$remote" > "$cleanup_log" 2>&1; then
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

    append_android_capture_diagnostics "$log" "$package" "$serial"
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
    local mode=retry remote_file="" adb_argv_log="$temp/adb-argv.log"
    local timeout_argv_log="$temp/timeout-argv.log"
    ANDROID_SCREENCAP_ATTEMPTS=3
    ANDROID_SCREENCAP_RETRY_DELAY=0
    ANDROID_EMULATOR_SCREENCAP_POLL_DELAY=0
    python3 - "$temp/good.png" "$temp/black.png" <<'PY'
import sys
from PIL import Image

good = Image.new("RGB", (2, 2), (220, 38, 38))
good.putpixel((1, 1), (255, 255, 255))
good.save(sys.argv[1])
Image.new("RGB", (8, 8), "black").save(sys.argv[2])
PY
    if [[ "$(ANDROID_SERIAL=device-123 EMULATOR_PORT=5554 android_serial_candidate)" == device-123 \
          && "$(ANDROID_SERIAL='' EMULATOR_PORT=5556 android_serial_candidate)" == emulator-5556 ]] \
       && ! (unset ANDROID_SERIAL EMULATOR_PORT; android_serial_candidate); then
        ok "the caller boundary resolves only an explicit device or runner emulator"
    else
        bad "the caller boundary resolves only an explicit device or runner emulator"
    fi
    mkdir -p "$temp/bounded-adb-bin"
    command cat > "$temp/bounded-adb-bin/adb" <<'SH'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$ANDROID_TIMEOUT_FIXTURE_ARGV"
[[ "${1:-}" == -s && "${2:-}" == emulator-5554 ]] || exit 97
shift 2
if [[ "$ANDROID_TIMEOUT_FIXTURE_MODE" == route-hang ]]; then
    while :; do :; done
elif [[ "${1:-}" == get-serialno ]]; then
    printf 'emulator-5554\n'
elif [[ "$*" == *"emu screenrecord screenshot"* ]]; then
    if [[ "$ANDROID_TIMEOUT_FIXTURE_MODE" == diagnostic-success ]]; then
        command cp "$ANDROID_TIMEOUT_FIXTURE_GOOD" "${!#}/Screenshot_fixture.png"
    else
        printf 'emulator console unavailable\n' >&2
        exit 1
    fi
else
    while :; do :; done
fi
SH
    chmod +x "$temp/bounded-adb-bin/adb"

    local started elapsed
    # macOS `date` prints the unsupported `%3N` literally; use a portable
    # millisecond clock so an arithmetic error cannot fall through to real adb.
    epoch_millis() {
        python3 -c 'import time; print(time.time_ns() // 1_000_000)'
    }
    started="$(epoch_millis)"
    if (export PATH="$temp/bounded-adb-bin:$PATH"
        export ANDROID_TIMEOUT_FIXTURE_MODE=route-hang
        export ANDROID_TIMEOUT_FIXTURE_GOOD="$temp/good.png"
        export ANDROID_TIMEOUT_FIXTURE_ARGV="$temp/route-timeout.argv"
        export ANDROID_EMULATOR_SCREENCAP_TIMEOUT=0.3
        capture_android_png "$temp/route-timeout.png" "$PKG" emulator-5554); then
        bad "a hung adb route selection fails closed"
    else
        elapsed=$(( $(epoch_millis) - started ))
        if [[ "$elapsed" -lt 3000 && ! -e "$temp/route-timeout.png" \
              && ! -e "$temp/route-timeout.png.failed" \
              && "$(< "$temp/route-timeout-screencap.log")" == *"route=trusted-serial status=124"* \
              && "$(< "$temp/route-timeout-screencap.log")" == *"screenshot not attempted"* \
              && "$(< "$temp/route-timeout.argv")" == "-s emulator-5554 get-serialno" \
              && "$(< "$temp/route-timeout-screencap.log")" != *"== adb state"* ]]; then
            ok "a hung targeted route check fails closed without device diagnostics"
        else
            bad "a hung targeted route check fails closed without device diagnostics" \
                "elapsed=${elapsed}ms; argv=$(< "$temp/route-timeout.argv"); $(< "$temp/route-timeout-screencap.log")"
        fi
    fi

    started="$(epoch_millis)"
    if (export PATH="$temp/bounded-adb-bin:$PATH"
        export ANDROID_TIMEOUT_FIXTURE_MODE=diagnostic-success
        export ANDROID_TIMEOUT_FIXTURE_GOOD="$temp/good.png"
        export ANDROID_TIMEOUT_FIXTURE_ARGV="$temp/diagnostic-success.argv"
        export ANDROID_EMULATOR_SCREENCAP_TIMEOUT=0.3
        export ANDROID_EMULATOR_SCREENCAP_POLL_DELAY=0
        capture_android_png "$temp/diagnostic-timeout-success.png" "$PKG" emulator-5554); then
        elapsed=$(( $(epoch_millis) - started ))
        if [[ "$elapsed" -lt 3000 && -s "$temp/diagnostic-timeout-success.png" \
              && "$(< "$temp/diagnostic-timeout-success-screencap.log")" == *"adb_status=124 timeout=0.3s serial=emulator-5554"* \
              && "$(< "$temp/diagnostic-timeout-success-screencap.log")" == *"expected_package=$PKG"* \
              && "$(wc -l < "$temp/diagnostic-success.argv")" -eq 6 ]] \
           && awk 'NF && ($1 != "-s" || $2 != "emulator-5554") { bad=1 } END { exit bad }' \
                "$temp/diagnostic-success.argv"; then
            ok "hung success diagnostics return with serial-targeted timeout evidence"
        else
            bad "hung success diagnostics return with serial-targeted timeout evidence" \
                "elapsed=${elapsed}ms; $(< "$temp/diagnostic-timeout-success-screencap.log")"
        fi
    else
        bad "hung success diagnostics return with serial-targeted timeout evidence" \
            "$(< "$temp/diagnostic-timeout-success-screencap.log")"
    fi

    started="$(epoch_millis)"
    if (export PATH="$temp/bounded-adb-bin:$PATH"
        export ANDROID_TIMEOUT_FIXTURE_MODE=diagnostic-failure
        export ANDROID_TIMEOUT_FIXTURE_GOOD="$temp/good.png"
        export ANDROID_TIMEOUT_FIXTURE_ARGV="$temp/diagnostic-failure.argv"
        export ANDROID_EMULATOR_SCREENCAP_TIMEOUT=0.3
        export ANDROID_SCREENCAP_RETRY_DELAY=0
        capture_android_png "$temp/diagnostic-timeout-failure.png" "$PKG" emulator-5554); then
        bad "hung failure diagnostics return without publishing evidence"
    else
        elapsed=$(( $(epoch_millis) - started ))
        if [[ "$elapsed" -lt 10000 && ! -e "$temp/diagnostic-timeout-failure.png" \
              && ! -e "$temp/diagnostic-timeout-failure.png.failed" \
              && "$(< "$temp/diagnostic-timeout-failure-screencap.log")" == *"transport retries exhausted after 3 attempts"* \
              && "$(< "$temp/diagnostic-timeout-failure-screencap.log")" == *"adb_status=124 timeout=0.3s serial=emulator-5554"* \
              && "$(wc -l < "$temp/diagnostic-failure.argv")" -eq 8 ]] \
           && awk 'NF && ($1 != "-s" || $2 != "emulator-5554") { bad=1 } END { exit bad }' \
                "$temp/diagnostic-failure.argv"; then
            ok "hung failure diagnostics return without publishing evidence"
        else
            bad "hung failure diagnostics return without publishing evidence" \
                "elapsed=${elapsed}ms; $(< "$temp/diagnostic-timeout-failure-screencap.log")"
        fi
    fi

    : > "$adb_argv_log"
    : > "$timeout_argv_log"
    timeout() {
        printf '%s\n' "$*" >> "$timeout_argv_log"
        [[ "${1:-}" == --foreground && "${2:-}" == "${ANDROID_EMULATOR_SCREENCAP_TIMEOUT}s" ]] || return 98
        shift 2
        "$@"
    }
    adb() {
        printf '%s\n' "$*" >> "$adb_argv_log"
        [[ "${1:-}" == -s && -n "${2:-}" ]] || return 97
        local selected_serial="$2"
        shift 2
        if [[ "${1:-}" == "get-serialno" ]]; then
            printf '%s\n' "$selected_serial"
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
        if [[ "$mode" == console_success \
              || "$mode" == console_retry && "$console_calls" -eq 2 ]]; then
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
        if [[ "$mode" == console_corrupt ]]; then
            command head -c 24 "$temp/good.png" > "$2/Screenshot_fixture.png"
            return 0
        fi
        printf 'emulator console unavailable\n' >&2
        return 1
    }

    mode=console_success
    capture_android_png "$temp/console.png" "$PKG" emulator-5554
    if [[ "$console_calls" -eq 1 && "$calls" -eq 0 && "$fallback_calls" -eq 0 \
          && -s "$temp/console.png" && ! -e "$temp/console.png.failed" \
          && "$(< "$temp/console-screencap.log")" == *"transport=emulator-console"* ]]; then
        ok "an emulator uses the host console screenshot transport"
    else
        bad "an emulator uses the host console screenshot transport" \
            "console=$console_calls exec=$calls fallback=$fallback_calls; $(< "$temp/console-screencap.log")"
    fi

    calls=0 fallback_calls=0 pulls=0 cleanups=0 console_calls=0
    mode=console_retry
    capture_android_png "$temp/console-retried.png" "$PKG" emulator-5554
    if [[ "$console_calls" -eq 2 && "$calls" -eq 0 && "$fallback_calls" -eq 0 \
          && -s "$temp/console-retried.png" && ! -e "$temp/console-retried.png.failed" \
          && "$(< "$temp/console-retried-screencap.log")" == *"attempt=1 serial=emulator-5554 console_status=1"* \
          && "$(< "$temp/console-retried-screencap.log")" == *"capture_retry=emulator console command failed next_attempt=2"* \
          && "$(< "$temp/console-retried-screencap.log")" == *"attempt=2 serial=emulator-5554 console_status=0"* ]]; then
        ok "an emulator console transport failure is retried into a PNG"
    else
        bad "an emulator console transport failure is retried into a PNG" \
            "console=$console_calls exec=$calls fallback=$fallback_calls; $(< "$temp/console-retried-screencap.log")"
    fi

    calls=0 fallback_calls=0 pulls=0 cleanups=0 console_calls=0
    mode=console_failure
    if capture_android_png "$temp/console-failed.png" "$PKG" emulator-5554; then
        bad "emulator console transport retries are bounded"
    elif [[ "$console_calls" -eq 3 && "$calls" -eq 0 && "$fallback_calls" -eq 0 \
            && ! -e "$temp/console-failed.png" && ! -e "$temp/console-failed.png.failed" \
            && "$(< "$temp/console-failed-screencap.log")" == *"attempt=3 serial=emulator-5554 console_status=1"* \
            && "$(< "$temp/console-failed-screencap.log")" == *"transport retries exhausted after 3 attempts"* \
            && "$(< "$temp/console-failed-screencap.log")" == *"mCurrentFocus=$PKG"* ]]; then
        ok "emulator console transport retries exhaust with state diagnostics"
    else
        bad "emulator console transport retries exhaust with state diagnostics" \
            "console=$console_calls exec=$calls fallback=$fallback_calls; $(< "$temp/console-failed-screencap.log")"
    fi

    calls=0 fallback_calls=0 pulls=0 cleanups=0 console_calls=0
    mode=console_black
    if capture_android_png "$temp/console-black.png" "$PKG" emulator-5554; then
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
    mode=console_corrupt
    if capture_android_png "$temp/console-corrupt.png" "$PKG" emulator-5554; then
        bad "a corrupt emulator frame is rejected"
    elif [[ "$console_calls" -eq 1 && "$calls" -eq 0 && "$fallback_calls" -eq 0 \
            && ! -e "$temp/console-corrupt.png" && -s "$temp/console-corrupt.png.failed" \
            && "$(< "$temp/console-corrupt-screencap.log")" == *"failed PNG evidence validation; not retried"* ]]; then
        ok "a corrupt emulator frame is rejected without transport retry"
    else
        bad "a corrupt emulator frame is rejected without transport retry" \
            "console=$console_calls exec=$calls fallback=$fallback_calls; $(< "$temp/console-corrupt-screencap.log")"
    fi

    calls=0 fallback_calls=0 pulls=0 cleanups=0 console_calls=0
    mode=console_empty
    if capture_android_png "$temp/console-empty.png" "$PKG" emulator-5554; then
        bad "an empty emulator-console capture is rejected"
    elif [[ "$console_calls" -eq 3 && "$calls" -eq 0 && "$fallback_calls" -eq 0 \
            && ! -e "$temp/console-empty.png" && ! -e "$temp/console-empty.png.failed" \
            && "$(< "$temp/console-empty-screencap.log")" == *"console_status=0 bytes=0"* \
            && "$(< "$temp/console-empty-screencap.log")" == *"produced no Screenshot_*.png; transport retries exhausted after 3 attempts"* \
            && "$(< "$temp/console-empty-screencap.log")" == *"mCurrentFocus=$PKG"* ]]; then
        ok "an empty emulator-console capture is rejected with state diagnostics"
    else
        bad "an empty emulator-console capture is rejected with state diagnostics" \
            "console=$console_calls exec=$calls fallback=$fallback_calls; $(< "$temp/console-empty-screencap.log")"
    fi

    calls=0 fallback_calls=0 pulls=0 cleanups=0 console_calls=0
    mode=retry
    capture_android_png "$temp/retried.png" "$PKG" device-123
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
    if capture_android_png "$temp/black-direct.png" "$PKG" device-123; then
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
    capture_android_png "$temp/fallback.png" "$PKG" device-123
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
    if capture_android_png "$temp/direct-empty.png" "$PKG" device-123; then
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
    if capture_android_png "$temp/fallback-failed.png" "$PKG" device-123; then
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
    if capture_android_png "$temp/fallback-cleanup-failed.png" "$PKG" device-123; then
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
    if capture_android_png "$temp/fallback-corrupt.png" "$PKG" device-123; then
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
    if capture_android_png "$temp/corrupt.png" "$PKG" device-123; then
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

    if [[ "$(wc -l < "$adb_argv_log")" -eq "$(wc -l < "$timeout_argv_log")" ]] \
       && awk 'NF && ($1 != "-s" || $2 == "") { bad=1 } END { exit bad }' "$adb_argv_log" \
       && awk 'NF && ($1 != "--foreground" || $3 != "adb" || $4 != "-s" || $5 == "") { bad=1 } END { exit bad }' \
            "$timeout_argv_log"; then
        ok "every adb fixture call records a timeout and explicit serial"
    else
        bad "every adb fixture call records a timeout and explicit serial" \
            "adb=$(wc -l < "$adb_argv_log") timeout=$(wc -l < "$timeout_argv_log")"
    fi
}
