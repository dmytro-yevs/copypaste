#!/usr/bin/env bash
# android-smoke.sh — what one booted emulator can prove about the APK.
#
# Runs inside reactivecircus/android-emulator-runner with a single device
# attached and $APK pointing at a *debuggable* APK: every filesystem assertion
# below goes through `run-as`, which the platform allows only for those.
#
# The verdict vocabulary, the detectors and their fixtures are in
# android-smoke-lib.sh; `--self-test` runs those and needs no device.
set -uo pipefail

# shellcheck source=scripts/release/android-smoke-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-smoke-lib.sh"

APK="${APK:-}"
MAIN="$PKG/$APP_NAMESPACE.MainActivity"
INTAKE="$PKG/$APP_NAMESPACE.IntakeActivity"

# How long a debug build may take to reach a steady state on an emulator.
# Generous on purpose: the Rust half is unoptimised here, and a flaky timeout
# reads as a product failure.
SETTLE_SECS="${SETTLE_SECS:-25}"
CAPTURE_WAIT_SECS="${CAPTURE_WAIT_SECS:-40}"

# How long to keep asking the accessibility tree whether the UI is there. It
# starts after the settle, so the wall-clock budget from `am start` is this plus
# SETTLE_SECS. Deliberately generous for a first measurement — the probe reports
# what it actually took, and the margin can be cut once there are numbers.
PAINT_TIMEOUT="${PAINT_TIMEOUT:-90}"

if [[ "${1:-}" == "--self-test" ]]; then
    self_test
    exit $?
fi

mkdir -p "$OUT"

group "Preflight"
command -v adb >/dev/null 2>&1 || { echo "  FATAL adb is not on PATH"; exit 1; }
adb wait-for-device
devices="$(adb devices | grep -cE '\sdevice$')"
[[ "$devices" == "1" ]] || { echo "  FATAL expected one device, adb reports $devices"; adb devices; exit 1; }

sdk="$(sh_ getprop ro.build.version.sdk)"
abi="$(sh_ getprop ro.product.cpu.abi)"
printf '  device: API %s, %s, %s\n' "$sdk" "$abi" "$(sh_ getprop ro.build.fingerprint)"

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

group "Install"
adb uninstall "$PKG" >/dev/null 2>&1 || true
# -g grants the runtime permissions the app declares, so POST_NOTIFICATIONS
# does not put a system dialog in front of the WebView we are about to inspect.
if install_out="$(adb install -r -g "$APK" 2>&1)"; then
    ok "adb install"
else
    bad "adb install" "$install_out"
    summary; exit 1
fi

runas_id="$(sh_ run-as "$PKG" id)"
if grep -q uid <<<"$runas_id"; then
    ok "run-as reaches the app's private directory"
else
    bad "run-as reaches the app's private directory" \
        "the APK is not debuggable; every filesystem assertion below needs it: ${runas_id}"
    summary; exit 1
fi

group "1. It launches at all"
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
    dump_logcat launch1
    summary; exit 1
fi

sleep "$SETTLE_SECS"
pid_now="$(app_pid)"
dump_logcat launch1

# The whole of `setup` runs in this window: the keystore, SQLCipher, the
# backend. Every one of them is fatal to the process, so survival is the
# assertion and the log is the diagnosis.
if [[ "$pid_now" == "$pid1" ]]; then
    ok "the process survived ${SETTLE_SECS}s (setup did not abort)"
else
    bad "the process survived ${SETTLE_SECS}s (setup did not abort)" \
        "pid was $pid1, is now '${pid_now:-gone}' — $(process_gone_cause "$OUT/launch1.log" "$pid1")"
fi

crashes="$(crash_report "$OUT/launch1.log" "$pid1")"
if [[ -z "$crashes" ]]; then
    ok "no crash block naming this app in logcat"
else
    bad "no crash block naming this app in logcat" "$(head -n 20 <<<"$crashes")"
fi

keystore1="$(keystore_report "$OUT/launch1.log")"
if [[ -z "$keystore1" ]]; then
    ok "no keystore or history failure in logcat"
else
    bad "no keystore or history failure in logcat" "$keystore1"
fi

