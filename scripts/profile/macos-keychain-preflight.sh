#!/usr/bin/env bash
# Why daemon-idle.sh does not complete on macOS, answered on the machine itself.
#
# Leaves the Keychain as it found it: it never deletes or unlocks an existing
# item, never changes the default keychain or the search list, and always exits
# 0 — it is a report, not a gate. Two caveats it states rather than hides. The
# start probe can raise the very prompt it is diagnosing; cancel that dialog if
# it appears. And on a Mac with no device-secret item the probed daemon mints
# one, which the probe then removes again.
#
#   scripts/profile/macos-keychain-preflight.sh
#
# docs/rewrite/macos-idle-measurement.md decides what to do with the answer.
# Written for /bin/bash 3.2, which is what macOS ships.
set -uo pipefail

SERVICE="com.copypaste.daemon"
ACCOUNT="device-secret-key"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DAEMON="$ROOT/target/release/copypaste-daemon"
CLI="$ROOT/target/release/copypaste"
REAL_DATA_DIR="$HOME/Library/Application Support/com.copypaste.CopyPaste"
START_TIMEOUT="${START_TIMEOUT:-25}"

say()   { printf '  %s\n' "$*"; }
head_() { printf '\n== %s\n' "$*"; }

if [ "$(uname -s)" != Darwin ]; then
    echo "This probe only says anything on macOS (this host is $(uname -s))." >&2
    exit 0
fi

head_ "Session"
say "user:      $(whoami)"
say "session:   $(launchctl managername 2>/dev/null || echo unknown)"
say "os:        $(sw_vers -productVersion 2>/dev/null) ($(uname -r))"
say "load:      $(sysctl -n vm.loadavg | tr -d '{}')"
# Only an Aqua session can display SecurityAgent. In Background or StandardIO a
# read that would prompt returns errSecInteractionNotAllowed instead.
case "$(launchctl managername 2>/dev/null)" in
    Aqua) say "-> a prompt can be displayed and answered here" ;;
    *)    say "-> no window server: a prompting read fails rather than waiting" ;;
esac

head_ "Keychains (recorded so a later run can prove it changed none of them)"
security default-keychain 2>/dev/null | sed 's/^/  default: /'
security list-keychains 2>/dev/null | sed 's/^/  search:  /'
security show-keychain-info "$HOME/Library/Keychains/login.keychain-db" 2>&1 |
    sed 's/^/  login:   /'

head_ "Is there a real install on this Mac?"
# The question that decides whether the device-secret item may be touched at
# all. Deleting it leaves an installed history unopenable, and CLAUDE.md rule 4
# puts data loss above every other outcome.
if [ -f "$REAL_DATA_DIR/copypaste-v2.db" ]; then
    say "YES — a v2 history database is in the application-support directory."
    say "     Its device-secret item is load-bearing. Do not delete it."
    INSTALL_PRESENT=1
else
    say "no v2 history database in the application-support directory"
    INSTALL_PRESENT=0
fi

head_ "The device-secret item"
# Attributes only: no -w and no -g, so nothing is decrypted and no ACL is
# consulted. This is the one question about the item that can be asked without
# risking the prompt being diagnosed.
ITEM="$(security find-generic-password -s "$SERVICE" -a "$ACCOUNT" 2>&1)"
if printf '%s' "$ITEM" | grep -q 'could not be found'; then
    say "absent from every keychain in the search list"
    say "-> a first run would MINT one. A write creates the ACL rather than"
    say "   consulting it, so minting does not prompt."
    ITEM_PRESENT=0
else
    ITEM_PRESENT=1
    printf '%s\n' "$ITEM" | sed -n '1,2p' | sed 's/^/  /'
    say "-> present. A binary whose code identity is not in its ACL must be"
    say "   authorised before it can read it."
fi

head_ "The binary that would read it"
if [ -x "$DAEMON" ]; then
    # The cdhash is the whole argument. An ad-hoc or unsigned binary's
    # designated requirement is this value, so every `cargo build` produces a
    # different principal as far as the ACL is concerned (manifest 02 §3.8).
    codesign -dvvv "$DAEMON" 2>&1 | grep -E '^(CDHash|Signature|Identifier|Authority)' |
        sed 's/^/  /'
    say "-> record this CDHash. Rebuild and run again: if it moved, any"
    say "   'Always Allow' granted to the previous build no longer applies."
else
    say "not built: $DAEMON"
    say "-> cargo build --release -p copypaste-daemon -p copypaste-cli"
fi

head_ "A stable local signing identity"
# packaging/macos/selfsign.sh keeps one per machine. Signing the profiling
# binary with it is what would make an ACL grant survive a rebuild — if a
# Keychain ACL keys on the designated requirement the way a TCC grant does,
# which is exactly what nobody has observed.
SIGNING_KC="$REAL_DATA_DIR/signing/copypaste-signing.keychain-db"
if [ -f "$SIGNING_KC" ]; then
    security find-identity -v "$SIGNING_KC" 2>/dev/null | sed 's/^/  /'
else
    say "none — selfsign.sh has not run on this machine"
fi

