#!/usr/bin/env bash
# android-rungs.sh — the four surfaces README.md calls unverified.
#
# android-smoke.sh proves the app launches, keeps its keys and stores what the
# share sheet hands it. It asserts the rest only negatively: no foreground
# service, no notification claiming background capture. This script asserts them
# positively, and where an emulator cannot, says which capability is missing and
# runs the closest thing that can.
#
#   1. rung 2 — the shell-uid clipboard read (android-clipboard-access.md §4)
#   2. the Quick Settings tile (§3, rung 0)
#   3. the background capture service
#   4. FLAG_SECURE (INV-35)
#
# Needs a booted device with the app installed; set APK to install one first.
# The detectors and their fixtures are in android-rungs-lib.sh; `--self-test`
# runs those and needs no device.
set -uo pipefail

# shellcheck source=scripts/release/android-rungs-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-rungs-lib.sh"

APK="${APK:-}"
MAIN="$PKG/$APP_NAMESPACE.MainActivity"
TILE="$PKG/$APP_NAMESPACE.CaptureTileService"
CAPTURE_WAIT_SECS="${CAPTURE_WAIT_SECS:-40}"
SETTLE_SECS="${SETTLE_SECS:-45}"

# The app whose text field is driven to put a foreign clip on the clipboard.
# The field belongs to settings.intelligence, which is what getPrimaryClipSource
# reports and what the source attribution is then checked against.
SETTINGS_ACTION="android.settings.SETTINGS"
CLIP_OWNER="com.google.android.settings.intelligence"

# IClipboard transaction codes, in the order AOSP has declared them since
# API 30. Everything at or above getPrimaryClip is read-only, which is why the
# fallback scan starts there: clearPrimaryClip is code 3 and would destroy the
# canary the whole group depends on.
CLIP_GET=4
CLIP_HAS=6
CLIP_SOURCE=10
CLIP_SCAN_LAST=10

if [[ "${1:-}" == "--self-test" ]]; then
    rungs_self_test
    exit $?
fi

mkdir -p "$OUT"

# The four surfaces this script exists to assert. A rung that produced neither a
# run nor a not-applicable receipt is a rung nobody watched, and the run fails on
# the absence rather than reporting the assertions that did happen.
RUNGS_DECLARED=(rung-2 tile background-capture flag-secure)
RECEIPTS="$OUT/rung-receipts.tsv"

stamp="$(date +%s)$RANDOM"
CANARY_SHELL="CopyPasteShellCanary${stamp}"
CANARY_TILE="CopyPasteTileCanary${stamp}"

# Helpers that need the device

clip_call() {   # <tag> <code> <callingPackage>
    adb shell "service call clipboard $2 s16 $3 s16 null i32 0 i32 0" 2>&1 \
        | tr -d '\r' > "$OUT/parcel-$1.txt"
}

# Put text on the clipboard from an app that is not ours.
#
# Both Settings packages are stopped first: the search activity left open by an
# earlier call has a different layout, and the home screen's search bar is what
# the tap point is read from.
foreign_copy() {   # <alphanumeric text>
    for _ in 1 2; do
        foreign_copy_once "$1" && return 0
    done
    return 1
}

foreign_copy_once() {   # <alphanumeric text>
    sh_ am force-stop "$CLIP_OWNER" >/dev/null 2>&1
    sh_ am force-stop com.android.settings >/dev/null 2>&1
    sh_ am start -a "$SETTINGS_ACTION" >/dev/null 2>&1
    sleep 6
    dump_hierarchy_retry "$OUT/settings.xml" || return 1
    local centre
    centre="$(node_centre "$OUT/settings.xml" "search_action_bar")"
    [[ -n "$centre" ]] || return 1
    # shellcheck disable=SC2086
    sh_ input tap $centre >/dev/null 2>&1
    sleep 5
    sh_ input text "$1" >/dev/null 2>&1
    sleep 3

    # `input text` is silent when nothing is focused, and copying an empty
    # selection leaves the previous clip in place — so every assertion below
    # would read a stale canary as a pass. Confirm the field took it first.
    dump_hierarchy_retry "$OUT/typed.xml" || return 1
    ui_strings "$OUT/typed.xml" | grep -qF "$1" || return 1

    sh_ input keycombination 113 29 >/dev/null 2>&1   # CTRL_LEFT + A
    sleep 1
    sh_ input keyevent 278 >/dev/null 2>&1            # KEYCODE_COPY
    sleep 3
}

