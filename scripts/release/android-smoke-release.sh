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
# shellcheck source=scripts/release/android-ui-evidence-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-ui-evidence-lib.sh"
# shellcheck source=scripts/release/android-navigation-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-navigation-lib.sh"

APK="${APK:-}"
MAIN="$PKG/$APP_NAMESPACE.MainActivity"
INTAKE="$PKG/$APP_NAMESPACE.IntakeActivity"
SETTLE_SECS="${SETTLE_SECS:-25}"
PAINT_TIMEOUT="${PAINT_TIMEOUT:-90}"

history_capture_holds() { # <accessibility artifact> <canary>
    enabled_node_exists_exact "$1" "Search clipboard history, default|Search clipboard history, active" \
        && node_exists_exact "$1" "$2"
}

history_capture_current_holds() { # <accessibility artifact>
    history_capture_holds "$1" "$CANARY_SEND"
}

pairing_dialog_closed_holds() { # <accessibility artifact>
    ! node_exists_exact "$1" "copypaste-pairing-dialog-open" \
        && app_navigation_holds "$1"
}

release_history_self_test() { # <temporary directory>
    local temp="$1" canary="CopyPasteReleaseCanaryFixture"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node content-desc=\"Search clipboard history, default\" enabled=\"true\"/><node text=\"$canary\"/></hierarchy>" \
        > "$temp/release-history-good.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node content-desc="Search clipboard history, default" enabled="true"/></hierarchy>' \
        > "$temp/release-history-missing-canary.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node text=\"$canary\"/></hierarchy>" \
        > "$temp/release-history-no-toolbar.xml"

    history_capture_holds "$temp/release-history-good.xml" "$canary" \
        && ok "a release history receipt requires the canary and active history UI" \
        || bad "a release history receipt requires the canary and active history UI"
    history_capture_holds "$temp/release-history-missing-canary.xml" "$canary" \
        && bad "a release history receipt rejects an empty history" \
        || ok "a release history receipt rejects an empty history"
    history_capture_holds "$temp/release-history-no-toolbar.xml" "$canary" \
        && bad "a release history receipt rejects a canary outside Library" \
        || ok "a release history receipt rejects a canary outside Library"
}

