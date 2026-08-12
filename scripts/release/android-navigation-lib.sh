#!/usr/bin/env bash
# The app's own primary navigation, read as a state rather than assumed.
#
# `Sidebar` disables every tab until `androidStartupSettled` (App.tsx), so an app
# that has not finished starting exposes three `enabled="false"` tabs and not a
# missing screen. Run 31634096676 tapped one anyway after `am force-stop`, waited
# 45 s for a pane that could not open, and reported "History is reachable after
# restart" for an app that was still fetching its history.
set -uo pipefail

NAVIGATION_TABS=(History Devices Settings)

app_navigation_holds() { # <artifact>
    local tab
    for tab in "${NAVIGATION_TABS[@]}"; do
        [[ -n "$(action_center "$1" "$tab")" ]] || return 1
    done
}

# actionable / disabled / absent, per tab. "Disabled" is an app that is still
# starting, "absent" is one whose shell never rendered, and the two need
# different next steps.
navigation_state() { # <artifact>
    local tab report=""
    for tab in "${NAVIGATION_TABS[@]}"; do
        if [[ -n "$(action_center "$1" "$tab")" ]]; then
            report+="$tab=actionable "
        elif [[ -n "$(node_center_rendered "$1" "$tab")" ]]; then
            report+="$tab=disabled "
        else
            report+="$tab=absent "
        fi
    done
    printf '%s' "${report% }"
}

wait_app_navigable() { # <artifact> [timeout] [dump function]
    local artifact="$1" timeout="${2:-${WAIT_SECS:-45}}" dump="${3:-dump_hierarchy}"
    local started="$SECONDS"
    while (( SECONDS - started < timeout )); do
        "$dump" "$artifact" && app_navigation_holds "$artifact" && return 0
        sleep 1
    done
    return 1
}

android_navigation_self_test() { # <temp>
    local temp="$1" nav_open nav_starting
    nav_open='<node text="Primary" bounds="[0,570][320,640]"><node text="History" bounds="[17,583][113,635]" enabled="true" clickable="true"/><node text="Devices" bounds="[112,583][208,635]" enabled="true" clickable="true"/><node text="Settings" bounds="[207,583][303,635]" enabled="true" clickable="true"/></node>'
    nav_starting='<node text="Primary" bounds="[0,570][320,640]"><node text="History" bounds="[17,583][113,635]" enabled="false" clickable="true"/><node text="Devices" bounds="[112,583][208,635]" enabled="false" clickable="true"/><node text="Settings" bounds="[207,583][303,635]" enabled="false" clickable="true"/></node>'
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_open</node></hierarchy>" > "$temp/navigable.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_starting<node text=\"Loading…\" bounds=\"[29,405][291,434]\" enabled=\"true\"/></node></hierarchy>" > "$temp/starting.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node><node text="Loading…" bounds="[29,405][291,434]" enabled="true"/></node></hierarchy>' > "$temp/shell-less.xml"

    app_navigation_holds "$temp/navigable.xml" \
        && ok "an actionable tab bar is navigable" \
        || bad "an actionable tab bar is navigable"
    app_navigation_holds "$temp/starting.xml" \
        && bad "a still-starting tab bar is not navigable" \
        || ok "a still-starting tab bar is not navigable"
    app_navigation_holds "$temp/shell-less.xml" \
        && bad "a missing tab bar is not navigable" \
        || ok "a missing tab bar is not navigable"

    [[ "$(navigation_state "$temp/starting.xml")" == "History=disabled Devices=disabled Settings=disabled" ]] \
        && ok "a still-starting shell reports disabled tabs" \
        || bad "a still-starting shell reports disabled tabs" "$(navigation_state "$temp/starting.xml")"
    [[ "$(navigation_state "$temp/shell-less.xml")" == "History=absent Devices=absent Settings=absent" ]] \
        && ok "a shell that never rendered reports absent tabs" \
        || bad "a shell that never rendered reports absent tabs" "$(navigation_state "$temp/shell-less.xml")"

    ui_fixtures "$temp/starting.xml" "$temp/starting.xml" "$temp/navigable.xml"
    wait_app_navigable "$temp/observed.xml" 8 ui_fixture_dump \
        && [[ "$UI_FIXTURE_INDEX" == 3 ]] \
        && ok "readiness waits through a start that has not settled" \
        || bad "readiness waits through a start that has not settled" "$UI_FIXTURE_INDEX samples"
    ui_fixtures "$temp/starting.xml" "$temp/starting.xml"
    wait_app_navigable "$temp/observed.xml" 2 ui_fixture_dump \
        && bad "an app that never settles is never navigable" \
        || ok "an app that never settles is never navigable"
}