app_processes > "$OUT/processes.txt" 2>/dev/null || true
probe "processes named after the package" "$(tr '\n' '|' < "$OUT/processes.txt")"

[[ -n "$pid_now" ]] && sh_ run-as "$PKG" cat "/proc/$pid_now/maps" > "$OUT/maps.txt"
lines=0
[[ -s "$OUT/maps.txt" ]] && lines="$(wc -l < "$OUT/maps.txt")"

# Fewer than this is not a process's address space. Reading it through run-as
# could fail partially rather than outright, and a truncated read that FAILs
# would read as a missing library.
if [[ "$lines" -lt 20 ]]; then
    note "which code is mapped into the process" \
         "/proc/$pid_now/maps came back with $lines lines through run-as: $(head -c 200 "$OUT/maps.txt" 2>/dev/null)"
else
    probe "maps rows, and those naming us" \
          "$lines total, $(own_map_paths "$OUT/maps.txt" "$PKG" | wc -l) distinct paths"
    own_map_paths "$OUT/maps.txt" "$PKG" | sed 's/^/        /'

    native="$(own_code_maps "$OUT/maps.txt" "$PKG")"
    if [[ -n "$native" ]]; then
        ok "native code from this package is mapped executable"
        head -n 3 <<<"$native" | sed 's/^/        /'
    else
        # The listing above is the whole diagnosis: it names every mapping we
        # own, so a third spelling identifies itself instead of failing blind.
        bad "native code from this package is mapped executable" \
            "System.loadLibrary resolved to nothing that stayed loaded — see the paths listed above"
    fi

    grep -qi 'webview\|chromium\|trichrome' "$OUT/maps.txt" \
        && ok "a WebView implementation is mapped into the process" \
        || bad "a WebView implementation is mapped into the process" \
               "the activity is up but no WebView was ever instantiated"
fi

wake_screen

window="$(sh_ dumpsys window)"
focus="$(grep -E 'mCurrentFocus|mFocusedApp' <<<"$window" | head -n 4)"
if grep -q "$PKG" <<<"$focus"; then
    ok "the focused window belongs to $PKG"
else
    bad "the focused window belongs to $PKG" "${focus:-dumpsys window reported no focus}"
fi

assert_painted "$PAINT_TIMEOUT" "$launched_at" "$OUT/ui-launch1.xml" "$OUT/launch1.png"

# The precondition for e2e-android/, which drives this WebView over CDP. It is
# asserted here so that a build whose devtools were compiled out fails saying
# so, instead of the harness timing out on a connection and reading as a UI
# that never came up.
#
# It is that and nothing more. On the `userdebug` emulator images both
# workflows use, the WebView provider publishes this socket for every process,
# so its presence here is not evidence that wry enabled it. The security
# property lives in android-smoke-release.sh, and it is asserted against the
# APK rather than against a socket.
unix_sockets="$(sh_ cat /proc/net/unix)"
webview_socket="$(devtools_sockets "$unix_sockets" "$pid_now")"
if [[ -n "$webview_socket" ]]; then
    ok "the WebView published a devtools socket ($webview_socket)"
else
    bad "the WebView published a devtools socket" \
        "nothing matching @webview_devtools_remote_$pid_now is open; the UI harness in e2e-android/ cannot attach to this build"
fi

group "4. The database opens"
app_files > "$OUT/files-launch1.txt"
DB_REL="$(grep -E 'copypaste-v2\.db$' "$OUT/files-launch1.txt" | head -n 1)"
if [[ -n "$DB_REL" ]]; then
    ok "copypaste-v2.db exists in the app's private directory"
    printf '        %s\n' "$DB_REL"
else
    bad "copypaste-v2.db exists in the app's private directory" \
        "files present: $(tr '\n' ' ' < "$OUT/files-launch1.txt" | head -c 400)"
fi