focused_window() { sh_ dumpsys window | grep -E 'mCurrentFocus' | head -n 1; }

# Is this canary the clip that is actually on the clipboard right now?
#
# Read back over the shell-uid call, because "the script tried to put it there"
# is not the same fact. Every canary assertion below names the clip that is
# really there; a stale one would let them pass without the capture under test
# having happened at all.
clipboard_holds() {   # <alphanumeric text>
    clip_call now "$CLIP_GET" com.android.shell
    parcel_holds "$OUT/parcel-now.txt" "$1"
}

group "Preflight"
command -v adb >/dev/null 2>&1 || { echo "  FATAL adb is not on PATH"; exit 1; }
adb wait-for-device
devices="$(adb devices | grep -cE '\sdevice$')"
[[ "$devices" == "1" ]] || { echo "  FATAL expected one device, adb reports $devices"; adb devices; exit 1; }

# Every rung below decides from the API level — which clipboard codes to expect,
# which surfaces the image can show at all — so a level that was not read is not
# a level to carry on with.
sdk_said="$(sh_ getprop ro.build.version.sdk)"
sdk_status=$?
if ! sdk="$(api_level_from "$sdk_said" "$sdk_status")"; then
    echo "  FATAL the device's API level could not be read; every rung below is decided from it"
    exit 1
fi
printf '  device: API %s, %s\n' "$sdk" "$(sh_ getprop ro.product.cpu.abi)"
: > "$RECEIPTS"

if [[ -n "$APK" ]]; then
    [[ -f "$APK" ]] || { echo "  FATAL APK not found at '$APK'"; exit 1; }
    adb uninstall "$PKG" >/dev/null 2>&1 || true
    adb install -r -g "$APK" >/dev/null 2>&1 || { echo "  FATAL adb install failed"; exit 1; }
fi

if grep -q uid <<<"$(sh_ run-as "$PKG" id)"; then
    ok "the app is installed and debuggable"
else
    bad "the app is installed and debuggable" \
        "$PKG is absent or not debuggable; every filesystem assertion below needs run-as"
    summary; exit 1
fi

wake_screen
sh_ am force-stop "$PKG"
sh_ am start -W -n "$MAIN" >/dev/null 2>&1
sleep "$SETTLE_SECS"
app_files > "$OUT/files.txt"
DB_REL="$(grep -E 'copypaste-v2\.db$' "$OUT/files.txt" | head -n 1)"
[[ -n "$DB_REL" ]] \
    && ok "the history database exists" \
    || bad "the history database exists" "nothing named copypaste-v2.db under the app's data directory"

group "1. Rung 2 — the shell uid reads the clipboard with no focus"
# What this settles: android-clipboard-access.md §4 is derived from AOSP source
# and marked never observed. Shizuku's whole contribution is making our binder
# calls arrive as uid 2000 with callingPackage="com.android.shell"; adb shell
# already is that identity, so the platform half of the claim is testable here
# even though Shizuku is not installed.

if foreign_copy "$CANARY_SHELL"; then
    ok "another app put text on the clipboard"
else
    bad "another app put text on the clipboard" \
        "the Settings search field could not be driven; every assertion in this group needs a foreign clip"
fi

focus="$(focused_window)"
if grep -q "$PKG" <<<"$focus"; then
    bad "our app does not have focus while the clipboard is read" "$focus"
else
    ok "our app does not have focus while the clipboard is read"
    printf '        %s\n' "$focus"
fi

clip_call shell "$CLIP_GET" com.android.shell
found_at=""
parcel_holds "$OUT/parcel-shell.txt" "$CANARY_SHELL" && found_at="$CLIP_GET"
if [[ -z "$found_at" ]]; then
    # The transaction order is not API. If it moved, name the code that works
    # rather than reporting the mechanism as broken.
    for code in $(seq $((CLIP_GET + 1)) "$CLIP_SCAN_LAST"); do
        clip_call "scan$code" "$code" com.android.shell
        if parcel_holds "$OUT/parcel-scan$code.txt" "$CANARY_SHELL"; then
            found_at="$code"
            note "the IClipboard transaction order" \
                 "getPrimaryClip is code $code on API $sdk, not $CLIP_GET; ShizukuClipboard resolves by name and is unaffected, but this script's table is stale"
            break
        fi
    done
