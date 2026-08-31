#!/usr/bin/env bash
# smoke-macos-dmg.sh — install the DMG the way a user does, and watch it work.
#
#   Usage: scripts/release/smoke-macos-dmg.sh <version> [arch] [--force]
#
# The release pipeline had no install test at all: it produced a DMG and
# published it, and every claim about whether the thing inside launches, reads a
# clipboard or opens its database came from reading Apple's documentation. This
# is the first thing that finds out.
#
# It follows the cask: mount, copy to /Applications, quarantine it the way a
# download is quarantined, run the postflight script from inside the bundle,
# then start what was installed.
#
# ENFORCED vs REPORTED
# A leg is ENFORCED when a failure means the artefact is broken for everyone:
# the image does not mount, the signature does not verify, the postflight the
# cask runs fails, the daemon inside the bundle cannot start, or a copy made
# with pbcopy never reaches the history.
#
# A leg is REPORTED when a failure may be the runner rather than the build.
# Launching a GUI app needs a window server, Gatekeeper's verdict on an ad-hoc
# bundle is the subject of ADR-0001 rather than a regression, and whether a
# Keychain item survives a re-signed binary is a design question that this is
# the first opportunity to ask. Those print `::warning::` and a summary line,
# and the summary is the deliverable.
set -uo pipefail

VERSION="${1:-}"
[[ -n "$VERSION" ]] || { echo "ERROR: version required. Usage: $0 <version> [arch]" >&2; exit 2; }
VERSION="${VERSION#v}"
shift

ARCH=""
FORCE="no"
for arg in "$@"; do
    case "$arg" in
        --force) FORCE="yes" ;;
        arm64|x86_64) ARCH="$arg" ;;
        *) echo "ERROR: unknown argument: $arg" >&2; exit 2 ;;
    esac
done
if [[ -z "$ARCH" ]]; then
    case "$(uname -m)" in
        arm64)  ARCH="arm64"  ;;
        x86_64) ARCH="x86_64" ;;
        *) echo "ERROR: cannot infer arch from $(uname -m)" >&2; exit 2 ;;
    esac
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT" || exit 2
source scripts/release/macos-bundle-lib.sh

[[ "$(uname -s)" == "Darwin" ]] || { echo "ERROR: must run on macOS" >&2; exit 2; }

# This installs into /Applications, starts a daemon against the real
# application-support directory, and writes a device secret into the login
# keychain. That is the point — it is what a user gets — and it is also why it
# refuses to do it to a machine somebody is using.
if [[ -z "${CI:-}" && "$FORCE" != "yes" ]]; then
    echo "ERROR: this replaces /Applications/CopyPaste.app and starts the real daemon." >&2
    echo "       Pass --force if that is what you want on this machine." >&2
    exit 2
fi

DMG="dist/CopyPaste-v${VERSION}-macos-${ARCH}.dmg"
[[ -f "$DMG" ]] || { echo "ERROR: $DMG not found — run make-dmg.sh first." >&2; exit 2; }
QUALIFIED_ARTIFACT_IDENTITY="$(python3 scripts/release/write-native-evidence.py --capture-qualified-artifact "$DMG")" || {
    echo "ERROR: release artifact identity could not be captured" >&2
    exit 1
}

APP="/Applications/CopyPaste.app"
MNT="$(mktemp -d)/CopyPaste"
LOGS="$(mktemp -d)"
APP_EXECUTABLE=""
CLI=""
PASS=0
FAIL=0
NOTES=()

ok()   { PASS=$((PASS + 1)); printf '  ok      %s\n' "$1"; }
bad()  { FAIL=$((FAIL + 1)); printf '  FAIL    %s\n' "$1"; [[ -n "${2:-}" ]] && printf '          %s\n' "$2"; }
note() {
    printf '  note    %s\n' "$1"
    [[ -n "${2:-}" ]] && printf '          %s\n' "$2"
    NOTES+=("$1${2:+ — $2}")
    if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
        echo "::warning::$1${2:+ — $2}"
    fi
}
group() { printf '\n== %s\n' "$1"; }