release_history_navigation_self_test() { # <temporary directory>
    local temp="$1" canary="CopyPasteReleaseCanaryFixture"
    local tab_bar modal shell modal_over_shell
    tab_bar='<node text="Primary"><node text="Library" bounds="[20,40][100,90]" enabled="true" clickable="true"/><node text="Devices" bounds="[110,40][190,90]" enabled="true" clickable="true"/><node text="Settings" bounds="[200,40][300,90]" enabled="true" clickable="true"/></node>'
    modal='<node resource-id="copypaste-pairing-dialog-open"><node text="Close" bounds="[220,460][300,510]" enabled="true" clickable="true"/></node>'
    shell="<node>$tab_bar</node>"
    modal_over_shell="<node>$tab_bar$modal</node>"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$modal</node></hierarchy>" \
        > "$temp/release-pairing-modal.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$modal_over_shell</hierarchy>" \
        > "$temp/release-pairing-modal-over-shell.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$shell</hierarchy>" \
        > "$temp/release-pairing-closed.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$shell<node content-desc=\"Search clipboard history, default\" enabled=\"true\"/></hierarchy>" \
        > "$temp/release-history-pending.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$shell<node text=\"$canary\"/></hierarchy>" \
        > "$temp/release-history-canary-outside-library.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node content-desc=\"Search clipboard history, active\" enabled=\"true\"/><node text=\"$canary\"/></hierarchy>" \
        > "$temp/release-history-captured.xml"

    if (
        CANARY_SEND="$canary"
        ui_fixtures "$temp/release-pairing-modal-over-shell.xml" \
            "$temp/release-pairing-closed.xml" \
            "$temp/release-history-pending.xml" \
            "$temp/release-history-canary-outside-library.xml" \
            "$temp/release-history-captured.xml"
        tap_until_state "Close" "$temp/release-pairing-closed-observed.xml" \
            pairing_dialog_closed_holds none 3 ui_fixture_dump \
            navigation_fixture_scroll navigation_fixture_tap ui_fixture_pace \
            && cmp -s "$temp/release-pairing-closed-observed.xml" "$temp/release-pairing-closed.xml" \
            && tap_until_state "Library" "$temp/release-history-observed.xml" \
            history_capture_current_holds none 3 ui_fixture_dump \
            navigation_fixture_scroll navigation_fixture_tap ui_fixture_pace \
            && [[ $UI_FIXTURE_INDEX -eq 5 && $UI_FIXTURE_TAPS -eq 3 && $UI_FIXTURE_PACES -eq 3 ]] \
            && cmp -s "$temp/release-history-observed.xml" "$temp/release-history-captured.xml"
    ); then
        ok "release pairing teardown reaches Library before checking its canary"
    else
        bad "release pairing teardown reaches Library before checking its canary"
    fi
    if (
        ui_fixtures "$temp/release-pairing-closed.xml"
        tap_until_state "Close" "$temp/release-pairing-already-closed.xml" \
            pairing_dialog_closed_holds none 3 ui_fixture_dump \
            navigation_fixture_scroll navigation_fixture_tap ui_fixture_pace \
            && [[ $UI_FIXTURE_INDEX -eq 1 && $UI_FIXTURE_TAPS -eq 0 ]] \
            && cmp -s "$temp/release-pairing-already-closed.xml" "$temp/release-pairing-closed.xml"
    ); then
        ok "release pairing teardown does not tap after the shell is navigable"
    else
        bad "release pairing teardown does not tap after the shell is navigable"
    fi
    if (
        ui_fixtures "$temp/release-pairing-modal.xml"
        ! tap_until_state "Close" "$temp/release-pairing-never-closed.xml" \
            pairing_dialog_closed_holds none 1 ui_fixture_dump \
            navigation_fixture_scroll navigation_fixture_tap ui_fixture_pace \
            && [[ $UI_FIXTURE_TAPS -eq 1 ]] \
            && cmp -s "$temp/release-pairing-never-closed.xml" "$temp/release-pairing-modal.xml"
    ); then
        ok "release pairing teardown rejects a modal that never closes"
    else
        bad "release pairing teardown rejects a modal that never closes"
    fi
}

release_history_receipt_self_test() {
    local source artifact="history-ui" feature_state="--feature-state"
    source="$(<"${BASH_SOURCE[0]}")"
    [[ "$source" == *"--artifact screenshot=$artifact.png"* \
        && "$source" == *"--artifact accessibility=$artifact.xml"* \
        && "$source" != *"$feature_state history=$artifact"* ]] \
        && ok "pending history artifacts are supplemental, not feature-state evidence" \
        || bad "pending history artifacts are supplemental, not feature-state evidence"
}

if [[ "${1:-}" == "--self-test" ]]; then
    self_test || exit $?
    android_navigation_self_test "$SELF_TEST_TMP"
    release_history_self_test "$SELF_TEST_TMP"
    release_history_navigation_self_test "$SELF_TEST_TMP"
    release_history_receipt_self_test
    [[ $FAIL -eq 0 ]]
    exit $?
fi

mkdir -p "$OUT"

group "Preflight"
command -v adb >/dev/null 2>&1 || { echo "  FATAL adb is not on PATH"; exit 1; }
adb wait-for-device
sdk="$(sh_ getprop ro.build.version.sdk)"
abi="$(sh_ getprop ro.product.cpu.abi)"
printf '  device: API %s, %s\n' "$sdk" "$abi"

[[ -f "$APK" ]] || { echo "  FATAL APK not found at '${APK:-<unset>}'"; exit 1; }
qualified_artifact_identity="$(python3 scripts/release/write-native-evidence.py --capture-qualified-artifact "$APK")" || {
    echo "  FATAL release artifact identity could not be captured"
    exit 1
}
printf '  apk: %s (%s bytes)\n' "$APK" "$(android_file_size "$APK")"

apk_libs="$(unzip -l "$APK")"
if grep -q "lib/${abi}/libcopypaste_ui_lib.so" <<<"$apk_libs"; then
    ok "the APK carries libcopypaste_ui_lib.so for $abi"
