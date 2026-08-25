#!/usr/bin/env bash
# The macOS idle-daemon AFTER measurement, with the Keychain handled explicitly.
#
# Wraps daemon-idle.sh with the four things between a fresh clone and a
# comparable number: a route past the Keychain access prompt, the preconditions
# performance.md §2.2 was taken under, proof that the default keychain and
# search list were not touched, and the predicted figures printed beside the
# observed ones so the run can be read without the document.
#
#   scripts/profile/macos-keychain-preflight.sh          # first, always
#   NO_MDNS=1 scripts/profile/macos-idle-after.sh
#
# KEYCHAIN_ROUTE:
#   ephemeral   (default) builds the daemon with `dev-ephemeral-key`, then
#               COPYPASTE_EPHEMERAL_KEY short-circuits before any
#               Security-framework call. Nothing is read, written or prompted
#               for; shipped builds do not contain this path.
#   own-user    touch nothing and use the real Keychain. Correct in a throwaway
#               macOS user account, or when the preflight reported `ready`.
#   mint-fresh  delete the device-secret item so this run mints its own.
#               Requires NO_COPYPASTE_INSTALL=1 and refuses if a history
#               database is present.
#   signed      re-sign the built binaries with the machine's stable local
#               identity, so one 'Always Allow' outlives the next rebuild.
#
# Nothing here writes to `security default-keychain` or `security
# list-keychains`. That is the constraint this script exists to honour, and it
# is checked rather than asserted.
#
# Written for /bin/bash 3.2, which is what macOS ships.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
SERVICE="com.copypaste.daemon"
ACCOUNT="device-secret-key"
REAL_DATA_DIR="$HOME/Library/Application Support/com.copypaste.CopyPaste"
SIGNING_KC="$REAL_DATA_DIR/signing/copypaste-signing.keychain-db"
ROUTE="${KEYCHAIN_ROUTE:-ephemeral}"
WINDOW="${WINDOW:-300}"
INTERVAL="${INTERVAL:-500}"
LOG="${LOG:-$ROOT/target/macos-idle-after-$(date +%Y%m%d-%H%M%S).log}"

die()  { printf '\033[31m✗ %s\033[0m\n' "$*" >&2; exit 1; }
note() { printf '\033[1;36m▶ %s\033[0m\n' "$*" >&2; }

[ "$(uname -s)" = Darwin ] || die "this is the macOS measurement; this host is $(uname -s)"

# Preconditions. Each one, if violated, makes the comparison with §2.2 void
# rather than merely noisy — so each is a refusal, not a warning.
[ "$INTERVAL" = 500 ] || die "§2.2 was taken at poll_interval_ms=500, not $INTERVAL"
[ "${NO_MDNS:-0}" = 1 ] || die "set NO_MDNS=1: otherwise other daemons' advertisements are counted as this one's idle cost (§2.1)"
# F-IDLE-2's early return is keyed on `is_configured`, which is false only when
# no deployment reaches the daemon. With either of these set the branch under
# measurement is not the branch that runs.
[ -z "${COPYPASTE_CLOUD_URL:-}" ] || die "COPYPASTE_CLOUD_URL is set; F-IDLE-2's unconfigured path would not be exercised"
[ -z "${COPYPASTE_CLOUD_ANON_KEY:-}" ] || die "COPYPASTE_CLOUD_ANON_KEY is set; see above"

DEFAULT_BEFORE="$(security default-keychain 2>/dev/null)"
LIST_BEFORE="$(security list-keychains 2>/dev/null)"
MINTED_HERE=0

# Installed before the first route acts, so the "did this change your default
# keychain?" answer is printed even when a route dies half-way through.
cleanup() {
    if [ "$MINTED_HERE" = 1 ]; then
        security delete-generic-password -s "$SERVICE" -a "$ACCOUNT" >/dev/null 2>&1 &&
            note "removed the item this run minted"
    fi
    DEFAULT_AFTER="$(security default-keychain 2>/dev/null)"
    LIST_AFTER="$(security list-keychains 2>/dev/null)"
    if [ "$DEFAULT_BEFORE" = "$DEFAULT_AFTER" ] && [ "$LIST_BEFORE" = "$LIST_AFTER" ]; then
        note "default keychain and search list unchanged"
    else
        printf '\033[31m✗ the default keychain or search list CHANGED. Restore it:\033[0m\n' >&2
        printf '  was default: %s\n  now default: %s\n' "$DEFAULT_BEFORE" "$DEFAULT_AFTER" >&2
    fi
}
trap cleanup EXIT

