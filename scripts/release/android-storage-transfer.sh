#!/usr/bin/env bash
# Drive export and import through Android's document provider, then prove the
# imported row survives a process restart. The debug gate additionally reads
# the SQLCipher files through run-as and rejects plaintext.
set -uo pipefail

# shellcheck source=scripts/release/android-smoke-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-smoke-lib.sh"

MAIN="$PKG/.MainActivity"
INTAKE="$PKG/.IntakeActivity"
WAIT_SECS="${TRANSFER_WAIT_SECS:-45}"
REQUIRE_RUN_AS="${TRANSFER_REQUIRE_RUN_AS:-0}"
CANARY="CopyPasteStorageTransfer$(date +%s)$RANDOM"
EXPORT_FILE="copypaste-export.json"
INVALID_FILE="copypaste-invalid.json"

node_center() { # <xml> <selector alternatives separated by |>
    python3 - "$1" "$2" <<'PY'
import re
import sys
import xml.etree.ElementTree as ET

root = ET.parse(sys.argv[1]).getroot()
selectors = [part.casefold() for part in sys.argv[2].split("|")]
candidates = []
for node in root.iter("node"):
    attrs = [node.get(name, "") for name in ("text", "content-desc", "resource-id")]
    values = [value.casefold() for value in attrs if value]
    exact = any(selector == value or value.endswith("/" + selector) for selector in selectors for value in values)
    partial = any(selector in value for selector in selectors for value in values)
    bounds = re.fullmatch(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]", node.get("bounds", ""))
    if bounds and (exact or partial) and node.get("enabled", "true") != "false":
        points = tuple(map(int, bounds.groups()))
        clickable = node.get("clickable", "false") == "true"
        candidates.append((not exact, not clickable, points, attrs))
if candidates:
    _, _, (left, top, right, bottom), _ = min(candidates)
    print((left + right) // 2, (top + bottom) // 2)
PY
}

wait_selector() { # <selector> <artifact> [timeout]
    local selector="$1" artifact="$2" timeout="${3:-$WAIT_SECS}" started="$SECONDS"
    while (( SECONDS - started < timeout )); do
        if dump_hierarchy "$artifact" && [[ -n "$(node_center "$artifact" "$selector")" ]]; then
            return 0
        fi
        sleep 1
    done
    return 1
}

tap_selector() { # <selector> <artifact> [timeout]
    local selector="$1" artifact="$2" timeout="${3:-$WAIT_SECS}" point
    wait_selector "$selector" "$artifact" "$timeout" || return 1
    point="$(node_center "$artifact" "$selector")"
    sh_ input tap $point >/dev/null
}

screen_size() {
    sh_ wm size | sed -n 's/.* \([0-9][0-9]*\)x\([0-9][0-9]*\).*/\1 \2/p' | tail -n 1
}

tap_scrolling() { # <selector> <artifact> <up|down>
    local selector="$1" artifact="$2" direction="$3" width height point
    read -r width height <<<"$(screen_size)"
    width="${width:-1080}"; height="${height:-1920}"
    for _ in $(seq 1 8); do
        dump_hierarchy "$artifact" || true
        point="$(node_center "$artifact" "$selector")"
        if [[ -n "$point" ]]; then sh_ input tap $point >/dev/null; return 0; fi
        if [[ "$direction" == up ]]; then
            sh_ input swipe $((width / 2)) $((height * 3 / 4)) $((width / 2)) $((height / 3)) 250 >/dev/null
        else
            sh_ input swipe $((width / 2)) $((height / 3)) $((width / 2)) $((height * 3 / 4)) 250 >/dev/null
        fi
        sleep 1
    done
    return 1
}

open_downloads() { # <artifact prefix>
    local prefix="$1" focus
    focus="$(sh_ dumpsys window windows)"
    printf '%s\n' "$focus" > "$OUT/${prefix}-window.txt"
    if grep -qE 'com\.(google\.)?android\.documentsui' <<<"$focus"; then
        ok "the Android document picker owns the focused window"
    else
        bad "the Android document picker owns the focused window" "$(grep -E 'mCurrentFocus|mFocusedApp' <<<"$focus" | head -n 2)"
    fi

    if tap_selector "Show roots|Roots" "$OUT/${prefix}-roots.xml" 3; then sleep 1; fi
    tap_selector "Downloads|downloads" "$OUT/${prefix}-downloads.xml" 10
}

open_storage() {
    tap_selector "Settings" "$OUT/settings-nav.xml" || return 1
    tap_selector "Storage" "$OUT/settings-storage.xml" || return 1
}

capture_screen() { # <name>
    adb exec-out screencap -p > "$OUT/$1.png" 2>/dev/null || true
}

self_test_transfer() {
    local temp point
    temp="$(mktemp -d)"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text=""><node text="Clear history" bounds="[0,0][150,30]" enabled="true"/><node text="Clear history" bounds="[10,40][110,100]" enabled="true" clickable="true"/><node text="Export…" bounds="[10,110][110,170]" enabled="true" clickable="true"/><node content-desc="Save" resource-id="com.google.android.documentsui:id/action_menu_done" bounds="[200,40][300,100]" enabled="true" clickable="true"/></node></hierarchy>' > "$temp/ui.xml"
    point="$(node_center "$temp/ui.xml" "Export…")"
    [[ "$point" == "60 140" ]] && ok "an app label resolves to its tappable centre" || bad "an app label resolves to its tappable centre" "$point"
    point="$(node_center "$temp/ui.xml" "Clear history")"
    [[ "$point" == "60 70" ]] && ok "a clickable action wins over its duplicate title" || bad "a clickable action wins over its duplicate title" "$point"
    point="$(node_center "$temp/ui.xml" "action_menu_done")"
    [[ "$point" == "250 70" ]] && ok "a localized picker action resolves by resource id" || bad "a localized picker action resolves by resource id" "$point"
    [[ -z "$(node_center "$temp/ui.xml" "Import history")" ]] && ok "a missing selector is not reported as present" || bad "a missing selector is not reported as present"
    rm -rf "$temp"
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

group "Seed encrypted history"
sh_ mkdir -p /sdcard/Download >/dev/null
sh_ rm -f "/sdcard/Download/$EXPORT_FILE" "/sdcard/Download/$INVALID_FILE" >/dev/null
seed_out="$(sh_ am start -a android.intent.action.PROCESS_TEXT -t text/plain --es android.intent.extra.PROCESS_TEXT "$CANARY" -n "$INTAKE")"
grep -q 'Error' <<<"$seed_out" && bad "the transfer canary entered through ACTION_PROCESS_TEXT" "$seed_out" || ok "the transfer canary entered through ACTION_PROCESS_TEXT"
sleep 8
sh_ am start -W -n "$MAIN" >/dev/null
wait_selector "$CANARY" "$OUT/seed-history.xml" 30 && ok "the canary is visible before export" || bad "the canary is visible before export" "uiautomator did not expose it"

group "Export through DocumentsUI"
if open_storage && tap_selector "Export…" "$OUT/export-action.xml"; then
    tap_selector "Choose where to save" "$OUT/export-confirm.xml" || bad "the export confirmation is actionable"
    sleep 2
    open_downloads export-picker || bad "Downloads is selectable in the save picker"
    tap_selector "Save|action_menu_done" "$OUT/export-save.xml" 15 || bad "the picker exposes its save action"
else
    bad "Storage exposes the export action"
fi
if wait_selector "Exported" "$OUT/export-success.xml" 20; then
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
tap_selector "Clear all" "$OUT/clear-confirm.xml" || bad "the clear confirmation is actionable"
wait_selector "Cleared" "$OUT/clear-success.xml" 15 && ok "clear reports user-visible success" || bad "clear reports user-visible success"
tap_selector "History" "$OUT/clear-history-nav.xml" || bad "History is reachable after clearing"
sleep 3
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
tap_selector "Import" "$OUT/import-confirm.xml" 20 || bad "the import preview requires confirmation"
if wait_selector "Imported" "$OUT/import-success.xml" 20; then
    ok "import reports user-visible success"
else
    bad "import reports user-visible success" "no Imported toast appeared"
fi

group "Persisted ciphertext"
sh_ am force-stop "$PKG" >/dev/null
wait_for 20 no_pid || bad "force-stop ends the importing process" "pid $(app_pid) is still running"
sh_ am start -W -n "$MAIN" >/dev/null
wait_for 60 has_pid || bad "the app relaunches after import"
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
sh_ sh -c "echo not-json > /sdcard/Download/$INVALID_FILE" >/dev/null
open_storage || bad "Settings remains reachable after the persisted import"
tap_scrolling "Import…" "$OUT/invalid-action.xml" up || bad "Storage exposes import for the failure case"
sleep 2
open_downloads invalid-picker || bad "Downloads is selectable for the failure case"
tap_selector "$INVALID_FILE" "$OUT/invalid-file.xml" 15 || bad "the invalid document is selectable"
if wait_selector "isn't a CopyPaste export" "$OUT/import-failure.xml" 20; then
    ok "an invalid content URI reports a user-visible failure"
else
    bad "an invalid content URI reports a user-visible failure" "the authored error toast did not appear"
fi
capture_screen import-failure
tap_selector "History" "$OUT/history-nav.xml" || bad "History remains reachable after the rejected import"
wait_selector "$CANARY" "$OUT/history-after-failure.xml" 20 && ok "a rejected import leaves persisted history intact" || bad "a rejected import leaves persisted history intact"

dump_logcat storage-transfer
crashes="$(crash_report "$OUT/storage-transfer.log")"
[[ -z "$crashes" ]] && ok "no app crash occurred during storage transfer" || bad "no app crash occurred during storage transfer" "$(head -n 20 <<<"$crashes")"

printf '\n## Android storage transfer: %s\n\n%d assertions passed, %d failed.\n' "$([[ $FAIL -eq 0 ]] && echo passed || echo FAILED)" "$PASS" "$FAIL" | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
[[ $FAIL -eq 0 ]]
