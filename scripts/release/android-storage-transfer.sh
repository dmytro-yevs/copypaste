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
SETTINGS_SECTIONS="Settings sections"
STORAGE_SECTION_ACTION="Storage & history Stored items, cleanup, transfer and recovery"

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
    enabled_node_exists_exact "$1" "$SETTINGS_SECTIONS"
}

storage_transfer_actions_holds() { # <artifact>
    node_exists_exact "$1" "Storage & history" \
        && enabled_action_exists_exact "$1" "Export…" \
        && enabled_action_exists_exact "$1" "Import…"
}

tap_storage_import() { # <artifact>
    tap_scrolling "Import…" "$1" up
}

tap_storage_export() { # <artifact>
    tap_scrolling "Export…" "$1" up
}

# Four stages open this pane, and every one of them wrote the same three
# artifacts: run 31634096676 failed at the first stage and published the third
# stage's screens, so which step had failed was no longer in the evidence.
# Each step also names itself, because "Storage exposes import for the seed" was
# what a swallowed Settings tap reported.
#
# Each transition overwrites only its own stage artifact with a fresh hierarchy;
# a swallowed tap therefore leaves the exact source pane that failed to advance.
open_storage() { # <stage>
    local stage="$1" nav="$OUT/$1-settings-nav.xml"
    local ready="$OUT/$1-storage-ready.xml"
    tap_until_state "Settings" "$nav" settings_pane_holds none || {
        bad "Settings opens at $stage" "$(navigation_state "$nav")"
        return 1
    }
    tap_until_state "$STORAGE_SECTION_ACTION" "$ready" \
        storage_transfer_actions_holds up || {
        bad "Storage & history exposes its transfer actions at $stage" \
            "$(control_state "$ready" "$STORAGE_SECTION_ACTION"); $(navigation_state "$ready")"
        return 1
    }
}

history_toolbar_holds() { # <artifact>
    enabled_node_exists_exact "$1" "Search clipboard history, default|Search clipboard history, active"
}

history_unfiltered_holds() { # <artifact>
    enabled_node_exists_exact "$1" "Search clipboard history, default" \
        && ! node_exists_exact "$1" "Clear search"
}

history_item_count_holds() { # <artifact>
    python3 - "$1" <<'PY'
import re
import sys
import xml.etree.ElementTree as ET

try:
    root = ET.parse(sys.argv[1]).getroot()
except (OSError, ET.ParseError):
    raise SystemExit(1)
counts = []
for node in root.iter("node"):
    for name in ("text", "content-desc", "hint"):
        match = re.fullmatch(r"(\d+) items?", (node.get(name) or "").casefold())
        if match:
            counts.append(int(match.group(1)))
raise SystemExit(0 if not counts or all(count == 0 for count in counts) else 1)
PY
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
        && node_exists_exact "$1" "$EMPTY_HISTORY_TITLES" \
        && history_item_count_holds "$1" \
        && ! node_exists_exact "$1" "$CANARY"
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
    local artifact="$1"
    tap_until_state "Library" "$artifact" history_toolbar_holds none || return 1
    tap_until_state "Clear search" "$artifact" history_unfiltered_holds none
}

