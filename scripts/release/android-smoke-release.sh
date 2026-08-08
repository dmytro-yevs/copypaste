#!/usr/bin/env bash
# android-smoke-release.sh — the minified APK, which is the one people install.
#
# The debug leg proves the stack: the Android Keystore, SQLCipher, both rung 0
# doorways all the way into the database. It cannot prove the *artefact*.
# `run-as` needs a debuggable package and R8 runs only on the release build
# type, so the two cannot be the same APK. The scheduled Android workflow uses
# a never-published signed fixture; release.yml invokes this same harness
# against the exact signed universal artifact before publication.
#
# R8 is the reason this leg exists. `CapturePlugin` is resolved by reflection
# from a package and a class name held in Rust — `PLUGIN_PACKAGE` and
# `PLUGIN_CLASS` in capture/android.rs — and reflection is exactly what a
# shrinker cannot see. proguard-rules.pro keeps `KeystoreContext` and says
# nothing about the plugin. The symptom would be an app that launches and does
# nothing, on the only build we ship.
#
# The question is therefore narrower than the debug leg's, and the half this
# cannot see is reported as NOT ASSERTED rather than left to look like a pass.
set -uo pipefail

# shellcheck source=scripts/release/android-smoke-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-smoke-lib.sh"

APK="${APK:-}"
MAIN="$PKG/.MainActivity"
INTAKE="$PKG/.IntakeActivity"
SETTLE_SECS="${SETTLE_SECS:-25}"
PAINT_TIMEOUT="${PAINT_TIMEOUT:-90}"

if [[ "${1:-}" == "--self-test" ]]; then
    self_test
    exit $?
fi

mkdir -p "$OUT"

# ---------------------------------------------------------------------------
group "Preflight"
# ---------------------------------------------------------------------------
command -v adb >/dev/null 2>&1 || { echo "  FATAL adb is not on PATH"; exit 1; }
adb wait-for-device
sdk="$(sh_ getprop ro.build.version.sdk)"
abi="$(sh_ getprop ro.product.cpu.abi)"
printf '  device: API %s, %s\n' "$sdk" "$abi"

[[ -f "$APK" ]] || { echo "  FATAL APK not found at '${APK:-<unset>}'"; exit 1; }
printf '  apk: %s (%s bytes)\n' "$APK" "$(stat -c %s "$APK")"

apk_libs="$(unzip -l "$APK")"
if grep -q "lib/${abi}/libcopypaste_ui_lib.so" <<<"$apk_libs"; then
    ok "the APK carries libcopypaste_ui_lib.so for $abi"
else
    bad "the APK carries libcopypaste_ui_lib.so for $abi" \
        "$(grep -E 'lib/.*\.so' <<<"$apk_libs" || echo 'no native libraries in the APK at all')"
    summary; exit 1
fi

# ---------------------------------------------------------------------------
group "Install"
# ---------------------------------------------------------------------------
adb uninstall "$PKG" >/dev/null 2>&1 || true
if install_out="$(adb install -r -g "$APK" 2>&1)"; then
    ok "adb install"
else
    bad "adb install" "$install_out"
    summary; exit 1
fi

# The whole point of this leg is that R8 ran. A debuggable package means the
# workflow handed over the debug APK, and every assertion below would pass
# while proving nothing about the build people install.
runas_id="$(sh_ run-as "$PKG" id)"
if grep -q uid <<<"$runas_id"; then
    bad "the installed package is NOT debuggable" \
        "run-as succeeded, so this is a debug build and R8 never ran on it"
    summary; exit 1
else
    ok "the installed package is not debuggable (R8 ran on it)"
fi

# ---------------------------------------------------------------------------
group "1. The shipped build launches"
# ---------------------------------------------------------------------------
adb logcat -c || true
launched_at="$SECONDS"
start_out="$(sh_ am start -W -n "$MAIN")"
printf '%s\n' "$start_out" | sed 's/^/        /'
if grep -q '^Status: ok' <<<"$start_out" && ! grep -qi 'Error' <<<"$start_out"; then
    ok "am start reports the activity started"
else
    bad "am start reports the activity started" "see the block above"
fi

wait_for 60 has_pid || true
pid1="$(app_pid)"
if [[ -n "$pid1" ]]; then
    ok "the app process exists (pid $pid1)"
