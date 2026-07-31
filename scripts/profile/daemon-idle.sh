#!/usr/bin/env bash
# What an idle daemon costs: CPU-seconds per hour and wakeups per minute.
#
# Those two are the battery proxy. Battery itself cannot be measured here, and
# the clipboard backend on this host is the fake one — see the transferability
# note in docs/rewrite/performance.md before quoting any of this for macOS.
#
# NO_MDNS=1 turns LAN visibility off and restarts before measuring. Do that
# whenever the number has to be the daemon's *own* cost: the mDNS browse thread
# wakes on every `_copypaste._tcp` advertisement on the segment, so on a host
# where other daemons are running it dominates the total and reports their
# traffic as this daemon's idle cost.
#
#   scripts/profile/daemon-idle.sh [seconds] [poll_interval_ms ...]
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

SECONDS_PER_RUN="${1:-60}"
shift || true
INTERVALS=("${@:-500}")

trap stop_daemon EXIT
build_release
require_quiet

for interval in "${INTERVALS[@]}"; do
    start_daemon
    "$CLI" config set --poll-interval-ms "$interval" >/dev/null
    if [[ "${NO_MDNS:-0}" == 1 ]]; then
        # `lan_visibility` is read once, at start: the registration is made
        # there or not at all. So it takes a restart, not a reconfigure.
        "$CLI" config set --lan-visibility false >/dev/null
        kill "$DAEMON_PID" 2>/dev/null; wait "$DAEMON_PID" 2>/dev/null
        "$DAEMON" --foreground >>"$DAEMON_DATA_DIR/daemon.log" 2>&1 &
        DAEMON_PID=$!
        for _ in $(seq 1 100); do "$CLI" status >/dev/null 2>&1 && break; sleep 0.2; done
    fi
    # One interval of settling, so the reconfigure itself is outside the window.
    sleep 2

    note "idle, poll_interval_ms=$interval, ${SECONDS_PER_RUN}s, load $(load_now)"
    PROFILE_THREAD_DUMP="$DAEMON_DATA_DIR/threads" \
        sample_for "$DAEMON_PID" "$SECONDS_PER_RUN" | report_rates
    echo "  Wakeups by thread:"
    thread_delta "$DAEMON_DATA_DIR/threads.before" "$DAEMON_DATA_DIR/threads.after"
    stop_daemon
done