fi

if [[ -n "$found_at" ]]; then
    ok "getPrimaryClip as the shell uid returns the clip taken in another app"
else
    bad "getPrimaryClip as the shell uid returns the clip taken in another app" \
        "$(parcel_message "$OUT/parcel-shell.txt" | head -c 300) — rung 2 is not available on this image, and android-clipboard-access.md §4 rests on it"
fi

# The identity is what grants the read, not the transport. If any callingPackage
# were accepted, Shizuku would be unnecessary and so would this rung.
clip_call ours "$CLIP_GET" "$PKG"
if parcel_refused "$OUT/parcel-ours.txt" && ! parcel_holds "$OUT/parcel-ours.txt" "$CANARY_SHELL"; then
    ok "the same call claiming our own package is refused"
    printf '        %s\n' "$(parcel_message "$OUT/parcel-ours.txt" | head -c 160)"
else
    bad "the same call claiming our own package is refused" \
        "checkPackage did not refuse a package the shell uid does not own, so the clipboard is readable by anyone who can reach the binder"
fi

clip_call has "$CLIP_HAS" com.android.shell
if ! parcel_refused "$OUT/parcel-has.txt"; then
    ok "hasPrimaryClip answers under the same argument vector"
else
    bad "hasPrimaryClip answers under the same argument vector" \
        "spike item 4: (String callingPackage, String attributionTag, int userId, int deviceId) is not this API level's order — $(parcel_message "$OUT/parcel-has.txt" | head -c 200)"
fi

clip_call source "$CLIP_SOURCE" com.android.shell
if parcel_holds "$OUT/parcel-source.txt" "$(tr -d '.' <<<"$CLIP_OWNER")"; then
    ok "getPrimaryClipSource names the app that took the copy"
else
    probe "getPrimaryClipSource" \
          "$(parcel_message "$OUT/parcel-source.txt" | head -c 160) — ShizukuClipboard.sourcePackage tolerates its absence"
fi

# Rule 4. A clipboard read that logs its result puts the clip in a buffer any
# app holding READ_LOGS can tail; this is the current ClipCascade signal.
dump_logcat rung2
if grep -aq "$CANARY_SHELL" "$OUT/rung2.log"; then
    bad "the clipboard read leaves no plaintext in logcat" \
        "$(grep -a "$CANARY_SHELL" "$OUT/rung2.log" | head -n 3)"
else
    ok "the clipboard read leaves no plaintext in logcat"
fi

note "the Shizuku transport itself" \
     "ShizukuBinderWrapper, IClipboard\$Stub.asInterface by reflection, the IOnPrimaryClipChangedListener descriptor and whether the listener ever fires (spike items 2, 3, 8) all need Shizuku installed and paired over wireless debugging by hand. What is settled above is the platform's half: the uid, the callingPackage and the argument vector"
note "the Android 12+ access toast and OEM battery managers" \
     "spike items 6 and 7 need a phone — a toast is not in any dumpsys this image answers, and no emulator reproduces a vendor task killer"

rung_receipt "$RECEIPTS" rung-2 run \
    "API $sdk; getPrimaryClip as the shell uid answered at code ${found_at:-none}"

group "2. The Quick Settings tile"
# android-spike.md records this as unproven because `cmd statusbar add-tile`
# and `click-tile` printed nothing. They print nothing and work: add-tile is
# visible in sysui_qs_tiles, and click-tile reaches the tile once SystemUI has
# bound it — which it does lazily, so the panel has to be opened once first.

sh_ cmd statusbar add-tile "$TILE" >/dev/null 2>&1
sleep 2
tiles="$(sh_ settings get secure sysui_qs_tiles)"
if tile_present "$tiles" "$TILE"; then
    ok "add-tile put the tile in the Quick Settings list"
else
    bad "add-tile put the tile in the Quick Settings list" "$tiles"
fi