else
    bad "the app process exists" "nothing named $PKG was running 60s after am start"
    dump_logcat release-launch
    summary; exit 1
fi

sleep "$SETTLE_SECS"
pid_now="$(app_pid)"
dump_logcat release-launch

if [[ "$pid_now" == "$pid1" ]]; then
    ok "the process survived ${SETTLE_SECS}s (setup did not abort)"
else
    bad "the process survived ${SETTLE_SECS}s (setup did not abort)" \
        "pid was $pid1, is now '${pid_now:-gone}'"
fi

crashes="$(crash_report "$OUT/release-launch.log")"
if [[ -z "$crashes" ]]; then
    ok "no crash block naming this app in logcat"
else
    bad "no crash block naming this app in logcat" "$(head -n 20 <<<"$crashes")"
fi

# Named separately from the crash check, because the reader needs the
# hypothesis handed to them: on this build type, these three are R8.
stripped="$(r8_report "$OUT/release-launch.log")"
if [[ -z "$stripped" ]]; then
    ok "no ClassNotFoundException, NoSuchMethodException or UnsatisfiedLinkError"
else
    bad "no ClassNotFoundException, NoSuchMethodException or UnsatisfiedLinkError" \
        "R8 stripped or renamed something the app looks up by name — CapturePlugin is resolved by reflection and proguard-rules.pro does not keep it: $(head -n 12 <<<"$stripped")"
fi

# ---------------------------------------------------------------------------
group "2. The UI paints"
# ---------------------------------------------------------------------------
wake_screen

window="$(sh_ dumpsys window)"
focus="$(grep -E 'mCurrentFocus|mFocusedApp' <<<"$window" | head -n 4)"
if grep -q "$PKG" <<<"$focus"; then
    ok "the focused window belongs to $PKG"
else
    bad "the focused window belongs to $PKG" "${focus:-dumpsys window reported no focus}"
fi

# uiautomator needs no run-as, so this survives the loss of the debug build —
# and a WebView that mounts, loads its assets and renders them is the strongest
# single signal that the plugin surface came through R8 intact.
assert_painted "$PAINT_TIMEOUT" "$launched_at" "$OUT/release-ui.xml" "$OUT/release.png"
paint_elapsed_ms=$(((SECONDS - launched_at) * 1000))

# The security half of the UI harness in e2e-android/. That harness attaches to
# the WebView over CDP, which is only possible because wry calls
# setWebContentsDebuggingEnabled under
# `#[cfg(any(debug_assertions, feature = "devtools"))]`. Nothing in this
# workspace enables that feature, so the call is not in this binary — asserted
# here rather than reasoned about, because it is what keeps a remote debugger
# off the build people install.
unix_sockets="$(sh_ cat /proc/net/unix)"
if [[ -n "$pid_now" && -n "$(devtools_sockets "$unix_sockets" "$pid_now")" ]]; then
    bad "the shipped build publishes no WebView devtools socket" \
        "$(devtools_sockets "$unix_sockets" "$pid_now") is open on pid $pid_now — anyone with adb can attach a debugger to the WebView"
else
    ok "the shipped build publishes no WebView devtools socket"
fi

# ---------------------------------------------------------------------------
group "3. Native code is mapped"
# ---------------------------------------------------------------------------
# Expected to be unreadable here: /proc/<pid>/maps of another uid's process
# needs ptrace access, which the shell user does not have without run-as. It is
# attempted rather than assumed so that a device where it *is* readable gives
# the same evidence the debug leg gets.
[[ -n "$pid_now" ]] && sh_ cat "/proc/$pid_now/maps" > "$OUT/release-maps.txt"
lines=0
[[ -s "$OUT/release-maps.txt" ]] && lines="$(wc -l < "$OUT/release-maps.txt")"
# The refusal itself, not a line count. `sh_` folds stderr into the file, so on
# a denial that file holds the reason and nothing else — which is the thing
# worth printing.
maps_err="$(head -c 120 "$OUT/release-maps.txt" 2>/dev/null | tr -d '\n')"

if (( lines >= 20 )); then
    own_map_paths "$OUT/release-maps.txt" "$PKG" | sed 's/^/        /'
    if [[ -n "$(own_code_maps "$OUT/release-maps.txt" "$PKG")" ]]; then
        ok "native code from this package is mapped executable"
    else
        bad "native code from this package is mapped executable" \
            "see the paths listed above"
    fi
