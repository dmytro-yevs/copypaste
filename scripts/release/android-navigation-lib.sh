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

# What the dump says about a tap whose destination never rendered.
#
# Never "the tap missed the app": every primary tab stays enabled and clickable
# after a successful switch (`Sidebar.tsx`), and Chromium does not map its
# `aria-current` onto the accessibility `selected` flag — in run 31634096676 all
# three tabs read `selected="false"` on a Settings screen. So a still-actionable
# source control is evidence of nothing, and claiming otherwise turns a pane that
# rendered late into a delivery failure. A control that does carry `selected` —
# the Settings tab strip does — is evidence, and only that is stated as a cause.
tap_landing() { # <artifact> <control>
    if [[ -n "$(node_center_current "$1" "$2")" ]]; then
        printf '%s is the current selection, so the tap landed and the pane did not render' "$2"
    elif [[ -n "$(action_center "$1" "$2")" ]]; then
        printf '%s is actionable and not marked current; navigation was %s' \
            "$2" "$(navigation_state "$1")"
    else
        printf '%s is not on screen; navigation was %s' "$2" "$(navigation_state "$1")"
    fi
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

    # The decisive pair. Both are screens where the destination marker is absent
    # while the source control is still actionable, which is the ordinary state
    # after a tap that *did* land: neither may be reported as a missed tap.
    local strip='<node text="Appearance" bounds="[17,78][110,122]" enabled="true" clickable="true" selected="false"/><node text="Storage" bounds="[217,126][284,170]" enabled="true" clickable="true" selected="true"/>'
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_open<node text=\"Settings sections\" bounds=\"[12,73][308,223]\"/>$strip</node></hierarchy>" > "$temp/storage-current.xml"

    [[ "$(tap_landing "$temp/storage-current.xml" Storage)" == *"current selection, so the tap landed"* ]] \
        && ok "a current tab whose pane is incomplete is reported as a landed tap" \
        || bad "a current tab whose pane is incomplete is reported as a landed tap" \
               "$(tap_landing "$temp/storage-current.xml" Storage)"
    [[ "$(tap_landing "$temp/navigable.xml" Settings)" != *"tap"* ]] \
        && ok "an always-actionable primary tab claims nothing about its tap" \
        || bad "an always-actionable primary tab claims nothing about its tap" \
               "$(tap_landing "$temp/navigable.xml" Settings)"
    [[ "$(tap_landing "$temp/storage-current.xml" Appearance)" != *"tap landed"* ]] \
        && ok "an unselected sibling tab is not credited with the tap" \
        || bad "an unselected sibling tab is not credited with the tap" \
               "$(tap_landing "$temp/storage-current.xml" Appearance)"
    [[ "$(tap_landing "$temp/shell-less.xml" Settings)" == *"not on screen"* ]] \
        && ok "a control that is absent is reported as absent" \
        || bad "a control that is absent is reported as absent" \
               "$(tap_landing "$temp/shell-less.xml" Settings)"

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
