#!/usr/bin/env bash
set -uo pipefail

mac_ax() { # <dump|press|set> [label] [value]
    osascript - "$@" <<'APPLESCRIPT'
on run argv
    set actionMode to item 1 of argv
    set targetLabel to ""
    set inputValue to ""
    if (count of argv) > 1 then set targetLabel to item 2 of argv
    if (count of argv) > 2 then set inputValue to item 3 of argv
    set outputLines to {}
    tell application "System Events" to tell process "CopyPaste"
        set elementsList to entire contents of window 1
        repeat with elementRef in elementsList
            set roleText to ""
            set nameText to ""
            set descriptionText to ""
            set helpText to ""
            set valueText to ""
            try
                set roleText to role of elementRef as text
            end try
            try
                set nameText to name of elementRef as text
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
            if actionMode is "dump" then
                set end of outputLines to roleText & tab & nameText & tab & descriptionText & tab & helpText & tab & valueText
            else if nameText is targetLabel or descriptionText is targetLabel or helpText is targetLabel or valueText is targetLabel then
                if actionMode is "press" then
                    try
                        perform action "AXPress" of elementRef
                        return "ok"
                    end try
                else if actionMode is "set" then
                    try
                        set value of elementRef to inputValue
                        return "ok"
                    end try
                end if
            end if
        end repeat
    end tell
    if actionMode is "dump" then
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
        mac_ax dump > "$dump" 2>/dev/null || true
        mac_ax_contains "$dump" "$label" && return 0
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
    local fixture="$1/ax.txt"
    printf 'AXButton\tSign in\t\t\t\nAXStaticText\tConnected\t\t\t\n' > "$fixture"
    mac_ax_contains "$fixture" "Sign in" \
        && ok "an accessible action is found in a native dump" \
        || bad "an accessible action is found in a native dump"
    mac_ax_contains "$fixture" "Signed out" \
        && bad "an absent accessibility state is not found" \
        || ok "an absent accessibility state is not found"
}
