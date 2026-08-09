#!/usr/bin/env bash
set -uo pipefail

mac_evidence_executable() { # <app bundle>
    macos_bundle_executable_path "$1"
}

mac_stop_executable() { # <bundle executable>
    local name="${1##*/}" started="$SECONDS"
    pkill -x "$name" 2>/dev/null || true
    while pgrep -x "$name" >/dev/null 2>&1; do
        (( SECONDS - started < 10 )) || return 1
        sleep 0.1
    done
}

mac_wait_executable_pid() { # <bundle executable> [timeout]
    local name="${1##*/}" timeout="${2:-30}" started="$SECONDS" pids
    while (( SECONDS - started < timeout )); do
        pids="$(pgrep -x "$name" 2>/dev/null || true)"
        if [[ "$pids" =~ ^[0-9]+$ ]]; then
            printf '%s\n' "$pids"
            return 0
        fi
        sleep 0.1
    done
    return 1
}

mac_set_app_pid() { # <pid>
    MAC_APP_PID="$1"
}

mac_ax() { # <ready|surface|dump|find|press|set> [label] [value]
    [[ -n "${MAC_APP_PID:-}" ]] || {
        echo "macOS accessibility target PID is unavailable" >&2
        return 1
    }
    osascript - "$MAC_APP_PID" "$@" <<'APPLESCRIPT'
on run argv
    set appPid to item 1 of argv as integer
    set actionMode to item 2 of argv
    set targetLabel to ""
    set inputValue to ""
    if (count of argv) > 2 then set targetLabel to item 3 of argv
    if (count of argv) > 3 then set inputValue to item 4 of argv
    set outputLines to {}
    tell application "System Events"
        set processMatches to every process whose unix id is appPid
        if (count of processMatches) is not 1 then error "CopyPaste process is unavailable"
        set processRef to item 1 of processMatches
        tell processRef
            if (count of windows) is 0 then error "CopyPaste window is unavailable"
            if actionMode is "ready" then
                if (count of menu bars) is 0 then error "CopyPaste menu bar is unavailable"
                set windowName to name of window 1 as text
                if windowName is "" then error "CopyPaste window is unnamed"
                return "ok"
            end if
            if actionMode is "surface" then
                set elementsList to entire contents
            else
                set elementsList to entire contents of window 1
            end if
            repeat with elementRef in elementsList
                set roleText to ""
                set nameText to ""
                set descriptionText to ""
                set helpText to ""
                set valueText to ""
                try
                    set nameText to name of elementRef as text
                end try
                if actionMode is "find" and nameText contains targetLabel then
                    try
                        set roleText to role of elementRef as text
                    end try
                    try
                        set descriptionText to description of elementRef as text
                    end try
                    try
                        set helpText to help of elementRef as text
                    end try
                    try
                        set valueText to value of elementRef as text
                    end try
                    return roleText & tab & nameText & tab & descriptionText & tab & helpText & tab & valueText
                end if
                try
                    set roleText to role of elementRef as text
                end try
                if actionMode is "surface" then
                    set end of outputLines to roleText & tab & nameText
                else
                    try
                        set descriptionText to description of elementRef as text
                    end try
                    try
                        set helpText to help of elementRef as text
                    end try
                    try
                        set valueText to value of elementRef as text
                    end try
                    if actionMode is "dump" then
                        set end of outputLines to roleText & tab & nameText & tab & descriptionText & tab & helpText & tab & valueText
                    else if actionMode is "find" then
                        if descriptionText contains targetLabel or helpText contains targetLabel or valueText contains targetLabel then
                            return roleText & tab & nameText & tab & descriptionText & tab & helpText & tab & valueText
                        end if
                    else if nameText is targetLabel or descriptionText is targetLabel or helpText is targetLabel or valueText is targetLabel then
                        if actionMode is "press" then
                            try
                                perform action "AXPress" of elementRef
                                return "ok"
                            end try
                        else if actionMode is "set" then
                            try
                                set focused of elementRef to true
                                keystroke "a" using command down
                                key code 51
                                keystroke inputValue
                                return "ok"
                            end try
                        end if
                    end if
                end if
            end repeat
        end tell
    end tell
    if actionMode is "dump" or actionMode is "surface" then
        set AppleScript's text item delimiters to linefeed
        return outputLines as text
    end if
    error "no accessible element named " & targetLabel
end run
APPLESCRIPT
}

mac_ax_contains() { # <dump> <label>
    grep -Fq "$2" "$1"
}

