#!/usr/bin/env bash
# The app's own primary navigation, read as a state rather than assumed.
#
# `Sidebar` disables every tab until `androidStartupSettled` (App.tsx), so an app
# that has not finished starting exposes three `enabled="false"` tabs and not a
# missing screen. Run 31634096676 tapped one anyway after `am force-stop`, waited
# 45 s for a pane that could not open, and reported "History is reachable after
# restart" for an app that was still fetching its history.
set -uo pipefail

NAVIGATION_TABS=(Library Devices Settings)

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

tap_transition_point() { # <"x y">
    local x y
    read -r x y <<<"$1"
    [[ -n "$x" && -n "$y" ]] || return 1
    sh_ input tap "$x" "$y" >/dev/null
}

# Run 33127930226 kept the source pane after an enabled navigation tap. Every
# retry is aimed from a fresh dump, and only the destination predicate returns.
tap_until_state() { # <selector> <artifact> <predicate> <none|up|down> [timeout] [dump] [scroll] [tap] [pace]
    local selector="$1" artifact="$2" predicate="$3" direction="$4"
    local timeout="${5:-${WAIT_SECS:-45}}" dump="${6:-dump_hierarchy}"
    local scroll="${7:-scroll_content}" tap="${8:-tap_transition_point}"
    local pace="${9:-settle_pace}" point started="$SECONDS"
    [[ "$direction" == none || "$direction" == up || "$direction" == down ]] || return 2
    while (( SECONDS - started < timeout )); do
        if "$dump" "$artifact"; then
            "$predicate" "$artifact" && return 0
            point=""
            if enabled_action_exists_exact "$artifact" "$selector"; then
                point="$(action_center "$artifact" "$selector")"
            fi
            if [[ -n "$point" ]]; then
                "$tap" "$point" || return 1
            elif [[ "$direction" != none ]]; then
                "$scroll" "$direction"
            fi
        fi
        "$pace"
    done
    return 1
}

wait_app_navigable() { # <artifact> [timeout] [dump function]
    local artifact="$1" timeout="${2:-${WAIT_SECS:-45}}" dump="${3:-dump_hierarchy}"
    local started="$SECONDS"
    while (( SECONDS - started < timeout )); do
        if "$dump" "$artifact"; then
            app_navigation_holds "$artifact" && return 0
            if [[ "$dump" == dump_hierarchy ]]; then
                tap_selector_scrolling "Explore first" "$artifact" up 8 || true
            fi
        fi
        sleep 1
    done
    return 1
}

navigation_fixture_destination_holds() { # <artifact>
    enabled_node_exists_exact "$1" "Destination ready"
}

NAVIGATION_FIXTURE_DIRECTION=""

navigation_fixture_scroll() {
    NAVIGATION_FIXTURE_DIRECTION="$1"
    ui_fixture_scroll
}

navigation_fixture_tap() { UI_FIXTURE_TAPS=$((UI_FIXTURE_TAPS + 1)); }

