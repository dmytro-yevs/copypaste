#!/usr/bin/env bash
# android-smoke-lib.sh — the verdicts, the detectors, and their self-test.
#
# Sourced by android-smoke.sh, which is the sequence that drives a device. The
# split is rule 5's: what counts as a crash, as an unencrypted database or as
# leaked plaintext is one responsibility, and it is the only one that can be
# proved on a machine with no Android SDK — `android-smoke.sh --self-test` runs
# [self_test] below against fixtures, and check.sh runs that.
#
# Three verdicts, and the difference between them is the point:
#
#   ok / FAIL      an assertion; a FAIL fails the job
#   NOT ASSERTED   this environment cannot observe it — printed, never skipped
#   probe          an observation kept for the next round, not a gate
set -uo pipefail

PKG="${PKG:-com.copypaste.app}"
OUT="${SMOKE_OUT:-artifacts/android-smoke}"

PASS=0
FAIL=0
NOTES=()
PROBES=()

ok()   { PASS=$((PASS + 1)); printf '  ok    %s\n' "$1"; }
bad()  { FAIL=$((FAIL + 1)); printf '  FAIL  %s\n' "$1"
         [[ -n "${2:-}" ]] && printf '        %s\n' "$2"; return 0; }
note() { NOTES+=("$1 — $2"); printf '  ----  NOT ASSERTED  %s\n              %s\n' "$1" "$2"; }
probe(){ PROBES+=("$1 -> $2"); printf '  ....  probe  %s -> %s\n' "$1" "$2"; }
group(){ printf '\n== %s\n' "$1"; }

# ---------------------------------------------------------------------------
# Detection logic. Everything here is pure: a file in, a verdict out, so
# --self-test can prove each one actually fails when it should.
# ---------------------------------------------------------------------------

# `Process … has died` is deliberately not here: force-stop, which this script
# does on purpose between launches, prints it.
CRASH_PATTERNS='FATAL EXCEPTION|E AndroidRuntime|UnsatisfiedLinkError|Fatal signal|beginning of crash|did not include required runtime symbols'

# The crash blocks in a logcat dump that belong to *this* app.
#
# A crash elsewhere on the emulator is not ours, so the block has to name us —
# and libc truncates the process to 15 characters ("m.copypaste.app"), which is
# why the token matched is `copypaste` rather than the package.
crash_report() {
    awk -v pat="$CRASH_PATTERNS" '
        $0 ~ pat && left == 0 { left = 14; block = $0 "\n"; hit = (tolower($0) ~ /copypaste/); next }
        left > 0 {
            block = block $0 "\n"
            if (tolower($0) ~ /copypaste/) hit = 1
            if (--left == 0 && hit) printf "%s", block
        }
        END { if (left > 0 && hit) printf "%s", block }
    ' "$1"
}

# Our own keystore and history failures, and nothing else's.
#
# The word "Keystore" alone is not a signal: a stock emulator logs `Fido:
# [FidoV2EnrollmentController] Attempting to enroll Autofill Keystore keys` and
# `BiometricService: ... cannot participate in Keystore operations` on every
# boot. Matching it failed run 1 twice while the round-trip it guards passed.
KEYSTORE_FAILURES='could not open history|could not open the keystore|KeystoreEntryUnusable|KeystoreUnavailable|device secret'

keystore_report() {
    grep -aE "$KEYSTORE_FAILURES" "$1" | head -n 5
}

# A history database that opens without SQLCipher is the failure ADR-0007 is
# about: it would be readable clipboard plaintext at rest.
looks_encrypted() {
    [[ -s "$1" ]] || return 1
    local magic
    magic="$(head -c 15 "$1" | tr -d '\000')"
    [[ "$magic" != "SQLite format 3" ]]
}

# SQLCipher's page 1 opens with a per-database random salt. It survives every
# write and changes only when the file is recreated, which makes it the one
# cheap proof that the *same* database was reopened.
salt_of() { od -An -v -tx1 -N16 "$1" | tr -d ' \n'; }

holds_text() { grep -aqF "$2" "$1"; }

