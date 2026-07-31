#!/usr/bin/env bash
# Shared helpers for the profile scripts: build once, run a daemon on a
# throwaway data directory, and read its cost out of /proc.
#
# /proc directly rather than `/usr/bin/time -v` or `perf stat`: neither is
# installed on the container these numbers were taken on, and both read the
# same counters. `utime`/`stime` in `/proc/<pid>/stat` already aggregate every
# thread; the context-switch counters in `/proc/<pid>/status` do **not** — they
# are the leader thread's alone — so they are summed over `task/*`.

PROFILE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CARGO="cargo"
cargo +1.96 --version >/dev/null 2>&1 && CARGO="cargo +1.96"

DAEMON="$PROFILE_ROOT/target/release/copypaste-daemon"
CLI="$PROFILE_ROOT/target/release/copypaste"
CLK_TCK=$(getconf CLK_TCK)

note() { printf '\033[1;36m▶ %s\033[0m\n' "$*" >&2; }
warn() { printf '\033[33m! %s\033[0m\n' "$*" >&2; }
die()  { printf '\033[31m✗ %s\033[0m\n' "$*" >&2; exit 1; }

build_release() {
    note "building (release)"
    $CARGO build --release -p copypaste-daemon -p copypaste-cli >&2
}

# A number is not a number if six other builds were running while it was taken.
# Reported, not enforced: on a shared host the honest thing is to print the load
# beside the measurement rather than refuse to measure.
load_now() { cut -d' ' -f1 </proc/loadavg; }

require_quiet() {
    local limit="${1:-1.5}" load
    load=$(load_now)
    if awk -v l="$load" -v m="$limit" 'BEGIN { exit !(l > m) }'; then
        warn "1-minute load average is $load (> $limit): this number is contended"
    fi
}

# Start a daemon on its own XDG_DATA_HOME. Sets DAEMON_PID and DAEMON_DATA_DIR.
start_daemon() {
    DAEMON_DATA_DIR="$(mktemp -d)"
    export XDG_DATA_HOME="$DAEMON_DATA_DIR"
    [[ -x "$DAEMON" ]] || die "no daemon binary; run build_release first"
    "$DAEMON" --foreground >"$DAEMON_DATA_DIR/daemon.log" 2>&1 &
    DAEMON_PID=$!
    for _ in $(seq 1 100); do
        "$CLI" status >/dev/null 2>&1 && return 0
        sleep 0.2
    done
    die "daemon did not become ready (see $DAEMON_DATA_DIR/daemon.log)"
}

stop_daemon() {
    [[ -n "${DAEMON_PID:-}" ]] && kill "$DAEMON_PID" 2>/dev/null
    wait "${DAEMON_PID:-}" 2>/dev/null
    [[ -n "${DAEMON_DATA_DIR:-}" ]] && rm -rf "$DAEMON_DATA_DIR"
    DAEMON_PID=""
}

# CPU jiffies (user+system, all threads) for a pid.
#
# `comm` is field 2 and may contain spaces and brackets, so everything up to
# the last `) ` goes first; `utime`/`stime` are then fields 12 and 13. Counting
# whitespace fields from the left instead reads `majflt`, which is always 0 at
# idle and looks exactly like a daemon that uses no CPU.
cpu_ticks() {
    awk '{ sub(/^.*\) /, ""); print $12 + $13 }' "/proc/$1/stat"
}

# Context switches summed over every thread — the wakeup proxy.
ctxt_switches() {
    awk '/_ctxt_switches/ { total += $2 } END { print total + 0 }' \
        /proc/"$1"/task/*/status 2>/dev/null
}

rss_kb()  { awk '/^VmRSS:/ { print $2 }' "/proc/$1/status"; }
threads() { awk '/^Threads:/ { print $2 }' "/proc/$1/status"; }

# `tid comm switches` per thread — what a wakeup total is made of.
thread_snapshot() {
    local t tid
    for t in /proc/"$1"/task/*; do
        tid="${t##*/}"
        printf '%s %s %s\n' "$tid" "$(cat "$t/comm" 2>/dev/null)" \
            "$(awk '/_ctxt_switches/ { s += $2 } END { print s + 0 }' "$t/status" 2>/dev/null)"
    done
}

thread_delta() {
    awk 'NR == FNR { was[$1] = $3; next }
         { d = $3 - (($1 in was) ? was[$1] : 0); if (d > 0) printf "    %-18s %8d\n", $2, d }' \
        "$1" "$2" | sort -k2 -rn
}

# Sample a pid for N seconds and print one line of
# `wall cpu_s ctxt rss_start_kb rss_end_kb threads`.
#
# With PROFILE_THREAD_DUMP set to a path prefix, the per-thread counters are
# written beside it so a wakeup total can be attributed to a loop.
sample_for() {
    local pid="$1" seconds="$2"
    local t0 t1 c0 c1 x0 x1 r0 r1
    t0=$(date +%s.%N); c0=$(cpu_ticks "$pid"); x0=$(ctxt_switches "$pid")
    r0=$(rss_kb "$pid")
    [[ -n "${PROFILE_THREAD_DUMP:-}" ]] && thread_snapshot "$pid" >"$PROFILE_THREAD_DUMP.before"
    sleep "$seconds"
    t1=$(date +%s.%N); c1=$(cpu_ticks "$pid"); x1=$(ctxt_switches "$pid")
    r1=$(rss_kb "$pid")
    [[ -n "${PROFILE_THREAD_DUMP:-}" ]] && thread_snapshot "$pid" >"$PROFILE_THREAD_DUMP.after"
    awk -v t0="$t0" -v t1="$t1" -v c0="$c0" -v c1="$c1" -v x0="$x0" -v x1="$x1" \
        -v r0="$r0" -v r1="$r1" -v hz="$CLK_TCK" -v th="$(threads "$pid")" \
        'BEGIN { printf "%.3f %.4f %d %d %d %d\n",
                        t1 - t0, (c1 - c0) / hz, x1 - x0, r0, r1, th }'
}

# `wall cpu_s ctxt …` -> the two figures that stand in for battery.
report_rates() {
    awk '{ printf "  CPU:      %.3f s over %.1f s  =  %.1f%%  =  %.1f CPU-s/hour\n",
                  $2, $1, 100 * $2 / $1, 3600 * $2 / $1;
           printf "  Wakeups:  %d over %.1f s  =  %.1f/min\n", $3, $1, 60 * $3 / $1;
           printf "  RSS:      %d KB -> %d KB   Threads: %d\n", $4, $5, $6 }'
}