# The route
AUTH_WAIT=20

case "$ROUTE" in
ephemeral)
    # I-23: read once into a OnceLock, before any Security-framework call. The
    # secret is thrown away with the mktemp data directory the run created.
    export COPYPASTE_EPHEMERAL_KEY=1
    note "route ephemeral: no keystore is touched at all"
    ;;

own-user)
    note "route own-user: the Keychain is left exactly as it is"
    ;;

mint-fresh)
    # A write creates the item's ACL instead of consulting one, so a daemon that
    # mints its own secret never prompts however often it is rebuilt. The cost
    # is the existing item, and that item is somebody's history.
    [ "${NO_COPYPASTE_INSTALL:-0}" = 1 ] ||
        die "mint-fresh deletes the device-secret item. Set NO_COPYPASTE_INSTALL=1 only after the preflight has reported no history database."
    [ ! -f "$REAL_DATA_DIR/copypaste-v2.db" ] ||
        die "a v2 history database is present: deleting its device secret would make it permanently unopenable (AGENTS.md rule 4)"
    note "route mint-fresh: removing the device-secret item so this run mints its own"
    if security delete-generic-password -s "$SERVICE" -a "$ACCOUNT" >/dev/null 2>&1; then
        note "  removed"
    else
        note "  nothing to remove (or the delete was refused — the run will say which)"
    fi
    MINTED_HERE=1
    ;;

signed)
    # The argument selfsign.sh makes for TCC, applied to a Keychain ACL: a
    # signature made with a stable certificate has a designated requirement that
    # does not move, so a grant given once should still match after the next
    # build. Should. Whether a Keychain ACL keys on the designated requirement
    # the way TCC does is unobserved — this route is also the test of it
    # (manifest 02 §5, "the ad-hoc-signature question").
    [ -f "$SIGNING_KC" ] || die "no local signing identity; run packaging/macos/selfsign.sh against an installed bundle first"
    SHA1="$(security find-identity -v "$SIGNING_KC" 2>/dev/null |
        awk '/CopyPaste Local Signing/ { for (i = 1; i <= NF; i++) if ($i ~ /^[0-9A-F]{40}$/) { print $i; exit } }')"
    [ -n "$SHA1" ] || die "the local signing certificate was not found in its keychain"
    AUTH_WAIT=180
    note "route signed: re-signing with $SHA1"
    ;;
*)
    die "KEYCHAIN_ROUTE must be ephemeral, own-user, mint-fresh or signed (got: $ROUTE)"
    ;;
esac

# Build, sign if asked, check it can start, measure
CARGO="cargo"
cargo +1.96 --version >/dev/null 2>&1 && CARGO="cargo +1.96"
note "building (release)"
if [ "$ROUTE" = ephemeral ]; then
    $CARGO build --release -p copypaste-daemon -p copypaste-cli \
        --features copypaste-daemon/dev-ephemeral-key || die "the build failed"
else
    $CARGO build --release -p copypaste-daemon -p copypaste-cli || die "the build failed"
fi

if [ "$ROUTE" = signed ]; then
    # Unlock the signing keychain the way selfsign.sh does, so codesign does not
    # raise a second prompt on the way to suppressing the first.
    PASSFILE="$REAL_DATA_DIR/signing/keychain-password"
    [ -s "$PASSFILE" ] &&
        security unlock-keychain -p "$(cat "$PASSFILE")" "$SIGNING_KC" >/dev/null 2>&1
    # Signed after the build, never before: cargo would overwrite the signature
    # with its own output. daemon-idle.sh calls `cargo build` again, a no-op
    # against an up-to-date cache — if it is not a no-op, the run has silently
    # reverted to an ad-hoc identity and the prompt comes back.
    for bin in copypaste-daemon copypaste; do
        codesign --force --sign "$SHA1" --identifier "com.copypaste.$bin" \
            --timestamp=none --keychain "$SIGNING_KC" \
            "$ROOT/target/release/$bin" 2>&1 | sed 's/^/  /'
    done
    note "answer the prompt with 'Always Allow'. Then rebuild and run this again:"
    note "a second prompt means a Keychain ACL does not key on the designated"
    note "requirement, and this route does not work."
fi

