#!/usr/bin/env bash
# Drive export and import through Android's document provider, then prove the
# imported row survives a process restart. The debug gate additionally reads
# the SQLCipher files through run-as and rejects plaintext.
set -uo pipefail

# shellcheck source=scripts/release/android-smoke-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-smoke-lib.sh"
# shellcheck source=scripts/release/android-ui-evidence-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-ui-evidence-lib.sh"

MAIN="$PKG/$APP_NAMESPACE.MainActivity"
export WAIT_SECS="${TRANSFER_WAIT_SECS:-45}"
REQUIRE_RUN_AS="${TRANSFER_REQUIRE_RUN_AS:-0}"
CANARY="CopyPasteStorageTransfer$(date +%s)$RANDOM"
SEED_FILE="copypaste-seed.json"
EXPORT_FILE="copypaste-export.json"
INVALID_FILE="copypaste-invalid.json"

open_downloads() { # <artifact prefix>
    local prefix="$1" focus
    focus="$(sh_ dumpsys window windows)"
    printf '%s\n' "$focus" > "$OUT/${prefix}-window.txt"
    if grep -qE 'com\.(google\.)?android\.documentsui' <<<"$focus"; then
        ok "the Android document picker owns the focused window"
    else
        bad "the Android document picker owns the focused window" "$(grep -E 'mCurrentFocus|mFocusedApp' <<<"$focus" | head -n 2)"
    fi

    if tap_selector "Show roots|Roots" "$OUT/${prefix}-roots.xml" 15; then sleep 1; fi
    tap_selector "Downloads|downloads" "$OUT/${prefix}-downloads.xml" 30
}

open_storage() {
    tap_selector "Settings" "$OUT/settings-nav.xml" || return 1
    tap_selector "Storage" "$OUT/settings-storage.xml" || return 1
    wait_selector "Export…|Import…" "$OUT/settings-storage-ready.xml" || return 1
}

open_history() { # <artifact>
    local artifact="$1" point
    tap_selector "History" "$artifact" || return 1
    wait_selector "Select multiple items" "$artifact" || return 1
    point="$(action_center "$artifact" "Clear search")"
    [[ -z "$point" ]] || sh_ input tap $point >/dev/null
}

capture_screen() { # <name>
    local name="$1" ax="$OUT/$1.xml" png="$OUT/$1.png"
    if ! dump_hierarchy "$ax" || [[ ! -s "$ax" ]]; then
        bad "$name accessibility evidence exists"
    fi
    if ! capture_png "$png"; then
        bad "$name screenshot evidence is a complete PNG" \
            "$(tail -n 12 "${png%.png}-screencap.log" | tr '\n' ' ')"
    fi
}

self_test_transfer() {
    android_ui_self_test
    printf '\n%d transfer selector tests passed, %d failed\n' "$PASS" "$FAIL"
    [[ $FAIL -eq 0 ]]
}

if [[ "${1:-}" == "--self-test" ]]; then
    self_test_transfer
    exit $?
fi

mkdir -p "$OUT"
command -v adb >/dev/null 2>&1 || { echo "  FATAL adb is not on PATH"; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "  FATAL python3 is not on PATH"; exit 1; }
adb wait-for-device
adb logcat -c || true

group "Seed encrypted history"
sh_ mkdir -p /sdcard/Download >/dev/null
sh_ rm -f "/sdcard/Download/$SEED_FILE" "/sdcard/Download/$EXPORT_FILE" "/sdcard/Download/$INVALID_FILE" >/dev/null
python3 - "$CANARY" <<'PY' | adb shell dd "of=/sdcard/Download/$SEED_FILE" >/dev/null
import json
import sys
import time

