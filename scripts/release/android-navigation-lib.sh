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

# action_center stays package-blind for DocumentsUI. App tabs must not accept
# NexusLauncher Settings (run 34007760276 pairing-shell.xml).
app_owned_point() { # <xml> <selector> <"x y">
    python3 - "$1" "$2" "$3" "${PKG:-com.copypaste.app}" <<'PY'
import re
import sys
import xml.etree.ElementTree as ET

try:
    root = ET.parse(sys.argv[1]).getroot()
except (OSError, ET.ParseError):
    raise SystemExit(1)
selectors = [part.casefold() for part in sys.argv[2].split("|")]
x, y = map(int, sys.argv[3].split())
owned = sys.argv[4]
for node in root.iter("node"):
    values = [(node.get(name) or "").casefold()
              for name in ("text", "content-desc", "resource-id", "hint")]
    exact = any(selector == value or value.endswith("/" + selector)
                for selector in selectors for value in values if value)
    if not exact:
        continue
    bounds = re.fullmatch(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]", node.get("bounds") or "")
    if not bounds:
        continue
    left, top, right, bottom = map(int, bounds.groups())
    if (left + right) // 2 != x or (top + bottom) // 2 != y:
        continue
    package = node.get("package") or ""
    if package and package != owned:
        raise SystemExit(1)
    raise SystemExit(0)
raise SystemExit(1)
PY
}

app_owned_action_center() { # <xml> <selector>
    local point
    point="$(action_center "$1" "$2")"
    [[ -n "$point" ]] || return 1
    app_owned_point "$1" "$2" "$point" || return 1
    printf '%s\n' "$point"
}

app_navigation_holds() { # <artifact>
    local tab
    for tab in "${NAVIGATION_TABS[@]}"; do
        [[ -n "$(app_owned_action_center "$1" "$tab")" ]] || return 1
    done
}

settings_tab_holds() { # <artifact>
    [[ -n "$(app_owned_action_center "$1" "Settings")" ]]
}

hierarchy_package() { # <artifact>
    python3 - "$1" <<'PY'
import sys
import xml.etree.ElementTree as ET

try:
    root = ET.parse(sys.argv[1]).getroot()
except (OSError, ET.ParseError):
    raise SystemExit(0)
for node in root.iter("node"):
    package = node.get("package") or ""
    if package:
        print(package)
        break
PY
}

hierarchy_is_foreign() { # <artifact>
    local package
    package="$(hierarchy_package "$1")"
    [[ -n "$package" && "$package" != "${PKG:-com.copypaste.app}" ]]
}