if [[ -n "$DB_REL" ]]; then
    db_fingerprint launch1 | sed 's/^/        /'
    if [[ -s "$OUT/launch1-copypaste-v2.db" ]]; then
        looks_encrypted "$OUT/launch1-copypaste-v2.db" \
            && ok "the database is not a readable SQLite file (SQLCipher is on)" \
            || bad "the database is not a readable SQLite file (SQLCipher is on)" \
                   "it starts with the plain SQLite magic — clipboard plaintext at rest"
        salt1="$(salt_of "$OUT/launch1-copypaste-v2.db")"
        printf '        page-1 salt: %s\n' "$salt1"
    else
        bad "the database could be read back through run-as" "cat produced nothing"
        salt1=""
    fi
else
    salt1=""
fi

group "2. The keystore round-trips (second launch)"
prefs="$(grep -E 'shared_prefs/' "$OUT/files-launch1.txt" | tr '\n' ' ')"
prefs_hash_1=""
if [[ -n "$prefs" ]]; then
    probe "shared_prefs after the first launch" "$prefs"
    for f in $prefs; do
        pull_file "$f" "$OUT/prefs1-$(basename "$f")" \
            && prefs_hash_1+="$(sha256sum < "$OUT/prefs1-$(basename "$f")" | cut -d' ' -f1) "
    done
else
    note "that the wrapped secret is a SharedPreferences blob" \
         "no shared_prefs file was written; ADR-0003 describes one, so either the store keeps it elsewhere or nothing was minted"
fi

sh_ am force-stop "$PKG"
if wait_for 20 no_pid; then
    ok "force-stop ended the first process"
else
    bad "force-stop ended the first process" "pid $(app_pid) is still running"
fi

second_launch() {
    adb logcat -c || true
    start2="$(sh_ am start -W -n "$MAIN")"
    wait_for 60 has_pid || true
    pid2="$(app_pid)"
    sleep "$SETTLE_SECS"
    dump_logcat launch2
}

# Only a process that is gone *and* was taken away by something outside this
# package. Anything else stays the caller's problem to assert on.
second_launch_foreign_kill() {
    [[ -z "$(app_pid)" ]] || return 0
    foreign_dependency_kill "$OUT/launch2.log" "$pid2" || true
}

second_launch
foreign_kill="$(second_launch_foreign_kill)"
if [[ -n "$foreign_kill" ]]; then
    # GMS restarts on its own on a google_apis emulator and the platform kills
    # every client of its providers; the retry is what keeps the round-trip an
    # assertion instead of an excuse.
    probe "the platform killed the second launch once" "$foreign_kill"
    second_launch
    foreign_kill="$(second_launch_foreign_kill)"
fi

if [[ -n "$pid2" && "$pid2" != "$pid1" ]]; then
    ok "the second launch is a new process (pid $pid2)"
else
    bad "the second launch is a new process" "pid1=$pid1 pid2=${pid2:-gone}; $start2"
fi

survived=no
if [[ -n "$(app_pid)" ]]; then
    survived=yes
    ok "the second process survived ${SETTLE_SECS}s"
elif [[ -n "$foreign_kill" ]]; then
    note "that the second process survived ${SETTLE_SECS}s" \
         "the platform killed it twice, both times because a process outside this package died: $foreign_kill. ApplicationExitInfo records that as REASON_DEPENDENCY_DIED, so it is the emulator image and not the artefact under test"
else
    bad "the second process survived ${SETTLE_SECS}s" \
        "$(process_gone_cause "$OUT/launch2.log" "$pid2")"
fi

crashes2="$(crash_report "$OUT/launch2.log" "$pid2")"
[[ -z "$crashes2" ]] \
    && ok "no crash block on the second launch" \
    || bad "no crash block on the second launch" "$(head -n 20 <<<"$crashes2")"

keystore2="$(keystore_report "$OUT/launch2.log")"
[[ -z "$keystore2" ]] \
    && ok "no keystore or history failure on the second launch" \
    || bad "no keystore or history failure on the second launch" "$keystore2"

# The proof ADR-0003 asks for. A second launch that minted a fresh secret would
# either fail to open this file or replace it; an unchanged page-1 salt says
# the same database was reopened with a key that still works.
if [[ -z "$DB_REL" || -z "$salt1" ]]; then
    note "the keystore round-trip" "there was no first-launch database to compare against"
