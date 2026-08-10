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

# shellcheck source=scripts/release/android-screencap-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-screencap-lib.sh"
# shellcheck source=scripts/release/android-log-report-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-log-report-lib.sh"

metadata_tool="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/android-metadata.mjs"
APP_NAMESPACE="$(node "$metadata_tool" --field releaseApplicationId)"
PKG="${PKG:-$APP_NAMESPACE}"
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

# Detection logic. Everything here is pure: a file in, a verdict out, so
# --self-test can prove each one actually fails when it should.

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

# A source-level guard for the state that Android must never restore without
# the non-exportable Keystore key that opens it. Prints one line per defect.
backup_rules_report() {   # <manifest> <Android 11 rules> <Android 12+ rules>
    python3 - "$@" <<'PY'
import sys
import xml.etree.ElementTree as ET

ANDROID = "{http://schemas.android.com/apk/res/android}"
DOMAINS = (
    "root", "file", "database", "sharedpref", "external",
    "device_root", "device_file", "device_database", "device_sharedpref",
)
errors = []

def parse(path):
    try:
        return ET.parse(path).getroot()
    except (OSError, ET.ParseError) as error:
        errors.append(f"{path}: {error}")
        return None

def require_excludes(root, label):
    excluded = {
        node.get("domain")
        for node in root.findall("exclude")
        if node.get("path") in (".", "./")
    }
    for domain in DOMAINS:
        if domain not in excluded:
            errors.append(f"{label} does not exclude {domain} at its root")

manifest = parse(sys.argv[1])
legacy = parse(sys.argv[2])
modern = parse(sys.argv[3])

if manifest is not None:
    app = manifest.find("application")
    if app is None:
        errors.append("AndroidManifest.xml has no application")
    else:
        expected = {
            "allowBackup": "false",
            "fullBackupContent": "@xml/backup_rules",
            "dataExtractionRules": "@xml/data_extraction_rules",
        }
        for name, value in expected.items():
            if app.get(ANDROID + name) != value:
                errors.append(f"AndroidManifest.xml {name} must be {value}")

if legacy is not None:
    if legacy.tag != "full-backup-content":
        errors.append("backup_rules.xml root is not full-backup-content")
    else:
        require_excludes(legacy, "backup_rules.xml")

if modern is not None:
    if modern.tag != "data-extraction-rules":
        errors.append("data_extraction_rules.xml root is not data-extraction-rules")
    else:
        for section_name in ("cloud-backup", "device-transfer"):
            section = modern.find(section_name)
            if section is None:
                errors.append(f"data_extraction_rules.xml has no {section_name}")
            else:
                require_excludes(section, f"data_extraction_rules.xml {section_name}")

print("\n".join(errors))
PY
}

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

webs = [n for n in root.iter("node") if n.get("class") == "android.webkit.WebView"]
if not webs:
    print("0 0 no WebView node in the dump")
    sys.exit(0)

# Every WebView node, not the first: run 3's dump held two, and which of them
# carries the document is not something to depend on.
kids = [k for w in webs for k in list(w.iter("node"))[1:]]
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

# The WebView's remote-debugging socket, if the process published one.
#
# It is an abstract socket named after the pid, so the name is different after
# every restart and a lookup by a remembered name silently describes nothing.
devtools_sockets() {   # <cat /proc/net/unix output> <pid>
    grep -oE "@webview_devtools_remote_$2\$" <<<"$1"
}

# Every pid publishing one, ours included.
#
# Only `webview_devtools_remote_<digits>`: an emulator also carries the WebView
# zygote's socket and, on a Google image, Stetho's
# `@stetho_<package>_devtools_remote` — neither is a Chromium endpoint keyed by
# pid, and treating them as one would name a process that does not exist.
devtools_socket_pids() {   # <cat /proc/net/unix output>
    grep -oE '@webview_devtools_remote_[0-9]+$' <<<"$1" | sed 's/.*_//'
}

# The three wry JNI method names in a file, as "<name> <count>" lines.
#
# Three, not one. `setWebContentsDebuggingEnabled` is the call that turns the
# remote debugger on and it is compiled out under
# `#[cfg(any(debug_assertions, feature = "devtools"))]`.
# `setWebViewClient` and `setWebChromeClient` sit in the same wry function
# under no cfg at all. Reporting all three is what makes a zero for the first
# mean "the cfg removed it" instead of "this scan never read the library" —
# the failure mode of a grep against a path that moved.
#
# `grep -a` on the binary rather than strings(1): binutils is not on every
# machine this runs from, and a missing tool would read as a clean build.
wry_jni_counts() {   # <file>
    local name
    for name in setWebContentsDebuggingEnabled setWebViewClient setWebChromeClient; do
        printf '%s %s\n' "$name" "$(LC_ALL=C grep -a -o -- "$name" "$1" 2>/dev/null | wc -l | tr -d ' ')"
    done
}

