#!/usr/bin/env bash
# What the app's polling costs the daemon.
#
# The app runs three pollers — history every POLL_ACTIVE_MS (3 s), status every
# STATUS_POLL_MS (2 s), peers every PEERS_POLL_MS (10 s). This measures the
# daemon-side CPU of each request against a primed history and prints what one
# minute of that pattern costs, so the constants can be argued about with a
# number.
#
#   scripts/profile/ipc-cost.sh [history_items] [requests_per_method]
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

ITEMS="${1:-10000}"
# Enough requests that the delta is many clock ticks: at 100 Hz, a method
# costing 0.1 ms needs hundreds of calls before the counter moves at all.
REQUESTS="${2:-500}"

trap stop_daemon EXIT
build_release
require_quiet
start_daemon

note "priming $ITEMS items"
python3 - "$ITEMS" >"$DAEMON_DATA_DIR/seed.json" <<'PY'
import json, sys
n = int(sys.argv[1])
body = "the quick brown fox jumps over the lazy dog " * 12
print(json.dumps({
    "items": [
        {"content": f"item {i:07d} {body}", "content_type": "text",
         "created_at": 1700000000000 + i * 1000, "pinned": False,
         "is_sensitive": False}
        for i in range(n)
    ],
    "skipped_non_text": 0, "skipped_sensitive": 0, "skipped_undecryptable": 0,
}))
PY
"$CLI" import "$DAEMON_DATA_DIR/seed.json"

# CPU the daemon burns while `$1` is issued `$REQUESTS` times, minus what it
# would have burnt idling for the same wall time.
measure() {
    local label="$1"; shift
    local c0 c1 t0 t1
    c0=$(cpu_ticks "$DAEMON_PID"); t0=$(date +%s.%N)
    for _ in $(seq 1 "$REQUESTS"); do "$CLI" "$@" >/dev/null 2>&1; done
    c1=$(cpu_ticks "$DAEMON_PID"); t1=$(date +%s.%N)
    awk -v c0="$c0" -v c1="$c1" -v t0="$t0" -v t1="$t1" -v n="$REQUESTS" \
        -v hz="$CLK_TCK" -v idle="$IDLE_CPU_PER_S" -v l="$label" \
        'BEGIN { w = t1 - t0; cpu = (c1 - c0) / hz - idle * w;
                 printf "  %-22s %7.2f ms/request  (%d in %.1f s)\n",
                        l, 1000 * cpu / n, n, w }'
}

note "idle baseline with $ITEMS items"
IDLE=$(sample_for "$DAEMON_PID" 20)
echo "$IDLE" | report_rates
IDLE_CPU_PER_S=$(echo "$IDLE" | awk '{ print $2 / $1 }')

note "per-request daemon CPU, load $(load_now)"
measure "list -n 200" list -n 200
measure "list -n 50" list -n 50
measure "status" status
measure "search" search fox -n 20

# What one refetch of a scrolled-open list costs.
#
# `useHistory` is a TanStack `useInfiniteQuery` with `refetchInterval` and no
# `maxPages`, and a refetch with no direction re-fetches **every loaded page**
# in order (`infiniteQueryBehavior.js`: `remainingPages = oldPages.length`). So
# a user who has scrolled to the bottom does not poll one page every 3 s, they
# poll all of them.
note "one refetch of a fully scrolled list, load $(load_now)"
c0=$(cpu_ticks "$DAEMON_PID"); t0=$(date +%s.%N)
cursor=""; page_count=0
while :; do
    if [[ -z "$cursor" ]]; then
        out=$("$CLI" --json list -n 200 2>/dev/null)
    else
        out=$("$CLI" --json list -n 200 --cursor "$cursor" 2>/dev/null)
    fi
    page_count=$((page_count + 1))
    cursor=$(tr -d ' \n' <<<"$out" | grep -o '"next_cursor":"[0-9a-f]*"' | cut -d'"' -f4)
    [[ -z "$cursor" ]] && break
done
c1=$(cpu_ticks "$DAEMON_PID"); t1=$(date +%s.%N)
awk -v c0="$c0" -v c1="$c1" -v t0="$t0" -v t1="$t1" -v p="$page_count" -v hz="$CLK_TCK" \
    'BEGIN { cpu = (c1 - c0) / hz;
             printf "  %d pages  %.3f s daemon CPU  %.2f s wall\n", p, cpu, t1 - t0;
             printf "  at POLL_ACTIVE_MS=3000 that is %.1f%% of one core, forever\n",
                    100 * cpu / 3 }'
