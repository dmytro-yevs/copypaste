#!/usr/bin/env bash
# android-smoke-levels.sh — run the smoke harness once per API level.
#
# Clipboard access is API-gated (docs/rewrite/android-clipboard-access.md), so a
# run at one level is one point on a curve. This boots one AVD at a time, runs
# scripts/release/android-smoke.sh against it, and keeps each level's evidence
# in its own directory.
#
# docs/rewrite/android-api-levels.md says which assertions are expected to
# differ per level, and names the ones that pass identically everywhere.
#
#   APK=/path/to/app-debug.apk ./scripts/release/android-smoke-levels.sh 29 33 34 36
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SMOKE="$HERE/android-smoke.sh"

LEVELS=("$@")
[[ ${#LEVELS[@]} -eq 0 ]] && read -r -a LEVELS <<<"${API_LEVELS:-29 33 34 36}"

AVD_PREFIX="${AVD_PREFIX:-copypaste-api}"
OUT_ROOT="${SMOKE_OUT_ROOT:-artifacts/android-smoke-levels}"
BOOT_TIMEOUT="${BOOT_TIMEOUT:-300}"
EMULATOR_OPTS="${EMULATOR_OPTS:--no-window -gpu swiftshader_indirect -no-snapshot -noaudio -no-boot-anim -camera-back none -camera-front none}"

fatal() { printf '  FATAL %s\n' "$*" >&2; exit 1; }

# The harness makes every adb call unqualified and refuses to run with more than
# one device attached. Booting a second emulator next to one somebody else is
# driving does not merely confuse this script — it fails theirs. Asserted before
# anything is started, because after that the damage is done.
require_exclusive_adb() {
    local attached
    attached="$(adb devices | tr -d '\r' | grep -cE '\sdevice$')"
    [[ "$attached" == "0" ]] || {
        adb devices
        fatal "$attached device(s) already attached. This script owns adb for its whole run; another emulator or a smoke run in progress would be broken by it."
    }
}

boot() {          # <avd>
    local avd="$1" waited=0
    "$EMULATOR_BIN" -avd "$avd" $EMULATOR_OPTS >"$OUT_ROOT/$avd-emulator.log" 2>&1 &
    EMU_PID=$!
    adb wait-for-device
    while [[ "$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" != "1" ]]; do
        sleep 5
        waited=$((waited + 5))
        [[ $waited -ge $BOOT_TIMEOUT ]] && return 1
        kill -0 "$EMU_PID" 2>/dev/null || return 1
    done
    # Same three the emulator-runner action sets. uiautomator refuses to dump
    # while the screen animates, and the harness's paint assertion is a dump.
    adb shell settings put global window_animation_scale 0 >/dev/null 2>&1
    adb shell settings put global transition_animation_scale 0 >/dev/null 2>&1
    adb shell settings put global animator_duration_scale 0 >/dev/null 2>&1
    return 0
}

shutdown() {
    adb emu kill >/dev/null 2>&1
    local waited=0
    while adb devices | tr -d '\r' | grep -qE '\sdevice$'; do
        sleep 2
        waited=$((waited + 2))
        [[ $waited -ge 60 ]] && break
    done
    [[ -n "${EMU_PID:-}" ]] && kill "$EMU_PID" 2>/dev/null
    EMU_PID=""
}

command -v adb >/dev/null 2>&1 || fatal "adb is not on PATH"
[[ -x "$SMOKE" ]] || fatal "$SMOKE is not executable"
[[ -f "${APK:-}" ]] || fatal "APK not found at '${APK:-<unset>}'"

EMULATOR_BIN="${EMULATOR_BIN:-}"
if [[ -z "$EMULATOR_BIN" ]]; then
    for candidate in \
        "${ANDROID_HOME:-}/emulator/emulator" \
        "${ANDROID_SDK_ROOT:-}/emulator/emulator" \
        "${LOCALAPPDATA:-}/Android/Sdk/emulator/emulator"; do
        [[ -x "$candidate" || -x "$candidate.exe" ]] && { EMULATOR_BIN="$candidate"; break; }
    done
fi
[[ -n "$EMULATOR_BIN" ]] || EMULATOR_BIN="$(command -v emulator || true)"
[[ -n "$EMULATOR_BIN" ]] || fatal "no emulator binary; set EMULATOR_BIN or ANDROID_HOME"

mkdir -p "$OUT_ROOT"
require_exclusive_adb

# Only after the exclusivity check: `adb emu kill` takes whatever emulator is
# attached, so arming this earlier would let a refused run kill somebody else's.
trap shutdown EXIT

declare -a RESULTS=()

for level in "${LEVELS[@]}"; do
    avd="${AVD_PREFIX}${level}"
    out="$OUT_ROOT/api$level"
    mkdir -p "$out"
    printf '\n=== API %s (%s) ===\n' "$level" "$avd"

    if ! boot "$avd"; then
        RESULTS+=("$level BOOT-FAILED -")
        printf '  did not reach sys.boot_completed in %ss; see %s\n' "$BOOT_TIMEOUT" "$OUT_ROOT/$avd-emulator.log"
        shutdown
        continue
    fi

    # An AVD's name is not evidence of its system image. A spread that silently
    # ran the same level four times would be the exact failure this script is
    # meant to remove.
    actual="$(adb shell getprop ro.build.version.sdk | tr -d '\r')"
    if [[ "$actual" != "$level" ]]; then
        RESULTS+=("$level WRONG-IMAGE api$actual")
        printf '  %s reports API %s, not %s — skipped\n' "$avd" "$actual" "$level"
        shutdown
        continue
    fi

    # Recorded per level because it is the one thing about the WebView an
    # emulator can say: which build the system image pinned. The shipped app
    # gets whatever Play last pushed, which is why testing-policy.md marks the
    # WebView version NOT VERIFIED IN CI.
    adb shell dumpsys webviewupdate > "$out/webviewupdate.txt" 2>&1
    printf '  webview: %s\n' \
        "$(grep -m1 'Current WebView package' "$out/webviewupdate.txt" | tr -d '\r' | sed 's/.*: //')"
    adb shell getprop ro.build.fingerprint | tr -d '\r' > "$out/fingerprint.txt"

    ANDROID_SERIAL="$(adb devices | tr -d '\r' | awk '/\sdevice$/ {print $1; exit}')" \
    SMOKE_OUT="$out" APK="$APK" "$SMOKE"
    code=$?
    [[ $code -eq 0 ]] && RESULTS+=("$level PASS -") || RESULTS+=("$level FAIL exit-$code")

    shutdown
done

printf '\n=== Per level ===\n'
failed=0
for r in "${RESULTS[@]}"; do
    read -r level verdict detail <<<"$r"
    printf '  API %-3s %-12s %s\n' "$level" "$verdict" "$detail"
    [[ "$verdict" == "PASS" ]] || failed=1
done
printf '  evidence under %s\n' "$OUT_ROOT"

# A level that did not run is not a level that passed. Anything other than PASS
# on every requested level fails the run, so a missing AVD cannot read as
# coverage.
exit $failed