# The same counts over every native library packed in an APK, as
# "<devtools-call> <unconditional-neighbours>".
#
# Extracted first, never grepped through the zip. Whether an .so is stored or
# deflated inside an APK is a packaging flag, and a scan that silently depended
# on it would start reporting a clean build the day the flag changed.
apk_wry_jni_counts() {   # <apk>
    local work call=0 others=0 so name count
    work="$(mktemp -d)"
    unzip -o -q "$1" 'lib/*/*.so' -d "$work" 2>/dev/null || true
    for so in "$work"/lib/*/*.so; do
        [[ -f "$so" ]] || continue
        while read -r name count; do
            if [[ "$name" == setWebContentsDebuggingEnabled ]]; then
                call=$((call + count))
            else
                others=$((others + count))
            fi
        done < <(wry_jni_counts "$so")
    done
    rm -rf "$work"
    printf '%s %s' "$call" "$others"
}

# adb helpers

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

# The package a live CDP endpoint names for <pid>, or nothing.
#
# A socket name is not an exposure — the mistake e2e-android/ already paid for
# from the other side. `adb forward` binds the local port whether or not the
# abstract name is listening, and the connection is then accepted and closed
# with no bytes, so a check that stops at the name reports a debugger that no
# debugger could use. This speaks the protocol and reads back who answered.
#
# curl missing is not a pass: the caller is handed a name it can never match,
# so an environment that cannot prove the endpoint dead fails the way an open
# one does (AGENTS.md rule 4, fail closed).
devtools_cdp_package() {   # <pid>
    local port="${CDP_PROBE_PORT:-19229}" json
    command -v curl >/dev/null 2>&1 || { printf 'UNPROVEN(no curl)'; return 0; }
    adb forward "tcp:$port" "localabstract:webview_devtools_remote_$1" >/dev/null 2>&1 || return 0
    json="$(curl -s --max-time 10 "http://127.0.0.1:$port/json/version" 2>/dev/null)"
    adb forward --remove "tcp:$port" >/dev/null 2>&1 || true
    sed -n 's/.*"Android-Package"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' <<<"$json"
}

# Started only to *produce* a control; the search below takes any qualifying
# process, so an image without this package still passes if something else on
# it serves CDP.
CONTROL_ACTIVITY="${CONTROL_ACTIVITY:-com.android.htmlviewer/.HTMLViewerActivity}"

# A WebView that is not ours and not debuggable, on this device, answering the
# same protocol — or nothing.
#
# The control for what an endpoint on our own pid cannot answer: whether a
# devtools endpoint here says anything about our artefact. A `userdebug` image
# sets `ro.debuggable=1` and the WebView provider then enables remote debugging
# for every process, so an app nobody here built, which `run-as` also refuses,
# serving CDP tells that apart from a build that switched its own debugger on.
# A debuggable control would prove nothing about a non-debuggable one.
#
# Finding none returns empty and fails the caller: "could not show the device
# does this to everyone" must not read as "the build is clean".

control_cdp_package() {
    local pid pkg i
    # An explicit component, and a URL that need not resolve: htmlviewer
    # declares only the `content:` scheme, so nothing resolves this intent by
    # filter, and a WebView showing an error page is still a WebView publishing
    # a socket.
    adb shell am start -a android.intent.action.VIEW \
        -d file:///data/local/tmp/copypaste-devtools-control.html \
        -t text/html -n "$CONTROL_ACTIVITY" >/dev/null 2>&1 || true

    for (( i = 0; i < 20; i++ )); do
        for pid in $(devtools_socket_pids "$(sh_ cat /proc/net/unix)"); do
            [[ "$pid" == "$(app_pid)" ]] && continue
            pkg="$(devtools_cdp_package "$pid")"
            [[ -n "$pkg" && "$pkg" != "$PKG" ]] || continue
            grep -q uid <<<"$(sh_ run-as "$pkg" id)" && continue
            adb shell am force-stop "${CONTROL_ACTIVITY%%/*}" >/dev/null 2>&1 || true
            printf '%s (pid %s)' "$pkg" "$pid"
            return 0
        done
        sleep 1
    done
    adb shell am force-stop "${CONTROL_ACTIVITY%%/*}" >/dev/null 2>&1 || true
}

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

# A screen that was never on and a UI that never painted are different
# failures, so the screen is dealt with first and reported on its own. Sets
# AWAKE, which [assert_painted] refuses to draw a conclusion without.
AWAKE=no
WAKEFULNESS=""
wake_screen() {
    sh_ input keyevent KEYCODE_WAKEUP >/dev/null 2>&1 || true
    sh_ wm dismiss-keyguard >/dev/null 2>&1 || true
    # Two spellings because one of them is a dumpsys format away from silently
    # reporting a dark screen and turning the paint assertion off.
    local power
    power="$(sh_ dumpsys power; sh_ dumpsys display)"
    WAKEFULNESS="$(grep -oE 'mWakefulness=[A-Za-z_]+|mScreenState=[A-Z]+|Display Power: state=[A-Z]+' <<<"$power" \
                   | sort -u | tr '\n' ' ')"
    AWAKE=no
    grep -qE 'mWakefulness=Awake|mScreenState=ON|state=ON' <<<"$WAKEFULNESS" && AWAKE=yes
    if [[ "$AWAKE" == yes ]]; then
        ok "the screen is awake"
    else
        bad "the screen is awake" \
            "KEYCODE_WAKEUP left it at '${WAKEFULNESS:-unreported}' — nothing below can tell a dark screen from a dead UI"
    fi
}

# The one assertion in this harness about the React side rather than about
# Rust, and the one that catches an R8 run that stripped the plugin.
#
# Polled, not sampled: run 2 dumped once at 25s, found the WebView node empty,
# and passed anyway because this was then a probe. Chromium also builds its
# accessibility tree only once a client asks, so the first dump after a launch
# can be empty on a UI that is perfectly alive. Measured paint times so far:
# 33s and 38s after `am start`.
assert_painted() {   # <timeout> <launched_at> <dump path> <screenshot path>
    local timeout="$1" launched_at="$2" dump="$3" png="$4"
    local painted_at="" kids=0 named=0 first="" dumps=0 shot=0 nodes poll_started="$SECONDS"

    while (( SECONDS - poll_started < timeout )); do
        if dump_hierarchy "$dump"; then
            dumps=$((dumps + 1))
            read -r kids named first <<<"$(webview_content "$dump")"
            if (( named > 0 )); then painted_at=$((SECONDS - launched_at)); break; fi
        fi
        sleep 2
    done

    if ! capture_android_png "$png" "$PKG"; then
        bad "a native PNG screenshot was captured" \
            "$(tail -n 12 "${png%.png}-screencap.log" | tr '\n' ' ')"
    fi
    [[ -s "$png" ]] && shot="$(stat -c %s "$png")"
    if [[ -s "$dump" ]]; then
        nodes="$(grep -o 'class="[^"]*"' "$dump" | sort | uniq -c | sort -rn | head -n 6 | tr '\n' ' ')"
        probe "the view hierarchy" "${nodes:-none}"
    fi
    # Kept for a human to look at, and deliberately not read as evidence: run 4
    # painted a full UI — 33 named nodes, "CopyPaste" among them — while
    # screencap returned the same 2 KB it returns for a blank screen. Under
    # `-gpu swiftshader_indirect -no-window` the captured framebuffer does not
    # track what the WebView is showing. The accessibility tree does.
    probe "the screenshot" "$shot bytes — size says nothing here, see the artifact"

    if [[ "$AWAKE" != yes ]]; then
        bad "the WebView painted a UI with content" \
            "screen state was '${WAKEFULNESS:-unreported}', so native paint could not be observed; screenshot $shot bytes"
    elif [[ -n "$painted_at" ]] && ! looks_like_error_page "$first"; then
        ok "the WebView painted a UI with content"
        # Printed on the way past, so the timeout has a measured margin rather
        # than a guessed one, and a paint that slows down is visible before it
        # fails.
        probe "how long the UI took to paint" \
              "${painted_at}s after am start, ${named} named nodes under the WebView, first: ${first}"
    elif [[ -n "$painted_at" ]]; then
        bad "the WebView painted a UI with content" \
            "it painted Chromium's error page in ${painted_at}s: ${first} — the app cannot load its own assets"
    elif (( dumps == 0 )); then
        # A tool that never answered is not a UI that never painted, and calling
        # it one would send the next round after the wrong thing.
        bad "the WebView painted a UI with content" \
            "uiautomator produced no dump in ${timeout}s, so native paint could not be observed; screenshot $shot bytes"
    else
        bad "the WebView painted a UI with content" \
            "$dumps dumps over ${timeout}s and the WebView subtree held $kids nodes, $named named; screenshot $shot bytes (${first})"
    fi
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

# --self-test: prove the detectors above fail when they should

self_test() {
    local t
    SELF_TEST_TMP="$(mktemp -d)"
    t="$SELF_TEST_TMP"
    trap 'rm -rf "$SELF_TEST_TMP"' EXIT
    android_log_report_self_test "$t"

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

    group "self-test: the WebView devtools socket"

    # Real /proc/net/unix rows. The zygote's socket and another process's are
    # both present, because "some webview socket exists" is not the question.
    local unix_dump
    unix_dump="$(printf '%s\n' \
        '0000000000000000: 00000002 00000000 00010000 0001 01 14836 @com.android.internal.os.WebViewZygoteInit/069a548f' \
        '0000000000000000: 00000002 00000000 00010000 0001 01 22902 @stetho_com.google.android.apps.messaging_devtools_remote' \
        '0000000000000000: 00000002 00000000 00010000 0001 01 53464 @webview_devtools_remote_8689' \
        '0000000000000000: 00000002 00000000 00010000 0001 01 53999 @webview_devtools_remote_99999')"

    [[ -n "$(devtools_sockets "$unix_dump" 8689)" ]] \
        && ok "the socket published by this pid is found" \
        || bad "the socket published by this pid is found"

    [[ -z "$(devtools_sockets "$unix_dump" 868)" ]] \
        && ok "a pid that is a prefix of another's socket does not match it" \
        || bad "a pid that is a prefix of another's socket does not match it" \
               "$(devtools_sockets "$unix_dump" 868)"

    [[ -z "$(devtools_sockets "$unix_dump" 4242)" ]] \
        && ok "a process that published nothing reports nothing" \
        || bad "a process that published nothing reports nothing"

    [[ "$(devtools_socket_pids "$unix_dump" | tr '\n' ' ')" == "8689 99999 " ]] \
        && ok "every publishing pid is listed, and only Chromium's sockets" \
        || bad "every publishing pid is listed, and only Chromium's sockets" \
               "$(devtools_socket_pids "$unix_dump" | tr '\n' ' ')"

    group "self-test: the devtools call in the artefact"

    # The byte sequences the real scan matches, around filler, because these are
    # read out of a 20 MB native library and not off a line of their own.
    local call others
    printf 'ELF\000\000setWebViewClient\000setWebChromeClient\000RustWebView\000' > "$t/lib-release.so"
    read -r call others <<<"$(wry_jni_counts "$t/lib-release.so" | awk '
        $1 == "setWebContentsDebuggingEnabled" { c = $2; next } { o += $2 }
        END { print c + 0, o + 0 }')"
    [[ "$call" -eq 0 && "$others" -eq 2 ]] \
        && ok "a release library reports no devtools call and both neighbours" \
        || bad "a release library reports no devtools call and both neighbours" "got '$call $others'"

    printf 'ELF\000\000setWebViewClient\000setWebContentsDebuggingEnabled\000setWebChromeClient\000' > "$t/lib-debug.so"
    [[ "$(wry_jni_counts "$t/lib-debug.so" | awk '$1 == "setWebContentsDebuggingEnabled" { print $2 }')" -eq 1 ]] \
        && ok "a debug library reports the devtools call" \
        || bad "a debug library reports the devtools call" "$(wry_jni_counts "$t/lib-debug.so")"

    # The failure this scan is shaped to survive: pointed at the wrong file, it
    # must not read as a clean build. Zero neighbours is how the caller knows.
    printf 'ELF\000\000nothing of interest here\000' > "$t/lib-elsewhere.so"
    [[ "$(wry_jni_counts "$t/lib-elsewhere.so" | awk '{ s += $2 } END { print s + 0 }')" -eq 0 ]] \
        && ok "a library that is not wry's reports no neighbours either" \
        || bad "a library that is not wry's reports no neighbours either" "$(wry_jni_counts "$t/lib-elsewhere.so")"

    group "self-test: the WebView painted"

    android_screencap_self_test "$t"

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

    printf '%s%s<node class="android.webkit.WebView"/><node class="android.webkit.WebView"><node class="android.widget.TextView" text="CopyPaste"/></node>%s' \
        "$head" "$shell_open" "$shell_close" > "$t/paint-two-webviews.xml"
    read -r kids named first <<<"$(webview_content "$t/paint-two-webviews.xml")"
    [[ "$kids" -eq 1 && "$named" -eq 1 && "$first" == "CopyPaste" ]] \
        && ok "content under the second of two WebView nodes still counts" \
        || bad "content under the second of two WebView nodes still counts" "got '$kids $named $first'"

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

    local paint_verdict
    paint_verdict="$(
        adb() { return 1; }
        AWAKE=no WAKEFULNESS="" OUT="$t" assert_painted 0 0 "$t/paint-unobserved.xml" "$t/paint-unobserved.png"
    )"
    [[ "$paint_verdict" == *"FAIL  the WebView painted a UI with content"* \
       && "$paint_verdict" == *"screen state was 'unreported'"* \
       && "$paint_verdict" == *"screenshot 0 bytes"* ]] \
        && ok "missing screen-state observability fails paint with diagnostics" \
        || bad "missing screen-state observability fails paint with diagnostics" "$paint_verdict"

    paint_verdict="$(
        adb() { return 1; }
        AWAKE=yes WAKEFULNESS="mWakefulness=Awake" OUT="$t" assert_painted 0 0 "$t/paint-no-dump.xml" "$t/paint-no-dump.png"
    )"
    [[ "$paint_verdict" == *"FAIL  the WebView painted a UI with content"* \
       && "$paint_verdict" == *"uiautomator produced no dump in 0s"* \
       && "$paint_verdict" == *"screenshot 0 bytes"* ]] \
        && ok "missing hierarchy observability fails paint with diagnostics" \
        || bad "missing hierarchy observability fails paint with diagnostics" "$paint_verdict"

    group "self-test: keystore detection"

    printf 'I Fido    : [FidoV2EnrollmentController] Attempting to enroll Autofill Keystore keys\nD BiometricService: Sensor ID(0) cannot participate in Keystore operations\n' > "$t/android-keystore.log"
    [[ -z "$(keystore_report "$t/android-keystore.log")" ]] \
        && ok "Android's own Keystore chatter is not our failure" \
        || bad "Android's own Keystore chatter is not our failure" "$(keystore_report "$t/android-keystore.log")"

    printf 'E copypaste: could not open the keystore: KeystoreUnavailable\n' > "$t/ours-keystore.log"
    [[ -n "$(keystore_report "$t/ours-keystore.log")" ]] \
        && ok "our own keystore failure is reported" \
        || bad "our own keystore failure is reported"

    group "self-test: backup exclusions"

    local root manifest legacy modern report
    root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
    manifest="$root/crates/copypaste-ui/src-tauri/gen/android/app/src/main/AndroidManifest.xml"
    legacy="$root/crates/copypaste-ui/src-tauri/gen/android/app/src/main/res/xml/backup_rules.xml"
    modern="$root/crates/copypaste-ui/src-tauri/gen/android/app/src/main/res/xml/data_extraction_rules.xml"
    report="$(backup_rules_report "$manifest" "$legacy" "$modern")"
    [[ -z "$report" ]] \
        && ok "both backup formats exclude every CopyPaste storage domain" \
        || bad "both backup formats exclude every CopyPaste storage domain" "$report"

    sed '/android:dataExtractionRules=/d' "$manifest" > "$t/manifest-no-modern.xml"
    report="$(backup_rules_report "$t/manifest-no-modern.xml" "$legacy" "$modern")"
    [[ "$report" == *"dataExtractionRules"* ]] \
        && ok "a missing Android 12+ manifest reference is reported" \
        || bad "a missing Android 12+ manifest reference is reported" "$report"

    sed '/domain="database"/d' "$legacy" > "$t/legacy-no-database.xml"
    report="$(backup_rules_report "$manifest" "$t/legacy-no-database.xml" "$modern")"
    [[ "$report" == *"backup_rules.xml does not exclude database"* ]] \
        && ok "a missing Android 11 storage exclusion is reported" \
        || bad "a missing Android 11 storage exclusion is reported" "$report"

    printf '<data-extraction-rules><cloud-backup/></data-extraction-rules>\n' > "$t/modern-no-transfer.xml"
    report="$(backup_rules_report "$manifest" "$legacy" "$t/modern-no-transfer.xml")"
    [[ "$report" == *"data_extraction_rules.xml has no device-transfer"* ]] \
        && ok "a missing Android 12+ transfer section is reported" \
        || bad "a missing Android 12+ transfer section is reported" "$report"

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

# The summary, printed on every exit path

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