# What the WebView is actually showing, read out of the accessibility tree.
#
# Chromium exposes the DOM as *children* of the `android.webkit.WebView` node,
# so "the UI painted" is a question about that subtree and not about the node
# itself: run 2 had the node with nothing under it and a 2 KB screenshot, while
# run 1 had eleven Views, six Buttons and four TextViews under it.
#
# Named descendants rather than a node count, because an empty document still
# exposes a bare View or two. And Chromium's own error page is text-bearing —
# a failed asset load would otherwise read as a painted UI — so its wording is
# reported here and refused by the caller.
#
# Prints: <descendants> <named> <first named string>
webview_content() {   # <uiautomator dump>
    python3 - "$1" <<'PY'
import sys, xml.etree.ElementTree as ET

try:
    root = ET.parse(sys.argv[1]).getroot()
except Exception as e:
    print("0 0 unparsable:", str(e)[:60])
    sys.exit(0)

web = next((n for n in root.iter("node") if n.get("class") == "android.webkit.WebView"), None)
if web is None:
    print("0 0 no WebView node in the dump")
    sys.exit(0)

kids = list(web.iter("node"))[1:]
names = [t for n in kids for t in ((n.get("text") or "").strip(), (n.get("content-desc") or "").strip()) if t]
print(len(kids), len(names), (names[0] if names else "")[:60])
PY
}

# Chromium's error page, which is text-bearing and would otherwise satisfy any
# "the UI painted" predicate. A Tauri app that cannot load its own assets shows
# exactly this.
WEBVIEW_ERRORS='ERR_|net::|Webpage not available|This site can.t be reached'
looks_like_error_page() { grep -aqE "$WEBVIEW_ERRORS" <<<"$1"; }

# Native code from our own package, as `/proc/<pid>/maps` actually spells it.
#
# Run 1 asserted the literal `libcopypaste_ui_lib.so` and failed while the
# database, the keystore round-trip and the intake drain — all of them that
# library running — passed. The name is a packaging decision, not a fact about
# loading: AGP injects `extractNativeLibs="false"` for minSdk >= 23, and the
# linker then maps the library out of the APK, so the pathname column is
# `…/base.apk` and no `.so` appears anywhere.
#
# So the evidence asked for is an *executable* file-backed mapping belonging to
# this package. Ahead-of-time dex is excluded: `base.odex` and friends live
# under the same directory and are mapped executable too, and they are Java.
own_code_maps() {   # <maps file> <package>
    awk -v pkg="$2" '
        {
            path = ""
            for (i = 6; i <= NF; i++) path = (path == "" ? $i : path " " $i)
        }
        path == "" || $2 !~ /x/ { next }
        path ~ /\.(odex|vdex|art|dm)$/ || path ~ /\/oat\// { next }
        index(path, pkg) || index(path, "libcopypaste") { print $2 "  " path }
    ' "$1"
}

# Every distinct path in maps that names this package, executable or not, so a
# mapping we did not predict names itself in the next run instead of failing
# blind.
own_map_paths() {   # <maps file> <package>
    awk -v pkg="$2" '
        {
            path = ""
            for (i = 6; i <= NF; i++) path = (path == "" ? $i : path " " $i)
        }
        path != "" && (index(path, pkg) || index(path, "libcopypaste")) { print $2 "  " path }
    ' "$1" | sort -u
}

# ---------------------------------------------------------------------------
# adb helpers
# ---------------------------------------------------------------------------

sh_() { adb shell "$@" 2>&1 | tr -d '\r'; }

# The app's own process, and not a WebView renderer.
#
# Chromium's sandboxed children are named after the package too, and `pidof`
# prints every match in an order nothing documents — so taking its first token
# was a coin toss over which process the next read describes. An exact name
# match is not.
app_pid() {
    local pid
    pid="$(adb shell ps -A -o PID,NAME 2>/dev/null | tr -d '\r' \
           | awk -v p="$PKG" '$2 == p { print $1; exit }')"
    [[ -n "$pid" ]] || pid="$(adb shell pidof "$PKG" 2>/dev/null | tr -d '\r' | awk '{print $1}')"
    printf '%s' "$pid"
}

# Every process the device names after this package, for the record.
app_processes() { adb shell ps -A -o PID,NAME 2>/dev/null | tr -d '\r' | grep -F "$PKG"; }

has_pid() { [[ -n "$(app_pid)" ]]; }
no_pid()  { [[ -z "$(app_pid)" ]]; }

# Wait up to <secs> for a *function* to succeed. It has to be a function: a
# command built here would have its arguments expanded once, before the wait.
wait_for() {
    local secs="$1"; shift
    local i=0
    while (( i < secs )); do
        "$@" >/dev/null 2>&1 && return 0
        sleep 1
        i=$((i + 1))
    done
    return 1
}