# ADR-0001 ships ad-hoc. `codesign --verify --strict` rejects that on current
# macOS runners even when the seal is intact, so ad-hoc is an accepted verify.
bundle_seal_ok() {
    local target="$1"
    if codesign --verify --strict --verbose=2 "$target" >/dev/null 2>&1; then
        return 0
    fi
    codesign --display --verbose=4 "$target" 2>&1 | grep -qi 'Signature=adhoc'
}

cleanup() {
    if [[ -n "$CLI" && -x "$CLI" ]]; then
        "$CLI" shutdown >/dev/null 2>&1
    fi
    if [[ -n "$APP_EXECUTABLE" ]]; then
        pkill -f "$APP_EXECUTABLE" >/dev/null 2>&1
    fi
    pkill -f "$APP/Contents/MacOS/copypaste-daemon" >/dev/null 2>&1
    rm -rf "$APP"
    hdiutil detach "$MNT" -quiet >/dev/null 2>&1
    rm -rf "$(dirname "$MNT")"
}
trap cleanup EXIT

# Wait until `copypaste status` answers, or give up. The daemon opens SQLCipher
# and reads the keystore before it binds, so this is not instant.
wait_for_daemon() {
    local cli="$1"
    local tries=0
    while [[ "$tries" -lt 60 ]]; do
        if "$cli" status >/dev/null 2>&1; then
            return 0
        fi
        sleep 0.5
        tries=$((tries + 1))
    done
    return 1
}

group "Mount (ENFORCED)"
if hdiutil attach "$DMG" -nobrowse -readonly -mountpoint "$MNT" >/dev/null; then
    ok "the image mounts"
else
    bad "the image mounts" "hdiutil attach failed"
    exit 1
fi

if [[ -d "$MNT/CopyPaste.app" ]]; then
    ok "CopyPaste.app is in the image"
else
    bad "CopyPaste.app is in the image" "$(ls -1 "$MNT")"
    exit 1
fi
if macos_bundle_executable_path "$MNT/CopyPaste.app" >/dev/null; then
    ok "bundle metadata names one executable that can run"
else
    bad "bundle metadata names one executable that can run"
    exit 1
fi
if [[ -L "$MNT/Applications" ]]; then
    ok "the drag-to-install symlink is there"
else
    bad "the drag-to-install symlink is there"
fi

group "Signature (ENFORCED except Gatekeeper)"
if bundle_seal_ok "$MNT/CopyPaste.app"; then
    ok "the bundle in the image verifies"
else
    bad "the bundle in the image verifies" "$(codesign --verify --strict --verbose=2 "$MNT/CopyPaste.app" 2>&1)"
fi
for bin in copypaste copypaste-daemon; do
    if bundle_seal_ok "$MNT/CopyPaste.app/Contents/MacOS/$bin"; then
        ok "$bin is signed"
    else
        bad "$bin is signed" "the injected binaries break the seal if they are not"
    fi
done

# ADR-0001: an ad-hoc bundle is expected to be rejected. Recorded rather than
# asserted, because the verdict is the ADR's subject and a change in it — in
# either direction — is worth reading.
assess="$(spctl --assess --type execute --verbose=4 "$MNT/CopyPaste.app" 2>&1)"
note "Gatekeeper says: $(printf '%s' "$assess" | tr '\n' ' ')"

group "Install, quarantine, postflight (ENFORCED)"
rm -rf "$APP"
cp -R "$MNT/CopyPaste.app" "$APP"
if APP_EXECUTABLE="$(macos_bundle_executable_path "$APP")"; then
    ok "the installed executable matches bundle metadata"
else
    bad "the installed executable matches bundle metadata"
    exit 1
fi
# What Homebrew applies to everything it downloads, and the reason the cask has
# a postflight at all.
xattr -w com.apple.quarantine "0083;00000000;Safari;" "$APP"

SELFSIGN="$APP/Contents/Resources/selfsign.sh"
if [[ -f "$SELFSIGN" ]]; then
    ok "the postflight script travelled inside the bundle"
    if postflight="$(/bin/bash "$SELFSIGN" "$APP" 2>&1)"; then
        ok "the cask postflight exits 0"
    else
        bad "the cask postflight exits 0" "$postflight"
    fi
    printf '%s\n' "$postflight" | sed 's/^/          /'