else
    bad "the APK carries libcopypaste_ui_lib.so for $abi" \
        "$(grep -E 'lib/.*\.so' <<<"$apk_libs" || echo 'no native libraries in the APK at all')"
    summary; exit 1
fi

group "Install"
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

group "1. The shipped build launches"
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

crashes="$(crash_report "$OUT/release-launch.log" "$pid1")"
if [[ -z "$crashes" ]]; then
    ok "no crash block naming this app in logcat"
else
    bad "no crash block naming this app in logcat" "$(head -n 20 <<<"$crashes")"
fi

# Named separately from the crash check, because the reader needs the
# hypothesis handed to them: on this build type, these three are R8.
stripped="$(r8_report "$OUT/release-launch.log" "$pid1")"
if [[ -z "$stripped" ]]; then
    ok "no ClassNotFoundException, NoSuchMethodException or UnsatisfiedLinkError"
else
    bad "no ClassNotFoundException, NoSuchMethodException or UnsatisfiedLinkError" \
        "R8 stripped or renamed something the app looks up by name — CapturePlugin is resolved by reflection and proguard-rules.pro does not keep it: $(head -n 12 <<<"$stripped")"
fi

group "2. The UI paints"
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

if native_ax="$(PKG="$PKG" SMOKE_OUT="$OUT" NATIVE_AX_TREE="$OUT/release-ui.xml" \
    "$(dirname "${BASH_SOURCE[0]}")/android-native-accessibility.sh" 2>&1)"; then
    ok "$native_ax"
else
    bad "the native Android accessibility surface is observable and usable" "$native_ax"
fi

group "2a. Android pairing entry is a scanner"
if reach_settings_tab "$OUT/pairing-shell.xml" 30 \
    && tap_selector "Devices" "$OUT/pairing-devices-action.xml" 15 \
    && wait_selector "Connect a device" "$OUT/pairing-devices.xml" 15 \
    && tap_selector "Connect a device" "$OUT/pairing-launcher-action.xml" 15 \
    && wait_selector "Scan pairing code" "$OUT/pairing-entry.xml" 15; then
    if [[ -n "$(node_center "$OUT/pairing-entry.xml" "Show pairing code")" ]] \
        && [[ -n "$(node_center "$OUT/pairing-entry.xml" "Scan pairing code")" ]] \
        && [[ -z "$(node_center "$OUT/pairing-entry.xml" "Enter pairing code")" ]]; then
        ok "Android offers Show pairing code and Scan pairing code"
    else
        bad "Android offers Show pairing code and Scan pairing code" \
            "the launcher did not expose the platform-specific pairing actions"
    fi
    if pairing_ax="$(PKG="$PKG" SMOKE_OUT="$OUT" NATIVE_AX_TREE="$OUT/pairing-entry.xml" \
        "$(dirname "${BASH_SOURCE[0]}")/android-native-accessibility.sh" 2>&1)"; then
        ok "the Android pairing entry is covered by the native accessibility surface: $pairing_ax"
    else
        bad "the Android pairing entry is covered by the native accessibility surface" "$pairing_ax"
    fi
    capture_png "$OUT/pairing-entry.png" \
        && ok "the Android pairing launcher screenshot is complete" \
        || bad "the Android pairing launcher screenshot is complete"
    if tap_until_state "Close" "$OUT/pairing-closed.xml" \
        pairing_dialog_closed_holds none 15; then
        ok "the Android pairing launcher closes to the navigable shell"
    else
        bad "the Android pairing launcher closes to the navigable shell" \
            "Close did not restore an actionable Library, Devices, and Settings tab bar"
    fi
else
    bad "the Android pairing scanner entry is reachable" \
        "uiautomator could not open Devices > Connect a device > Scan pairing code"
fi

