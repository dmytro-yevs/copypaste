#!/usr/bin/env bash
# Runs the existing smoke harness across API boundaries without duplicating it.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SMOKE="$HERE/android-smoke.sh"
DEFAULT_LEVELS=(29 33 34 36)

AVD_PREFIX="${AVD_PREFIX:-copypaste-api}"
OUT_ROOT="${SMOKE_OUT_ROOT:-artifacts/android-smoke-levels}"
BOOT_TIMEOUT="${BOOT_TIMEOUT:-300}"
EMULATOR_OPTS="${EMULATOR_OPTS:--no-window -gpu swiftshader_indirect -no-snapshot -noaudio -no-boot-anim -camera-back none -camera-front none}"

fatal() { printf '  FATAL %s\n' "$*" >&2; exit 1; }

select_levels() {
    LEVELS=("$@")
    if [[ ${#LEVELS[@]} -eq 0 ]]; then
        read -r -a LEVELS <<<"${API_LEVELS:-${DEFAULT_LEVELS[*]}}"
    fi
}

validate_levels() {
    local level
    [[ ${#LEVELS[@]} -gt 0 ]] || return 1
    for level in "${LEVELS[@]}"; do
        [[ "$level" =~ ^[0-9]+$ ]] || return 1
    done
}

all_passed() {
    local result level verdict detail
    [[ $# -gt 0 ]] || return 1
    for result in "$@"; do
        read -r level verdict detail <<<"$result"
        [[ -n "$level" && "$verdict" == "PASS" ]] || return 1
    done
}

self_test() {
    local failures=0

    unset API_LEVELS
    select_levels
    [[ "${LEVELS[*]}" == "29 33 34 36" ]] || failures=$((failures + 1))

    API_LEVELS="24 36" select_levels
    [[ "${LEVELS[*]}" == "24 36" ]] || failures=$((failures + 1))
    unset API_LEVELS

    select_levels 29 33 34 36
    validate_levels || failures=$((failures + 1))
    select_levels invalid
    validate_levels && failures=$((failures + 1))

    all_passed "29 PASS -" "36 PASS -" || failures=$((failures + 1))
    all_passed "29 PASS -" "33 WRONG-IMAGE api36" && failures=$((failures + 1))
    all_passed "34 BOOT-FAILED -" && failures=$((failures + 1))
    all_passed && failures=$((failures + 1))

    if [[ $failures -eq 0 ]]; then
        printf 'PASS android-smoke-levels self-test\n'
        return 0
    fi
    printf 'FAIL android-smoke-levels self-test (%s)\n' "$failures" >&2
    return 1
}

require_exclusive_adb() {
    local attached
    attached="$(adb devices | tr -d '\r' | awk 'NR > 1 && NF {count++} END {print count + 0}')"
    [[ "$attached" == "0" ]] || {
        adb devices
        fatal "$attached device(s) already attached; use the serialized Android validation task"
    }
}

boot() {
    local avd="$1" log="$2" waited=0
    # shellcheck disable=SC2086 -- EMULATOR_OPTS is an argument vector override.
    "$EMULATOR_BIN" -avd "$avd" $EMULATOR_OPTS >"$log" 2>&1 &
    EMU_PID=$!
    adb wait-for-device
    EMU_SERIAL="$(adb devices | tr -d '\r' | awk '/\sdevice$/ {print $1; exit}')"
    [[ "$EMU_SERIAL" == emulator-* ]] || return 1
    while [[ "$(adb -s "$EMU_SERIAL" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" != "1" ]]; do
        sleep 5
        waited=$((waited + 5))
        [[ $waited -ge $BOOT_TIMEOUT ]] && return 1
        kill -0 "$EMU_PID" 2>/dev/null || return 1
    done
    adb -s "$EMU_SERIAL" shell settings put global window_animation_scale 0 >/dev/null 2>&1
    adb -s "$EMU_SERIAL" shell settings put global transition_animation_scale 0 >/dev/null 2>&1
    adb -s "$EMU_SERIAL" shell settings put global animator_duration_scale 0 >/dev/null 2>&1
}

shutdown() {
    if [[ -n "${EMU_SERIAL:-}" ]]; then
        adb -s "$EMU_SERIAL" emu kill >/dev/null 2>&1 || true
    fi
    if [[ -n "${EMU_PID:-}" ]]; then
        kill "$EMU_PID" 2>/dev/null || true
    fi
    EMU_SERIAL=""
    EMU_PID=""
}

if [[ "${1:-}" == "--self-test" ]]; then
    self_test
    exit $?
fi

select_levels "$@"
validate_levels || fatal "API levels must be one or more numeric values"
command -v adb >/dev/null 2>&1 || fatal "adb is not on PATH"
[[ -x "$SMOKE" ]] || fatal "$SMOKE is not executable"
[[ -f "${APK:-}" ]] || fatal "APK not found at '${APK:-<unset>}'"

EMULATOR_BIN="${EMULATOR_BIN:-}"
if [[ -z "$EMULATOR_BIN" ]]; then
    for candidate in \
        "${ANDROID_HOME:-}/emulator/emulator" \
        "${ANDROID_SDK_ROOT:-}/emulator/emulator" \
        "${LOCALAPPDATA:-}/Android/Sdk/emulator/emulator"; do
        if [[ -x "$candidate" ]]; then
            EMULATOR_BIN="$candidate"
            break
        elif [[ -x "$candidate.exe" ]]; then
            EMULATOR_BIN="$candidate.exe"
            break
        fi
    done
fi
[[ -n "$EMULATOR_BIN" ]] || EMULATOR_BIN="$(command -v emulator || true)"
[[ -n "$EMULATOR_BIN" ]] || fatal "no emulator binary; set EMULATOR_BIN or ANDROID_HOME"

mkdir -p "$OUT_ROOT"
require_exclusive_adb
trap shutdown EXIT

RESULTS=()
for level in "${LEVELS[@]}"; do
    avd="${AVD_PREFIX}${level}"
    out="$OUT_ROOT/api$level"
    mkdir -p "$out"
    printf '\n=== API %s (%s) ===\n' "$level" "$avd"

    if ! boot "$avd" "$out/emulator.log"; then
        RESULTS+=("$level BOOT-FAILED -")
        shutdown
        continue
    fi

    actual="$(adb -s "$EMU_SERIAL" shell getprop ro.build.version.sdk | tr -d '\r')"
    if [[ "$actual" != "$level" ]]; then
        RESULTS+=("$level WRONG-IMAGE api$actual")
        shutdown
        continue
    fi

    adb -s "$EMU_SERIAL" shell dumpsys webviewupdate >"$out/webviewupdate.txt" 2>&1
    adb -s "$EMU_SERIAL" shell getprop ro.build.fingerprint | tr -d '\r' >"$out/fingerprint.txt"
    ANDROID_SERIAL="$EMU_SERIAL" SMOKE_OUT="$out" APK="$APK" "$SMOKE"
    code=$?
    [[ $code -eq 0 ]] && RESULTS+=("$level PASS -") || RESULTS+=("$level FAIL exit-$code")
    shutdown
done

printf '\n=== Per level ===\n'
for result in "${RESULTS[@]}"; do
    read -r level verdict detail <<<"$result"
    printf '  API %-3s %-12s %s\n' "$level" "$verdict" "$detail"
done
printf '  evidence under %s\n' "$OUT_ROOT"

all_passed "${RESULTS[@]}"