print(json.dumps({
    "items": [{
        "content": sys.argv[1],
        "content_type": "text/plain",
        "created_at": int(time.time() * 1000),
        "pinned": False,
        "is_sensitive": False,
    }],
    "skipped_non_text": 0,
    "skipped_sensitive": 0,
    "skipped_undecryptable": 0,
}))
PY
sh_ am start -W -n "$MAIN" >/dev/null
if open_storage && tap_scrolling "Import…" "$OUT/seed-import-action.xml" up; then
    sleep 2
    open_downloads seed-picker || bad "Downloads is selectable for the seed import"
    tap_selector "$SEED_FILE" "$OUT/seed-file.xml" 15 || bad "the seed export is selectable"
    prepare_action "Import" "$OUT/seed-import-confirm.xml" 20 || bad "the seed import requires confirmation"
    start_accessibility_events "$OUT/seed-import-events.log" || bad "seed import accessibility events are observable"
    tap_prepared_action || bad "the seed import confirmation remains actionable"
    wait_accessibility_event "Imported" "$OUT/seed-import-events.log" 20 && ok "seed import reports user-visible success" || bad "seed import reports user-visible success"
else
    bad "Storage exposes import for the seed"
fi
open_history "$OUT/seed-history-nav.xml" || bad "History is reachable before export"
wait_selector "$CANARY" "$OUT/seed-history.xml" 30 && ok "the canary is visible before export" || bad "the canary is visible before export" "uiautomator did not expose it"

group "Export through DocumentsUI"
if open_storage && tap_selector "Export…" "$OUT/export-action.xml"; then
    tap_selector "Choose where to save" "$OUT/export-confirm.xml" || bad "the export confirmation is actionable"
    sleep 2
    open_downloads export-picker || bad "Downloads is selectable in the save picker"
    prepare_action "Save|action_menu_done" "$OUT/export-save.xml" 15 || bad "the picker exposes its save action"
    start_accessibility_events "$OUT/export-events.log" || bad "export accessibility events are observable"
    tap_prepared_action || bad "the picker save action remains actionable"
else
    bad "Storage exposes the export action"
fi
if wait_accessibility_event "Exported" "$OUT/export-events.log" 20; then
    ok "export reports user-visible success"
else
    bad "export reports user-visible success" "no Exported toast appeared"
fi
capture_screen export-success
export_path="$(sh_ find /sdcard/Download -maxdepth 1 -type f -name "$EXPORT_FILE" | head -n 1)"
[[ -n "$export_path" ]] && ok "the selected document contains the export" || bad "the selected document contains the export" "Downloads has no $EXPORT_FILE"
exported_text=""
[[ -n "$export_path" ]] && exported_text="$(sh_ cat "$export_path")"
grep -qF "$CANARY" <<<"$exported_text" && ok "the content URI received the captured history" || bad "the content URI received the captured history" "the exported document has no canary"

group "Clear and import through DocumentsUI"
tap_scrolling "Clear history" "$OUT/clear-action.xml" up || bad "Storage exposes the clear action"
prepare_action "Clear all" "$OUT/clear-confirm.xml" || bad "the clear confirmation is actionable"
start_accessibility_events "$OUT/clear-events.log" || bad "clear accessibility events are observable"
tap_prepared_action || bad "the clear confirmation remains actionable"
wait_accessibility_event "Cleared" "$OUT/clear-events.log" 15 && ok "clear reports user-visible success" || bad "clear reports user-visible success"
open_history "$OUT/clear-history-nav.xml" || bad "History is reachable after clearing"
wait_selector "0 items" "$OUT/clear-history-ready.xml" 30 || bad "cleared history settles at zero items"
if dump_hierarchy "$OUT/cleared-history.xml" && [[ -z "$(node_center "$OUT/cleared-history.xml" "$CANARY")" ]]; then
    ok "the exported canary is absent before import"
else
    bad "the exported canary is absent before import" "the clear did not produce an inspectable history without it"
fi
open_storage || bad "Storage is reachable for import"
tap_scrolling "Import…" "$OUT/import-action.xml" down || bad "Storage exposes the import action"
sleep 2
open_downloads import-picker || bad "Downloads is selectable in the open picker"
tap_selector "$EXPORT_FILE" "$OUT/import-file.xml" 15 || bad "the exported document is selectable"
prepare_action "Import" "$OUT/import-confirm.xml" 20 || bad "the import preview requires confirmation"
start_accessibility_events "$OUT/import-events.log" || bad "import accessibility events are observable"
tap_prepared_action || bad "the import confirmation remains actionable"
if wait_accessibility_event "Imported" "$OUT/import-events.log" 20; then
    ok "import reports user-visible success"