dump_logcat() { adb logcat -d -b all > "$OUT/$1.log" 2>&1 || true; }

# uiautomator refuses while the screen is animating ("could not get idle
# state"), which is ordinary during a launch — the caller retries.
dump_hierarchy() {   # <local path>
    adb shell uiautomator dump /sdcard/ui.xml >/dev/null 2>&1 || return 1
    adb pull /sdcard/ui.xml "$1" >/dev/null 2>&1 || return 1
    [[ -s "$1" ]]
}

# Everything under the app's private directory, addressed the way run-as sees
# it: run-as chdir's to the data directory, so these paths stay relative.
app_files() { adb shell run-as "$PKG" find . -type f 2>/dev/null | tr -d '\r'; }

pull_file() {
    local remote="$1" local_="$2"
    adb exec-out run-as "$PKG" cat "$remote" > "$local_" 2>/dev/null
    [[ -s "$local_" ]] || { rm -f "$local_"; return 1; }
}

# The database is in WAL mode, so a write can land entirely in the -wal file
# and leave the main database byte-identical. Anything asking "did the store
# change?" has to hash the set.
DB_REL=""
db_fingerprint() {
    local tag="$1" f base out="$OUT/dbset-$1.txt"
    : > "$out"
    for f in "$DB_REL" "${DB_REL}-wal"; do
        base="$(basename "$f")"
        if pull_file "$f" "$OUT/${tag}-${base}"; then
            printf '%s  %s\n' "$(sha256sum < "$OUT/${tag}-${base}" | cut -d' ' -f1)" "$base" >> "$out"
        else
            printf '%s  %s\n' "absent" "$base" >> "$out"
        fi
    done
    cat "$out"
}

# ---------------------------------------------------------------------------
# --self-test: prove the detectors above fail when they should
# ---------------------------------------------------------------------------