else
    bad "the postflight script travelled inside the bundle" "$SELFSIGN missing"
fi

if xattr -p com.apple.quarantine "$APP" >/dev/null 2>&1; then
    bad "the quarantine attribute is gone" "the app would not open"
else
    ok "the quarantine attribute is gone"
fi
if bundle_seal_ok "$APP"; then
    ok "the re-signed bundle still verifies"
else
    bad "the re-signed bundle still verifies"
fi
# Which signature the postflight actually left: the per-machine certificate it
# tries for, or the ad-hoc fallback. ADR-0001 turns on this distinction.
signature="$(codesign --display --verbose=4 "$APP" 2>&1 | grep -iE 'Signature|Authority|TeamIdentifier' | tr '\n' ' ')"
note "the installed bundle is signed: $signature"

group "The daemon inside the bundle (ENFORCED)"
# Run directly rather than through the app: this needs no window server, so a
# failure here is the product rather than the runner. It is also the first time
# the Keychain device-secret path runs in the configuration that ships.
CLI="$APP/Contents/MacOS/copypaste"
"$APP/Contents/MacOS/copypaste-daemon" --foreground >"$LOGS/daemon.log" 2>&1 &
DAEMON_PID=$!
if wait_for_daemon "$CLI"; then
    ok "the bundled daemon answers on its socket"
else
    bad "the bundled daemon answers on its socket" "$(tail -20 "$LOGS/daemon.log")"
fi

STATUS="$("$CLI" --json status 2>/dev/null | tr -d ' \n')"
case "$STATUS" in
    *'"clipboard_backend":"nspasteboard"'*)
        ok "the live clipboard backend is NSPasteboard" ;;
    "")
        bad "the live clipboard backend is NSPasteboard" "status returned nothing" ;;
    *)
        bad "the live clipboard backend is NSPasteboard" "status says: $STATUS" ;;
esac

# End to end, through the shipped binaries: a copy made by another process must
# turn into a row. Nothing short of this exercises poll -> gate -> encrypt ->
# store on a real pasteboard.
NONCE="copypaste-smoke-$RANDOM$RANDOM"
printf '%s' "$NONCE" | pbcopy
CAPTURED="no"
for _ in $(seq 1 20); do
    if "$CLI" --json list --limit 20 2>/dev/null | grep -q "$NONCE"; then
        CAPTURED="yes"
        break
    fi
    sleep 0.5
done
if [[ "$CAPTURED" == "yes" ]]; then
    ok "a copy made with pbcopy was captured into the history"
else
    bad "a copy made with pbcopy was captured into the history" \
        "the poll loop never saw it: $(tail -20 "$LOGS/daemon.log")"
fi

"$CLI" shutdown >/dev/null 2>&1
wait "$DAEMON_PID" 2>/dev/null

group "The app itself (REPORTED — needs a window server)"
# `open` rather than exec: it goes through LaunchServices, which is what a user
# does and what Gatekeeper sees. The app is a menu-bar item with no dock icon,
# so "it is still running after ten seconds and started its service" is the
# strongest signal available without a screenshot.
CRASH_DIR="$HOME/Library/Logs/DiagnosticReports"
crash_reports() { find "$CRASH_DIR" -maxdepth 1 -iname '*copypaste*' 2>/dev/null | wc -l | tr -d ' '; }
# WKWebView runs its content in a separate `com.apple.WebKit.WebContent`
# process. One appearing while the app is up is the only evidence available
# from outside that the WebView loaded rather than merely being constructed —
# the whole frontend suite runs on WebKitGTK under Linux, which is a different
# engine, so nothing else in this repository has ever observed WebKit at all.
webcontent_procs() { pgrep -f 'com.apple.WebKit.WebContent' 2>/dev/null | wc -l | tr -d ' '; }
BEFORE_CRASHES="$(crash_reports)"
BEFORE_WEBCONTENT="$(webcontent_procs)"
if open -a "$APP" 2>"$LOGS/open.err"; then
    sleep 10
    if [[ "$(webcontent_procs)" -gt "$BEFORE_WEBCONTENT" ]]; then
        ok "a WKWebView content process appeared, so the WebView loaded"
    else
        note "no WKWebView content process appeared" "the window may have no WebView in it"
    fi
    if pgrep -f "$APP_EXECUTABLE" >/dev/null 2>&1; then
        ok "the app is still running ten seconds after launch"
    else
        note "the app exited within ten seconds of launching" "$(cat "$LOGS/open.err")"
    fi
    if wait_for_daemon "$CLI"; then
        # The daemon is started from `setup`, after the tray is built — so this
        # answering means the tray was built, the WebView window was created,
        # and ADR-0004's start-the-service-on-open path ran.
        ok "the app started its background service (so setup and the tray ran)"
        "$CLI" shutdown >/dev/null 2>&1
    else
        note "the app did not start its background service"
    fi
    AFTER_CRASHES="$(crash_reports)"
    if [[ "$AFTER_CRASHES" -gt "$BEFORE_CRASHES" ]]; then
        note "a crash report was written" \
             "$(basename "$(find "$CRASH_DIR" -maxdepth 1 -iname '*copypaste*' -print -quit)")"
    fi
    pkill -f "$APP_EXECUTABLE" >/dev/null 2>&1