else
    bad "import reports user-visible success" "no Imported toast appeared"
fi

group "Persisted ciphertext"
sh_ am force-stop "$PKG" >/dev/null
wait_for 20 no_pid || bad "force-stop ends the importing process" "pid $(app_pid) is still running"
sh_ am start -W -n "$MAIN" >/dev/null
wait_for 60 has_pid || bad "the app relaunches after import"
transfer_pid="$(app_pid)"
open_history "$OUT/transfer-history-nav.xml" || bad "History is reachable after restart"
if wait_selector "$CANARY" "$OUT/transfer-persisted.xml" 45; then
    ok "the imported history survives a process restart"
else
    bad "the imported history survives a process restart" "the canary is absent after relaunch"
fi
capture_screen transfer-persisted

runas="$(sh_ run-as "$PKG" id)"
if grep -q uid <<<"$runas"; then
    app_files > "$OUT/transfer-files.txt"
    DB_REL="$(grep -E 'copypaste-v2\.db$' "$OUT/transfer-files.txt" | head -n 1)"
    leaks=""
    if [[ -n "$DB_REL" ]]; then
        db_fingerprint transfer-persisted >/dev/null
        for file in "$OUT"/transfer-persisted-*; do
            [[ -f "$file" ]] && holds_text "$file" "$CANARY" && leaks+="$(basename "$file") "
        done
    fi
    [[ -n "$DB_REL" && -z "$leaks" ]] && ok "persisted database files do not expose the imported plaintext" || bad "persisted database files do not expose the imported plaintext" "database=${DB_REL:-missing}, leaks=${leaks:-none}"
elif [[ "$REQUIRE_RUN_AS" == 1 ]]; then
    bad "the debug storage gate can read the database" "$runas"
else
    note "ciphertext bytes in the non-debuggable build" "Android denies run-as; the debug transfer gate inspects the same database files"
fi

group "Rejected import is visible and non-destructive"
printf 'not-json\n' | adb shell dd "of=/sdcard/Download/$INVALID_FILE" >/dev/null
open_storage || bad "Settings remains reachable after the persisted import"
tap_scrolling "Import…" "$OUT/invalid-action.xml" up || bad "Storage exposes import for the failure case"
sleep 2
open_downloads invalid-picker || bad "Downloads is selectable for the failure case"
prepare_action "$INVALID_FILE" "$OUT/invalid-file.xml" 15 || bad "the invalid document is selectable"
start_accessibility_events "$OUT/import-failure-events.log" || bad "rejected import accessibility events are observable"
tap_prepared_action || bad "the invalid document remains actionable"
if wait_accessibility_event "isn't a CopyPaste export" "$OUT/import-failure-events.log" 20; then
    ok "an invalid content URI reports a user-visible failure"
else
    bad "an invalid content URI reports a user-visible failure" "the authored error toast did not appear"
fi
capture_screen import-failure
open_history "$OUT/history-nav.xml" || bad "History remains reachable after the rejected import"
wait_selector "$CANARY" "$OUT/history-after-failure.xml" 20 && ok "a rejected import leaves persisted history intact" || bad "a rejected import leaves persisted history intact"

dump_logcat storage-transfer
crashes="$(crash_report "$OUT/storage-transfer.log" "$transfer_pid")"
[[ -z "$crashes" ]] && ok "no app crash occurred during storage transfer" || bad "no app crash occurred during storage transfer" "$(head -n 20 <<<"$crashes")"

printf '\n## Android storage transfer: %s\n\n%d assertions passed, %d failed.\n' "$([[ $FAIL -eq 0 ]] && echo passed || echo FAILED)" "$PASS" "$FAIL" | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
[[ $FAIL -eq 0 ]]