# The security half of the UI harness in e2e-android/: that harness attaches to
# the WebView over CDP and reads the DOM, which on this product is clipboard
# content. Two assertions follow, and this is the one that travels to a user's
# device, because it is a property of the artefact.
#
# wry emits the only call that can switch the debugger on under
# `#[cfg(any(debug_assertions, feature = "devtools"))]`, and this workspace
# enables neither. R8 cannot remove a Rust JNI call, so the native library
# inside the APK is where it would still show.
read -r dt_call dt_neighbours <<<"$(apk_wry_jni_counts "$APK")"
if (( dt_neighbours == 0 )); then
    bad "the shipped APK carries no setWebContentsDebuggingEnabled call" \
        "neither unconditional wry JNI name was found in $APK either, so this scan read nothing and proved nothing"
elif (( dt_call == 0 )); then
    ok "the shipped APK carries no setWebContentsDebuggingEnabled call"
else
    bad "the shipped APK carries no setWebContentsDebuggingEnabled call" \
        "$dt_call reference(s) in the APK: this build can switch the WebView debugger on, and anything that reaches it reads clipboard content out of the DOM"
fi

# The device half cannot mean what it looks like on its own. Both workflows run
# `google_apis` images, which are built `userdebug` and so set
# `ro.debuggable=1`, and the WebView provider then enables remote debugging for
# every process on the device. Run 31229976299 failed here on the emulator's
# endpoint, not the build's. So it is excused only against a live control, and
# never in place of the assertion above — which is what still catches a build
# that had switched its own debugger on.
#
# The pid is resolved now and not reused from the launch group: the app may
# have been restarted by anything in between, and a stale pid passes on nothing.
pid_cdp="$(app_pid)"
cdp_pkg="$(devtools_cdp_package "${pid_cdp:-0}")"
printf '        device: ro.debuggable=%s ro.build.type=%s\n' \
    "$(sh_ getprop ro.debuggable)" "$(sh_ getprop ro.build.type)"

if [[ -z "$cdp_pkg" ]]; then
    ok "nothing answers the WebView devtools protocol for pid ${pid_cdp:-<gone>}"
elif [[ "$cdp_pkg" != "$PKG" ]]; then
    bad "nothing answers the WebView devtools protocol for pid ${pid_cdp:-<gone>}" \
        "an endpoint named for our pid answered as '$cdp_pkg'"
elif control="$(control_cdp_package)" && [[ -n "$control" ]]; then
    ok "the devtools endpoint on pid $pid_cdp is this device's, not this build's"
    printf '        control: %s serves CDP too, and is not debuggable\n' "$control"
else
    bad "the shipped build serves no WebView devtools protocol" \
        "CDP answered as $PKG on pid $pid_cdp, and no app we did not build serves one on this device — so this endpoint is the build's own, and anything that can reach adb can read clipboard content out of the DOM"
fi

group "3. Native code is mapped"
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

group "4. The doorways still accept a clip"
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

crashes2="$(crash_report "$OUT/release-capture.log" "$pid1")"
[[ -z "$crashes2" ]] \
    && ok "no crash block while capturing" \
    || bad "no crash block while capturing" "$(head -n 20 <<<"$crashes2")"

stripped2="$(r8_report "$OUT/release-capture.log" "$pid1")"
[[ -z "$stripped2" ]] \
    && ok "no stripped symbol while capturing" \
    || bad "no stripped symbol while capturing" "$(head -n 12 <<<"$stripped2")"

# GMS FontsProvider or another provider host can die while the doorways run,
# and the platform kills every client of the dying provider — including this
# app. Relaunch and retry once; a second foreign kill is the emulator image's
# own problem and stays the caller's to note.
release_doorway_foreign_kill="$(foreign_dependency_kill "$OUT/release-capture.log" "$pid1" 2>/dev/null || true)"
if [[ -z "$(app_pid)" && -n "$release_doorway_foreign_kill" ]]; then
    probe "the platform killed the release app during doorways" "$release_doorway_foreign_kill"
    sh_ am force-stop "$PKG" 2>/dev/null || true
    sleep 2
    adb logcat -c || true
    sh_ am start -W -n "$MAIN" >/dev/null || true
    wait_for 30 has_pid || true
    pid1="$(app_pid)"
    sleep "$SETTLE_SECS"
    dump_logcat release-capture-retry
    sh_ am start -a android.intent.action.SEND -t text/plain --es android.intent.extra.TEXT "$CANARY_SEND" -n "$INTAKE"
    sh_ am start -a android.intent.action.PROCESS_TEXT -t text/plain --es android.intent.extra.PROCESS_TEXT "${CANARY_SEND}Proc" -n "$INTAKE"
    sleep 10
    dump_logcat release-capture-retry-done