elif [[ "$survived" == no ]]; then
    # An unchanged salt on a launch that was killed before it opened anything
    # is a file nobody touched, not a key that still works.
    db_fingerprint launch2 | sed 's/^/        /'
    note "the keystore round-trip" \
         "the second process did not live long enough to reopen the database"
else
    db_fingerprint launch2 | sed 's/^/        /'
    salt2="$(salt_of "$OUT/launch2-copypaste-v2.db" 2>/dev/null || true)"
    if [[ -n "$salt2" && "$salt2" == "$salt1" ]]; then
        ok "the second launch reopened the same database (salt unchanged)"
    else
        bad "the second launch reopened the same database (salt unchanged)" \
            "salt was $salt1, is now ${salt2:-<database gone>} — the device secret did not survive"
    fi
fi

if [[ -n "$prefs_hash_1" ]]; then
    prefs_hash_2=""
    for f in $prefs; do
        pull_file "$f" "$OUT/prefs2-$(basename "$f")" \
            && prefs_hash_2+="$(sha256sum < "$OUT/prefs2-$(basename "$f")" | cut -d' ' -f1) "
    done
    [[ "$prefs_hash_1" == "$prefs_hash_2" ]] \
        && ok "the wrapped secret was read, not rewritten" \
        || bad "the wrapped secret was read, not rewritten" \
               "the shared_prefs blob changed between launches, which is what minting a second secret looks like"
fi

group "3. A doorway captures"
CANARY_SEND="CopyPasteCanarySend$(date +%s)$RANDOM"
CANARY_PROC="CopyPasteCanaryProc$(date +%s)$RANDOM"

before="$(db_fingerprint before-capture)"
sleep 6
quiet="$(db_fingerprint quiescence)"
[[ "$before" == "$quiet" ]] \
    && probe "the store is quiescent before the capture" "yes — a change after this is the capture" \
    || probe "the store is quiescent before the capture" "no, it changes on its own; the assertion below is necessary but not sufficient"

doorway_capture() {
    adb logcat -c || true
    sh_ am start -a android.intent.action.SEND -t text/plain --es android.intent.extra.TEXT "$CANARY_SEND" -n "$INTAKE"
    sh_ am start -a android.intent.action.PROCESS_TEXT -t text/plain --es android.intent.extra.PROCESS_TEXT "$CANARY_PROC" -n "$INTAKE"
}

send_out="$(sh_ am start -a android.intent.action.SEND -t text/plain --es android.intent.extra.TEXT "$CANARY_SEND" -n "$INTAKE")"
grep -q 'Error' <<<"$send_out" \
    && bad "the share-sheet doorway accepted an ACTION_SEND" "$send_out" \
    || ok "the share-sheet doorway accepted an ACTION_SEND"

proc_out="$(sh_ am start -a android.intent.action.PROCESS_TEXT -t text/plain --es android.intent.extra.PROCESS_TEXT "$CANARY_PROC" -n "$INTAKE")"
grep -q 'Error' <<<"$proc_out" \
    && bad "the text-selection doorway accepted an ACTION_PROCESS_TEXT" "$proc_out" \
    || ok "the text-selection doorway accepted an ACTION_PROCESS_TEXT"

changed=0
for _ in $(seq 1 $((CAPTURE_WAIT_SECS / 5))); do
    sleep 5
    if [[ "$(db_fingerprint after-capture)" != "$before" ]]; then changed=1; break; fi
done
dump_logcat capture

crashes3="$(crash_report "$OUT/capture.log" "$pid2")"
[[ -z "$crashes3" ]] \
    && ok "no crash block while capturing" \
    || bad "no crash block while capturing" "$(head -n 20 <<<"$crashes3")"

# GMS FontsProvider or another provider host can die while the doorways run,
# and the platform kills every client of the dying provider — including this
# app. Relaunch and retry once; a second foreign kill is the emulator image's
# own problem and stays the caller's to note.
doorway_foreign_kill="$(foreign_dependency_kill "$OUT/capture.log" "$pid2" || true)"
if [[ -z "$(app_pid)" && -n "$doorway_foreign_kill" ]]; then
    probe "the platform killed the app during doorways" "$doorway_foreign_kill"
    sh_ am force-stop "$PKG" 2>/dev/null || true
    sleep 2
    adb logcat -c || true
    sh_ am start -W -n "$MAIN" >/dev/null || true
    wait_for 30 has_pid || true
    pid2="$(app_pid)"
    sleep "$SETTLE_SECS"
    dump_logcat capture-retry
    doorway_capture
    sleep 10
    dump_logcat capture-retry-done
