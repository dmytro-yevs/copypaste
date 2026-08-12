#!/usr/bin/env bash
# Drive export and import through Android's document provider, then prove the
# imported row survives a process restart. The debug gate additionally reads
# the SQLCipher files through run-as and rejects plaintext.
set -uo pipefail

# shellcheck source=scripts/release/android-smoke-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-smoke-lib.sh"
# shellcheck source=scripts/release/android-ui-evidence-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-ui-evidence-lib.sh"
# shellcheck source=scripts/release/android-navigation-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-navigation-lib.sh"

MAIN="$PKG/$APP_NAMESPACE.MainActivity"
export WAIT_SECS="${TRANSFER_WAIT_SECS:-45}"
# A cold start has to reach the store before the shell enables navigation, and
# on the emulator that is minutes rather than the seconds a tap is allowed.
READY_SECS="${TRANSFER_READY_SECS:-240}"
REQUIRE_RUN_AS="${TRANSFER_REQUIRE_RUN_AS:-0}"
CANARY="CopyPasteStorageTransferT$(date +%s)-R$RANDOM"
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

# Cold, never "brought to front". `am start` on a live task is
# DELIVERED_TO_TOP: it keeps the previous leg's view, its filters and its soft
# keyboard. In run 31634096676 the UI leg left the keyboard over the tab bar and
# the first Settings tap went to the IME window instead of the app
# (`input_interaction: Interaction with: … InputMethod`), so the leg spent 45 s
# looking for a Storage pane it had never opened and blamed the seed import.
restart_app() { # <stage>
    local stage="$1"
    sh_ am force-stop "$PKG" >/dev/null
    wait_for 20 no_pid || bad "the app stops at $stage" "pid $(app_pid) is still running"
    sh_ am start -W -n "$MAIN" >/dev/null
    wait_for 60 has_pid || bad "the app relaunches at $stage"
    if wait_app_navigable "$OUT/$stage-navigable.xml" "$READY_SECS"; then
        ok "the app's navigation is actionable at $stage"
    else
        bad "the app's navigation is actionable at $stage" \
            "$(navigation_state "$OUT/$stage-navigable.xml")"
        return 1
    fi
}

settings_pane_holds() { # <artifact>
    [[ -n "$(node_center_exact "$1" "Settings sections")" ]]
}

storage_transfer_actions_holds() { # <artifact>
    [[ -n "$(action_center "$1" "Export…")" ]] \
        && [[ -n "$(action_center "$1" "Import…")" ]]
}

# Four stages open this pane, and every one of them wrote the same three
# artifacts: run 31634096676 failed at the first stage and published the third
# stage's screens, so which step had failed was no longer in the evidence.
# Each step also names itself, because "Storage exposes import for the seed" was
# what a swallowed Settings tap reported.
#
# `prepare_action` leaves the dump it aimed each tap from, so every stage already
# holds the state immediately before its own tap: pass it, and the diagnostic can
# tell a control that changed from one that was current all along.
open_storage() { # <stage>
    local stage="$1" nav="$OUT/$1-settings-nav.xml"
    local pane="$OUT/$1-settings-pane.xml" tab="$OUT/$1-settings-storage.xml"
    local ready="$OUT/$1-storage-ready.xml"
    tap_selector "Settings" "$nav" || {
        bad "the Settings tab is actionable at $stage" "$(navigation_state "$nav")"
        return 1
    }
    wait_history_state "$pane" settings_pane_holds || {
        bad "Settings opens at $stage" "$(tap_landing "$pane" Settings "$nav")"
        return 1
    }
    tap_selector "Storage" "$tab" || {
        bad "the Storage tab is actionable at $stage" "$(navigation_state "$pane")"
        return 1
    }
    wait_history_state "$ready" storage_transfer_actions_holds || {
        bad "Storage exposes its transfer actions at $stage" "$(tap_landing "$ready" Storage "$tab")"
        return 1
    }
}

