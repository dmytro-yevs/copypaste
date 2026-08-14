#!/usr/bin/env bash
# android-rungs-lib.sh — the detectors for the four surfaces README.md calls
# unverified, and their self-test.
#
# Sourced by android-rungs.sh, which is the sequence that drives a device.
# Everything here is pure — a file in, a verdict out — so `--self-test` can
# prove each detector fails when it should on a machine with no Android SDK.
#
# The verdict vocabulary (ok / FAIL / NOT ASSERTED / probe) is
# android-smoke-lib.sh's and is not repeated.
set -uo pipefail

# shellcheck source=scripts/release/android-smoke-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-smoke-lib.sh"

# `service call` replies
#
# The rung 2 test calls IClipboard as the shell uid, which is the one identity
# on an emulator that holds READ_CLIPBOARD_IN_BACKGROUND. `service call` prints
# the reply parcel as hex plus a printable column, and that column is the whole
# of what can be read back.

# The printable column, concatenated. One quoted group per row.
parcel_text() {   # <reply file>
    sed -n "s/^[^']*'\(.*\)'[^']*$/\1/p" "$1" | tr -d '\n'
}

# Binder writes the exception code first: 0 is a value, anything else is a
# throw. This is what tells "the clipboard was empty" from "the call was
# refused", which is the distinction spike item 5 turns on.
#
# Two layouts, because `service` prints a reply that fits on one line inline
# after `Parcel(` and a longer one as offset rows. Reading only the offset form
# reported every short reply — hasPrimaryClip's, among them — as a refusal.
parcel_status() {   # <reply file>
    sed -e 's/^Result: Parcel(//' -e 's/^0x00000000: //' -e 's/^[[:space:]]*//' "$1" \
        | grep -oE '^[0-9a-f]{8}' | head -n 1
}

parcel_refused() { [[ "$(parcel_status "$1")" != "00000000" ]]; }

# Exception messages are UTF-16, so the printable column renders them with a
# dot for every high byte: `P.a.c.k.a.g.e. .c.o.m.`. Stripping dots recovers
# the sentence, and spaces survive because their high byte is the dot.
parcel_message() { parcel_text "$1" | tr -d '.'; }

# A canary in the reply, whichever width the parcel used.
#
# ClipData's text arrives 8-bit and reads straight out of the column;
# exceptions and package names arrive UTF-16 and only read after the dots go.
# Both spellings are tried because which one a value gets is a platform
# implementation detail, not something to depend on. Canaries are alphanumeric
# so the dot-stripped haystack cannot manufacture a match.
parcel_holds() {   # <reply file> <alphanumeric needle>
    local text
    text="$(parcel_text "$1")"
    grep -qF "$2" <<<"$text" || grep -qF "$2" <<<"${text//./}"
}

# Window flags

# The `fl=` line of the named window, or nothing if the dump has no such window.
#
# Empty and "not secure" are different failures: one is an app that never
# opened a window, the other is INV-35 broken.
window_flags() {   # <dumpsys window windows dump> <component>
    awk -v w="$2" '
        /Window #[0-9]+ Window\{/ { inwin = (index($0, w) > 0) }
        inwin && /^[[:space:]]*fl=/ { sub(/^[[:space:]]*fl=/, ""); print; inwin = 0 }
    ' "$1"
}

window_is_secure() {   # <dump> <component>
    local flags
    flags="$(window_flags "$1" "$2")"
    [[ -n "$flags" ]] && grep -qw SECURE <<<"$flags"
}

# The first window in the dump that is not ours and does not carry SECURE.
#
# The control for the assertion that ours does: a reader that answered SECURE
# about everything would find none. Named generically because system windows
# are titled rather than componentised — `Window{7b6067b u0 StatusBar}` — so
# there is no component to name in advance, and which launcher an image ships
# is not something to hard-code.
other_unprotected_window() {   # <dump> <package>
    awk -v pkg="$2" '
        /Window #[0-9]+ Window\{/ {
            name = $0
            sub(/.*Window\{[0-9a-f]+ u[0-9]+ /, "", name)
            sub(/\}:?[[:space:]]*$/, "", name)
            mine = (index($0, pkg) > 0)
            next
        }
        name != "" && /^[[:space:]]*fl=/ {
            if (!mine && index(" " $0 " ", " SECURE ") == 0) { print name; exit }
            name = ""
        }
    ' "$1"
}

# Quick Settings and the capture service

# SystemUI records a third-party tile in sysui_qs_tiles as `custom(<component>)`.
tile_present() {   # <sysui_qs_tiles value> <component>
    grep -qF "custom($2)" <<<"$1"
}

