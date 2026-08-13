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

# Per tab: disabled is an app that is still starting, absent is one whose shell
# never rendered.
navigation_state() { # <artifact>
    local tab report=""
    for tab in "${NAVIGATION_TABS[@]}"; do
        report+="$tab=$(control_state "$1" "$tab") "
    done
    printf '%s' "${report% }"
}

# What the dumps say about a tap whose destination never rendered.
#
# Two states or none. A control that is current afterwards says which pane is
# selected, not that this tap selected it — it may have been current already,
# and a swallowed tap then reads exactly like a delivered one. Only a control
# observed not-current before and current after has a transition to report.
# Neither branch says the tap missed: every primary tab stays enabled and
# clickable after a successful switch (`Sidebar.tsx`), and Chromium never maps
# its `aria-current` onto `selected`, so those tabs answer nothing either way.
tap_landing() { # <post artifact> <control> [pre artifact]
    local post="$1" control="$2" pre="${3:-}" was_current="" is_current=""
    [[ -n "$(node_center_current "$post" "$control")" ]] && is_current=yes
    [[ -n "$pre" && -s "$pre" ]] || pre=""
    [[ -n "$pre" && -n "$(node_center_current "$pre" "$control")" ]] && was_current=yes

    if [[ "$is_current" == yes && -n "$pre" && "$was_current" != yes ]]; then
        printf '%s was not current before the tap and is current now; the pane did not render' "$control"
    elif [[ "$is_current" == yes && -n "$pre" ]]; then
        printf '%s was already current before the tap; the pane did not render' "$control"
    elif [[ "$is_current" == yes ]]; then
        printf '%s is current and the pane did not render' "$control"
    elif [[ -n "$(action_center "$post" "$control")" ]]; then
        printf '%s is actionable and not current; navigation was %s' \
            "$control" "$(navigation_state "$post")"
    else
        printf '%s is not on screen; navigation was %s' "$control" "$(navigation_state "$post")"
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

    # Screens where the destination marker is absent while the source control is
    # still actionable — the ordinary state after a tap that landed. What
    # separates the two cases is the state *before* the tap, so both are built.
    local sections='<node text="Settings sections" bounds="[12,73][308,223]"/><node text="Appearance" bounds="[17,78][110,122]" enabled="true" clickable="true" selected="false"/>'
    local storage_on='<node text="Storage" bounds="[217,126][284,170]" enabled="true" clickable="true" selected="true"/>'
    local storage_off='<node text="Storage" bounds="[217,126][284,170]" enabled="true" clickable="true" selected="false"/>'
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_open$sections$storage_on</node></hierarchy>" > "$temp/storage-current.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_open$sections$storage_off</node></hierarchy>" > "$temp/storage-not-current.xml"

    # Selected before the tap and selected after it: a swallowed tap reads
    # exactly like a delivered one, so neither may be claimed.
    [[ "$(tap_landing "$temp/storage-current.xml" Storage "$temp/storage-current.xml")" \
       == *"already current before the tap; the pane did not render"* ]] \
        && ok "a control current before its tap is not credited with a transition" \
        || bad "a control current before its tap is not credited with a transition" \
               "$(tap_landing "$temp/storage-current.xml" Storage "$temp/storage-current.xml")"
    [[ "$(tap_landing "$temp/storage-current.xml" Storage "$temp/storage-not-current.xml")" \
       == *"was not current before the tap and is current now"* ]] \
        && ok "a control that became current reports the transition" \
        || bad "a control that became current reports the transition" \
               "$(tap_landing "$temp/storage-current.xml" Storage "$temp/storage-not-current.xml")"
    [[ "$(tap_landing "$temp/storage-current.xml" Storage)" == *"is current and the pane did not render"* ]] \
        && ok "with no pre-state a current control is reported, not explained" \
        || bad "with no pre-state a current control is reported, not explained" \
               "$(tap_landing "$temp/storage-current.xml" Storage)"
    [[ "$(tap_landing "$temp/navigable.xml" Settings "$temp/navigable.xml")" != *"tap"* ]] \
        && ok "an always-actionable primary tab claims nothing about its tap" \
        || bad "an always-actionable primary tab claims nothing about its tap" \
               "$(tap_landing "$temp/navigable.xml" Settings "$temp/navigable.xml")"
    [[ "$(tap_landing "$temp/storage-current.xml" Appearance "$temp/storage-not-current.xml")" \
       != *"current now"* ]] \
        && ok "an unselected sibling tab is not credited with the tap" \
        || bad "an unselected sibling tab is not credited with the tap" \
               "$(tap_landing "$temp/storage-current.xml" Appearance "$temp/storage-not-current.xml")"
    [[ "$(tap_landing "$temp/shell-less.xml" Settings "$temp/navigable.xml")" == *"not on screen"* ]] \
        && ok "a control that is absent is reported as absent" \
        || bad "a control that is absent is reported as absent" \
               "$(tap_landing "$temp/shell-less.xml" Settings "$temp/navigable.xml")"

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