history_toolbar_holds() { # <artifact>
    [[ -n "$(node_center_exact "$1" "Select multiple items")" ]]
}

history_unfiltered_holds() { # <artifact>
    history_toolbar_holds "$1" \
        && [[ -z "$(node_center_exact "$1" "Clear search")" ]]
}

# Two authored titles mean "this history is empty", and which one the app shows
# depends on whether background capture is running. The emulator never grants
# capture, so pinning the assertion to the never-copied title alone made a
# correctly cleared history look like one that had not settled. A loading, key
# or private-mode empty state still fails, which is the point of naming them.
EMPTY_HISTORY_TITLES="Nothing copied yet|Clipboard capture is paused"

# INV-12: the WebView never renders a backend sentence, so the rejected import
# arrives as the `invalid_request` code and the toast reads the catalogue copy
# for it. Asserting the Rust `MSG_NOT_AN_EXPORT` text was asserting a string
# the product cannot show, and no wait length would have found it.
IMPORT_FAILURE_COPY="couldn't complete that action"
ERROR_CATALOGUE="crates/copypaste-ui/src/i18n/en/common.ts"

cleared_history_holds() { # <artifact>
    history_unfiltered_holds "$1" \
        && [[ -n "$(node_center_exact "$1" "$EMPTY_HISTORY_TITLES")" ]] \
        && [[ -n "$(node_center_exact "$1" "0 items")" ]] \
        && [[ -z "$(node_center_exact "$1" "$CANARY")" ]]
}

history_state_holds() { # <artifact> <predicate> <dump function>
    local artifact="$1" predicate="$2" dump="$3"
    "$dump" "$artifact" && "$predicate" "$artifact"
}

wait_history_state() { # <artifact> <predicate> [timeout] [dump function]
    local artifact="$1" predicate="$2" timeout="${3:-$WAIT_SECS}" dump="${4:-dump_hierarchy}"
    wait_for "$timeout" history_state_holds "$artifact" "$predicate" "$dump"
}

wait_cleared_history() { # <artifact> [timeout] [dump function]
    local artifact="$1" timeout="${2:-$WAIT_SECS}" dump="${3:-dump_hierarchy}"
    wait_history_state "$artifact" cleared_history_holds "$timeout" "$dump"
}