# Can this binary serve a history at all? Asked once, here, before committing to
# a ${WINDOW}s window. daemon-idle.sh's readiness poll gives up after 20 s and
# says only "did not become ready" — the same sentence for a Keychain prompt, a
# halted daemon and a broken build. A halted daemon matters most: it runs no
# capture loop and no sync loops, so measuring one would report a spectacular
# improvement that never happened.
[ "$AUTH_WAIT" = 20 ] || note "waiting up to ${AUTH_WAIT}s for you to answer the prompt"
PROBE_DIR="$(mktemp -d)"
(
    export COPYPASTE_SOCKET="$PROBE_DIR/daemon.sock"
    "$ROOT/target/release/copypaste-daemon" --foreground --data-dir "$PROBE_DIR" \
        >"$PROBE_DIR/daemon.log" 2>&1 &
    echo $! >"$PROBE_DIR/pid"
    i=0
    while [ "$i" -lt $((AUTH_WAIT * 5)) ]; do
        COPYPASTE_SOCKET="$PROBE_DIR/daemon.sock" "$ROOT/target/release/copypaste" status \
            >/dev/null 2>&1 && exit 0
        kill -0 "$(cat "$PROBE_DIR/pid")" 2>/dev/null || exit 2
        sleep 0.2
        i=$((i + 1))
    done
    exit 1
)
PROBE_STATUS=$?
PROBE_TAIL="$(tail -6 "$PROBE_DIR/daemon.log" 2>/dev/null)"
PROBE_HALTED=1
grep -q 'holding the socket' "$PROBE_DIR/daemon.log" 2>/dev/null || PROBE_HALTED=0
kill "$(cat "$PROBE_DIR/pid" 2>/dev/null)" 2>/dev/null
rm -rf "$PROBE_DIR"
case "$PROBE_STATUS" in
    0) note "the daemon serves a history; measuring" ;;
    *) printf '%s\n' "$PROBE_TAIL" >&2
       if [ "$PROBE_HALTED" = 1 ]; then
           die "the daemon halted: it bound the socket to report a keyring or database failure, and a halted daemon is not what §2.2 measured. Its last lines are above."
       fi
       die "the daemon never became ready. Run scripts/profile/macos-keychain-preflight.sh and pick another route." ;;
esac

note "measuring: ${WINDOW}s at ${INTERVAL}ms, NO_MDNS=1, route $ROUTE"
NO_MDNS=1 "$HERE/daemon-idle.sh" "$WINDOW" "$INTERVAL" 2>&1 | tee "$LOG"

# What was expected, beside what happened
OBS_WAKE="$(awk '/Wakeups:/ { print $NF; exit }' "$LOG" | sed 's|/min||')"
OBS_CPU="$(awk '/CPU-s\/hour/ { print $(NF - 1); exit }' "$LOG")"
OBS_THREADS="$(awk '/Threads:/ { print $NF; exit }' "$LOG")"

cat <<REPORT

================================================================
  performance.md §2.2, host M, 300 s window — before vs observed

                   before      predicted        observed
  Wakeups/min      281.7       268 - 274        ${OBS_WAKE:-?}
  CPU-s/hour       0.012       0.012            ${OBS_CPU:-?}
  Threads          17          16               ${OBS_THREADS:-?}

  The predictions are arithmetic on f09f7334, not a measurement:
    F-IDLE-1  the paste-file sweeper is no longer spawned at
              construction, so a daemon that never pastes a file back
              carries one fewer OS thread and one fewer 30 s timer.
    F-IDLE-2  cloud refresh returns before its loop when no deployment
              is configured, removing a 10 s tick — 6 timer expiries a
              minute, and §2.1 measured about two wakeups per tick.

  Read it like this:
    threads still 17      F-IDLE-1 did not take on this platform. The
                          strongest single assertion in the run.
    threads 18            a timed-out Keychain read left keystore-load
                          parked on a prompt. Discard the run.
    wakeups >= 281.7      neither fix reached the idle path, or
                          something else regressed. These two changes
                          cannot raise the number.
    wakeups well under
    250                   something beyond these two moved. Do not
                          attribute it to them without the per-thread
                          table above.
    CPU well above
    0.012                 a regression on the real NSPasteboard path.
                          §2.2's figure is already at the counter floor.

  Record the load printed above: §2.2's before-figure was taken at 24.7.
  Log: $LOG
================================================================
REPORT