hierarchy_is_app() { # <artifact>
    [[ "$(hierarchy_package "$1")" == "${PKG:-com.copypaste.app}" ]]
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
            (( SECONDS - started < timeout )) || return 1
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

wait_app_navigable() { # <artifact> [timeout] [dump] [scroll] [tap] [pace]
    tap_until_state "Explore first" "$1" app_navigation_holds up \
        "${2:-${WAIT_SECS:-45}}" "${3:-dump_hierarchy}" \
        "${4:-scroll_content}" "${5:-tap_transition_point}" "${6:-settle_pace}"
}

# Mid-content upward swipe inside the dump window, above Primary / the reserved
# nav band. Not scroll_content: that uses wm size and can leave the app.
onboarding_content_swipe() { # <artifact> -> "x y1 y2"
    python3 - "$1" <<'PY'
import re
import sys
import xml.etree.ElementTree as ET

try:
    root = ET.parse(sys.argv[1]).getroot()
except (OSError, ET.ParseError):
    raise SystemExit(1)
window = None
nav_top = None
for node in root.iter("node"):
    bounds = re.fullmatch(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]", node.get("bounds") or "")
    if not bounds:
        continue
    left, top, right, bottom = map(int, bounds.groups())
    if window is None and right > left and bottom > top:
        window = (left, top, right, bottom)
    if node.get("text") == "Primary":
        nav_top = top
if window is None:
    raise SystemExit(1)
left, top, right, bottom = window
width, height = right - left, bottom - top
if width < 32 or height < 32:
    raise SystemExit(1)
if nav_top is None:
    nav_top = bottom - max(80, height // 8)
content_top = top + 16
content_bottom = min(nav_top, bottom) - 16
if content_bottom - content_top < 32:
    raise SystemExit(1)
span = content_bottom - content_top
print((left + right) // 2, content_top + span * 2 // 3, content_top + span // 3)
PY
}

swipe_onboarding_content() { # <artifact>
    local x y1 y2
    read -r x y1 y2 <<<"$(onboarding_content_swipe "$1")"
    [[ "$x" =~ ^[0-9]+$ && "$y1" =~ ^[0-9]+$ && "$y2" =~ ^[0-9]+$ ]] || return 1
    sh_ input swipe "$x" "$y1" "$x" "$y2" 250 >/dev/null
}

android_welcome_holds() { # <artifact>
    hierarchy_is_app "$1" || return 1
    node_exists_exact "$1" "WELCOME|Explore first"
}

# Exact component, same bounded `am start -W` the launch path already uses.
android_relaunch_main_activity() {
    local pkg="${PKG:-com.copypaste.app}"
    local ns="${APP_NAMESPACE:-$pkg}"
    sh_ am start -W -n "${MAIN:-$pkg/$ns.MainActivity}" >/dev/null
}

android_app_shell_missing() {
    local pkg="${PKG:-com.copypaste.app}" pid focus
    pid="$(app_pid)"
    [[ -z "$pid" ]] && return 0
    focus="$(sh_ dumpsys window | grep -E 'mCurrentFocus|mFocusedApp' | head -n 4)"
    [[ -n "$focus" && "$focus" != *"$pkg"* ]]
}

android_shell_not_missing() { return 1; }

# Run 34029102311: FontsProvider killed the proven Library pid; settings
# then dumped NexusLauncher All Apps. One-shot handoff, never a foreign tap.
ANDROID_OWNED_SHELL_HANDOFF=0

# Run 34007760276: API 33 Welcome kept Explore first at [0,0][0,0];
# tap_until_state then called scroll_content and landed on NexusLauncher.
# Run 34016710899: GMS then dependency-killed the app; the next dump was
# NexusLauncher All Apps. Relaunch MainActivity once; never gesture foreign.
android_recover_onboarding() { # <artifact> [timeout] [dump] [swipe] [tap] [pace] [relaunch] [shell_missing]
    local artifact="$1" timeout="${2:-${WAIT_SECS:-30}}"
    local dump="${3:-dump_hierarchy}" swipe="${4:-swipe_onboarding_content}"
    local tap="${5:-tap_transition_point}" pace="${6:-settle_pace}"
    local relaunch="${7:-android_relaunch_main_activity}"
    local shell_missing="${8:-}"
    local point started="$SECONDS" saw_welcome=0 relaunched=0
    ANDROID_OWNED_SHELL_HANDOFF=0
    if [[ -z "$shell_missing" ]]; then
        if [[ "$dump" == dump_hierarchy ]]; then
            shell_missing=android_app_shell_missing
        else
            shell_missing=android_shell_not_missing
        fi
    fi
    while (( SECONDS - started < timeout )); do
        if "$dump" "$artifact"; then
            (( SECONDS - started < timeout )) || return 1
            if hierarchy_is_app "$artifact" && app_navigation_holds "$artifact"; then
                ANDROID_OWNED_SHELL_HANDOFF=1
                return 0
            fi
            if android_welcome_holds "$artifact"; then
                saw_welcome=1
            fi
            if hierarchy_is_foreign "$artifact" || "$shell_missing" "$artifact"; then
                if (( saw_welcome && relaunched == 0 )); then
                    "$relaunch" || return 1
                    relaunched=1
                    continue
                fi
                return 1
            fi
            point="$(action_center "$artifact" "Explore first")"
            if [[ -n "$point" ]]; then
                "$tap" "$point" || return 1
            elif enabled_action_exists_exact "$artifact" "Explore first"; then
                "$swipe" "$artifact" || return 1
            fi
        elif (( saw_welcome && relaunched == 0 )) && "$shell_missing" "$artifact"; then
            "$relaunch" || return 1
            relaunched=1
            continue
        fi
        "$pace"
    done
    return 1
}

reach_settings_tab() { # <artifact> [timeout]
    local artifact="$1" timeout="${2:-${WAIT_SECS:-45}}"
    local started="$SECONDS" relaunched=0 point
    while (( SECONDS - started < timeout )); do
        if dump_hierarchy "$artifact"; then
            (( SECONDS - started < timeout )) || return 1
            settings_tab_holds "$artifact" && return 0
            if hierarchy_is_foreign "$artifact" || android_app_shell_missing "$artifact"; then
                if (( ANDROID_OWNED_SHELL_HANDOFF != 0 && relaunched == 0 )); then
                    ANDROID_OWNED_SHELL_HANDOFF=0
                    android_relaunch_main_activity || return 1
                    relaunched=1
                    continue
                fi
                return 1
            fi
            point=""
            if enabled_action_exists_exact "$artifact" "Explore first"; then
                point="$(action_center "$artifact" "Explore first")"
            fi
            if [[ -n "$point" ]]; then
                tap_transition_point "$point" || return 1
            else
                scroll_content up
            fi
        elif (( ANDROID_OWNED_SHELL_HANDOFF != 0 && relaunched == 0 )) \
            && android_app_shell_missing "$artifact"; then
            ANDROID_OWNED_SHELL_HANDOFF=0
            android_relaunch_main_activity || return 1
            relaunched=1
            continue
        fi
        settle_pace
    done
    return 1
}

navigation_fixture_destination_holds() { # <artifact>
    enabled_node_exists_exact "$1" "Destination ready"
}

NAVIGATION_FIXTURE_DIRECTION=""
ONBOARDING_FIXTURE_SWIPES=0
ONBOARDING_FIXTURE_RELAUNCHES=0
ONBOARDING_FIXTURE_RELAUNCH_ARGV=""

navigation_fixture_scroll() {
    NAVIGATION_FIXTURE_DIRECTION="$1"
    ui_fixture_scroll
}

navigation_fixture_tap() { UI_FIXTURE_TAPS=$((UI_FIXTURE_TAPS + 1)); }

onboarding_fixture_swipe() { ONBOARDING_FIXTURE_SWIPES=$((ONBOARDING_FIXTURE_SWIPES + 1)); }

onboarding_fixture_missing_pid() { (( ONBOARDING_FIXTURE_RELAUNCHES == 0 )); }

onboarding_record_relaunch() {
    ONBOARDING_FIXTURE_RELAUNCHES=$((ONBOARDING_FIXTURE_RELAUNCHES + 1))
    ONBOARDING_FIXTURE_RELAUNCH_ARGV="$*"
}

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

    (
        navigation_fixture_late_dump() {
            ui_fixture_dump "$@"
            SECONDS=$((SECONDS + 2))
        }

        ui_fixtures "$temp/transition-ready.xml"
        ! tap_until_state "Open destination" "$temp/transition-late.xml" \
            navigation_fixture_destination_holds none 1 navigation_fixture_late_dump \
            navigation_fixture_scroll navigation_fixture_tap ui_fixture_pace \
            && cmp -s "$temp/transition-late.xml" "$temp/transition-ready.xml"
    ) \
        && ok "a dump completing after the deadline cannot prove a transition" \
        || bad "a dump completing after the deadline cannot prove a transition"
}

navigation_shell_readiness_self_test() { # <temp>
    local temp="$1" onboarding ready disabled absent zero covered observed
    onboarding='<node text="Explore first" bounds="[20,400][300,450]" enabled="true" clickable="true"/>'
    ready='<node text="Settings" bounds="[207,583][303,635]" enabled="true" clickable="true"/>'
    disabled='<node text="Settings" bounds="[207,583][303,635]" enabled="false" clickable="true"/>'
    absent='<node text="Loading…" bounds="[29,405][291,434]" enabled="true"/>'
    zero='<node text="Settings" bounds="[0,0][0,0]" enabled="true" clickable="true"/>'
    covered='<node text="Primary" bounds="[0,570][320,640]"><node text="Settings" bounds="[207,583][303,635]" enabled="true" clickable="true"/></node><node bounds="[0,570][320,640]"><node content-desc="Close toast" bounds="[280,580][310,610]" enabled="true" clickable="true"/></node>'
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$onboarding</hierarchy>" > "$temp/onboarding.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$ready</hierarchy>" > "$temp/settings-ready.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$disabled</hierarchy>" > "$temp/settings-disabled.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$absent</hierarchy>" > "$temp/settings-absent.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$zero</hierarchy>" > "$temp/settings-zero.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$covered</hierarchy>" > "$temp/settings-covered.xml"
    observed="$temp/settings-observed.xml"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        android_app_shell_missing() { return 1; }

        ui_fixtures "$temp/onboarding.xml" "$temp/settings-ready.xml"
        reach_settings_tab "$observed" 3 \
            && [[ $UI_FIXTURE_INDEX -eq 2 && $UI_FIXTURE_TAPS -eq 1 ]] \
            && cmp -s "$observed" "$temp/settings-ready.xml"
    ) \
        && ok "shell readiness rechecks Settings after Explore first" \
        || bad "shell readiness rechecks Settings after Explore first"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        android_app_shell_missing() { return 1; }

        ui_fixtures "$temp/settings-ready.xml"
        reach_settings_tab "$observed" 3 \
            && [[ $UI_FIXTURE_INDEX -eq 1 && $UI_FIXTURE_TAPS -eq 0 ]] \
            && cmp -s "$observed" "$temp/settings-ready.xml"
    ) \
        && ok "ready Settings does not tap onboarding again" \
        || bad "ready Settings does not tap onboarding again"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        android_app_shell_missing() { return 1; }

        ui_fixtures "$temp/settings-disabled.xml"
        ! reach_settings_tab "$observed" 1 \
            && [[ $UI_FIXTURE_TAPS -eq 0 ]] \
            && cmp -s "$observed" "$temp/settings-disabled.xml"
    ) \
        && ok "a disabled Settings tab fails closed" \
        || bad "a disabled Settings tab fails closed"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        android_app_shell_missing() { return 1; }

        ui_fixtures "$temp/settings-absent.xml"
        ! reach_settings_tab "$observed" 1 \
            && [[ $UI_FIXTURE_TAPS -eq 0 ]] \
            && cmp -s "$observed" "$temp/settings-absent.xml"
    ) \
        && ok "an absent Settings tab fails closed" \
        || bad "an absent Settings tab fails closed"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        android_app_shell_missing() { return 1; }

        ui_fixtures "$temp/settings-zero.xml"
        ! reach_settings_tab "$observed" 1 \
            && [[ $UI_FIXTURE_TAPS -eq 0 ]] \
            && cmp -s "$observed" "$temp/settings-zero.xml"
    ) \
        && ok "a zero-sized Settings tab fails closed" \
        || bad "a zero-sized Settings tab fails closed"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        android_app_shell_missing() { return 1; }

        ui_fixtures "$temp/settings-covered.xml"
        ! reach_settings_tab "$observed" 1 \
            && [[ $UI_FIXTURE_TAPS -eq 0 ]] \
            && cmp -s "$observed" "$temp/settings-covered.xml"
    ) \
        && ok "a toast-covered Settings tab fails closed" \
        || bad "a toast-covered Settings tab fails closed"
}