open_history() { # <artifact>
    local artifact="$1" point
    tap_selector "History" "$artifact" || return 1
    wait_history_state "$artifact" history_toolbar_holds || return 1
    point="$(action_center "$artifact" "Clear search")"
    [[ -z "$point" ]] || sh_ input tap $point >/dev/null
    wait_history_state "$artifact" history_unfiltered_holds || return 1
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

# `open_storage` against a screen it cannot open, with adb stubbed out: the
# verdict has to name the step that failed and leave that step's screen behind
# under its own stage.
storage_stage_self_test() { # <temp>
    local temp="$1" verdict nav_open nav_starting
    nav_open='<node text="Primary" bounds="[0,570][320,640]"><node text="History" bounds="[17,583][113,635]" enabled="true" clickable="true"/><node text="Devices" bounds="[112,583][208,635]" enabled="true" clickable="true"/><node text="Settings" bounds="[207,583][303,635]" enabled="true" clickable="true"/></node>'
    nav_starting="${nav_open//enabled=\"true\" clickable/enabled=\"false\" clickable}"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_open<node text=\"0 items\" bounds=\"[12,126][52,142]\"/></node></hierarchy>" > "$temp/stuck-history.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_starting<node text=\"Loading…\" bounds=\"[29,405][291,434]\"/></node></hierarchy>" > "$temp/stuck-start.xml"
    # Settings open and the transfer actions never rendered, with Storage already
    # selected and not yet selected: the stage may only claim the transition when
    # it observed one, and `prepare_action`'s own dump is what it observes it in.
    local sections="<node text=\"Settings sections\" bounds=\"[12,73][308,223]\"/>"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_open$sections<node text=\"Storage\" bounds=\"[217,126][284,170]\" enabled=\"true\" clickable=\"true\" selected=\"true\"/></node></hierarchy>" > "$temp/storage-empty-pane.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_open$sections<node text=\"Storage\" bounds=\"[217,126][284,170]\" enabled=\"true\" clickable=\"true\" selected=\"false\"/></node></hierarchy>" > "$temp/storage-unselected.xml"

    verdict="$(
        OUT="$temp" WAIT_SECS=2
        sh_() { :; }
        dump_hierarchy() { cp "$temp/stuck-history.xml" "$1"; }
        PASS=0 FAIL=0
        open_storage seed-import
    )"
    [[ "$verdict" == *"FAIL  Settings opens at seed-import"* \
       && "$verdict" == *"actionable and not current"* \
       && "$verdict" != *"did not reach"* ]] \
        && ok "a stage that never opened names its step without blaming the tap" \
        || bad "a stage that never opened names its step without blaming the tap" "$verdict"

    verdict="$(
        OUT="$temp" WAIT_SECS=2
        sh_() { :; }
        dump_hierarchy() { cp "$temp/storage-empty-pane.xml" "$1"; }
        PASS=0 FAIL=0
        open_storage export
    )"
    [[ "$verdict" == *"FAIL  Storage exposes its transfer actions at export"* \
       && "$verdict" == *"already current before the tap"* \
       && "$verdict" != *"current now"* ]] \
        && ok "a stage whose tab was already current claims no transition" \
        || bad "a stage whose tab was already current claims no transition" "$verdict"

    # The three dumps `open_storage` takes before its Storage tap see the tab
    # unselected; every dump after it sees the pane it selected but never filled.
    verdict="$(
        OUT="$temp" WAIT_SECS=2
        sh_() { :; }
        taken=0
        dump_hierarchy() {
            taken=$((taken + 1))
            if (( taken <= 3 )); then cp "$temp/storage-unselected.xml" "$1"
            else cp "$temp/storage-empty-pane.xml" "$1"; fi
        }
        PASS=0 FAIL=0
        open_storage import
    )"
    [[ "$verdict" == *"FAIL  Storage exposes its transfer actions at import"* \
       && "$verdict" == *"was not current before the tap and is current now"* ]] \
        && ok "a stage that observed the tab change reports the transition" \
        || bad "a stage that observed the tab change reports the transition" "$verdict"

    verdict="$(
        OUT="$temp" WAIT_SECS=2
        sh_() { :; }
        dump_hierarchy() { cp "$temp/stuck-start.xml" "$1"; }
        PASS=0 FAIL=0
        open_storage rejected-import
    )"
    [[ "$verdict" == *"FAIL  the Settings tab is actionable at rejected-import"* \
       && "$verdict" == *"Settings=disabled"* ]] \
        && ok "an app that has not settled is reported as disabled navigation" \
        || bad "an app that has not settled is reported as disabled navigation" "$verdict"

    [[ -s "$temp/seed-import-settings-pane.xml" && -s "$temp/rejected-import-settings-nav.xml" ]] \
        && ! cmp -s "$temp/seed-import-settings-pane.xml" "$temp/rejected-import-settings-nav.xml" \
        && ok "each stage keeps its own screen instead of overwriting the last one's" \
        || bad "each stage keeps its own screen instead of overwriting the last one's"
}