# A fresh clip is preferred, but group 1's is equally foreign and is already on
# the clipboard. What must never happen is testing against a canary that is not
# there: the plaintext assertions would then pass over a capture that never
# occurred, which is the shape of a test that proves nothing.
CLIP_NOW="$CANARY_SHELL"
if foreign_copy "$CANARY_TILE" && clipboard_holds "$CANARY_TILE"; then
    CLIP_NOW="$CANARY_TILE"
    ok "another app put a second clip on the clipboard for the tile to save"
else
    note "a second clip for the tile" \
         "the Settings field could not be driven again; the tile is tested against the clip group 1 left there, which is equally foreign"
fi

if clipboard_holds "$CLIP_NOW"; then
    ok "the clipboard holds a clip taken in another app for the tile to read"
else
    bad "the clipboard holds a clip taken in another app for the tile to read" \
        "nothing foreign is on the clipboard, so the assertions below would pass without the tile capturing anything"
    CLIP_NOW=""
fi

before="$(db_fingerprint before-tile)"
adb logcat -c || true
sh_ cmd statusbar expand-settings >/dev/null 2>&1
sleep 5
sh_ cmd statusbar click-tile "$TILE" >/dev/null 2>&1
sleep 6
sh_ cmd statusbar collapse >/dev/null 2>&1
dump_logcat tile

if grep -aq "cmp=$PKG/$APP_NAMESPACE.ClipboardCaptureActivity" "$OUT/tile.log"; then
    ok "one tile tap started the activity whose focus makes the read legal"
else
    bad "one tile tap started the activity whose focus makes the read legal" \
        "no ClipboardCaptureActivity start in logcat; SystemUI binds a third-party tile lazily, so a panel that never opened is one way this fails"
fi

changed=0
for _ in $(seq 1 $((CAPTURE_WAIT_SECS / 5))); do
    sleep 5
    if [[ "$(db_fingerprint after-tile)" != "$before" ]]; then changed=1; break; fi
done

if [[ $changed -eq 1 ]]; then
    ok "the tile's capture reached SQLCipher"
else
    bad "the tile's capture reached SQLCipher" \
        "the store did not change in ${CAPTURE_WAIT_SECS}s: the focus read was refused, or ClipQueue was never drained"
fi

# Rule 4, on the path that handles a clip taken from another app. Only
# meaningful once something was stored and the canary is the clip that was
# really on the clipboard.
if [[ $changed -eq 1 && -n "$CLIP_NOW" ]]; then
    plain=""
    for f in "$OUT"/after-tile-*; do
        [[ -f "$f" ]] || continue
        holds_text "$f" "$CLIP_NOW" && plain+="$(basename "$f") "
    done
    [[ -z "$plain" ]] \
        && ok "the tile's capture is not readable in the database files" \
        || bad "the tile's capture is not readable in the database files" "found in: $plain"

    leaks="$(sh_ run-as "$PKG" grep -rl "$CLIP_NOW" . 2>/dev/null)"
    [[ -z "$leaks" ]] \
        && ok "the tile's capture is nowhere else under the app's data directory" \
        || bad "the tile's capture is nowhere else under the app's data directory" "$leaks"
else
    note "that the tile's capture is unreadable at rest" \
         "nothing was stored, or no known clip was on the clipboard to look for"
fi

# The tile is a system surface: SystemUI renders its label and any app can see
# it. Neither it nor the recents entry may carry what was captured. logcat is
# scoped to our own lines because the app the clip was copied *from* still has
# it on screen and legitimately logs about its own text field.
sh_ dumpsys notification --noredact > "$OUT/notifications.txt" 2>/dev/null \
    || sh_ dumpsys notification > "$OUT/notifications.txt"
sh_ dumpsys activity recents > "$OUT/recents.txt" 2>/dev/null
if [[ -n "$CLIP_NOW" ]]; then
    exposed=""
    holds_text "$OUT/notifications.txt" "$CLIP_NOW" && exposed+="notifications "
    holds_text "$OUT/recents.txt" "$CLIP_NOW" && exposed+="recents "
    grep -a "$CLIP_NOW" "$OUT/tile.log" 2>/dev/null | grep -qi copypaste && exposed+="our-logcat "
    [[ -z "$exposed" ]] \
        && ok "the captured clip reaches no notification, recents entry or log line of ours" \
        || bad "the captured clip reaches no notification, recents entry or log line of ours" "$exposed"