fi

[[ -n "$(app_pid)" ]] \
    && ok "the app is still running after both doorways" \
    || bad "the app is still running after both doorways"

group "4a. The captured text appears in history"
if tap_until_state "Library" "$OUT/history-ui.xml" \
    history_capture_current_holds none; then
    capture_png "$OUT/history-ui.png" \
        && ok "the captured history screenshot is complete" \
        || bad "the captured history screenshot is complete"
else
    bad "the captured text is visible in the active history UI" \
        "Library did not expose the release canary with its history search control"
fi

note "that the captured text reached SQLCipher on this build" \
     "the database is inside a non-debuggable package's private directory and cannot be read from here; the debug leg asserts the storage half, this leg asserts that the doorways and the process survive R8"

note "rung 2 on the shipped build" \
     "Shizuku cannot be granted on a stock emulator; the debug leg asserts the honest consequence and this leg does not repeat it"

dump_logcat release-final

if [[ $FAIL -eq 0 ]]; then
    configured_budget_ms=$(((SETTLE_SECS + PAINT_TIMEOUT) * 1000))
    paint_budget_ms="$(python3 scripts/release/native_evidence_policy.py value --platform android --field budget_ms)"
    scenario="$(python3 scripts/release/native_evidence_policy.py value --platform android --field scenario)"
    if [[ "$configured_budget_ms" != "$paint_budget_ms" ]]; then
        bad "release smoke timeout matches native evidence policy" \
            "configured ${configured_budget_ms}ms, policy ${paint_budget_ms}ms"
    fi
fi

if [[ $FAIL -eq 0 ]]; then
    python3 - "$OUT/latency.json" "$scenario" "$paint_elapsed_ms" "$paint_budget_ms" <<'PY'
import json, pathlib, sys
pathlib.Path(sys.argv[1]).write_text(json.dumps({
    "scenario": sys.argv[2],
    "elapsed_ms": int(sys.argv[3]),
    "budget_ms": int(sys.argv[4]),
}) + "\n")
PY
    receipt_route_log="$OUT/release-receipt-route.log"
    serial_candidate="$(android_serial_candidate)" || serial_candidate=""
    environment="$(python3 scripts/release/native_evidence_policy.py value --platform android --field environment)"
    if ! serial="$(verified_android_serial "$serial_candidate" "$receipt_route_log")"; then
        bad "native evidence receipt was written" \
            "$(tail -n 4 "$receipt_route_log" | tr '\n' ' ')"
    elif [[ "$serial" != emulator-* || "$(sh_ getprop ro.kernel.qemu)" != "1" ]]; then
        bad "native evidence receipt was written" \
            "the Android release policy requires an emulator receipt; serial=$serial qemu=$(sh_ getprop ro.kernel.qemu)"
    elif python3 scripts/release/write-native-evidence.py \
        --output "$OUT/native-evidence.json" \
        --platform android \
        --environment "$environment" \
        --os-version "API $sdk" \
        --architecture "$abi" \
        --commit "${GITHUB_SHA:-$(git rev-parse HEAD)}" \
        --run-id "${GITHUB_RUN_ID:-local-$(git rev-parse --short HEAD)}" \
        --elapsed-ms "$paint_elapsed_ms" \
        --qualified-artifact "$APK" \
        --qualified-artifact-identity "$qualified_artifact_identity" \
        --feature-state devices=scan-pairing-code,screenshot=pairing-entry.png,accessibility=pairing-entry.xml \
        --artifact screenshot=pairing-entry.png \
        --artifact accessibility=pairing-entry.xml \
        --artifact screenshot=history-ui.png \
        --artifact accessibility=history-ui.xml \
        --artifact measurement=latency.json \
        --artifact diagnostic-log=release-final.log; then
        ok "native evidence receipt was written"
    else
        bad "native evidence receipt was written"
    fi
fi
summary
[[ $FAIL -eq 0 ]]