fi

[[ -n "$(app_pid)" ]] \
    && ok "the app is still running after both doorways" \
    || bad "the app is still running after both doorways"

if [[ $changed -eq 1 ]]; then
    ok "the history store changed within ${CAPTURE_WAIT_SECS}s of the capture intents"
else
    bad "the history store changed within ${CAPTURE_WAIT_SECS}s of the capture intents" \
        "Kotlin took the clip and nothing reached SQLCipher: ClipQueue, the intake drain, or ingest"
fi

# Only meaningful if something was stored. Asserting "the plaintext is not in
# the database" over a database nothing was written to is the shape of a test
# that passes without proving anything.
if [[ $changed -eq 1 ]]; then
    plain=""
    for f in "$OUT"/after-capture-*; do
        [[ -f "$f" ]] || continue
        holds_text "$f" "$CANARY_SEND" && plain+="$(basename "$f") "
    done
    [[ -z "$plain" ]] \
        && ok "the captured text is not readable in the database files" \
        || bad "the captured text is not readable in the database files" \
               "found in: $plain — the store is holding clipboard plaintext"
else
    note "that the stored text is unreadable at rest" \
         "nothing reached the database, so there was nothing to look for"
fi

leaks="$(sh_ run-as "$PKG" grep -rl "$CANARY_SEND" . 2>/dev/null)"
probe "the canary anywhere else under the app's data directory" "${leaks:-nowhere}"

group "Rung 2 reports unavailable rather than claiming to work (ADR-0005)"
shizuku="$(sh_ pm list packages | grep -i shizuku)"
[[ -z "$shizuku" ]] \
    && ok "Shizuku is absent, as it must be on a stock emulator" \
    || bad "Shizuku is absent, as it must be on a stock emulator" "$shizuku"
note "rung 2 itself — the shell-uid clipboard read" \
     "Shizuku needs a manual wireless-debugging pairing and cannot be granted here; docs/rewrite/android-spike.md items 2-7 stay open"

services="$(sh_ dumpsys activity services "$PKG")"
printf '%s\n' "$services" > "$OUT/services.txt"
grep -q 'CaptureService' <<<"$services" \
    && bad "no foreground capture service is running" \
           "CaptureService is up, but nothing granted rung 2 — the app is claiming to capture in the background" \
    || ok "no foreground capture service is running"

notifs="$(sh_ dumpsys notification --noredact 2>/dev/null || sh_ dumpsys notification)"
printf '%s\n' "$notifs" > "$OUT/notifications.txt"
grep -q 'Capturing from every app' <<<"$notifs" \
    && bad "no notification claims background capture" "the ongoing capture notification is posted with rung 2 unavailable" \
    || ok "no notification claims background capture"

group "Probes for the next round"
tile_add="$(sh_ cmd statusbar add-tile "$PKG/$APP_NAMESPACE.CaptureTileService")"
tile_click="$(sh_ cmd statusbar click-tile "$PKG/$APP_NAMESPACE.CaptureTileService")"
probe "the Quick Settings tile" "add: ${tile_add:-<no output>} / click: ${tile_click:-<no output>}"
probe "adb can set the primary clip" "$(sh_ cmd clipboard set-primary-clip --text CopyPasteTileProbe 2>&1 | head -n 1)"
sleep 8
probe "the store changed after the tile probe" \
      "$([[ "$(db_fingerprint after-tile)" != "$(cat "$OUT/dbset-after-capture.txt" 2>/dev/null)" ]] && echo yes || echo no)"

note "R8 and the release build type" \
     "this APK is the debug one, because run-as needs it; the minified release APK's Tauri plugin reflection is not exercised here"
note "OEM battery managers killing the foreground service" \
     "spike item 6 needs a real phone left idle; an emulator will not reproduce it"

dump_logcat final
summary
[[ $FAIL -eq 0 ]]