mac_wait_label() { # <label> <dump> [timeout]
    local label="$1" dump="$2" timeout="${3:-30}" started="$SECONDS"
    while (( SECONDS - started < timeout )); do
        mac_ax find "$label" > "$dump" 2>/dev/null && return 0
        sleep 1
    done
    return 1
}

mac_capture_state() { # <directory>
    mkdir -p "$1"
    mac_ax dump > "$1/ax.txt"
    screencapture -x "$1/screenshot.png"
    [[ -s "$1/ax.txt" && -s "$1/screenshot.png" ]]
}

mac_ui_self_test() {
    local fixture="$1/ax.txt" probe="$1/probe.txt" absent="$1/absent.txt"
    local app="$1/Test.app" binary pid press_result set_result
    local PLIST_BUDDY="$1/plist-reader"

    mkdir -p "$app/Contents/MacOS"
    printf 'RenamedProduct' > "$app/Contents/Info.plist"
    printf '#!/usr/bin/env bash\ncat "$3"\n' > "$PLIST_BUDDY"
    printf '#!/usr/bin/env bash\nexit 0\n' > "$app/Contents/MacOS/RenamedProduct"
    chmod +x "$PLIST_BUDDY" "$app/Contents/MacOS/RenamedProduct"
    binary="$(mac_evidence_executable "$app")"

    pgrep() {
        [[ "$1" == "-x" && "$2" == "RenamedProduct" ]] || return 1
        printf '4242\n'
    }
    pid="$(mac_wait_executable_pid "$binary" 1)"
    unset -f pgrep
    [[ "$pid" == "4242" ]] \
        && ok "bundle executable name selects the launched process" \
        || bad "bundle executable name selects the launched process"

    osascript() {
        [[ "$1" == "-" && "$2" == "4242" ]] || return 1
        case "$3" in
            ready) printf 'ok\n' ;;
            surface) printf 'AXMenuBar\tCopyPaste\nAXMenuBarItem\tCopyPaste\n' ;;
            dump) printf 'AXButton\tSign in\t\t\t\nAXStaticText\tConnected\t\t\t\n' ;;
            find) [[ "$4" == "skipped" ]] && printf 'AXStaticText\tCloud sync finished: 1 skipped\t\t\t\n' ;;
            press) [[ "$4" == "Sign in" ]] && printf 'ok\n' ;;
            set) [[ "$4" == "Email" && "$5" == "native@example.test" ]] && printf 'ok\n' ;;
            *) return 1 ;;
        esac
    }
    mac_set_app_pid "$pid"
    mac_ax ready >/dev/null
    mac_ax surface > "$1/surface.txt"
    mac_ax dump > "$fixture"
    mac_wait_label "skipped" "$probe" 1
    if mac_ax find "Skipped" > "$absent"; then
        bad "absent label probes return failure"
    elif [[ -s "$absent" ]]; then
        bad "absent label probes emit no evidence"
    else
        ok "absent label probes fail without evidence"
    fi
    press_result="$(mac_ax press "Sign in")"
    set_result="$(mac_ax set "Email" "native@example.test")"
    unset -f osascript
    mac_ax_contains "$1/surface.txt" "AXMenuBar" \
        && ok "the full native surface retains menu bar evidence" \
        || bad "the full native surface retains menu bar evidence"
    mac_ax_contains "$fixture" "AXMenuBar" \
        && bad "window scans exclude unrelated process chrome" \
        || ok "window scans exclude unrelated process chrome"
    mac_ax_contains "$fixture" "Sign in" \
        && ok "an accessible action is found for the launched PID" \
        || bad "an accessible action is found for the launched PID"
    [[ "$(cat "$probe")" == $'AXStaticText\tCloud sync finished: 1 skipped\t\t\t' ]] \
        && ok "label probes retain the matched accessibility row" \
        || bad "label probes retain the matched accessibility row"
    [[ "$press_result" == "ok" && "$set_result" == "ok" ]] \
        && ok "accessibility actions retain their result shapes" \
        || bad "accessibility actions retain their result shapes"
    grep -Fq 'set focused of elementRef to true' "${BASH_SOURCE[0]}" \
        && grep -Fq 'keystroke inputValue' "${BASH_SOURCE[0]}" \
        && ok "field input dispatches keyboard events" \
        || bad "field input dispatches keyboard events"
    mac_ax_contains "$fixture" "Signed out" \
        && bad "an absent accessibility state is not found" \
        || ok "an absent accessibility state is not found"
}