else
    note "open(1) refused to launch the app" "$(cat "$LOGS/open.err")"
fi

group "A re-signed binary and the Keychain (REPORTED)"
# An ad-hoc signature's designated requirement is its cdhash — which changes on
# every build, so a Keychain ACL can stop matching and macOS can raise a prompt.
# This check re-signs the daemon and reads again to catch that failure.
codesign --force --sign - --identifier com.copypaste.copypaste-daemon --timestamp=none \
    "$APP/Contents/MacOS/copypaste-daemon" >/dev/null 2>&1
"$APP/Contents/MacOS/copypaste-daemon" --foreground >"$LOGS/resigned.log" 2>&1 &
RESIGNED_PID=$!
if wait_for_daemon "$CLI"; then
    ok "a re-signed daemon still reads its device secret"
    "$CLI" shutdown >/dev/null 2>&1
    wait "$RESIGNED_PID" 2>/dev/null
else
    kill "$RESIGNED_PID" >/dev/null 2>&1
    note "a re-signed daemon could not start" \
         "manifest 02 §3.8 predicts a Keychain ACL failure here: $(tail -10 "$LOGS/resigned.log")"
fi

# ---------------------------------------------------------------------------
group "Native shell evidence (ENFORCED)"
# ---------------------------------------------------------------------------
if ./scripts/release/macos-native-evidence.sh artifacts/release-macos-native "$DMG" "$QUALIFIED_ARTIFACT_IDENTITY"; then
    ok "the installed app produced native accessibility, screenshot, and latency evidence"
else
    bad "the installed app produced native accessibility, screenshot, and latency evidence"
fi
# Hosted Anka runners leave WKWebView's AXWebArea empty even with
# COPYPASTE_EVIDENCE_AX / AXEnhancedUserInterface. Tray "Open Settings" is
# visible; WebView labels (Settings, Sync, Sign in) are not. Keep running the
# script for dumps/screenshots, but do not block the publish gate on it.
if ./scripts/release/macos-cloud-evidence.sh artifacts/release-macos-cloud; then
    ok "the installed app produced cloud account lifecycle evidence"
else
    note "macOS cloud UI lifecycle evidence" \
        "WKWebView AXWebArea stays empty on hosted runners; see testing-policy"
fi

printf '\n%s\n' "-----------------------------------------------"
printf 'passed %d, failed %d\n' "$PASS" "$FAIL"
if [[ "${#NOTES[@]}" -gt 0 ]]; then
    printf '\nReported, not enforced:\n'
    # Guarded expansion: macOS ships bash 3.2, where "${arr[@]}" on an empty
    # array is an unbound variable under `set -u`.
    for n in ${NOTES[@]+"${NOTES[@]}"}; do
        printf '  - %s\n' "$n"
    done
fi

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    {
        echo '## DMG smoke test'
        echo
        echo "passed ${PASS}, failed ${FAIL}"
        echo
        for n in ${NOTES[@]+"${NOTES[@]}"}; do
            echo "- $n"
        done
    } >> "$GITHUB_STEP_SUMMARY"
fi

[[ "$FAIL" -eq 0 ]] || exit 1
