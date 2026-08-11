#!/usr/bin/env bash
set -uo pipefail

selector_center() { # <xml> <selector alternatives separated by |> <any|exact|action>
    python3 - "$1" "$2" "$3" <<'PY'
import re
import sys
import xml.etree.ElementTree as ET

root = ET.parse(sys.argv[1]).getroot()
selectors = [part.casefold() for part in sys.argv[2].split("|")]
action = sys.argv[3] == "action"
exact_only = action or sys.argv[3] == "exact"
primary = next((node for node in root.iter("node") if node.get("text") == "Primary"), None)
primary_nodes = {id(node) for node in primary.iter()} if primary is not None else set()
primary_bounds = re.fullmatch(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]", primary.get("bounds", "")) if primary is not None else None
primary_top = int(primary_bounds.group(2)) if primary_bounds else None
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
    if bounds and (exact if exact_only else exact or partial) and (actionable or not action) and node.get("enabled", "true") != "false":
        points = tuple(map(int, bounds.groups()))
        left, top, right, bottom = points
        if right - left < 8 or bottom - top < 8:
            continue
        if primary_top is not None and id(node) not in primary_nodes and (top + bottom) // 2 >= primary_top:
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

PREPARED_ACTION_POINT=""

prepare_action() { # <selector> <artifact> [timeout]
    local selector="$1" artifact="$2" timeout="${3:-${WAIT_SECS:-45}}" point started="$SECONDS"
    PREPARED_ACTION_POINT=""
    while (( SECONDS - started < timeout )); do
        point=""
        if dump_hierarchy "$artifact"; then
            point="$(action_center "$artifact" "$selector")"
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
        if "$dump" "$artifact" && [[ -n "$(selector_center "$artifact" "$selector" "$mode")" ]]; then
            return 0
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
        if "$dump" "$artifact" && [[ -n "$(selector_center "$artifact" "$selector" "$mode")" ]]; then
            return 0
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
    point="$(action_center "$temp/ui.xml" "copypaste-export.json")"
    [[ "$point" == "230 140" ]] && ok "an exact DocumentsUI row label resolves its action" || bad "an exact DocumentsUI row label resolves its action" "$point"
    [[ -z "$(node_center "$temp/ui.xml" "Import history")" ]] && ok "a missing selector is not reported as present" || bad "a missing selector is not reported as present"
    android_ui_scroll_self_test "$temp"
    android_screencap_self_test "$temp"
    android_adb_self_test
    rm -rf "$temp"
}