self_test_transfer() {
    local temp CANARY="CopyPasteStorageTransferFixture"
    android_ui_self_test
    temp="$(mktemp -d)"
    android_navigation_self_test "$temp"
    storage_stage_self_test "$temp"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node content-desc="Search clipboard history" bounds="[0,0][200,40]"/><node content-desc="Clear search" clickable="true" bounds="[200,0][240,40]"/><node content-desc="Select multiple items" clickable="true" bounds="[240,0][280,40]"/><node text="0 items" bounds="[280,0][340,40]"/><node text="No results for &quot;fixture&quot;" bounds="[0,50][300,90]"/></hierarchy>' > "$temp/filtered.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node content-desc=\"Search clipboard history\" bounds=\"[0,0][200,40]\"/><node content-desc=\"Select multiple items\" clickable=\"true\" bounds=\"[240,0][280,40]\"/><node text=\"0 items\" bounds=\"[280,0][340,40]\"/><node text=\"$CANARY\" bounds=\"[0,50][300,90]\"/></hierarchy>" > "$temp/delayed.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node content-desc="Search clipboard history" bounds="[0,0][200,40]"/><node content-desc="Select multiple items" clickable="true" bounds="[240,0][280,40]"/><node text="0 items" bounds="[280,0][340,40]"/><node text="Nothing copied yet" bounds="[0,50][300,90]"/></hierarchy>' > "$temp/ready.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node content-desc="Search clipboard history" bounds="[0,0][200,40]"/><node content-desc="Select multiple items" clickable="true" bounds="[240,0][280,40]"/><node text="0 items" bounds="[280,0][340,40]"/><node text="Clipboard capture is paused" bounds="[0,50][300,90]"/></hierarchy>' > "$temp/paused.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node content-desc="Search clipboard history" bounds="[0,0][200,40]"/><node content-desc="Select multiple items" clickable="true" bounds="[240,0][280,40]"/><node text="0 items" bounds="[280,0][340,40]"/><node text="Waiting for the key store" bounds="[0,50][300,90]"/></hierarchy>' > "$temp/locked.xml"
    cleared_history_holds "$temp/filtered.xml" \
        && bad "a retained zero-result search cannot prove cleared history" \
        || ok "a retained zero-result search cannot prove cleared history"
    cleared_history_holds "$temp/delayed.xml" \
        && bad "a stale canary cannot prove unfiltered convergence" \
        || ok "a stale canary cannot prove unfiltered convergence"
    cleared_history_holds "$temp/ready.xml" \
        && ok "an unfiltered empty history without the canary is ready" \
        || bad "an unfiltered empty history without the canary is ready"
    cleared_history_holds "$temp/paused.xml" \
        && ok "a paused-capture empty history is also cleared" \
        || bad "a paused-capture empty history is also cleared"
    cleared_history_holds "$temp/locked.xml" \
        && bad "an unreadable history is not a cleared one" \
        || ok "an unreadable history is not a cleared one"
    grep -qF "invalid_request: \"CopyPaste $IMPORT_FAILURE_COPY" "$ERROR_CATALOGUE" \
        && ok "the rejected-import selector is the copy the app renders" \
        || bad "the rejected-import selector is the copy the app renders" \
               "$ERROR_CATALOGUE no longer spells errors.invalid_request that way"
    local canary_sample="CopyPasteStorageTransferT1786494647-R12345"
    ! grep -Eq '[0-9]([[:space:]-]?[0-9]){12,18}' <<<"$canary_sample" \
        && ok "the storage canary cannot look like a card number" \
        || bad "the storage canary cannot look like a card number" "$canary_sample"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="Storage" bounds="[0,0][60,44]" enabled="true" clickable="true" selected="true"/><node text="Import history" bounds="[24,70][140,90]" enabled="true"/><node text="Import…" bounds="[0,0][0,0]" enabled="true" clickable="true"/></hierarchy>' > "$temp/storage-title-only.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="Storage" bounds="[0,0][60,44]" enabled="true" clickable="true" selected="true"/><node text="Export…" bounds="[20,80][120,124]" enabled="true" clickable="true"/><node text="Import…" bounds="[20,140][120,184]" enabled="true" clickable="true"/></hierarchy>' > "$temp/storage-ready.xml"
    storage_transfer_actions_holds "$temp/storage-title-only.xml" \
        && bad "storage readiness requires actionable transfer buttons" \
        || ok "storage readiness requires actionable transfer buttons"
    storage_transfer_actions_holds "$temp/storage-ready.xml" \
        && ok "storage readiness accepts visible transfer actions" \
        || bad "storage readiness accepts visible transfer actions"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node text=\"Notifications alt+T\" bounds=\"[12,572][308,572]\"><node text=\"CopyPaste $IMPORT_FAILURE_COPY. Try again.\" bounds=\"[49,524][263,563]\"/></node></hierarchy>" > "$temp/rejected.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="Notifications alt+T" bounds="[12,572][308,572]"><node text="Imported 1 item" bounds="[49,524][263,563]"/></node></hierarchy>' > "$temp/accepted.xml"
    ui_fixtures "$temp/rejected.xml"
    wait_authored_feedback "$IMPORT_FAILURE_COPY" "$temp/observed.xml" 2 ui_fixture_dump \
        && ok "the authored rejection toast is observed" \
        || bad "the authored rejection toast is observed"
    ui_fixtures "$temp/accepted.xml" "$temp/accepted.xml"
    wait_authored_feedback "$IMPORT_FAILURE_COPY" "$temp/observed.xml" 2 ui_fixture_dump \
        && bad "a successful import cannot satisfy the rejection assertion" \
        || ok "a successful import cannot satisfy the rejection assertion"
    ui_fixtures "$temp/filtered.xml" "$temp/delayed.xml" "$temp/paused.xml"
    wait_cleared_history "$temp/observed.xml" 3 ui_fixture_dump \
        && [[ "$UI_FIXTURE_INDEX" == 3 ]] \
        && ok "clear readiness waits through retained search and delayed convergence" \
        || bad "clear readiness waits through retained search and delayed convergence"
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
adb logcat -c || true

group "Seed encrypted history"
sh_ mkdir -p /sdcard/Download >/dev/null
sh_ rm -f "/sdcard/Download/$SEED_FILE" "/sdcard/Download/$EXPORT_FILE" "/sdcard/Download/$INVALID_FILE" >/dev/null
python3 - "$CANARY" <<'PY' | adb_ shell dd "of=/sdcard/Download/$SEED_FILE" >/dev/null
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
restart_app seed-launch
if ! open_storage seed-import; then
    note "the seeded import through the picker" "Storage never opened at seed-import"
elif ! tap_selector "Import…" "$OUT/seed-import-action.xml"; then
    bad "Storage exposes import for the seed"
else
    sleep 2
    open_downloads seed-picker || bad "Downloads is selectable for the seed import"
    tap_selector "$SEED_FILE" "$OUT/seed-file.xml" 15 || bad "the seed export is selectable"
    prepare_action "Import" "$OUT/seed-import-confirm.xml" 20 || bad "the seed import requires confirmation"
    tap_prepared_action || bad "the seed import confirmation remains actionable"
    wait_authored_feedback "Imported" "$OUT/seed-import-toast.xml" 20 \
        && ok "seed import reports user-visible success" \
        || bad "seed import reports user-visible success" "no Imported toast appeared"
fi
open_history "$OUT/seed-history-nav.xml" \
    || bad "History is reachable before export" "$(navigation_state "$OUT/seed-history-nav.xml")"
wait_selector "$CANARY" "$OUT/seed-history.xml" 30 && ok "the canary is visible before export" || bad "the canary is visible before export" "uiautomator did not expose it"

group "Export through DocumentsUI"
if ! open_storage export; then
    note "the export through the picker" "Storage never opened at export"
elif ! tap_selector "Export…" "$OUT/export-action.xml"; then
    bad "Storage exposes the export action"
else
    tap_selector "Choose where to save" "$OUT/export-confirm.xml" || bad "the export confirmation is actionable"
    sleep 2
    open_downloads export-picker || bad "Downloads is selectable in the save picker"
    prepare_action "Save|action_menu_done" "$OUT/export-save.xml" 15 || bad "the picker exposes its save action"
    tap_prepared_action || bad "the picker save action remains actionable"
    if wait_authored_feedback "Exported" "$OUT/export-toast.xml" 20; then
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
fi

group "Clear and import through DocumentsUI"
tap_scrolling "Clear history" "$OUT/clear-action.xml" up || bad "Storage exposes the clear action"
prepare_action "Clear all" "$OUT/clear-confirm.xml" || bad "the clear confirmation is actionable"
tap_prepared_action || bad "the clear confirmation remains actionable"
wait_authored_feedback "Cleared" "$OUT/clear-toast.xml" 15 \
    && ok "clear reports user-visible success" \
    || bad "clear reports user-visible success" "no Cleared toast appeared"
open_history "$OUT/clear-history-nav.xml" \
    || bad "History is reachable after clearing" "$(navigation_state "$OUT/clear-history-nav.xml")"
if wait_cleared_history "$OUT/cleared-history.xml" 30; then
    ok "cleared unfiltered history settles without the exported canary"
else
    bad "cleared unfiltered history settles without the exported canary" "search state, empty history, total count, and canary absence did not converge together"
fi
if ! open_storage import; then
    note "the import through the picker" "Storage never opened at import"
else
    tap_scrolling "Import…" "$OUT/import-action.xml" down || bad "Storage exposes the import action"
    sleep 2
    open_downloads import-picker || bad "Downloads is selectable in the open picker"
    tap_selector "$EXPORT_FILE" "$OUT/import-file.xml" 15 || bad "the exported document is selectable"
    prepare_action "Import" "$OUT/import-confirm.xml" 20 || bad "the import preview requires confirmation"
    tap_prepared_action || bad "the import confirmation remains actionable"
    if wait_authored_feedback "Imported" "$OUT/import-toast.xml" 20; then
        ok "import reports user-visible success"
    else
        bad "import reports user-visible success" "no Imported toast appeared"
    fi
fi

group "Persisted ciphertext"
restart_app post-import-restart
transfer_pid="$(app_pid)"
open_history "$OUT/transfer-history-nav.xml" \
    || bad "History is reachable after restart" "$(navigation_state "$OUT/transfer-history-nav.xml")"
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
printf 'not-json\n' | adb_ shell dd "of=/sdcard/Download/$INVALID_FILE" >/dev/null
if ! open_storage rejected-import; then
    note "the rejected import through the picker" "Storage never opened at rejected-import"
else
    tap_scrolling "Import…" "$OUT/invalid-action.xml" up || bad "Storage exposes import for the failure case"
    sleep 2
    open_downloads invalid-picker || bad "Downloads is selectable for the failure case"
    prepare_action "$INVALID_FILE" "$OUT/invalid-file.xml" 15 || bad "the invalid document is selectable"
    tap_prepared_action || bad "the invalid document remains actionable"
    if wait_authored_feedback "$IMPORT_FAILURE_COPY" "$OUT/import-failure-toast.xml" 20; then
        ok "an invalid content URI reports a user-visible failure"
    else
        bad "an invalid content URI reports a user-visible failure" "the authored error toast did not appear"
    fi
    capture_screen import-failure
fi
open_history "$OUT/history-nav.xml" \
    || bad "History remains reachable after the rejected import" "$(navigation_state "$OUT/history-nav.xml")"
wait_selector "$CANARY" "$OUT/history-after-failure.xml" 20 && ok "a rejected import leaves persisted history intact" || bad "a rejected import leaves persisted history intact"

dump_logcat storage-transfer
crashes="$(crash_report "$OUT/storage-transfer.log" "$transfer_pid")"
[[ -z "$crashes" ]] && ok "no app crash occurred during storage transfer" || bad "no app crash occurred during storage transfer" "$(head -n 20 <<<"$crashes")"

printf '\n## Android storage transfer: %s\n\n%d assertions passed, %d failed.\n' "$([[ $FAIL -eq 0 ]] && echo passed || echo FAILED)" "$PASS" "$FAIL" | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
[[ $FAIL -eq 0 ]]