android_onboarding_recovery_self_test() { # <temp>
    local temp="$1" pkg="${PKG:-com.copypaste.app}"
    local main="${MAIN:-$pkg/${APP_NAMESPACE:-$pkg}.MainActivity}"
    local welcome_zero welcome_edge tappable shell settings_only launcher all_apps
    welcome_zero="<?xml version=\"1.0\"?><hierarchy><node package=\"$pkg\" bounds=\"[0,0][320,640]\"><node text=\"WELCOME\" bounds=\"[24,101][85,115]\" enabled=\"true\"/><node text=\"Explore first\" class=\"android.widget.Button\" package=\"$pkg\" enabled=\"true\" clickable=\"true\" bounds=\"[0,0][0,0]\"/></node></hierarchy>"
    welcome_edge="<?xml version=\"1.0\"?><hierarchy><node package=\"$pkg\" bounds=\"[0,0][320,640]\"><node text=\"WELCOME\" bounds=\"[24,101][85,115]\" enabled=\"true\"/><node text=\"Explore first\" class=\"android.widget.Button\" package=\"$pkg\" enabled=\"true\" clickable=\"true\" bounds=\"[24,616][296,616]\"/></node></hierarchy>"
    tappable="<?xml version=\"1.0\"?><hierarchy><node package=\"$pkg\" bounds=\"[0,0][320,640]\"><node text=\"Explore first\" class=\"android.widget.Button\" package=\"$pkg\" enabled=\"true\" clickable=\"true\" bounds=\"[20,400][300,450]\"/></node></hierarchy>"
    shell="<?xml version=\"1.0\"?><hierarchy><node package=\"$pkg\" bounds=\"[0,0][320,640]\"><node text=\"Primary\" bounds=\"[0,570][320,640]\"><node text=\"Library\" package=\"$pkg\" bounds=\"[17,583][113,635]\" enabled=\"true\" clickable=\"true\"/><node text=\"Devices\" package=\"$pkg\" bounds=\"[112,583][208,635]\" enabled=\"true\" clickable=\"true\"/><node text=\"Settings\" package=\"$pkg\" bounds=\"[207,583][303,635]\" enabled=\"true\" clickable=\"true\"/></node></node></hierarchy>"
    settings_only="<?xml version=\"1.0\"?><hierarchy><node package=\"$pkg\" bounds=\"[0,0][320,640]\"><node text=\"Settings\" package=\"$pkg\" bounds=\"[207,583][303,635]\" enabled=\"true\" clickable=\"true\"/></node></hierarchy>"
    launcher='<?xml version="1.0"?><hierarchy><node package="com.google.android.apps.nexuslauncher" bounds="[0,0][320,640]"><node text="Settings" package="com.google.android.apps.nexuslauncher" bounds="[247,464][305,579]" enabled="true" clickable="true"/></node></hierarchy>'
    all_apps='<?xml version="1.0"?><hierarchy><node package="com.google.android.apps.nexuslauncher" bounds="[0,0][320,640]"><node resource-id="com.google.android.apps.nexuslauncher:id/apps_view" package="com.google.android.apps.nexuslauncher" bounds="[0,0][320,640]"><node resource-id="com.google.android.apps.nexuslauncher:id/apps_list_view" package="com.google.android.apps.nexuslauncher" bounds="[0,64][320,640]"><node text="CopyPaste" resource-id="com.google.android.apps.nexuslauncher:id/icon" class="android.widget.TextView" package="com.google.android.apps.nexuslauncher" enabled="true" clickable="true" bounds="[15,349][73,464]"/><node text="Settings" resource-id="com.google.android.apps.nexuslauncher:id/icon" class="android.widget.TextView" package="com.google.android.apps.nexuslauncher" enabled="true" clickable="true" bounds="[247,464][305,579]"/></node><node resource-id="com.google.android.apps.nexuslauncher:id/all_apps_header" package="com.google.android.apps.nexuslauncher" bounds="[0,64][320,248]"/></node></node></hierarchy>'
    printf '%s\n' "$welcome_zero" > "$temp/welcome-zero.xml"
    printf '%s\n' "$welcome_edge" > "$temp/welcome-edge.xml"
    printf '%s\n' "$tappable" > "$temp/welcome-tappable.xml"
    printf '%s\n' "$shell" > "$temp/welcome-shell.xml"
    printf '%s\n' "$settings_only" > "$temp/welcome-settings-only.xml"
    printf '%s\n' "$launcher" > "$temp/welcome-launcher.xml"
    printf '%s\n' "$all_apps" > "$temp/welcome-all-apps.xml"

    [[ "$(onboarding_content_swipe "$temp/welcome-zero.xml")" == "160 368 192" ]] \
        && [[ "$(onboarding_content_swipe "$temp/welcome-edge.xml")" == "160 368 192" ]] \
        && ok "Welcome dumps use a contained mid-content swipe above the nav band" \
        || bad "Welcome dumps use a contained mid-content swipe above the nav band" \
               "$(onboarding_content_swipe "$temp/welcome-zero.xml")"
    [[ "$(onboarding_content_swipe "$temp/welcome-shell.xml")" == "160 374 195" ]] \
        && ok "a Primary band keeps the onboarding swipe above it" \
        || bad "a Primary band keeps the onboarding swipe above it" \
               "$(onboarding_content_swipe "$temp/welcome-shell.xml")"

    settings_tab_holds "$temp/welcome-launcher.xml" \
        && bad "NexusLauncher Settings is not an app Settings tab" \
        || ok "NexusLauncher Settings is not an app Settings tab"
    app_navigation_holds "$temp/welcome-launcher.xml" \
        && bad "NexusLauncher is not the app-owned shell" \
        || ok "NexusLauncher is not the app-owned shell"
    hierarchy_is_foreign "$temp/welcome-all-apps.xml" \
        && ! android_welcome_holds "$temp/welcome-all-apps.xml" \
        && ! app_navigation_holds "$temp/welcome-all-apps.xml" \
        && ok "NexusLauncher All Apps is a foreign dump, not Welcome" \
        || bad "NexusLauncher All Apps is a foreign dump, not Welcome"
    android_welcome_holds "$temp/welcome-zero.xml" \
        && ok "an app-owned Welcome-zero dump is Welcome" \
        || bad "an app-owned Welcome-zero dump is Welcome"
    settings_tab_holds "$temp/welcome-settings-only.xml" \
        && ! app_navigation_holds "$temp/welcome-settings-only.xml" \
        && ok "Settings-only is not full app navigation" \
        || bad "Settings-only is not full app navigation"
    app_navigation_holds "$temp/welcome-shell.xml" \
        && hierarchy_is_app "$temp/welcome-shell.xml" \
        && ok "an app-owned tab bar is the recovery destination" \
        || bad "an app-owned tab bar is the recovery destination"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }

        ui_fixtures "$temp/welcome-zero.xml" "$temp/welcome-tappable.xml" \
            "$temp/welcome-shell.xml"
        ONBOARDING_FIXTURE_SWIPES=0
        android_recover_onboarding "$temp/welcome-zero-observed.xml" 3 \
            ui_fixture_dump onboarding_fixture_swipe navigation_fixture_tap \
            ui_fixture_pace \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 1 && $UI_FIXTURE_TAPS -eq 1 \
                  && $UI_FIXTURE_SCROLLS -eq 0 ]] \
            && cmp -s "$temp/welcome-zero-observed.xml" "$temp/welcome-shell.xml"
    ) \
        && ok "API 33 Welcome zero bounds swipes contained then taps Explore first" \
        || bad "API 33 Welcome zero bounds swipes contained then taps Explore first"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }

        ui_fixtures "$temp/welcome-edge.xml" "$temp/welcome-tappable.xml" \
            "$temp/welcome-shell.xml"
        ONBOARDING_FIXTURE_SWIPES=0
        android_recover_onboarding "$temp/welcome-edge-observed.xml" 3 \
            ui_fixture_dump onboarding_fixture_swipe navigation_fixture_tap \
            ui_fixture_pace \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 1 && $UI_FIXTURE_TAPS -eq 1 \
                  && $UI_FIXTURE_SCROLLS -eq 0 ]] \
            && cmp -s "$temp/welcome-edge-observed.xml" "$temp/welcome-shell.xml"
    ) \
        && ok "API 36 Welcome edge zero height swipes contained then taps Explore first" \
        || bad "API 36 Welcome edge zero height swipes contained then taps Explore first"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }

        ui_fixtures "$temp/welcome-tappable.xml" "$temp/welcome-shell.xml"
        ONBOARDING_FIXTURE_SWIPES=0
        android_recover_onboarding "$temp/welcome-tappable-observed.xml" 3 \
            ui_fixture_dump onboarding_fixture_swipe navigation_fixture_tap \
            ui_fixture_pace \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 0 && $UI_FIXTURE_TAPS -eq 1 \
                  && $UI_FIXTURE_SCROLLS -eq 0 ]] \
            && cmp -s "$temp/welcome-tappable-observed.xml" "$temp/welcome-shell.xml"
    ) \
        && ok "a tappable Explore first is tapped without a swipe" \
        || bad "a tappable Explore first is tapped without a swipe"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }

        ui_fixtures "$temp/welcome-shell.xml"
        ONBOARDING_FIXTURE_SWIPES=0
        android_recover_onboarding "$temp/welcome-ready-observed.xml" 3 \
            ui_fixture_dump onboarding_fixture_swipe navigation_fixture_tap \
            ui_fixture_pace \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 0 && $UI_FIXTURE_TAPS -eq 0 \
                  && $UI_FIXTURE_SCROLLS -eq 0 && $UI_FIXTURE_INDEX -eq 1 \
                  && $ANDROID_OWNED_SHELL_HANDOFF -eq 1 ]] \
            && cmp -s "$temp/welcome-ready-observed.xml" "$temp/welcome-shell.xml"
    ) \
        && ok "an already-navigable app-owned shell returns without a gesture" \
        || bad "an already-navigable app-owned shell returns without a gesture"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        sh_() { onboarding_record_relaunch "$@"; }

        ui_fixtures "$temp/welcome-zero.xml" "$temp/welcome-all-apps.xml" \
            "$temp/welcome-shell.xml"
        ONBOARDING_FIXTURE_SWIPES=0
        ONBOARDING_FIXTURE_RELAUNCHES=0
        ONBOARDING_FIXTURE_RELAUNCH_ARGV=""
        android_recover_onboarding "$temp/welcome-kill-observed.xml" 3 \
            ui_fixture_dump onboarding_fixture_swipe navigation_fixture_tap \
            ui_fixture_pace android_relaunch_main_activity \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 1 && $UI_FIXTURE_TAPS -eq 0 \
                  && $UI_FIXTURE_SCROLLS -eq 0 \
                  && $ONBOARDING_FIXTURE_RELAUNCHES -eq 1 \
                  && "$ONBOARDING_FIXTURE_RELAUNCH_ARGV" == "am start -W -n $main" ]] \
            && cmp -s "$temp/welcome-kill-observed.xml" "$temp/welcome-shell.xml"
    ) \
        && ok "Welcome-zero then All Apps relaunches once onto the app shell" \
        || bad "Welcome-zero then All Apps relaunches once onto the app shell"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        sh_() { onboarding_record_relaunch "$@"; }

        ui_fixtures "$temp/welcome-zero.xml" "$temp/welcome-all-apps.xml" \
            "$temp/welcome-all-apps.xml"
        ONBOARDING_FIXTURE_SWIPES=0
        ONBOARDING_FIXTURE_RELAUNCHES=0
        ONBOARDING_FIXTURE_RELAUNCH_ARGV=""
        ! android_recover_onboarding "$temp/welcome-foreign-observed.xml" 3 \
            ui_fixture_dump onboarding_fixture_swipe navigation_fixture_tap \
            ui_fixture_pace android_relaunch_main_activity \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 1 && $UI_FIXTURE_TAPS -eq 0 \
                  && $UI_FIXTURE_SCROLLS -eq 0 \
                  && $ONBOARDING_FIXTURE_RELAUNCHES -eq 1 \
                  && "$ONBOARDING_FIXTURE_RELAUNCH_ARGV" == "am start -W -n $main" ]]
    ) \
        && ok "a second foreign dump after one MainActivity relaunch fails closed" \
        || bad "a second foreign dump after one MainActivity relaunch fails closed"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        sh_() { onboarding_record_relaunch "$@"; }

        ui_fixtures "$temp/welcome-zero.xml" "$temp/welcome-shell.xml"
        ONBOARDING_FIXTURE_SWIPES=0
        ONBOARDING_FIXTURE_RELAUNCHES=0
        ONBOARDING_FIXTURE_RELAUNCH_ARGV=""
        android_recover_onboarding "$temp/welcome-missing-pid-observed.xml" 3 \
            ui_fixture_dump onboarding_fixture_swipe navigation_fixture_tap \
            ui_fixture_pace android_relaunch_main_activity \
            onboarding_fixture_missing_pid \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 0 && $UI_FIXTURE_TAPS -eq 0 \
                  && $UI_FIXTURE_SCROLLS -eq 0 \
                  && $ONBOARDING_FIXTURE_RELAUNCHES -eq 1 \
                  && "$ONBOARDING_FIXTURE_RELAUNCH_ARGV" == "am start -W -n $main" ]] \
            && cmp -s "$temp/welcome-missing-pid-observed.xml" "$temp/welcome-shell.xml"
    ) \
        && ok "a missing app pid after Welcome relaunches once without a gesture" \
        || bad "a missing app pid after Welcome relaunches once without a gesture"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }

        ui_fixtures "$temp/welcome-settings-only.xml"
        ONBOARDING_FIXTURE_SWIPES=0
        ! android_recover_onboarding "$temp/welcome-settings-observed.xml" 1 \
            ui_fixture_dump onboarding_fixture_swipe navigation_fixture_tap \
            ui_fixture_pace \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 0 && $UI_FIXTURE_SCROLLS -eq 0 ]]
    ) \
        && ok "Settings-only never yields the app-owned shell" \
        || bad "Settings-only never yields the app-owned shell"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }

        ui_fixtures "$temp/welcome-tappable.xml" "$temp/welcome-tappable.xml" \
            "$temp/welcome-tappable.xml"
        ONBOARDING_FIXTURE_SWIPES=0
        ! android_recover_onboarding "$temp/welcome-explore-never.xml" 1 \
            ui_fixture_dump onboarding_fixture_swipe navigation_fixture_tap \
            ui_fixture_pace \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 0 && $UI_FIXTURE_TAPS -ge 1 \
                  && $UI_FIXTURE_SCROLLS -eq 0 ]]
    ) \
        && ok "Explore first that never yields the shell fails closed" \
        || bad "Explore first that never yields the shell fails closed"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }

        ui_fixtures "$temp/welcome-zero.xml" "$temp/welcome-zero.xml"
        ONBOARDING_FIXTURE_SWIPES=0
        ! android_recover_onboarding "$temp/welcome-never.xml" 1 \
            ui_fixture_dump onboarding_fixture_swipe navigation_fixture_tap \
            ui_fixture_pace \
            && [[ $ONBOARDING_FIXTURE_SWIPES -ge 1 && $UI_FIXTURE_SCROLLS -eq 0 \
                  && $ANDROID_OWNED_SHELL_HANDOFF -eq 0 ]]
    ) \
        && ok "a Welcome dump that never yields the shell fails after contained swipes" \
        || bad "a Welcome dump that never yields the shell fails after contained swipes"

    (
        onboarding_late_dump() {
            ui_fixture_dump "$@"
            SECONDS=$((SECONDS + 2))
        }

        ui_fixtures "$temp/welcome-shell.xml"
        ONBOARDING_FIXTURE_SWIPES=0
        ! android_recover_onboarding "$temp/welcome-late.xml" 1 \
            onboarding_late_dump onboarding_fixture_swipe navigation_fixture_tap \
            ui_fixture_pace \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 0 && $UI_FIXTURE_TAPS -eq 0 ]] \
            && cmp -s "$temp/welcome-late.xml" "$temp/welcome-shell.xml"
    ) \
        && ok "a dump completing after the deadline cannot prove onboarding recovery" \
        || bad "a dump completing after the deadline cannot prove onboarding recovery"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        android_app_shell_missing() { return 1; }
        sh_() { onboarding_record_relaunch "$@"; }

        ui_fixtures "$temp/welcome-tappable.xml" "$temp/welcome-shell.xml"
        ANDROID_OWNED_SHELL_HANDOFF=1
        ONBOARDING_FIXTURE_SWIPES=0
        ONBOARDING_FIXTURE_RELAUNCHES=0
        reach_settings_tab "$temp/handoff-explore-observed.xml" 3 \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 0 && $UI_FIXTURE_TAPS -eq 1 \
                  && $UI_FIXTURE_SCROLLS -eq 0 \
                  && $ONBOARDING_FIXTURE_RELAUNCHES -eq 0 \
                  && $ANDROID_OWNED_SHELL_HANDOFF -eq 1 ]] \
            && cmp -s "$temp/handoff-explore-observed.xml" "$temp/welcome-shell.xml"
    ) \
        && ok "an app-owned Explore first dump still taps through to Settings" \
        || bad "an app-owned Explore first dump still taps through to Settings"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        android_app_shell_missing() { return 1; }
        sh_() { onboarding_record_relaunch "$@"; }

        ui_fixtures "$temp/welcome-all-apps.xml" "$temp/welcome-shell.xml"
        ANDROID_OWNED_SHELL_HANDOFF=1
        ONBOARDING_FIXTURE_SWIPES=0
        ONBOARDING_FIXTURE_RELAUNCHES=0
        ONBOARDING_FIXTURE_RELAUNCH_ARGV=""
        reach_settings_tab "$temp/handoff-all-apps-observed.xml" 3 \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 0 && $UI_FIXTURE_TAPS -eq 0 \
                  && $UI_FIXTURE_SCROLLS -eq 0 \
                  && $ONBOARDING_FIXTURE_RELAUNCHES -eq 1 \
                  && "$ONBOARDING_FIXTURE_RELAUNCH_ARGV" == "am start -W -n $main" \
                  && $ANDROID_OWNED_SHELL_HANDOFF -eq 0 ]] \
            && cmp -s "$temp/handoff-all-apps-observed.xml" "$temp/welcome-shell.xml"
    ) \
        && ok "a proven-shell handoff relaunches once from All Apps onto Settings" \
        || bad "a proven-shell handoff relaunches once from All Apps onto Settings"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        android_app_shell_missing() { return 1; }
        sh_() { onboarding_record_relaunch "$@"; }

        ui_fixtures "$temp/welcome-all-apps.xml"
        ANDROID_OWNED_SHELL_HANDOFF=0
        ONBOARDING_FIXTURE_SWIPES=0
        ONBOARDING_FIXTURE_RELAUNCHES=0
        ! reach_settings_tab "$temp/handoff-token0-observed.xml" 3 \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 0 && $UI_FIXTURE_TAPS -eq 0 \
                  && $UI_FIXTURE_SCROLLS -eq 0 \
                  && $ONBOARDING_FIXTURE_RELAUNCHES -eq 0 ]]
    ) \
        && ok "a first foreign dump with no handoff token fails without a gesture" \
        || bad "a first foreign dump with no handoff token fails without a gesture"

    (
        dump_hierarchy() { cp "$temp/welcome-tappable.xml" "$1"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        sh_() { onboarding_record_relaunch "$@"; }
        android_app_shell_missing() { return 0; }

        ANDROID_OWNED_SHELL_HANDOFF=0
        ONBOARDING_FIXTURE_SWIPES=0
        ONBOARDING_FIXTURE_RELAUNCHES=0
        UI_FIXTURE_TAPS=0
        UI_FIXTURE_SCROLLS=0
        ! reach_settings_tab "$temp/handoff-token0-missing-observed.xml" 3 \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 0 && $UI_FIXTURE_TAPS -eq 0 \
                  && $UI_FIXTURE_SCROLLS -eq 0 \
                  && $ONBOARDING_FIXTURE_RELAUNCHES -eq 0 ]]
    ) \
        && ok "a missing shell with no handoff token fails without a gesture" \
        || bad "a missing shell with no handoff token fails without a gesture"

    (
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        android_app_shell_missing() { return 1; }
        sh_() { onboarding_record_relaunch "$@"; }

        ui_fixtures "$temp/welcome-all-apps.xml" "$temp/welcome-all-apps.xml"
        ANDROID_OWNED_SHELL_HANDOFF=1
        ONBOARDING_FIXTURE_SWIPES=0
        ONBOARDING_FIXTURE_RELAUNCHES=0
        ONBOARDING_FIXTURE_RELAUNCH_ARGV=""
        ! reach_settings_tab "$temp/handoff-second-foreign-observed.xml" 3 \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 0 && $UI_FIXTURE_TAPS -eq 0 \
                  && $UI_FIXTURE_SCROLLS -eq 0 \
                  && $ONBOARDING_FIXTURE_RELAUNCHES -eq 1 \
                  && "$ONBOARDING_FIXTURE_RELAUNCH_ARGV" == "am start -W -n $main" ]]
    ) \
        && ok "a second foreign dump after one settings relaunch fails closed" \
        || bad "a second foreign dump after one settings relaunch fails closed"

    (
        dump_hierarchy() {
            if (( ONBOARDING_FIXTURE_RELAUNCHES == 0 )); then
                return 1
            fi
            cp "$temp/welcome-shell.xml" "$1"
        }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        sh_() { onboarding_record_relaunch "$@"; }
        android_app_shell_missing() { onboarding_fixture_missing_pid; }

        ANDROID_OWNED_SHELL_HANDOFF=1
        ONBOARDING_FIXTURE_SWIPES=0
        ONBOARDING_FIXTURE_RELAUNCHES=0
        ONBOARDING_FIXTURE_RELAUNCH_ARGV=""
        UI_FIXTURE_TAPS=0
        UI_FIXTURE_SCROLLS=0
        reach_settings_tab "$temp/handoff-missing-pid-observed.xml" 3 \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 0 && $UI_FIXTURE_TAPS -eq 0 \
                  && $UI_FIXTURE_SCROLLS -eq 0 \
                  && $ONBOARDING_FIXTURE_RELAUNCHES -eq 1 \
                  && "$ONBOARDING_FIXTURE_RELAUNCH_ARGV" == "am start -W -n $main" ]] \
            && cmp -s "$temp/handoff-missing-pid-observed.xml" "$temp/welcome-shell.xml"
    ) \
        && ok "a missing app pid after a proven shell relaunches once onto Settings" \
        || bad "a missing app pid after a proven shell relaunches once onto Settings"

    (
        settings_handoff_late_dump() {
            ui_fixture_dump "$@"
            SECONDS=$((SECONDS + 2))
        }

        dump_hierarchy() { settings_handoff_late_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        sh_() { onboarding_record_relaunch "$@"; }

        ui_fixtures "$temp/welcome-shell.xml"
        ANDROID_OWNED_SHELL_HANDOFF=1
        ONBOARDING_FIXTURE_SWIPES=0
        ONBOARDING_FIXTURE_RELAUNCHES=0
        ! reach_settings_tab "$temp/handoff-late.xml" 1 \
            && [[ $ONBOARDING_FIXTURE_SWIPES -eq 0 && $UI_FIXTURE_TAPS -eq 0 \
                  && $UI_FIXTURE_SCROLLS -eq 0 \
                  && $ONBOARDING_FIXTURE_RELAUNCHES -eq 0 ]] \
            && cmp -s "$temp/handoff-late.xml" "$temp/welcome-shell.xml"
    ) \
        && ok "a dump completing after the deadline cannot prove Settings" \
        || bad "a dump completing after the deadline cannot prove Settings"
}