# A ServiceRecord with a process attached. `app=null` is the shape a refused
# `am start-foreground-service` leaves behind, and reading it as "running"
# would turn the assertion that nothing claims to capture into a false alarm.
service_is_running() {   # <dumpsys activity services dump> <class name>
    awk -v c="$2" '
        /ServiceRecord\{/ { inrec = (index($0, c) > 0) }
        inrec && /app=/ && $0 !~ /app=null/ { print "running"; exit }
    ' "$1" | grep -q running
}

# Driving another app
#
# There is no shell command that puts text on the clipboard: `cmd clipboard`
# resolves to the service and then answers "No shell command implementation" on
# API 36. Another app's text field, selected and copied, does work — and
# getPrimaryClipSource then names that app, so the clip is genuinely foreign
# and the tile's read is the product's read rather than a self-test.

# uiautomator refuses while the screen is animating, which is ordinary right
# after a launch or a tap. A single dump that fails downgrades an assertion to
# NOT ASSERTED, so every caller here retries rather than reporting a busy
# screen as a UI it could not read.
dump_hierarchy_retry() {   # <local path> [attempts]
    local attempts="${2:-8}" i
    for ((i = 0; i < attempts; i++)); do
        dump_hierarchy "$1" && return 0
        sleep 3
    done
    return 1
}

# The tap point for a node, so nothing here hard-codes a screen coordinate.
node_centre() {   # <uiautomator dump> <resource-id or text substring>
    python3 - "$1" "$2" <<'PY'
import re
import sys

xml = open(sys.argv[1], encoding="utf-8", errors="replace").read()
for match in re.finditer(r"<node[^>]*>", xml):
    node = match.group(0)
    if sys.argv[2] not in node:
        continue
    bounds = re.search(r'bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"', node)
    if bounds:
        x1, y1, x2, y2 = (int(v) for v in bounds.groups())
        print((x1 + x2) // 2, (y1 + y2) // 2)
        break
PY
}

# Named text and content-desc under the whole hierarchy. [webview_content]
# answers "did anything paint"; this answers "what does it say", which is what
# the capture-state assertions need.
ui_strings() {   # <uiautomator dump>
    python3 - "$1" <<'PY'
import re
import sys

xml = open(sys.argv[1], encoding="utf-8", errors="replace").read()
for match in re.finditer(r"<node[^>]*>", xml):
    for text in re.findall(r'(?:text|content-desc)="([^"]+)"', match.group(0)):
        print(text)
PY
}

# The device's API level, or a failure that says what the device actually said.
#
# `sh_` folds stderr into stdout, so a probe that failed still produces a value:
# `sdk="$(sh_ getprop ro.build.version.sdk)"` set sdk to
# `adb.exe: device 'emulator-5554' not found` and the run carried on and printed
# `device: API adb.exe: device 'emulator-5554' not found`. Every later decision
# that reads $sdk — which rungs apply, which IClipboard codes to expect — was
# then taken against a sentence. A level is a bare integer or it is not a level.
api_level_from() {   # <probe output> <probe exit status>
    local said="$1" status="$2"
    if [[ "$status" != 0 ]]; then
        printf 'the API probe failed (exit %s): %s\n' "$status" "${said:-adb said nothing}" >&2
        return 1
    fi
    said="$(printf '%s' "$said" | tr -d '\r' | head -n 1)"
    said="${said#"${said%%[![:space:]]*}"}"
    said="${said%"${said##*[![:space:]]}"}"
    if [[ ! "$said" =~ ^[0-9]+$ ]]; then
        printf 'the API probe returned no level: %s\n' "${said:-nothing at all}" >&2
        return 1
    fi
    printf '%s\n' "$said"
}

# One receipt per rung, so a rung that never ran is an absence this script can
# see rather than a summary that counts only what did run. `not-applicable`
# is a decision with a reason attached; silence is not.
rung_receipt() {   # <receipt file> <rung> <run|not-applicable> <why>
    case "$3" in
        run | not-applicable) ;;
        *) printf 'a rung receipt is run or not-applicable, not %s\n' "$3" >&2; return 1 ;;
    esac
    printf '%s\t%s\t%s\n' "$2" "$3" "$4" >> "$1"
}

# The declared rungs with no receipt. Empty output is the only pass.
rungs_without_receipt() {   # <receipt file> <rung>...
    local file="$1" rung
    shift
    for rung in "$@"; do
        grep -q "^$rung	" "$file" 2>/dev/null || printf '%s\n' "$rung"
    done
}

# --self-test

rungs_self_test() {
    local t
    t="$(mktemp -d)"
    # shellcheck disable=SC2064
    trap "rm -rf '$t'" EXIT

    group "self-test: service call replies"

    # Trimmed from the real reply on API 36: getPrimaryClip as com.android.shell
    # with CopyPasteProbe12345 on the clipboard.
    cat > "$t/clip.txt" <<'EOF'
Result: Parcel(
0x00000000: 00000000 00000001 00000001 ffffffff '................'
0x00000010: 00000001 0000000a 00650074 00740078 '........t.e.x.t.'
0x00000050: 00000000 00000013 79706f43 74736150 '........CopyPast'
0x00000060: 6f725065 32316562 00353433 00000014 'eProbe12345.....')
EOF
    # hasPrimaryClip's real reply: short enough that `service` prints it inline.
    printf "Result: Parcel(\t00000000 00000001   '........')\n" > "$t/short.txt"
    [[ "$(parcel_status "$t/short.txt")" == "00000000" ]] \
        && ok "a reply printed inline has its exception code read" \
        || bad "a reply printed inline has its exception code read" "$(parcel_status "$t/short.txt")"
    parcel_refused "$t/short.txt" \
        && bad "a short reply carrying a value is not a refusal" \
        || ok "a short reply carrying a value is not a refusal"

    [[ "$(parcel_status "$t/clip.txt")" == "00000000" ]] \
        && ok "a reply carrying a value has a zero exception code" \
        || bad "a reply carrying a value has a zero exception code" "$(parcel_status "$t/clip.txt")"
    parcel_refused "$t/clip.txt" \
        && bad "a reply carrying a value is not a refusal" \
        || ok "a reply carrying a value is not a refusal"
    parcel_holds "$t/clip.txt" "CopyPasteProbe12345" \
        && ok "an 8-bit clip string is found across the row break" \
        || bad "an 8-bit clip string is found across the row break" "$(parcel_text "$t/clip.txt")"
    parcel_holds "$t/clip.txt" "CopyPasteProbe99999" \
        && bad "a canary absent from the reply is not found" \
        || ok "a canary absent from the reply is not found"

    # The same call with a callingPackage the shell uid does not own. This is
    # spike item 5's shape, and the message is AppOpsManager.checkPackage's.
    cat > "$t/refused.txt" <<'EOF'
Result: Parcel(
0x00000000: ffffffff 00000031 00610050 006b0063 '....1...P.a.c.k.'
0x00000010: 00670061 00200065 006f0063 002e006d 'a.g.e. .c.o.m...'
0x00000020: 006f0063 00790070 00610070 00740073 'c.o.p.y.p.a.s.t.'
0x00000030: 002e0065 00700061 00200070 006f0064 'e...a.p.p. .d.o.'
0x00000040: 00730065 006e0020 0074006f 00620020 'e.s. .n.o.t. .b.'
0x00000050: 006c0065 006e006f 00200067 006f0074 'e.l.o.n.g. .t.o.'
0x00000060: 00320020 00300030 00000030 000003d4 ' .2.0.0.0.......')
EOF
    parcel_refused "$t/refused.txt" \
        && ok "a thrown reply is a refusal" \
        || bad "a thrown reply is a refusal" "$(parcel_status "$t/refused.txt")"
    grep -q "does not belong to 2000" <<<"$(parcel_message "$t/refused.txt")" \
        && ok "a UTF-16 exception message reads back once the dots go" \
        || bad "a UTF-16 exception message reads back once the dots go" \
               "$(parcel_message "$t/refused.txt")"
    parcel_holds "$t/refused.txt" "CopyPasteProbe12345" \
        && bad "a refusal carries no clip text" \
        || ok "a refusal carries no clip text"

    group "self-test: window flags"

    cat > "$t/windows.txt" <<'EOF'
  Window #7 Window{aaa u0 com.android.settings/com.android.settings.Settings}:
    mAttrs={(0,0)(fillxfill) ty=BASE_APPLICATION
      fl=LAYOUT_IN_SCREEN LAYOUT_INSET_DECOR SPLIT_TOUCH HARDWARE_ACCELERATED
      pfl=FORCE_DRAW_STATUS_BAR_BACKGROUND
  Window #8 Window{bbb u0 com.copypaste.app/com.copypaste.app.MainActivity}:
    mAttrs={(0,0)(fillxfill) ty=BASE_APPLICATION
      fl=LAYOUT_IN_SCREEN SECURE LAYOUT_INSET_DECOR SPLIT_TOUCH HARDWARE_ACCELERATED
      pfl=FORCE_DRAW_STATUS_BAR_BACKGROUND
EOF
    window_is_secure "$t/windows.txt" "com.copypaste.app/com.copypaste.app.MainActivity" \
        && ok "a window carrying SECURE is reported secure" \
        || bad "a window carrying SECURE is reported secure" \
               "$(window_flags "$t/windows.txt" "com.copypaste.app/com.copypaste.app.MainActivity")"
    window_is_secure "$t/windows.txt" "com.android.settings/com.android.settings.Settings" \
        && bad "another app's unprotected window is not reported secure" \
        || ok "another app's unprotected window is not reported secure"
    [[ -z "$(window_flags "$t/windows.txt" "com.copypaste.app/.NoSuchActivity")" ]] \
        && ok "a window that is not in the dump reports no flags at all" \
        || bad "a window that is not in the dump reports no flags at all"
    window_is_secure "$t/windows.txt" "com.copypaste.app/.NoSuchActivity" \
        && bad "an absent window is not secure by accident" \
        || ok "an absent window is not secure by accident"

    # SECURE_WINDOW would satisfy a substring match and is a different flag.
    printf '  Window #1 Window{ccc u0 pkg/pkg.A}:\n      fl=LAYOUT_IN_SCREEN NOT_SECURE_ANYTHING\n' \
        > "$t/lookalike.txt"
    window_is_secure "$t/lookalike.txt" "pkg/pkg.A" \
        && bad "a flag that merely contains SECURE does not count" \
               "$(window_flags "$t/lookalike.txt" "pkg/pkg.A")" \
        || ok "a flag that merely contains SECURE does not count"

    # A titled system window, which is how SystemUI's appear: no component to
    # match, so the control has to find one generically.
    cat > "$t/control.txt" <<'EOF'
  Window #5 Window{7b6067b u0 StatusBar}:
    mAttrs={(0,0)(fillx128) ty=STATUS_BAR
      fl=NOT_FOCUSABLE SPLIT_TOUCH HARDWARE_ACCELERATED
  Window #8 Window{bbb u0 com.copypaste.app/com.copypaste.app.MainActivity}:
    mAttrs={(0,0)(fillxfill) ty=BASE_APPLICATION
      fl=LAYOUT_IN_SCREEN SECURE SPLIT_TOUCH
EOF
    [[ "$(other_unprotected_window "$t/control.txt" "com.copypaste.app")" == "StatusBar" ]] \
        && ok "a titled system window is found as the unprotected control" \
        || bad "a titled system window is found as the unprotected control" \
               "$(other_unprotected_window "$t/control.txt" "com.copypaste.app")"

    printf '  Window #8 Window{bbb u0 com.copypaste.app/com.copypaste.app.MainActivity}:\n      fl=LAYOUT_IN_SCREEN SECURE\n' \
        > "$t/only-ours.txt"
    [[ -z "$(other_unprotected_window "$t/only-ours.txt" "com.copypaste.app")" ]] \
        && ok "our own secure window is not offered as its own control" \
        || bad "our own secure window is not offered as its own control"

    group "self-test: quick settings and the capture service"

    local tiles="internet,bt,custom(com.copypaste.app/.CaptureTileService),alarm"
    tile_present "$tiles" "com.copypaste.app/.CaptureTileService" \
        && ok "an added tile is found in sysui_qs_tiles" \
        || bad "an added tile is found in sysui_qs_tiles"
    tile_present "internet,bt,alarm" "com.copypaste.app/.CaptureTileService" \
        && bad "a tile that was never added is not found" \
        || ok "a tile that was never added is not found"

    # What a refused `am start-foreground-service` leaves: a record with no
    # process. Reading it as running would fail the assertion that nothing
    # claims background capture, on a device where nothing does.
    cat > "$t/services-dead.txt" <<'EOF'
  User 0 active services:
  * ServiceRecord{4d38 u0 com.copypaste.app/.CaptureService c:com.android.shell}
    intent={cmp=com.copypaste.app/.CaptureService}
    app=null
    getFgsAllowWiu_new=DENIED
EOF
    service_is_running "$t/services-dead.txt" "CaptureService" \
        && bad "a ServiceRecord with no process is not a running service" \
        || ok "a ServiceRecord with no process is not a running service"

    cat > "$t/services-live.txt" <<'EOF'
  User 0 active services:
  * ServiceRecord{4d38 u0 com.copypaste.app/.CaptureService}
    intent={cmp=com.copypaste.app/.CaptureService}
    app=ProcessRecord{9c1 7575:com.copypaste.app/u0a216}
    isForeground=true
EOF
    service_is_running "$t/services-live.txt" "CaptureService" \
        && ok "a ServiceRecord with a process attached is running" \
        || bad "a ServiceRecord with a process attached is running"
    service_is_running "$t/services-live.txt" "SomeOtherService" \
        && bad "another service's record is not ours" \
        || ok "another service's record is not ours"

    group "self-test: reading a hierarchy dump"

    cat > "$t/ui.xml" <<'EOF'
<?xml version='1.0' encoding='UTF-8' standalone='yes' ?><hierarchy rotation="0">
<node index="0" text="" resource-id="com.android.settings:id/search_action_bar" class="android.widget.LinearLayout" bounds="[42,149][1038,338]" />
<node index="1" text="Background capture is off." resource-id="" class="android.widget.TextView" content-desc="" bounds="[0,400][1080,460]" />
<node index="2" text="" resource-id="" class="android.widget.Button" content-desc="Save the clipboard now" bounds="[0,500][1080,560]" />
</hierarchy>
EOF
    [[ "$(node_centre "$t/ui.xml" "search_action_bar")" == "540 243" ]] \
        && ok "a node's tap point is its centre" \
        || bad "a node's tap point is its centre" "$(node_centre "$t/ui.xml" "search_action_bar")"
    [[ -z "$(node_centre "$t/ui.xml" "no_such_node")" ]] \
        && ok "a node that is not there has no tap point" \
        || bad "a node that is not there has no tap point"

    local strings
    strings="$(ui_strings "$t/ui.xml")"
    grep -qxF "Background capture is off." <<<"$strings" \
        && ok "text nodes are read back" \
        || bad "text nodes are read back" "$strings"
    grep -qxF "Save the clipboard now" <<<"$strings" \
        && ok "content-desc nodes are read back too" \
        || bad "content-desc nodes are read back too" "$strings"
    grep -qF "Capturing from every app." <<<"$strings" \
        && bad "a headline that is not shown is not reported" \
        || ok "a headline that is not shown is not reported"

    group "self-test: the API probe"

    [[ "$(api_level_from "36" 0)" == "36" ]] \
        && ok "a bare API level is read" \
        || bad "a bare API level is read" "$(api_level_from "36" 0)"
    [[ "$(api_level_from "  24  " 0)" == "24" ]] \
        && ok "a level with surrounding whitespace is read" \
        || bad "a level with surrounding whitespace is read"

    # The defect: adb's own failure arriving as the value, because `sh_` merges
    # stderr into stdout and getprop was never asked anything.
    local said
    said="$(api_level_from "adb.exe: device 'emulator-5554' not found" 0 2>&1)" \
        && bad "an adb error is not accepted as an API level" "$said" \
        || ok "an adb error is not accepted as an API level"
    grep -qF "device 'emulator-5554' not found" <<<"$said" \
        && ok "the rejected probe reports what the device said" \
        || bad "the rejected probe reports what the device said" "$said"

    said="$(api_level_from "" 1 2>&1)" \
        && bad "a probe that exited non-zero is not accepted" "$said" \
        || ok "a probe that exited non-zero is not accepted"
    grep -qF "exit 1" <<<"$said" \
        && ok "a failed probe reports its exit status" \
        || bad "a failed probe reports its exit status" "$said"

    said="$(api_level_from "" 0 2>&1)" \
        && bad "an empty API probe is not accepted" "$said" \
        || ok "an empty API probe is not accepted"

    group "self-test: rung receipts"

    local receipts="$t/receipts.tsv"
    : > "$receipts"
    rung_receipt "$receipts" "rung-2" run "the shell uid read the clipboard"
    rung_receipt "$receipts" "flag-secure" not-applicable "no window was ever created"
    [[ "$(rungs_without_receipt "$receipts" rung-2 flag-secure | tr '\n' ' ')" == "" ]] \
        && ok "every rung with a receipt is accounted for" \
        || bad "every rung with a receipt is accounted for" \
               "$(rungs_without_receipt "$receipts" rung-2 flag-secure | tr '\n' ' ')"

    # The failure this exists for: a rung that never ran leaves no trace, and a
    # summary that counts only what ran calls the run complete.
    [[ "$(rungs_without_receipt "$receipts" rung-2 tile flag-secure | tr '\n' ' ')" == "tile " ]] \
        && ok "a rung that never ran is named as missing" \
        || bad "a rung that never ran is named as missing" \
               "$(rungs_without_receipt "$receipts" rung-2 tile flag-secure | tr '\n' ' ')"

    rung_receipt "$receipts" "tile" skipped "a status nothing declares" 2>/dev/null \
        && bad "an undeclared receipt status is refused" \
        || ok "an undeclared receipt status is refused"

    printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
    [[ $FAIL -eq 0 ]]
}