else
    note "that libcopypaste_ui_lib is mapped, read from /proc" \
         "maps is not readable without run-as and this build is not debuggable — ${maps_err:-$lines lines}. What stands in its place: the process survived setup, which loads the library before super.onCreate, no UnsatisfiedLinkError appears above, and the WebView painted assets that only the Rust side serves"
fi

# ---------------------------------------------------------------------------
group "4. The doorways still accept a clip"
# ---------------------------------------------------------------------------
CANARY_SEND="CopyPasteReleaseCanary$(date +%s)$RANDOM"

adb logcat -c || true
send_out="$(sh_ am start -a android.intent.action.SEND -t text/plain --es android.intent.extra.TEXT "$CANARY_SEND" -n "$INTAKE")"
grep -q 'Error' <<<"$send_out" \
    && bad "the share-sheet doorway accepted an ACTION_SEND" "$send_out" \
    || ok "the share-sheet doorway accepted an ACTION_SEND"

proc_out="$(sh_ am start -a android.intent.action.PROCESS_TEXT -t text/plain --es android.intent.extra.PROCESS_TEXT "${CANARY_SEND}Proc" -n "$INTAKE")"
grep -q 'Error' <<<"$proc_out" \
    && bad "the text-selection doorway accepted an ACTION_PROCESS_TEXT" "$proc_out" \
    || ok "the text-selection doorway accepted an ACTION_PROCESS_TEXT"

sleep 10
dump_logcat release-capture

crashes2="$(crash_report "$OUT/release-capture.log")"
[[ -z "$crashes2" ]] \
    && ok "no crash block while capturing" \
    || bad "no crash block while capturing" "$(head -n 20 <<<"$crashes2")"

stripped2="$(r8_report "$OUT/release-capture.log")"
[[ -z "$stripped2" ]] \
    && ok "no stripped symbol while capturing" \
    || bad "no stripped symbol while capturing" "$(head -n 12 <<<"$stripped2")"

[[ -n "$(app_pid)" ]] \
    && ok "the app is still running after both doorways" \
    || bad "the app is still running after both doorways"

note "that the captured text reached SQLCipher on this build" \
     "the database is inside a non-debuggable package's private directory and cannot be read from here; the debug leg asserts the storage half, this leg asserts that the doorways and the process survive R8"

note "rung 2 on the shipped build" \
     "Shizuku cannot be granted on a stock emulator; the debug leg asserts the honest consequence and this leg does not repeat it"

dump_logcat release-final

if [[ $FAIL -eq 0 ]]; then
    paint_budget_ms=$(((SETTLE_SECS + PAINT_TIMEOUT) * 1000))
    python3 - "$OUT/latency.json" "$paint_elapsed_ms" "$paint_budget_ms" <<'PY'
import json, pathlib, sys
pathlib.Path(sys.argv[1]).write_text(json.dumps({
    "scenario": "release-webview-ready",
    "elapsed_ms": int(sys.argv[2]),
    "budget_ms": int(sys.argv[3]),
}) + "\n")
PY
    serial="$(adb get-serialno | tr -d '\r')"
    environment=physical-device
    [[ "$serial" == emulator-* ]] && environment=emulator
    if python3 scripts/release/write-native-evidence.py \
        --output "$OUT/native-evidence.json" \
        --platform android \
        --environment "$environment" \
        --os-version "API $sdk" \
        --architecture "$abi" \
        --commit "${GITHUB_SHA:-$(git rev-parse HEAD)}" \
        --run-id "${GITHUB_RUN_ID:-local-$(git rev-parse --short HEAD)}" \
        --scenario release-webview-ready \
        --elapsed-ms "$paint_elapsed_ms" \
        --budget-ms "$paint_budget_ms" \
        --assertion "signed release app launched" \
        --assertion "WebView accessibility content painted" \
        --assertion "release smoke assertions passed" \
        --artifact screenshot=release.png \
        --artifact accessibility=release-ui.xml \
        --artifact measurement=latency.json \
        --artifact diagnostic-log=release-final.log; then
        ok "native evidence receipt was written"
    else
        bad "native evidence receipt was written"
    fi
fi
summary
[[ $FAIL -eq 0 ]]