clear_confirmation_holds() { # <artifact>
    enabled_action_exists_exact "$1" "Clear all"
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

storage_transfer_summary() {
    printf '\n## Android storage transfer: %s\n\n%d assertions passed, %d failed.\n' \
        "$([[ $FAIL -eq 0 ]] && echo passed || echo FAILED)" "$PASS" "$FAIL" \
        | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
}

stop_storage_transfer() {
    storage_transfer_summary
    exit 1
}

# `open_storage` against a screen it cannot open, with adb stubbed out: the
# verdict has to name the step that failed and leave that step's screen behind
# under its own stage.
storage_stage_self_test() { # <temp>
    local temp="$1" verdict nav_open nav_starting
    nav_open='<node text="Primary" bounds="[0,570][320,640]"><node text="Library" bounds="[17,583][113,635]" enabled="true" clickable="true"/><node text="Devices" bounds="[112,583][208,635]" enabled="true" clickable="true"/><node text="Settings" bounds="[207,583][303,635]" enabled="true" clickable="true"/></node>'
    nav_starting="${nav_open//enabled=\"true\" clickable/enabled=\"false\" clickable}"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_open<node text=\"0 items\" bounds=\"[12,126][52,142]\"/></node></hierarchy>" > "$temp/stuck-history.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_starting<node text=\"Loading…\" bounds=\"[29,405][291,434]\"/></node></hierarchy>" > "$temp/stuck-start.xml"
    # Settings is open while the detail pane never renders. The final stage dump
    # must retain whether the card stayed unselected or became current.
    local sections="<node text=\"Settings sections\" bounds=\"[12,73][308,223]\" enabled=\"true\"/>"
    local storage_card="Storage &amp; history Stored items, cleanup, transfer and recovery"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_open$sections<node text=\"$storage_card\" bounds=\"[13,285][307,351]\" enabled=\"true\" clickable=\"true\" selected=\"true\"/></node></hierarchy>" > "$temp/storage-empty-pane.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_open$sections<node text=\"$storage_card\" bounds=\"[13,285][307,351]\" enabled=\"true\" clickable=\"true\" selected=\"false\"/></node></hierarchy>" > "$temp/storage-unselected.xml"

    verdict="$(
        OUT="$temp" WAIT_SECS=2
        sh_() { :; }
        dump_hierarchy() { cp "$temp/stuck-history.xml" "$1"; }
        PASS=0 FAIL=0
        open_storage seed-import
    )"
    [[ "$verdict" == *"FAIL  Settings opens at seed-import"* \
       && "$verdict" == *"Library=actionable Devices=actionable Settings=actionable"* ]] \
        && ok "a stage that never opened names its step without blaming the tap" \
        || bad "a stage that never opened names its step without blaming the tap" "$verdict"

    verdict="$(
        OUT="$temp" WAIT_SECS=2
        sh_() { :; }
        dump_hierarchy() { cp "$temp/storage-empty-pane.xml" "$1"; }
        PASS=0 FAIL=0
        open_storage export
    )"
    [[ "$verdict" == *"FAIL  Storage & history exposes its transfer actions at export"* \
       && "$verdict" == *"actionable; Library=actionable"* ]] \
        && ok "a current card without its detail pane is not ready" \
        || bad "a current card without its detail pane is not ready" "$verdict"

    # The three dumps before the storage tap see the card unselected; every dump
    # after it sees the section selected but never filled.
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
    if [[ "$verdict" == *"FAIL  Storage & history exposes its transfer actions at import"* \
          && -s "$temp/import-storage-ready.xml" ]] \
        && enabled_action_exists_exact "$temp/import-storage-ready.xml" "$STORAGE_SECTION_ACTION"; then
        ok "a failed detail transition retains its final stage artifact"
    else
        bad "a failed detail transition retains its final stage artifact" "$verdict"
    fi

    verdict="$(
        OUT="$temp" WAIT_SECS=2
        sh_() { :; }
        dump_hierarchy() { cp "$temp/stuck-start.xml" "$1"; }
        PASS=0 FAIL=0
        open_storage rejected-import
    )"
    [[ "$verdict" == *"FAIL  Settings opens at rejected-import"* \
       && "$verdict" == *"Settings=disabled"* ]] \
        && ok "an app that has not settled is reported as disabled navigation" \
        || bad "an app that has not settled is reported as disabled navigation" "$verdict"

    [[ -s "$temp/seed-import-settings-nav.xml" && -s "$temp/rejected-import-settings-nav.xml" ]] \
        && ! cmp -s "$temp/seed-import-settings-nav.xml" "$temp/rejected-import-settings-nav.xml" \
        && ok "each stage keeps its own screen instead of overwriting the last one's" \
        || bad "each stage keeps its own screen instead of overwriting the last one's"
}

storage_import_scroll_fixture_holds() { # <temp>
    (
        local temp="$1"
        ui_fixtures "$temp/storage-import-below-fold.xml" "$temp/storage-import-visible.xml"
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { [[ "$1" == up ]] && ui_fixture_scroll; }
        sh_() { UI_FIXTURE_TAPS=$((UI_FIXTURE_TAPS + 1)); }
        tap_storage_import "$temp/storage-import-observed.xml" \
            && [[ $UI_FIXTURE_SCROLLS -eq 1 && $UI_FIXTURE_TAPS -eq 1 ]]
    )
}

storage_export_scroll_fixture_holds() { # <temp>
    (
        local temp="$1"
        ui_fixtures "$temp/storage-export-covered.xml" "$temp/storage-export-visible.xml"
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() {
            [[ "$1" == up ]] || return 1
            ui_fixture_scroll
        }
        sh_() { UI_FIXTURE_TAPS=$((UI_FIXTURE_TAPS + 1)); }
        tap_storage_export "$temp/storage-export-observed.xml" \
            && [[ $UI_FIXTURE_SCROLLS -eq 1 && $UI_FIXTURE_TAPS -eq 1 ]]
    )
}