android_navigation_self_test() { # <temp>
    local temp="$1" nav_open nav_starting nav_onboarding
    nav_open='<node text="Primary" bounds="[0,570][320,640]"><node text="Library" bounds="[17,583][113,635]" enabled="true" clickable="true"/><node text="Devices" bounds="[112,583][208,635]" enabled="true" clickable="true"/><node text="Settings" bounds="[207,583][303,635]" enabled="true" clickable="true"/></node>'
    nav_starting='<node text="Primary" bounds="[0,570][320,640]"><node text="Library" bounds="[17,583][113,635]" enabled="false" clickable="true"/><node text="Devices" bounds="[112,583][208,635]" enabled="false" clickable="true"/><node text="Settings" bounds="[207,583][303,635]" enabled="false" clickable="true"/></node>'
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_open</node></hierarchy>" > "$temp/navigable.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_starting<node text=\"Loading…\" bounds=\"[29,405][291,434]\" enabled=\"true\"/></node></hierarchy>" > "$temp/starting.xml"
    printf '%s\n' '<?xml version="1.0"?><hierarchy><node><node text="Loading…" bounds="[29,405][291,434]" enabled="true"/></node></hierarchy>' > "$temp/shell-less.xml"
    nav_onboarding='<node text="Explore first" bounds="[20,400][300,450]" enabled="true" clickable="true"/>'
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$nav_onboarding</node></hierarchy>" > "$temp/onboarding.xml"

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
        navigation_fixture_scroll navigation_fixture_tap ui_fixture_pace \
        && [[ "$UI_FIXTURE_INDEX" == 3 ]] \
        && ok "readiness waits through a start that has not settled" \
        || bad "readiness waits through a start that has not settled" "$UI_FIXTURE_INDEX samples"
    ui_fixtures "$temp/onboarding.xml" "$temp/navigable.xml"
    wait_app_navigable "$temp/observed.xml" 3 ui_fixture_dump \
        navigation_fixture_scroll navigation_fixture_tap ui_fixture_pace \
        && [[ "$UI_FIXTURE_INDEX" == 2 && $UI_FIXTURE_TAPS -eq 1 ]] \
        && cmp -s "$temp/observed.xml" "$temp/navigable.xml" \
        && ok "readiness rechecks navigation after Explore first" \
        || bad "readiness rechecks navigation after Explore first"
    ui_fixtures "$temp/starting.xml" "$temp/starting.xml"
    wait_app_navigable "$temp/observed.xml" 2 ui_fixture_dump \
        navigation_fixture_scroll navigation_fixture_tap ui_fixture_pace \
        && bad "an app that never settles is never navigable" \
        || ok "an app that never settles is never navigable"
    navigation_transition_self_test "$temp"
    navigation_shell_readiness_self_test "$temp"
    android_onboarding_recovery_self_test "$temp"
}