navigation_transition_self_test() { # <temp>
    local temp="$1" source target_above target_below destination
    source='<node text="Open destination" bounds="[220,500][300,550]" enabled="true" clickable="true"/>'
    target_above='<node text="Open destination" bounds="[0,0][0,0]" enabled="true" clickable="true"/>'
    target_below="$target_above"
    destination='<node text="Destination ready" bounds="[20,40][280,100]" enabled="true"/>'
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$source</hierarchy>" > "$temp/transition-source.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node text=\"Open destination\" bounds=\"[20,40][300,90]\" enabled=\"true\" clickable=\"true\"/></hierarchy>" > "$temp/transition-above-visible.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node text=\"Open destination\" bounds=\"[20,480][300,530]\" enabled=\"true\" clickable=\"true\"/></hierarchy>" > "$temp/transition-below-visible.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$target_above</hierarchy>" > "$temp/transition-above-hidden.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$target_below</hierarchy>" > "$temp/transition-below-hidden.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$destination</hierarchy>" > "$temp/transition-ready.xml"

    ui_fixtures "$temp/transition-source.xml" "$temp/transition-source.xml" "$temp/transition-ready.xml"
    tap_until_state "Open destination" "$temp/transition-observed.xml" \
        navigation_fixture_destination_holds none 3 ui_fixture_dump \
        navigation_fixture_scroll navigation_fixture_tap ui_fixture_pace \
        && [[ $UI_FIXTURE_INDEX -eq 3 && $UI_FIXTURE_TAPS -eq 2 ]] \
        && ok "a swallowed first tap is retried from a fresh source dump" \
        || bad "a swallowed first tap is retried from a fresh source dump" \
               "$UI_FIXTURE_INDEX samples, $UI_FIXTURE_TAPS taps"
    cmp -s "$temp/transition-observed.xml" "$temp/transition-ready.xml" \
        && ok "a stale source dump is never accepted as destination proof" \
        || bad "a stale source dump is never accepted as destination proof"

    ui_fixtures "$temp/transition-above-hidden.xml" "$temp/transition-above-visible.xml" "$temp/transition-ready.xml"
    NAVIGATION_FIXTURE_DIRECTION=""
    tap_until_state "Open destination" "$temp/transition-observed.xml" \
        navigation_fixture_destination_holds down 3 ui_fixture_dump \
        navigation_fixture_scroll navigation_fixture_tap ui_fixture_pace \
        && [[ "$NAVIGATION_FIXTURE_DIRECTION" == down && $UI_FIXTURE_SCROLLS -eq 1 ]] \
        && ok "an above-viewport action scrolls down before its verified transition" \
        || bad "an above-viewport action scrolls down before its verified transition"

    ui_fixtures "$temp/transition-below-hidden.xml" "$temp/transition-below-visible.xml" "$temp/transition-ready.xml"
    NAVIGATION_FIXTURE_DIRECTION=""
    tap_until_state "Open destination" "$temp/transition-observed.xml" \
        navigation_fixture_destination_holds up 3 ui_fixture_dump \
        navigation_fixture_scroll navigation_fixture_tap ui_fixture_pace \
        && [[ "$NAVIGATION_FIXTURE_DIRECTION" == up && $UI_FIXTURE_SCROLLS -eq 1 ]] \
        && ok "a below-viewport action scrolls up before its verified transition" \
        || bad "a below-viewport action scrolls up before its verified transition"

    ui_fixtures "$temp/transition-source.xml" "$temp/transition-source.xml"
    tap_until_state "Open destination" "$temp/transition-never.xml" \
        navigation_fixture_destination_holds none 1 ui_fixture_dump \
        navigation_fixture_scroll navigation_fixture_tap ui_fixture_pace \
        && bad "a transition that never renders cannot pass" \
        || ok "a transition that never renders cannot pass"
    cmp -s "$temp/transition-never.xml" "$temp/transition-source.xml" \
        && ok "a failed transition retains its last stage artifact" \
        || bad "a failed transition retains its last stage artifact"
}

android_navigation_self_test() { # <temp>
    local temp="$1" nav_open nav_starting
    nav_open='<node text="Primary" bounds="[0,570][320,640]"><node text="Library" bounds="[17,583][113,635]" enabled="true" clickable="true"/><node text="Devices" bounds="[112,583][208,635]" enabled="true" clickable="true"/><node text="Settings" bounds="[207,583][303,635]" enabled="true" clickable="true"/></node>'
    nav_starting='<node text="Primary" bounds="[0,570][320,640]"><node text="Library" bounds="[17,583][113,635]" enabled="false" clickable="true"/><node text="Devices" bounds="[112,583][208,635]" enabled="false" clickable="true"/><node text="Settings" bounds="[207,583][303,635]" enabled="false" clickable="true"/></node>'
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

    [[ "$(navigation_state "$temp/starting.xml")" == "Library=disabled Devices=disabled Settings=disabled" ]] \
        && ok "a still-starting shell reports disabled tabs" \
        || bad "a still-starting shell reports disabled tabs" "$(navigation_state "$temp/starting.xml")"
    [[ "$(navigation_state "$temp/shell-less.xml")" == "Library=absent Devices=absent Settings=absent" ]] \
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
    navigation_transition_self_test "$temp"
}