storage_export_uncovered_fixture_holds() { # <temp>
    (
        local temp="$1"
        ui_fixtures "$temp/storage-export-visible.xml"
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { ui_fixture_scroll; }
        sh_() { UI_FIXTURE_TAPS=$((UI_FIXTURE_TAPS + 1)); }
        tap_storage_export "$temp/storage-export-observed.xml" \
            && [[ $UI_FIXTURE_SCROLLS -eq 0 && $UI_FIXTURE_TAPS -eq 1 ]]
    )
}

storage_clear_scroll_fixture_holds() { # <temp>
    local temp="$1"
    ui_fixtures "$temp/storage-clear-above-fold.xml" \
        "$temp/storage-clear-visible.xml" "$temp/storage-clear-confirm.xml"
    NAVIGATION_FIXTURE_DIRECTION=""
    tap_until_state "Clear history" "$temp/storage-clear-observed.xml" \
        clear_confirmation_holds down 3 ui_fixture_dump navigation_fixture_scroll \
        navigation_fixture_tap ui_fixture_pace \
        && [[ "$NAVIGATION_FIXTURE_DIRECTION" == down \
              && $UI_FIXTURE_SCROLLS -eq 1 && $UI_FIXTURE_TAPS -eq 1 ]]
}

self_test_transfer() {
    local temp CANARY="CopyPasteStorageTransferFixture"
    android_ui_self_test
    temp="$(mktemp -d)"
    android_navigation_self_test "$temp"
    storage_stage_self_test "$temp"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node content-desc="Search clipboard history, active" bounds="[0,0][200,40]" enabled="true"/><node content-desc="Clear search" clickable="true" bounds="[200,0][240,40]" enabled="true"/><node text="0 items" bounds="[280,0][340,40]"/><node text="No results for &quot;fixture&quot;" bounds="[0,50][300,90]"/></hierarchy>' > "$temp/filtered.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node content-desc=\"Search clipboard history, default\" bounds=\"[0,0][200,40]\" enabled=\"true\"/><node text=\"0 items\" bounds=\"[15,141][16,143]\"/><node text=\"$CANARY\"/></hierarchy>" > "$temp/delayed.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node content-desc="Search clipboard history, default" bounds="[0,0][200,40]" enabled="true"/><node text="Nothing copied yet" bounds="[0,50][300,90]"/></hierarchy>' > "$temp/ready.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node content-desc="Search clipboard history, default" bounds="[0,0][200,40]" enabled="true"/><node text="0 items" bounds="[15,141][16,143]"/><node text="Clipboard capture is paused" bounds="[0,50][300,90]"/></hierarchy>' > "$temp/paused.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node content-desc="Search clipboard history, default" bounds="[0,0][200,40]" enabled="true"/><node text="3 items" bounds="[15,141][16,143]"/><node text="Clipboard capture is paused" bounds="[0,50][300,90]"/></hierarchy>' > "$temp/nonzero.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node content-desc="Search clipboard history, default" bounds="[0,0][200,40]" enabled="false"/><node text="Clipboard capture is paused" bounds="[0,50][300,90]"/></hierarchy>' > "$temp/search-disabled.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node content-desc="Search clipboard history, default" bounds="[0,0][200,40]"/><node text="Clipboard capture is paused" bounds="[0,50][300,90]"/></hierarchy>' > "$temp/search-missing-enabled.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node content-desc="Search clipboard history, default" bounds="[0,0][200,40]" enabled="true"/><node text="0 items" bounds="[280,0][340,40]"/><node text="Waiting for the key store" bounds="[0,50][300,90]"/></hierarchy>' > "$temp/locked.xml"
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
    cleared_history_holds "$temp/nonzero.xml" \
        && bad "a present nonzero count is not cleared history" \
        || ok "a present nonzero count is not cleared history"
    cleared_history_holds "$temp/search-disabled.xml" \
        && bad "a disabled default search is not settled history" \
        || ok "a disabled default search is not settled history"
    cleared_history_holds "$temp/search-missing-enabled.xml" \
        && bad "default search needs explicit enabled evidence" \
        || ok "default search needs explicit enabled evidence"
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
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="Storage &amp; history" bounds="[0,0][180,44]" enabled="true"/><node text="Import history" bounds="[24,70][140,90]" enabled="true"/><node text="Import…" bounds="[0,0][0,0]" enabled="true" clickable="true"/></hierarchy>' > "$temp/storage-title-only.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="Storage &amp; history" bounds="[12,12][308,640]" enabled="true"><node text="Export…" bounds="[192,533][291,578]" enabled="true" clickable="true"/><node text="Import…" enabled="true" clickable="true"/></node></hierarchy>' > "$temp/storage-ready.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="Storage &amp; history" bounds="[12,12][308,640]" enabled="true"><node text="Export…" bounds="[192,533][291,578]" enabled="true" clickable="true"/><node text="Import…" enabled="false" clickable="true"/></node></hierarchy>' > "$temp/storage-disabled.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="Storage &amp; history" bounds="[12,12][308,640]" enabled="true"><node text="Export…" bounds="[192,533][291,578]" enabled="true" clickable="true"/><node text="Import…" clickable="true"/></node></hierarchy>' > "$temp/storage-missing-enabled.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="CopyPaste" class="android.webkit.WebView" enabled="true" bounds="[0,0][320,640]"/></hierarchy>' > "$temp/blank-webview.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="Storage &amp; history" bounds="[12,12][308,640]" enabled="true"><node text="Primary" bounds="[49,548][271,604]" enabled="true"/><node text="Export…" bounds="[192,533][291,578]" enabled="true" clickable="true"/></node></hierarchy>' > "$temp/storage-export-covered.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="Storage &amp; history" bounds="[12,12][308,640]" enabled="true"><node text="Primary" bounds="[49,548][271,604]" enabled="true"/><node text="Export…" bounds="[192,420][291,465]" enabled="true" clickable="true"/></node></hierarchy>' > "$temp/storage-export-visible.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="Storage &amp; history" bounds="[12,12][308,640]" enabled="true"><node text="Export…" bounds="[192,533][291,578]" enabled="true" clickable="true"/><node text="Import…" enabled="true" clickable="true"/></node></hierarchy>' > "$temp/storage-import-below-fold.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="Storage &amp; history" bounds="[12,12][308,640]" enabled="true"><node text="Import…" bounds="[192,420][291,465]" enabled="true" clickable="true"/></node></hierarchy>' > "$temp/storage-import-visible.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="Storage &amp; history" bounds="[12,0][308,544]" enabled="true"><node text="Clear history" enabled="true" clickable="true"/><node text="Import…" bounds="[191,186][291,231]" enabled="true" clickable="true"/></node></hierarchy>' > "$temp/storage-clear-above-fold.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="Storage &amp; history" bounds="[12,0][308,544]" enabled="true"><node text="Clear history" bounds="[163,351][291,396]" enabled="true" clickable="true"/></node></hierarchy>' > "$temp/storage-clear-visible.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text="Clear all clipboard history?" bounds="[12,190][308,430]" enabled="true"><node text="Clear all" bounds="[180,360][290,410]" enabled="true" clickable="true"/></node></hierarchy>' > "$temp/storage-clear-confirm.xml"
    storage_transfer_actions_holds "$temp/storage-title-only.xml" \
        && bad "storage readiness requires actionable transfer buttons" \
        || ok "storage readiness requires actionable transfer buttons"
    storage_transfer_actions_holds "$temp/storage-ready.xml" \
        && ok "storage readiness accepts an enabled transfer action below the fold" \
        || bad "storage readiness accepts an enabled transfer action below the fold"
    storage_transfer_actions_holds "$temp/storage-disabled.xml" \
        && bad "storage readiness rejects a disabled transfer action" \
        || ok "storage readiness rejects a disabled transfer action"
    storage_transfer_actions_holds "$temp/storage-missing-enabled.xml" \
        && bad "storage readiness requires explicit enabled actions" \
        || ok "storage readiness requires explicit enabled actions"
    storage_transfer_actions_holds "$temp/blank-webview.xml" \
        && bad "a blank WebView is not a ready storage pane" \
        || ok "a blank WebView is not a ready storage pane"
    cleared_history_holds "$temp/blank-webview.xml" \
        && bad "a blank WebView is not a cleared history" \
        || ok "a blank WebView is not a cleared history"
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
    storage_import_scroll_fixture_holds "$temp" \
        && ok "seed import scrolls up and reacquires the below-fold action" \
        || bad "seed import scrolls up and reacquires the below-fold action"
    storage_import_scroll_fixture_holds "$temp" \
        && ok "post-clear import scrolls up and reacquires the below-fold action" \
        || bad "post-clear import scrolls up and reacquires the below-fold action"
    storage_export_scroll_fixture_holds "$temp" \
        && ok "export scrolls up once before tapping an action covered by the dock" \
        || bad "export scrolls up once before tapping an action covered by the dock"
    storage_export_uncovered_fixture_holds "$temp" \
        && ok "export taps an already uncovered action without scrolling" \
        || bad "export taps an already uncovered action without scrolling"
    storage_clear_scroll_fixture_holds "$temp" \
        && ok "clear history scrolls down to its above-viewport action" \
        || bad "clear history scrolls down to its above-viewport action"
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
# rm -f returns 0 even when files don't exist, so a non-zero exit means a real
# error: device offline, permission denied, or adb itself failed.  The old code
# swallowed this with `>/dev/null`, so a broken device continued into the
# picker-automation and failed much later with a confusing toast timeout.
cleanup_out="$(sh_ rm -f "/sdcard/Download/$SEED_FILE" "/sdcard/Download/$EXPORT_FILE" "/sdcard/Download/$INVALID_FILE" 2>&1)" || {
    bad "the transfer fixtures are cleared on the device" "${cleanup_out:-cleanup backend failure}"
    stop_storage_transfer
}
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
restart_app seed-launch || stop_storage_transfer
if ! open_storage seed-import; then
    note "the seeded import through the picker" "Storage never opened at seed-import"
    stop_storage_transfer
elif ! tap_storage_import "$OUT/seed-import-action.xml"; then
    bad "Storage exposes import for the seed"
    stop_storage_transfer
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
if ! open_history "$OUT/seed-history-nav.xml"; then
    bad "History is reachable before export" "$(navigation_state "$OUT/seed-history-nav.xml")"
    stop_storage_transfer
fi
wait_selector "$CANARY" "$OUT/seed-history.xml" 30 && ok "the canary is visible before export" || bad "the canary is visible before export" "uiautomator did not expose it"

group "Export through DocumentsUI"
if ! open_storage export; then
    note "the export through the picker" "Storage never opened at export"
    stop_storage_transfer
elif ! tap_storage_export "$OUT/export-action.xml"; then
    bad "Storage exposes the export action"
    stop_storage_transfer
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
if tap_until_state "Clear history" "$OUT/clear-confirm.xml" \
    clear_confirmation_holds down; then
    tap_found_action "Clear all" "$OUT/clear-confirm.xml" || {
        bad "the clear confirmation remains actionable"
        stop_storage_transfer
    }
else
    bad "the clear confirmation is actionable" \
        "Clear history did not open its confirmation"
    stop_storage_transfer
fi
wait_authored_feedback "Cleared" "$OUT/clear-toast.xml" 15 \
    && ok "clear reports user-visible success" \
    || bad "clear reports user-visible success" "no Cleared toast appeared"
if ! open_history "$OUT/clear-history-nav.xml"; then
    bad "History is reachable after clearing" "$(navigation_state "$OUT/clear-history-nav.xml")"
    stop_storage_transfer
fi
if wait_cleared_history "$OUT/cleared-history.xml" 30; then
    ok "cleared unfiltered history settles without the exported canary"
else
    bad "cleared unfiltered history settles without the exported canary" "search state, empty history, total count, and canary absence did not converge together"
fi
if ! open_storage import; then
    note "the import through the picker" "Storage never opened at import"
    stop_storage_transfer
else
    tap_storage_import "$OUT/import-action.xml" || {
        bad "Storage exposes the import action"
        stop_storage_transfer
    }
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
restart_app post-import-restart || stop_storage_transfer
transfer_pid="$(app_pid)"
if ! open_history "$OUT/transfer-history-nav.xml"; then
    bad "History is reachable after restart" "$(navigation_state "$OUT/transfer-history-nav.xml")"
    stop_storage_transfer
fi
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
    stop_storage_transfer
else
    tap_storage_import "$OUT/invalid-action.xml" || {
        bad "Storage exposes import for the failure case"
        stop_storage_transfer
    }
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
if ! open_history "$OUT/history-nav.xml"; then
    bad "History remains reachable after the rejected import" "$(navigation_state "$OUT/history-nav.xml")"
    stop_storage_transfer
fi
wait_selector "$CANARY" "$OUT/history-after-failure.xml" 20 && ok "a rejected import leaves persisted history intact" || bad "a rejected import leaves persisted history intact"

dump_logcat storage-transfer
crashes="$(crash_report "$OUT/storage-transfer.log" "$transfer_pid")"
[[ -z "$crashes" ]] && ok "no app crash occurred during storage transfer" || bad "no app crash occurred during storage transfer" "$(head -n 20 <<<"$crashes")"

storage_transfer_summary
[[ $FAIL -eq 0 ]]
