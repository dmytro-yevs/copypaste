#!/usr/bin/env bash
set -uo pipefail

selector_center() { # <xml> <selector alternatives separated by |> <any|exact|action|rendered|current>
    python3 - "$1" "$2" "$3" <<'PY'
import re
import sys
import xml.etree.ElementTree as ET

root = ET.parse(sys.argv[1]).getroot()
selectors = [part.casefold() for part in sys.argv[2].split("|")]
action = sys.argv[3] == "action"
exact_only = action or sys.argv[3] in ("exact", "rendered", "current")
# `rendered` is the one mode that keeps a disabled node. Every other mode drops
# it, so a control the app has painted but switched off is indistinguishable
# from one that was never laid out — and those need different next steps.
disabled_ok = sys.argv[3] == "rendered"
# `current` is the destination signal: Chromium maps `aria-selected` onto the
# accessibility `selected` flag, so the Settings tab strip says which pane the
# app is on. It does *not* map `aria-current`, so the primary tab bar never
# answers this and callers must not read a false negative as "the tap missed".
current_only = sys.argv[3] == "current"
primary = next((node for node in root.iter("node") if node.get("text") == "Primary"), None)
primary_nodes = {id(node) for node in primary.iter()} if primary is not None else set()
primary_bounds = re.fullmatch(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]", primary.get("bounds", "")) if primary is not None else None
primary_top = int(primary_bounds.group(2)) if primary_bounds else None

# Auto-dismiss pauses while a pointer is inside the toaster (06-ui-behaviour
# §3.7), and on a touch device a tap is a pointer that never leaves: one
# authored toast then sits over the next control for the rest of the run. The
# release gate tapped `Sign out` through the sync summary and read the
# unchanged pane as an app that never signed out. A covered control is not
# actionable; the toast's own close button still is.
parents = {id(child): node for node in root.iter("node") for child in node}
feedback = [parents[id(node)] for node in root.iter("node") if id(node) in parents
            and any(node.get(name, "") == "Close toast" for name in ("text", "content-desc"))]
feedback_nodes = {id(item) for parent in feedback for item in parent.iter()}
feedback_bounds = [re.fullmatch(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]", item.get("bounds", ""))
                   for parent in feedback for item in parent.iter()]
feedback_rects = [tuple(map(int, found.groups())) for found in feedback_bounds if found]

def covered(x, y):
    return any(left <= x <= right and top <= y <= bottom
               for left, top, right, bottom in feedback_rects)

candidates = []
for node in root.iter("node"):
    attrs = [node.get(name, "") for name in ("text", "content-desc", "resource-id", "hint")]
    values = [value.casefold() for value in attrs if value]
    exact = any(selector == value or value.endswith("/" + selector) for selector in selectors for value in values)
    partial = any(selector in value for selector in selectors for value in values)
    bounds = re.fullmatch(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]", node.get("bounds", ""))
    clickable = node.get("clickable", "false") == "true"
    documents_label = "documentsui" in node.get("package", "").casefold() and exact
    actionable = clickable or documents_label
    if bounds and (exact if exact_only else exact or partial) and (actionable or not action) and (disabled_ok or node.get("enabled", "true") != "false") and (not current_only or node.get("selected", "false") == "true"):
        points = tuple(map(int, bounds.groups()))
        left, top, right, bottom = points
        if right - left < 8 or bottom - top < 8:
            continue
        if primary_top is not None and id(node) not in primary_nodes and (top + bottom) // 2 >= primary_top:
            continue
        if action and id(node) not in feedback_nodes and covered((left + right) // 2, (top + bottom) // 2):
            continue
        candidates.append((not exact, not actionable, points, attrs))
if candidates:
    _, _, (left, top, right, bottom), _ = min(candidates)
    print((left + right) // 2, (top + bottom) // 2)
PY
}

node_center() { # <xml> <selector alternatives separated by |>
    selector_center "$1" "$2" any
}

node_center_exact() { # <xml> <selector alternatives separated by |>
    selector_center "$1" "$2" exact
}

action_center() { # <xml> <selector alternatives separated by |>
    selector_center "$1" "$2" action
}

node_center_rendered() { # <xml> <selector alternatives separated by |>
    selector_center "$1" "$2" rendered
}

node_center_current() { # <xml> <selector alternatives separated by |>
    selector_center "$1" "$2" current
}

wait_selector() { # <selector> <artifact> [timeout] [dump function]
    local selector="$1" artifact="$2" timeout="${3:-${WAIT_SECS:-45}}" dump="${4:-dump_hierarchy}"
    local started="$SECONDS"
    while (( SECONDS - started < timeout )); do
        if "$dump" "$artifact" && [[ -n "$(node_center "$artifact" "$selector")" ]]; then
            return 0
        fi
        sleep 1
    done
    return 1
}

# Chromium exposes a WebView toast in the accessibility *tree* while it is on
# screen and never as an accessibility *event*: it only generates web events
# while a real accessibility service is connected, and uiautomator's event
# stream is not one. Every authored storage toast was therefore invisible to the
# release gate while the app was reporting them correctly. Dump back to back so
# a 3 s toast cannot fall between two samples.
wait_authored_feedback() { # <selector> <artifact> [timeout] [dump function]
    local selector="$1" artifact="$2" timeout="${3:-${WAIT_SECS:-45}}" dump="${4:-dump_hierarchy}"
    local started="$SECONDS"
    while (( SECONDS - started < timeout )); do
        if "$dump" "$artifact"; then
            [[ -n "$(node_center "$artifact" "$selector")" ]] && return 0
        else
            sleep 1
        fi
    done
    return 1
}

# Only when a toast is what stands between a control and the tap: dismissing
# feedback the caller has not asserted yet would destroy the evidence its next
# assertion reads. Prints the authored close control's centre.
blocking_feedback_center() { # <artifact> <selector>
    [[ -n "$(node_center "$1" "$2")" ]] || return 1
    [[ -z "$(action_center "$1" "$2")" ]] || return 1
    action_center "$1" "Close toast"
}

dismiss_blocking_feedback() { # <artifact> <selector>
    local point
    point="$(blocking_feedback_center "$1" "$2")" || return 1
    [[ -n "$point" ]] || return 1
    sh_ input tap $point >/dev/null
}

# Best effort, and non-fatal by contract: no covering toast is the normal case,
# so a caller under `set -e` must be told "nothing was dismissed" and go on to
# scroll instead of dying here. Success means the target was covered and has
# just been uncovered, which the caller owes a fresh dump before any swipe —
# swiping now carries away the control it was about to tap. The settle covers
# the frame the toast needs to leave the tree, so the next sample is not the
# same covered one and the retry does not tap a close button that is gone.
dismiss_covering_feedback() { # <artifact> <selector> <any|action>
    [[ "$3" == action ]] || return 1
    dismiss_blocking_feedback "$1" "$2" || return 1
    sleep 1
}

PREPARED_ACTION_POINT=""

prepare_action() { # <selector> <artifact> [timeout]
    local selector="$1" artifact="$2" timeout="${3:-${WAIT_SECS:-45}}" point started="$SECONDS"
    PREPARED_ACTION_POINT=""
    while (( SECONDS - started < timeout )); do
        point=""
        if dump_hierarchy "$artifact"; then
            point="$(action_center "$artifact" "$selector")"
            [[ -n "$point" ]] || dismiss_blocking_feedback "$artifact" "$selector" || true
        fi
        [[ -n "$point" ]] && break
        sleep 1
    done
    [[ -n "$point" ]] || return 1
    PREPARED_ACTION_POINT="$point"
}

tap_prepared_action() {
    [[ -n "$PREPARED_ACTION_POINT" ]] || return 1
    sh_ input tap $PREPARED_ACTION_POINT >/dev/null
    PREPARED_ACTION_POINT=""
}

tap_selector() { # <selector> <artifact> [timeout]
    prepare_action "$@" && tap_prepared_action
}

screen_size() {
    sh_ wm size | sed -n 's/.* \([0-9][0-9]*\)x\([0-9][0-9]*\).*/\1 \2/p' | tail -n 1
}

scroll_content() { # <up|down>
    local width height
    read -r width height <<<"$(screen_size)"
    width="${width:-1080}"; height="${height:-1920}"
    if [[ "$1" == up ]]; then
        sh_ input swipe $((width / 2)) $((height * 3 / 4)) $((width / 2)) $((height / 3)) 250 >/dev/null
    else
        sh_ input swipe $((width / 2)) $((height / 3)) $((width / 2)) $((height * 3 / 4)) 250 >/dev/null
    fi
}

find_scrolling() { # <selector> <artifact> <up|down> [any|action] [dump fn] [scroll fn]
    local selector="$1" artifact="$2" direction="$3" mode="${4:-any}"
    local dump="${5:-dump_hierarchy}" scroll="${6:-scroll_content}"
    for _ in $(seq 1 8); do
        if "$dump" "$artifact"; then
            if [[ -n "$(selector_center "$artifact" "$selector" "$mode")" ]]; then
                return 0
            fi
            if dismiss_covering_feedback "$artifact" "$selector" "$mode"; then
                continue
            fi
        fi
        "$scroll" "$direction"
        sleep 1
    done
    return 1
}

# A settings pane is taller than a phone viewport, so a control that is only
# below the fold reads exactly like one that is missing. Bound by time rather
# than swipe count, because the callers are the ones under a latency budget.
wait_selector_scrolling() { # <selector> <artifact> <up|down> [timeout] [any|action] [dump fn] [scroll fn]
    local selector="$1" artifact="$2" direction="$3" timeout="${4:-${WAIT_SECS:-45}}"
    local mode="${5:-any}" dump="${6:-dump_hierarchy}" scroll="${7:-scroll_content}"
    local started="$SECONDS"
    while (( SECONDS - started < timeout )); do
        if "$dump" "$artifact"; then
            if [[ -n "$(selector_center "$artifact" "$selector" "$mode")" ]]; then
                return 0
            fi
            if dismiss_covering_feedback "$artifact" "$selector" "$mode"; then
                continue
            fi
        fi
        "$scroll" "$direction"
        sleep 1
    done
    return 1
}

tap_found_action() { # <selector> <artifact>
    local point
    point="$(action_center "$2" "$1")"
    [[ -n "$point" ]] || return 1
    sh_ input tap $point >/dev/null
}

tap_scrolling() { # <selector> <artifact> <up|down>
    find_scrolling "$@" action || return 1
    tap_found_action "$1" "$2"
}

tap_selector_scrolling() { # <selector> <artifact> <up|down> [timeout]
    wait_selector_scrolling "$1" "$2" "$3" "${4:-${WAIT_SECS:-45}}" action || return 1
    tap_found_action "$1" "$2"
}

capture_png() { # <path>
    local capture_serial=""
    capture_serial="$(android_serial_candidate)" || true
    capture_android_png "$1" "$PKG" "$capture_serial"
}

UI_FIXTURES=()
UI_FIXTURE_INDEX=0
UI_FIXTURE_SCROLLS=0
UI_FIXTURE_TAPS=0

ui_fixture_dump() { # <artifact>
    local source="${UI_FIXTURES[$UI_FIXTURE_INDEX]:-}"
    [[ -n "$source" ]] || return 1
    cp "$source" "$1"
    UI_FIXTURE_INDEX=$((UI_FIXTURE_INDEX + 1))
}

ui_fixture_scroll() { UI_FIXTURE_SCROLLS=$((UI_FIXTURE_SCROLLS + 1)); }

ui_fixtures() { # <artifact...>
    UI_FIXTURES=("$@")
    UI_FIXTURE_INDEX=0
    UI_FIXTURE_SCROLLS=0
    UI_FIXTURE_TAPS=0
}

UI_FIXTURE_COUNTS=""

# `set -e` here is the assertion, not the harness: a helper that aborts on a
# best-effort dismissal must show up as a case that never scrolled, not as a
# self-test that stops mid-file. So the run gets its own shell, and the counts
# come back through a file an aborted shell still writes from its EXIT trap.
# Errexit is also why nothing below may put the call in an `&&` list: bash
# exempts a whole function from `set -e` when it is called from one. The
# subshell scopes the stubbed tap too, so no case here reaches adb.
ui_fixture_run() { # <helper> <args...> -> "<found|missing> scrolls=<n> taps=<n>"
    local verdict
    : >"$UI_FIXTURE_COUNTS"
    verdict="$(
        set -e
        sh_() { UI_FIXTURE_TAPS=$((UI_FIXTURE_TAPS + 1)); }
        trap 'printf "scrolls=%s taps=%s" "$UI_FIXTURE_SCROLLS" "$UI_FIXTURE_TAPS" \
              >"$UI_FIXTURE_COUNTS"' EXIT
        "$@"
        printf 'found'
    )"
    printf '%s %s' "${verdict:-missing}" "$(cat "$UI_FIXTURE_COUNTS")"
}

android_ui_scroll_self_test() { # <temp>
    local temp="$1" nav='<node text="Primary" bounds="[0,570][320,640]"/>'
    # The release geometry this reproduces: Email cleared the tab bar, the field
    # under it did not, and the submit button had left the viewport entirely.
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav<node text=\"Email\" bounds=\"[24,514][296,558]\" enabled=\"true\" clickable=\"true\"/><node text=\"Password\" bounds=\"[24,566][296,610]\" enabled=\"true\" clickable=\"true\"/><node text=\"Sign in\" bounds=\"[0,0][0,0]\" enabled=\"true\" clickable=\"true\"/></node></hierarchy>" > "$temp/below-fold.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav<node text=\"Email\" bounds=\"[24,300][296,344]\" enabled=\"true\" clickable=\"true\"/><node text=\"Password\" bounds=\"[24,352][296,396]\" enabled=\"true\" clickable=\"true\"/><node text=\"Sign in\" bounds=\"[24,470][296,514]\" enabled=\"true\" clickable=\"true\"/></node></hierarchy>" > "$temp/scrolled.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav<node text=\"Imported 1 item\" bounds=\"[24,510][296,558]\" enabled=\"true\"/></node></hierarchy>" > "$temp/toast.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav<node text=\"Import history\" bounds=\"[24,100][296,144]\" enabled=\"true\"/></node></hierarchy>" > "$temp/no-toast.xml"

    ui_fixtures "$temp/below-fold.xml" "$temp/below-fold.xml"
    wait_selector "Sign in" "$temp/seen.xml" 2 ui_fixture_dump \
        && bad "a control below the fold is not reported as present" \
        || ok "a control below the fold is not reported as present"
    ui_fixtures "$temp/below-fold.xml" "$temp/below-fold.xml"
    wait_selector "Password" "$temp/seen.xml" 2 ui_fixture_dump \
        && bad "a field under the tab bar is not reported as present" \
        || ok "a field under the tab bar is not reported as present"
    ui_fixtures "$temp/below-fold.xml" "$temp/scrolled.xml"
    wait_selector_scrolling "Sign in" "$temp/seen.xml" up 5 action ui_fixture_dump ui_fixture_scroll \
        && (( UI_FIXTURE_SCROLLS == 1 )) \
        && ok "scrolling reaches a control that is only below the fold" \
        || bad "scrolling reaches a control that is only below the fold" "$UI_FIXTURE_SCROLLS swipes"
    # Nine samples inside six seconds: a wait that paused a second between
    # dumps runs out of budget here, and would miss a 3 s toast on a device for
    # the same reason.
    ui_fixtures "$temp/no-toast.xml" "$temp/no-toast.xml" "$temp/no-toast.xml" \
                "$temp/no-toast.xml" "$temp/no-toast.xml" "$temp/no-toast.xml" \
                "$temp/no-toast.xml" "$temp/no-toast.xml" "$temp/toast.xml"
    wait_authored_feedback "Imported" "$temp/seen.xml" 6 ui_fixture_dump \
        && ok "an authored toast is observed in the accessibility tree" \
        || bad "an authored toast is observed in the accessibility tree"
    ui_fixtures "$temp/no-toast.xml" "$temp/no-toast.xml"
    wait_authored_feedback "Imported" "$temp/seen.xml" 2 ui_fixture_dump \
        && bad "an absent authored toast is not reported" \
        || ok "an absent authored toast is not reported"
}

android_ui_feedback_self_test() { # <temp>
    local temp="$1" nav='<node text="Primary" bounds="[0,570][320,640]"/>'
    local row='<node text="Sign out" bounds="[200,538][296,582]" enabled="true" clickable="true"/>'
    # The release geometry this reproduces: the sync summary from the previous
    # action still on screen, its card covering the sign-out button's centre.
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav$row<node text=\"Notifications alt+T\" bounds=\"[12,572][308,572]\"><node text=\"Close toast Cloud sync finished\" bounds=\"[12,515][308,572]\"><node text=\"Close toast\" bounds=\"[275,533][295,554]\" enabled=\"true\" clickable=\"true\"/><node text=\"Cloud sync finished: 0 uploaded, 1 downloaded, 1 skipped\" bounds=\"[49,524][263,563]\"/></node></node></node></hierarchy>" > "$temp/covered.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav$row</node></hierarchy>" > "$temp/uncovered.xml"

    [[ -z "$(action_center "$temp/covered.xml" "Sign out")" ]] \
        && ok "an action under an authored toast is not tappable" \
        || bad "an action under an authored toast is not tappable"
    [[ -n "$(node_center "$temp/covered.xml" "Sign out")" ]] \
        && ok "a covered action is still reported as rendered" \
        || bad "a covered action is still reported as rendered"
    [[ "$(action_center "$temp/covered.xml" "Close toast")" == "285 543" ]] \
        && ok "the authored close control stays tappable inside the toast" \
        || bad "the authored close control stays tappable inside the toast" \
               "$(action_center "$temp/covered.xml" "Close toast")"
    [[ "$(blocking_feedback_center "$temp/covered.xml" "Sign out")" == "285 543" ]] \
        && ok "a blocked action resolves the toast to dismiss" \
        || bad "a blocked action resolves the toast to dismiss"
    [[ "$(action_center "$temp/uncovered.xml" "Sign out")" == "248 560" ]] \
        && ok "the same action is tappable once the toast is gone" \
        || bad "the same action is tappable once the toast is gone" \
               "$(action_center "$temp/uncovered.xml" "Sign out")"
    blocking_feedback_center "$temp/uncovered.xml" "Sign out" >/dev/null \
        && bad "a reachable action never dismisses authored feedback" \
        || ok "a reachable action never dismisses authored feedback"
    blocking_feedback_center "$temp/covered.xml" "Restore…" >/dev/null \
        && bad "an absent action never dismisses authored feedback" \
        || ok "an absent action never dismisses authored feedback"
}

android_ui_dismissal_scroll_self_test() { # <temp>
    local temp="$1" observed
    UI_FIXTURE_COUNTS="$temp/fixture-counts"
    local nav='<node text="Primary" bounds="[0,570][320,640]"/>'
    local row='<node text="Sign out" bounds="[200,538][296,582]" enabled="true" clickable="true"/>'
    local head='<node text="Notifications alt+T" bounds="[12,572][308,572]">' shut='</node>'
    local toast="<node text=\"Close toast Cloud sync finished\" bounds=\"[12,515][308,572]\"><node text=\"Close toast\" bounds=\"[275,533][295,554]\" enabled=\"true\" clickable=\"true\"/></node>"
    local stuck="<node text=\"Close toast Cloud sync finished\" bounds=\"[12,515][308,572]\"><node text=\"Close toast\" bounds=\"[275,533][295,554]\" enabled=\"false\" clickable=\"true\"/></node>"
    local other="<node text=\"Close toast Import finished\" bounds=\"[12,90][308,140]\"><node text=\"Close toast\" bounds=\"[20,100][40,121]\" enabled=\"true\" clickable=\"true\"/></node>"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav$row$head$toast$shut</node></hierarchy>" > "$temp/blocked.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav$row$head$stuck$shut</node></hierarchy>" > "$temp/stuck.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav$row$head$other$toast$shut</node></hierarchy>" > "$temp/two-toasts.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav$row</node></hierarchy>" > "$temp/cleared.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav<node text=\"Import history\" bounds=\"[24,100][296,144]\" enabled=\"true\"/></node></hierarchy>" > "$temp/gone.xml"

    ui_fixtures "$temp/blocked.xml" "$temp/cleared.xml"
    observed="$(ui_fixture_run wait_selector_scrolling "Sign out" "$temp/seen.xml" up 5 action \
        ui_fixture_dump ui_fixture_scroll)"
    [[ "$observed" == "found scrolls=0 taps=1" && "$(action_center "$temp/seen.xml" "Sign out")" == "248 560" ]] \
        && ok "a covered action is dismissed and read back without a swipe" \
        || bad "a covered action is dismissed and read back without a swipe" "$observed"
    ui_fixtures "$temp/blocked.xml" "$temp/cleared.xml"
    observed="$(ui_fixture_run find_scrolling "Sign out" "$temp/seen.xml" up action \
        ui_fixture_dump ui_fixture_scroll)"
    [[ "$observed" == "found scrolls=0 taps=1" && "$(action_center "$temp/seen.xml" "Sign out")" == "248 560" ]] \
        && ok "the counted search also samples the uncovered action first" \
        || bad "the counted search also samples the uncovered action first" "$observed"

    ui_fixtures "$temp/gone.xml" "$temp/cleared.xml"
    observed="$(ui_fixture_run wait_selector_scrolling "Sign out" "$temp/seen.xml" up 5 action \
        ui_fixture_dump ui_fixture_scroll)"
    [[ "$observed" == "found scrolls=1 taps=0" ]] \
        && ok "an offscreen action with no toast is still scrolled to" \
        || bad "an offscreen action with no toast is still scrolled to" "$observed"
    ui_fixtures "$temp/gone.xml" "$temp/cleared.xml"
    observed="$(ui_fixture_run find_scrolling "Sign out" "$temp/seen.xml" up action \
        ui_fixture_dump ui_fixture_scroll)"
    [[ "$observed" == "found scrolls=1 taps=0" ]] \
        && ok "the counted search scrolls when there is nothing to dismiss" \
        || bad "the counted search scrolls when there is nothing to dismiss" "$observed"

    ui_fixtures "$temp/stuck.xml" "$temp/cleared.xml"
    observed="$(ui_fixture_run find_scrolling "Sign out" "$temp/seen.xml" up action \
        ui_fixture_dump ui_fixture_scroll)"
    [[ "$observed" == "found scrolls=1 taps=0" ]] \
        && ok "a dismissal that fails does not short-circuit the search" \
        || bad "a dismissal that fails does not short-circuit the search" "$observed"
    ui_fixtures "$temp/blocked.xml" "$temp/blocked.xml"
    observed="$(ui_fixture_run find_scrolling "Sign out" "$temp/seen.xml" up any \
        ui_fixture_dump ui_fixture_scroll)"
    [[ "$observed" == "found scrolls=0 taps=0" ]] \
        && ok "observing a covered control dismisses nothing" \
        || bad "observing a covered control dismisses nothing" "$observed"
    ui_fixtures "$temp/gone.xml" "$temp/gone.xml"
    observed="$(ui_fixture_run wait_selector_scrolling "Sign out" "$temp/seen.xml" up 3 action \
        ui_fixture_dump ui_fixture_scroll)"
    [[ "$observed" == missing\ scrolls=[1-9]*\ taps=0 ]] \
        && ok "an action that is nowhere fails after scrolling for it" \
        || bad "an action that is nowhere fails after scrolling for it" "$observed"
    # One toast's close button is not the other's: dismissing the wrong one has
    # to be retried against a fresh dump, never paid for with a swipe.
    ui_fixtures "$temp/two-toasts.xml" "$temp/blocked.xml" "$temp/cleared.xml"
    observed="$(ui_fixture_run wait_selector_scrolling "Sign out" "$temp/seen.xml" up 8 action \
        ui_fixture_dump ui_fixture_scroll)"
    [[ "$observed" == "found scrolls=0 taps=2" ]] \
        && ok "a second toast over the action is dismissed without a swipe" \
        || bad "a second toast over the action is dismissed without a swipe" "$observed"
}

android_ui_self_test() {
    local temp point
    temp="$(mktemp -d)"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node text=""><node text="Primary" bounds="[0,240][320,300]"><node text="Devices" bounds="[110,245][210,295]" enabled="false" clickable="true"/><node text="Settings" bounds="[220,245][310,295]" enabled="true" clickable="true"/></node><node text="Clear history" bounds="[0,0][150,30]" enabled="true"/><node text="Clear history" bounds="[10,40][110,100]" enabled="true" clickable="true"/><node text="Export…" bounds="[10,110][110,170]" enabled="true" clickable="true"/><node content-desc="Save" resource-id="com.google.android.documentsui:id/action_menu_done" bounds="[200,40][300,100]" enabled="true" clickable="true"/><node text="copypaste-export.json" package="com.google.android.documentsui" bounds="[160,110][300,170]" enabled="true"/><node hint="Email" bounds="[10,180][190,230]" enabled="true" clickable="true"/><node text="Cloud sync" bounds="[10,238][190,240]" enabled="true"/><node text="Sign out" bounds="[10,220][190,270]" enabled="true" clickable="true"/><node text="Zero action" bounds="[0,0][0,0]" enabled="true" clickable="true"/></node></hierarchy>' > "$temp/ui.xml"
    point="$(action_center "$temp/ui.xml" "Export…")"
    [[ "$point" == "60 140" ]] && ok "an app label resolves to its tappable centre" || bad "an app label resolves to its tappable centre" "$point"
    [[ -z "$(node_center_exact "$temp/ui.xml" "Export")" ]] && ok "an exact selector rejects a partial label" || bad "an exact selector rejects a partial label"
    point="$(action_center "$temp/ui.xml" "Clear history")"
    [[ "$point" == "60 70" ]] && ok "a clickable action wins over its duplicate title" || bad "a clickable action wins over its duplicate title" "$point"
    point="$(action_center "$temp/ui.xml" "action_menu_done")"
    [[ "$point" == "250 70" ]] && ok "a localized picker action resolves by resource id" || bad "a localized picker action resolves by resource id" "$point"
    point="$(action_center "$temp/ui.xml" "Email")"
    [[ "$point" == "100 205" ]] && ok "a WebView input resolves by its accessibility hint" || bad "a WebView input resolves by its accessibility hint" "$point"
    [[ -z "$(node_center "$temp/ui.xml" "Cloud sync")" ]] && ok "a clipped semantic node is not visible" || bad "a clipped semantic node is not visible"
    [[ -z "$(action_center "$temp/ui.xml" "Sign out")" ]] && ok "an action obscured by app navigation is not actionable" || bad "an action obscured by app navigation is not actionable"
    [[ -z "$(action_center "$temp/ui.xml" "Zero action")" ]] && ok "a zero-sized action is not actionable" || bad "a zero-sized action is not actionable"
    point="$(action_center "$temp/ui.xml" "Settings")"
    [[ "$point" == "265 270" ]] && ok "an action inside app navigation remains actionable" || bad "an action inside app navigation remains actionable" "$point"
    [[ -z "$(action_center "$temp/ui.xml" "Devices")" ]] && ok "pending app navigation is not actionable" || bad "pending app navigation is not actionable"
    point="$(node_center_rendered "$temp/ui.xml" "Devices")"
    [[ "$point" == "160 270" ]] && ok "pending app navigation is still rendered" || bad "pending app navigation is still rendered" "$point"
    [[ -z "$(node_center_rendered "$temp/ui.xml" "Restore…")" ]] && ok "an absent control is not rendered either" || bad "an absent control is not rendered either"
    [[ -z "$(node_center_current "$temp/ui.xml" "Export…")" ]] && ok "a control with no selection flag is not current" || bad "a control with no selection flag is not current"
    point="$(action_center "$temp/ui.xml" "copypaste-export.json")"
    [[ "$point" == "230 140" ]] && ok "an exact DocumentsUI row label resolves its action" || bad "an exact DocumentsUI row label resolves its action" "$point"
    [[ -z "$(node_center "$temp/ui.xml" "Import history")" ]] && ok "a missing selector is not reported as present" || bad "a missing selector is not reported as present"
    android_ui_scroll_self_test "$temp"
    android_ui_feedback_self_test "$temp"
    android_ui_dismissal_scroll_self_test "$temp"
    android_screencap_self_test "$temp"
    android_adb_self_test
    rm -rf "$temp"
}