else
    note "that the captured clip reaches no system surface" "there was no known clip to look for"
fi

# The tile's activity reads the clipboard the moment it has focus. If another
# app could start it, that app would have a clipboard reader it does not own.
reader_start="$(sh_ am start -n "$PKG/$APP_NAMESPACE.ClipboardCaptureActivity")"
if grep -q 'not exported' <<<"$reader_start"; then
    ok "no other app can start the tile's clipboard reader"
else
    bad "no other app can start the tile's clipboard reader" "$reader_start"
fi

rung_receipt "$RECEIPTS" tile run "API $sdk; the tile was added, bound and clicked"

group "3. The background capture service"
# The service exists to keep the process alive while rung 2 is armed. With
# setup absent nothing may arm, and the failure this guards against is a
# service that runs anyway: an ongoing "Capturing from every app." notification
# over a reader that is not there. `CopyPaste-qzhu` forbids reporting working
# merely because a permission is present.

start_out="$(sh_ am start-foreground-service -n "$PKG/$APP_NAMESPACE.CaptureService")"
if grep -q 'not exported' <<<"$start_out"; then
    ok "no other app can start the capture service"
else
    bad "no other app can start the capture service" \
        "the shell started it: ${start_out} — any app could raise a notification claiming CopyPaste is capturing"
fi

# The state the service reads on a sticky restart, written directly so the
# guard is exercised without Shizuku. Written with the app stopped, or the
# in-memory SharedPreferences would overwrite it on the way out.
sh_ am force-stop "$PKG"
sleep 2
adb shell "run-as $PKG sh -c 'cat > shared_prefs/capture-service.xml'" <<'EOF'
<?xml version='1.0' encoding='utf-8' standalone='yes' ?>
<map>
    <boolean name="enabled" value="true" />
    <string name="text">Capturing from every app.</string>
    <string name="lostTitle">Background capture stopped.</string>
    <string name="lostBody">CopyPaste is only saving what you copy inside the app. Tap to restart.</string>
</map>
EOF
seeded="$(sh_ run-as "$PKG" cat shared_prefs/capture-service.xml)"
if grep -q 'name="enabled" value="true"' <<<"$seeded"; then
    ok "the app now believes background capture was armed"
else
    bad "the app now believes background capture was armed" \
        "the seed did not stick, so the guard below is not being exercised: $seeded"
fi

adb logcat -c || true
sh_ am start -W -n "$MAIN" >/dev/null 2>&1
sleep "$SETTLE_SECS"

sh_ dumpsys activity services "$PKG" > "$OUT/services.txt" 2>/dev/null
if service_is_running "$OUT/services.txt" "CaptureService"; then
    bad "no capture service runs when nothing is reading" \
        "CaptureService is up with setup absent — the process is being kept alive for a reader that does not exist"
else
    ok "no capture service runs when nothing is reading"
fi

sh_ dumpsys notification --noredact > "$OUT/notifications-armed.txt" 2>/dev/null \
    || sh_ dumpsys notification > "$OUT/notifications-armed.txt"
if holds_text "$OUT/notifications-armed.txt" "Capturing from every app."; then
    bad "no notification claims background capture" \
        "the ongoing notification is posted with nothing armed, which is the silent-failure outcome §5 exists to prevent"
else
    ok "no notification claims background capture"
fi

if dump_hierarchy_retry "$OUT/ui-armed.xml"; then
    strings="$(ui_strings "$OUT/ui-armed.xml")"
    if grep -qxF "Capturing from every app." <<<"$strings"; then
        bad "the UI does not claim background capture either" \
            "a persisted enabled flag reached the headline without a live reader — capture::model must only see enabled when ClipCascadeCapture.isListening()"
    else
        ok "the UI does not claim background capture either"
    fi
    probe "what the app is showing" \
          "$(wc -l <<<"$strings") named nodes: $(head -n 4 <<<"$strings" | tr '\n' '|')"
else
    note "what the capture card says" "uiautomator produced no dump, so the UI could not be read"
fi

# §5 rule 3 wants loss pushed, not polled. The death recipient that pushes it
# lives in the process that died, so a cold start after a reboot has nobody to
# post it. Recorded rather than asserted: whether the app should re-post on
# start from the persisted flag is a design decision, not a defect this script
# can decide.
if holds_text "$OUT/notifications-armed.txt" "Background capture stopped."; then
    probe "the loss notification after a process death" "posted"