self_test() {
    local t
    SELF_TEST_TMP="$(mktemp -d)"
    t="$SELF_TEST_TMP"
    trap 'rm -rf "$SELF_TEST_TMP"' EXIT
    group "self-test: crash detection"

    printf 'I ActivityManager: Start proc 1234:com.copypaste.app/u0a123\nI copypaste: hello\n' > "$t/clean.log"
    [[ -z "$(crash_report "$t/clean.log")" ]] \
        && ok "an ordinary log naming the app is not a crash" \
        || bad "an ordinary log naming the app is not a crash" "$(crash_report "$t/clean.log")"

    printf 'E AndroidRuntime: FATAL EXCEPTION: main\nE AndroidRuntime: Process: com.copypaste.app, PID: 4242\nE AndroidRuntime: java.lang.RuntimeException\n' > "$t/ours.log"
    [[ -n "$(crash_report "$t/ours.log")" ]] \
        && ok "a FATAL EXCEPTION naming our process is reported" \
        || bad "a FATAL EXCEPTION naming our process is reported"

    printf 'E AndroidRuntime: FATAL EXCEPTION: main\nE AndroidRuntime: Process: com.android.settings, PID: 99\n' > "$t/theirs.log"
    [[ -z "$(crash_report "$t/theirs.log")" ]] \
        && ok "another package's crash is not ours" \
        || bad "another package's crash is not ours" "$(crash_report "$t/theirs.log")"

    printf 'F libc: Fatal signal 11 (SIGSEGV) in tid 4242 (m.copypaste.app)\n' > "$t/libc.log"
    [[ -n "$(crash_report "$t/libc.log")" ]] \
        && ok "libc's truncated process name still matches" \
        || bad "libc's truncated process name still matches"

    printf 'E AndroidRuntime: java.lang.UnsatisfiedLinkError: couldnt find "libcopypaste_ui_lib.so"\n' > "$t/link.log"
    [[ -n "$(crash_report "$t/link.log")" ]] \
        && ok "a missing libcopypaste_ui_lib.so is reported" \
        || bad "a missing libcopypaste_ui_lib.so is reported"

    group "self-test: native code in /proc/<pid>/maps"

    # The two packagings, and the three things that must not be mistaken for
    # either. Fixture lines are real `maps` rows: perms in column 2, pathname
    # from column 6.
    local pkg="com.copypaste.app"
    printf '%s\n' \
        '7f0000000000-7f0000100000 r-xp 00000000 fd:00 100  /data/app/~~ab==/com.copypaste.app-cd==/lib/x86_64/libcopypaste_ui_lib.so' \
        > "$t/maps-extracted"
    [[ -n "$(own_code_maps "$t/maps-extracted" "$pkg")" ]] \
        && ok "an extracted .so counts as our native code" \
        || bad "an extracted .so counts as our native code"

    printf '%s\n' \
        '7f0000000000-7f0000100000 r-xp 00a00000 fd:00 100  /data/app/~~ab==/com.copypaste.app-cd==/base.apk' \
        > "$t/maps-inapk"
    [[ -n "$(own_code_maps "$t/maps-inapk" "$pkg")" ]] \
        && ok "an executable mapping of our own base.apk counts too" \
        || bad "an executable mapping of our own base.apk counts too" \
               "extractNativeLibs=false is the default; this is the spelling run 1 missed"

    printf '%s\n' \
        '7f0000000000-7f0000100000 r-xp 00000000 fd:00 101  /data/app/~~ab==/com.copypaste.app-cd==/oat/x86_64/base.odex' \
        '7f0000200000-7f0000300000 r--p 00000000 fd:00 102  /data/app/~~ab==/com.copypaste.app-cd==/base.apk' \
        > "$t/maps-dexonly"
    [[ -z "$(own_code_maps "$t/maps-dexonly" "$pkg")" ]] \
        && ok "ahead-of-time dex is not native code" \
        || bad "ahead-of-time dex is not native code" "$(own_code_maps "$t/maps-dexonly" "$pkg")"

    printf '%s\n' \
        '7f0000000000-7f0000100000 r-xp 00000000 fd:00 103  /apex/com.android.webview/lib64/libwebviewchromium.so' \
        > "$t/maps-webview"
    [[ -z "$(own_code_maps "$t/maps-webview" "$pkg")" ]] \
        && ok "another package's native code is not ours" \
        || bad "another package's native code is not ours"

    [[ "$(own_map_paths "$t/maps-dexonly" "$pkg" | wc -l)" -eq 2 ]] \
        && ok "own_map_paths reports every path naming us, executable or not" \
        || bad "own_map_paths reports every path naming us, executable or not" \
               "$(own_map_paths "$t/maps-dexonly" "$pkg")"

    group "self-test: the WebView painted"

    # Shaped after the two runs' hierarchy dumps: run 1 exposed the DOM as
    # named children of the WebView node, run 2 exposed the same chrome with
    # the WebView node a leaf.
    local head='<?xml version="1.0" encoding="UTF-8" standalone="yes" ?><hierarchy rotation="0">'
    local shell_open='<node class="android.widget.FrameLayout" package="com.copypaste.app"><node class="android.widget.LinearLayout">'
    local shell_close='</node></node></hierarchy>'

    printf '%s%s<node class="android.webkit.WebView"><node class="android.view.View" text="" content-desc=""/><node class="android.widget.TextView" text="Clipboard history" content-desc=""/><node class="android.widget.Button" text="" content-desc="Search"/></node>%s' \
        "$head" "$shell_open" "$shell_close" > "$t/paint-run1.xml"
    read -r kids named first <<<"$(webview_content "$t/paint-run1.xml")"
    [[ "$kids" -eq 3 && "$named" -eq 2 && "$first" == "Clipboard history" ]] \
        && ok "a painted WebView reports its named descendants" \
        || bad "a painted WebView reports its named descendants" "got '$kids $named $first'"

    printf '%s%s<node class="android.webkit.WebView"/>%s' "$head" "$shell_open" "$shell_close" > "$t/paint-run2.xml"
    read -r kids named _ <<<"$(webview_content "$t/paint-run2.xml")"
    [[ "$kids" -eq 0 && "$named" -eq 0 ]] \
        && ok "run 2's empty WebView is not a painted UI" \
        || bad "run 2's empty WebView is not a painted UI" "got '$kids $named'"

    # An empty document still exposes a View or two, which is why the predicate
    # counts named nodes and not nodes.
    printf '%s%s<node class="android.webkit.WebView"><node class="android.view.View" text="" content-desc=""/></node>%s' \
        "$head" "$shell_open" "$shell_close" > "$t/paint-empty.xml"
    read -r kids named _ <<<"$(webview_content "$t/paint-empty.xml")"
    [[ "$kids" -eq 1 && "$named" -eq 0 ]] \
        && ok "an unnamed subtree is not a painted UI" \
        || bad "an unnamed subtree is not a painted UI" "got '$kids $named'"

    printf '%s%s<node class="android.widget.TextView" text="Not a WebView"/>%s' \
        "$head" "$shell_open" "$shell_close" > "$t/paint-nowebview.xml"
    read -r kids named first <<<"$(webview_content "$t/paint-nowebview.xml")"
    [[ "$kids" -eq 0 && "$named" -eq 0 ]] \
        && ok "text outside the WebView does not count" \
        || bad "text outside the WebView does not count" "got '$kids $named $first'"

    printf 'not xml at all' > "$t/paint-broken.xml"
    read -r kids named _ <<<"$(webview_content "$t/paint-broken.xml")"
    [[ "$kids" -eq 0 ]] \
        && ok "an unparsable dump is not a painted UI" \
        || bad "an unparsable dump is not a painted UI" "got '$kids $named'"

    looks_like_error_page "Webpage not available" \
        && ok "Chromium's error page is refused" \
        || bad "Chromium's error page is refused"
    looks_like_error_page "net::ERR_FILE_NOT_FOUND" \
        && ok "a net:: error string is refused" \
        || bad "a net:: error string is refused"
    looks_like_error_page "Clipboard history" \
        && bad "ordinary UI text is not an error page" \
        || ok "ordinary UI text is not an error page"

    group "self-test: keystore detection"

    printf 'I Fido    : [FidoV2EnrollmentController] Attempting to enroll Autofill Keystore keys\nD BiometricService: Sensor ID(0) cannot participate in Keystore operations\n' > "$t/android-keystore.log"
    [[ -z "$(keystore_report "$t/android-keystore.log")" ]] \
        && ok "Android's own Keystore chatter is not our failure" \
        || bad "Android's own Keystore chatter is not our failure" "$(keystore_report "$t/android-keystore.log")"

    printf 'E copypaste: could not open the keystore: KeystoreUnavailable\n' > "$t/ours-keystore.log"
    [[ -n "$(keystore_report "$t/ours-keystore.log")" ]] \
        && ok "our own keystore failure is reported" \
        || bad "our own keystore failure is reported"

    group "self-test: database detection"

    printf 'SQLite format 3\000rest of an unencrypted database' > "$t/plain.db"
    looks_encrypted "$t/plain.db" \
        && bad "a plain SQLite header is not accepted as encrypted" \
        || ok "a plain SQLite header is not accepted as encrypted"

    head -c 4096 /dev/urandom > "$t/cipher.db"
    looks_encrypted "$t/cipher.db" \
        && ok "a SQLCipher-shaped file is accepted" \
        || bad "a SQLCipher-shaped file is accepted"

    looks_encrypted "$t/missing.db" \
        && bad "an absent database is not accepted" \
        || ok "an absent database is not accepted"

    local salt
    salt="$(salt_of "$t/cipher.db")"
    [[ ${#salt} -eq 32 ]] \
        && ok "salt_of reads 16 bytes as hex" \
        || bad "salt_of reads 16 bytes as hex" "got ${#salt} characters"

    printf 'binary\000CANARY-42\000more' > "$t/blob"
    holds_text "$t/blob" "CANARY-42" \
        && ok "a canary in a binary file is found" \
        || bad "a canary in a binary file is found"
    holds_text "$t/cipher.db" "CANARY-42" \
        && bad "a canary absent from a file is not found" \
        || ok "a canary absent from a file is not found"

    printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
    [[ $FAIL -eq 0 ]]
}

# ---------------------------------------------------------------------------
# The summary, printed on every exit path
# ---------------------------------------------------------------------------

summary() {
    local verdict="FAILED"
    [[ $FAIL -eq 0 ]] && verdict="passed"
    {
        printf '\n## Android emulator smoke test: %s\n\n' "$verdict"
        printf '%d assertions passed, %d failed.\n\n' "$PASS" "$FAIL"
        if [[ ${#NOTES[@]} -gt 0 ]]; then
            printf 'Not asserted — this emulator cannot observe these:\n\n'
            printf '* %s\n' "${NOTES[@]}"
            printf '\n'
        fi
        if [[ ${#PROBES[@]} -gt 0 ]]; then
            printf 'Observations, recorded but not gating:\n\n'
            printf '* %s\n' "${PROBES[@]}"
            printf '\n'
        fi
    } | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
}
