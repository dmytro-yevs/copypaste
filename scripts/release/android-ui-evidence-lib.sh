#!/usr/bin/env bash
set -uo pipefail

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
    local selector="$1" artifact="$2" timeout="${3:-${WAIT_SECS:-45}}" started="$SECONDS"
    while (( SECONDS - started < timeout )); do
        if dump_hierarchy "$artifact" && [[ -n "$(node_center "$artifact" "$selector")" ]]; then
            return 0
        fi
        sleep 1
    done
    return 1
}

tap_selector() { # <selector> <artifact> [timeout]
    local selector="$1" artifact="$2" timeout="${3:-${WAIT_SECS:-45}}" point
    wait_selector "$selector" "$artifact" "$timeout" || return 1
    point="$(node_center "$artifact" "$selector")"
    sh_ input tap $point >/dev/null
}

screen_size() {
    sh_ wm size | sed -n 's/.* \([0-9][0-9]*\)x\([0-9][0-9]*\).*/\1 \2/p' | tail -n 1
}

find_scrolling() { # <selector> <artifact> <up|down>
    local selector="$1" artifact="$2" direction="$3" width height point
    read -r width height <<<"$(screen_size)"
    width="${width:-1080}"; height="${height:-1920}"
    for _ in $(seq 1 8); do
        dump_hierarchy "$artifact" || true
        point="$(node_center "$artifact" "$selector")"
        [[ -n "$point" ]] && return 0
        if [[ "$direction" == up ]]; then
            sh_ input swipe $((width / 2)) $((height * 3 / 4)) $((width / 2)) $((height / 3)) 250 >/dev/null
        else
            sh_ input swipe $((width / 2)) $((height / 3)) $((width / 2)) $((height * 3 / 4)) 250 >/dev/null
        fi
        sleep 1
    done
    return 1
}

tap_scrolling() { # <selector> <artifact> <up|down>
    find_scrolling "$@" || return 1
    local point
    point="$(node_center "$2" "$1")"
    sh_ input tap $point >/dev/null
}

capture_png() { # <path>
    adb exec-out screencap -p > "$1" 2>/dev/null || true
}

android_ui_self_test() {
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
}