else
    probe "the loss notification after a process death" \
          "not posted — the binder death recipient died with the process, so a reboot produces no notification at all"
fi

sh_ run-as "$PKG" rm -f shared_prefs/capture-service.xml >/dev/null 2>&1

note "the foreground service surviving an OEM battery manager" \
     "spike item 6 needs a real phone left idle for an hour; the design is weakest here and an emulator cannot reproduce it"

rung_receipt "$RECEIPTS" background-capture run "API $sdk; the service was driven with the armed state seeded"

group "4. FLAG_SECURE (INV-35)"
# tao's set_content_protection is compiled for macOS and Windows only, so on
# Android nothing sets this but ScreenProtectionPlugin. Asserted on the window,
# because that is where the platform enforces it. screencap and screenrecord
# are not evidence either way: they run as the shell uid, which captures secure
# layers on purpose.

sh_ am force-stop "$PKG"
sleep 2
sh_ am start -W -n "$MAIN" >/dev/null 2>&1

# Polled from the launch rather than sampled at the end: a window that is
# created unprotected and secured a frame later is a window whose history was
# screenshot-able, and one late dump would call that a pass.
seen=0
unprotected=0
for i in $(seq 1 20); do
    sh_ dumpsys window windows > "$OUT/windows-$i.txt" 2>/dev/null
    if [[ -n "$(window_flags "$OUT/windows-$i.txt" "$PKG/$APP_NAMESPACE.MainActivity")" ]]; then
        seen=$((seen + 1))
        window_is_secure "$OUT/windows-$i.txt" "$PKG/$APP_NAMESPACE.MainActivity" || unprotected=$((unprotected + 1))
        cp "$OUT/windows-$i.txt" "$OUT/windows-last.txt"
    fi
    sleep 3
done

if (( seen == 0 )); then
    bad "the main window exists to be checked" "20 dumps over a minute held no window named $PKG"
elif (( unprotected == 0 )); then
    ok "every dump of the main window carried FLAG_SECURE ($seen dumps)"
    printf '        fl=%s\n' "$(window_flags "$OUT/windows-last.txt" "$PKG/$APP_NAMESPACE.MainActivity")"
else
    bad "every dump of the main window carried FLAG_SECURE" \
        "$unprotected of $seen dumps had the window without SECURE — the history is capturable in that window"
fi

# The control that keeps the assertion above honest: a reader that answered
# SECURE about every window would pass it without INV-35 holding at all.
control="$(other_unprotected_window "$OUT/windows-last.txt" "$PKG" 2>/dev/null)"
if [[ -n "$control" ]]; then
    ok "the same reader reports another window on this device as unprotected"
    printf '        control: %s\n' "$control"
else
    bad "the same reader reports another window on this device as unprotected" \
        "every window in the dump came back secure, so SECURE above says nothing about our window"
fi

probe "the recents entry for the app" \
      "$(grep -c "$PKG" "$OUT/recents.txt" 2>/dev/null || echo 0) lines name us; FLAG_SECURE blanks the thumbnail, which no dumpsys reports"
note "that a third-party recorder actually sees nothing" \
     "FLAG_SECURE is enforced against MediaProjection, which needs a second app and a consent dialog; screencap and screenrecord run as shell and capture secure layers by design, so neither can stand in for it"
note "turning protection back off" \
     "ScreenProtectionPlugin.setProtected(false) is reachable only from the WebView, and the setting that calls it is not driven here"

rung_receipt "$RECEIPTS" flag-secure run "API $sdk; $seen dump(s) of the main window over a minute"

missing="$(rungs_without_receipt "$RECEIPTS" "${RUNGS_DECLARED[@]}" | tr '\n' ' ')"
if [[ -z "${missing// /}" ]]; then
    ok "every declared rung recorded a receipt"
    printf '        %s\n' "$RECEIPTS"
else
    bad "every declared rung recorded a receipt" \
        "no run or not-applicable receipt for: ${missing% }— those rungs were never observed, so this run asserts nothing about them"
fi

dump_logcat final
summary
[[ $FAIL -eq 0 ]]