# ---------------------------------------------------------------------------
# Does the daemon start? Bounded, and told apart from the three things that
# look the same from outside: halted, exited, still waiting on a prompt.
# ---------------------------------------------------------------------------
probe_start() {
    local dir pid i
    dir="$(mktemp -d)"
    (
        export COPYPASTE_SOCKET="$dir/daemon.sock"
        "$DAEMON" --foreground --data-dir "$dir" >"$dir/daemon.log" 2>&1 &
        echo $! >"$dir/pid"
        i=0
        while [ "$i" -lt $((START_TIMEOUT * 5)) ]; do
            COPYPASTE_SOCKET="$dir/daemon.sock" "$CLI" status >/dev/null 2>&1 && exit 0
            kill -0 "$(cat "$dir/pid")" 2>/dev/null || exit 2
            sleep 0.2
            i=$((i + 1))
        done
        exit 1
    )
    case $? in
        0) PROBE_VERDICT=ready ;;
        2) PROBE_VERDICT=exited ;;
        # "holding the socket to say why" is what startup::halt_or_fail logs
        # before serve_halted binds. A halted daemon is alive and answering, and
        # is the failure that would otherwise be read as a hang.
        *) if grep -q 'holding the socket' "$dir/daemon.log" 2>/dev/null; then
               PROBE_VERDICT=halted
           else
               PROBE_VERDICT=blocked
           fi ;;
    esac
    PROBE_LOG="$(tail -6 "$dir/daemon.log" 2>/dev/null)"
    pid="$(cat "$dir/pid" 2>/dev/null)"
    [ -n "$pid" ] && kill "$pid" 2>/dev/null
    rm -rf "$dir"
}

if [ ! -x "$DAEMON" ] || [ ! -x "$CLI" ]; then
    head_ "Does the daemon start?"
    say "skipped: build the release binaries first"
    PROBE_VERDICT=unknown
    BYPASS_VERDICT=unknown
else
    head_ "Does the daemon start? (login Keychain, bounded ${START_TIMEOUT}s)"
    probe_start
    case "$PROBE_VERDICT" in
        ready)
            say "ready — this binary reads the device secret without prompting."
            say "-> measure NOW, before the next rebuild moves the CDHash."
            ;;
        halted)
            say "halted: the keyring failed, and the daemon bound the socket to"
            say "say so rather than exiting (server/halted.rs). Its last lines:"
            printf '%s\n' "$PROBE_LOG" | sed 's/^/    /'
            say "-> an 8 s Keychain timeout, a locked keychain or a denied ACL."
            ;;
        exited)
            say "the daemon exited before becoming ready. Its last lines:"
            printf '%s\n' "$PROBE_LOG" | sed 's/^/    /'
            ;;
        blocked)
            say "alive, not answering, and it did not log the halted path."
            say "-> look for a SecurityAgent dialog on the console display."
            ;;
    esac
    # The probe may only observe. If there was no item before and the daemon
    # minted one, take it back out — otherwise this report would have quietly
    # changed the state the next run depends on.
    if [ "$ITEM_PRESENT" = 0 ] &&
        ! security find-generic-password -s "$SERVICE" -a "$ACCOUNT" 2>&1 |
            grep -q 'could not be found'; then
        security delete-generic-password -s "$SERVICE" -a "$ACCOUNT" >/dev/null 2>&1 &&
            say "(removed the item the probe minted)"
    fi

    head_ "Does it start under COPYPASTE_EPHEMERAL_KEY? (touches no keystore)"
    # Keyring::load_or_create short-circuits on this before any
    # Security-framework call, so the answer here should not depend on anything
    # above. If it does, the block is not the Keychain.
    export COPYPASTE_EPHEMERAL_KEY=1
    probe_start
    BYPASS_VERDICT="$PROBE_VERDICT"
    unset COPYPASTE_EPHEMERAL_KEY
    if [ "$BYPASS_VERDICT" = ready ]; then
        say "ready — the bypass works and the measurement can run unattended."
    else
        say "$BYPASS_VERDICT — the bypass did NOT help, so the Keychain is not"
        say "what is stopping the daemon. Its last lines:"
        printf '%s\n' "$PROBE_LOG" | sed 's/^/    /'
    fi
fi

printf '\n\033[1mRecommended route\033[0m\n'
if [ "$BYPASS_VERDICT" = ready ]; then
    echo "  COPYPASTE_EPHEMERAL_KEY. Nothing is written to any keychain, nothing"
    echo "  needs answering, and a throwaway --data-dir never needed the real"
    echo "  device secret in the first place:"
    echo "      NO_MDNS=1 scripts/profile/macos-idle-after.sh"
elif [ "$PROBE_VERDICT" = ready ]; then
    echo "  KEYCHAIN_ROUTE=own-user, and do NOT rebuild first — the build is what"
    echo "  breaks the ACL match."
elif [ "$INSTALL_PRESENT" = 0 ]; then
    echo "  No history on this Mac, so the item protects nothing:"
    echo "      KEYCHAIN_ROUTE=mint-fresh NO_COPYPASTE_INSTALL=1"
else
    echo "  A real install is here and its secret must survive. Measure in a"
    echo "  throwaway macOS user account (KEYCHAIN_ROUTE=own-user), or authorise"
    echo "  this build once by hand and use KEYCHAIN_ROUTE=signed."
fi
echo
echo "  Every route, what each costs and what each would falsify:"
echo "  docs/rewrite/macos-idle-measurement.md"
exit 0
