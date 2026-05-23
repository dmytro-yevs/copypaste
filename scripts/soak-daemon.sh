#!/usr/bin/env bash
# soak-daemon.sh — Long-running soak test for copypaste-daemon.
#
# Spawns the daemon, then drives it with a configurable insert/list/delete
# loop while sampling RSS (KB) and CPU (%) every 30 seconds via `ps`.
# At the end, prints an ASCII memory curve and flags >10% RSS growth from
# the first stable sample to the last sample as a probable leak regression.
#
# Usage:
#   bash scripts/soak-daemon.sh                                   # 1h, 10 ops/s
#   bash scripts/soak-daemon.sh --duration 600 --rate 5
#   bash scripts/soak-daemon.sh --report-file reports/perf/soak-$(date +%s).csv
#   bash scripts/soak-daemon.sh --dry-run
#   bash scripts/soak-daemon.sh --help
#
# Companion analyzer: scripts/soak-report.sh
#
# Exit codes:
#   0  success, no regression
#   1  usage / setup error
#   2  daemon failed to start
#   3  memory growth >= threshold (likely leak)

set -euo pipefail

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
DURATION=3600          # seconds (1 hour)
RATE=10                # ops/sec across the driver loop
SAMPLE_INTERVAL=30     # seconds between ps samples
GROWTH_THRESHOLD=10    # percent — flag regression if RSS grows >= this
REPORT_FILE=""         # defaults to reports/perf/soak-<epoch>.csv
DRY_RUN=0
DAEMON_BIN=""
CLI_BIN=""

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
usage() {
    sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'
}

die() {
    echo "soak-daemon: $*" >&2
    exit 1
}

log() {
    printf '[%s] %s\n' "$(date '+%H:%M:%S')" "$*"
}

require() {
    command -v "$1" >/dev/null 2>&1 || die "missing required tool: $1"
}

# ---------------------------------------------------------------------------
# Arg parsing
# ---------------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
    case "$1" in
        --duration)        DURATION="$2"; shift 2 ;;
        --rate)            RATE="$2"; shift 2 ;;
        --sample-interval) SAMPLE_INTERVAL="$2"; shift 2 ;;
        --threshold)       GROWTH_THRESHOLD="$2"; shift 2 ;;
        --report-file)     REPORT_FILE="$2"; shift 2 ;;
        --daemon-bin)      DAEMON_BIN="$2"; shift 2 ;;
        --cli-bin)         CLI_BIN="$2"; shift 2 ;;
        --dry-run)         DRY_RUN=1; shift ;;
        -h|--help)         usage; exit 0 ;;
        *) die "unknown arg: $1 (try --help)" ;;
    esac
done

# Validate numbers
[[ "$DURATION" =~ ^[0-9]+$ ]]        || die "--duration must be integer seconds"
[[ "$RATE" =~ ^[0-9]+$ ]]            || die "--rate must be integer ops/sec"
[[ "$SAMPLE_INTERVAL" =~ ^[0-9]+$ ]] || die "--sample-interval must be integer"
[[ "$GROWTH_THRESHOLD" =~ ^[0-9]+$ ]] || die "--threshold must be integer percent"
(( RATE >= 1 ))                       || die "--rate must be >= 1"
(( DURATION >= SAMPLE_INTERVAL ))     || die "--duration must be >= --sample-interval"

# ---------------------------------------------------------------------------
# Resolve binaries
# ---------------------------------------------------------------------------
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
: "${DAEMON_BIN:=$REPO_ROOT/target/release/copypaste-daemon}"
: "${CLI_BIN:=$REPO_ROOT/target/release/copypaste}"

# Report file default after REPO_ROOT is known
if [[ -z "$REPORT_FILE" ]]; then
    REPORT_FILE="$REPO_ROOT/reports/perf/soak-$(date +%s).csv"
fi

require ps
require awk
require date

# ---------------------------------------------------------------------------
# Dry-run: print plan and exit
# ---------------------------------------------------------------------------
print_plan() {
    cat <<EOF
soak-daemon plan
================
  duration         : ${DURATION}s ($((DURATION/60)) min)
  driver rate      : ${RATE} ops/sec
  sample interval  : ${SAMPLE_INTERVAL}s
  growth threshold : ${GROWTH_THRESHOLD}% (peak RSS vs first stable sample)
  daemon bin       : $DAEMON_BIN
  cli bin          : $CLI_BIN
  report file      : $REPORT_FILE

driver cycle (per second, scaled by --rate):
  1. insert text via CLI clipboard write path
  2. list recent items
  3. delete one item

samples (every ${SAMPLE_INTERVAL}s) capture:
  epoch_secs, elapsed_secs, rss_kb, cpu_percent

end-of-run:
  - ASCII curve of RSS over time
  - peak / mean / final RSS
  - flag if (final - first_stable) / first_stable >= ${GROWTH_THRESHOLD}%
EOF
}

if (( DRY_RUN )); then
    print_plan
    exit 0
fi

# ---------------------------------------------------------------------------
# Pre-flight checks (only when actually running)
# ---------------------------------------------------------------------------
[[ -x "$DAEMON_BIN" ]] || die "daemon binary not found or not executable: $DAEMON_BIN"
[[ -x "$CLI_BIN" ]]    || die "cli binary not found or not executable: $CLI_BIN"

mkdir -p "$(dirname "$REPORT_FILE")"

# ---------------------------------------------------------------------------
# Spawn daemon
# ---------------------------------------------------------------------------
DAEMON_LOG="$(dirname "$REPORT_FILE")/soak-daemon-$$.log"
log "spawning daemon: $DAEMON_BIN (log: $DAEMON_LOG)"
"$DAEMON_BIN" >"$DAEMON_LOG" 2>&1 &
DAEMON_PID=$!

cleanup() {
    if kill -0 "$DAEMON_PID" 2>/dev/null; then
        log "stopping daemon pid=$DAEMON_PID"
        kill -TERM "$DAEMON_PID" 2>/dev/null || true
        sleep 1
        kill -KILL "$DAEMON_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

# Give daemon time to bind ipc socket
sleep 2
if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
    echo "--- daemon log ---" >&2
    cat "$DAEMON_LOG" >&2 || true
    exit 2
fi
log "daemon up pid=$DAEMON_PID"

# ---------------------------------------------------------------------------
# CSV header
# ---------------------------------------------------------------------------
echo "epoch_secs,elapsed_secs,rss_kb,cpu_percent" >"$REPORT_FILE"

# ---------------------------------------------------------------------------
# Driver loop (background) — insert / list / delete cycles
# ---------------------------------------------------------------------------
SLEEP_BETWEEN=$(awk -v r="$RATE" 'BEGIN { printf "%.4f", 1.0/r }')

driver_loop() {
    local i=0
    while kill -0 "$DAEMON_PID" 2>/dev/null; do
        i=$((i+1))
        # insert: simulate clipboard write via CLI (importing a 1-item JSON)
        # We use printf to keep payload bounded and deterministic.
        local payload
        payload="soak-test-$(date +%s%N)-$i"
        # Best-effort calls; tolerate transient errors so the soak driver
        # never crashes the run itself.
        "$CLI_BIN" import --stdin >/dev/null 2>&1 <<JSON || true
[{"content":"$payload","content_type":"text/plain"}]
JSON
        "$CLI_BIN" list --limit 5 >/dev/null 2>&1 || true
        # delete the oldest visible id (best-effort)
        local oldest_id
        oldest_id=$("$CLI_BIN" list --limit 1 2>/dev/null | awk 'NR==2 {print $1}' || true)
        if [[ -n "${oldest_id:-}" ]]; then
            "$CLI_BIN" delete "$oldest_id" >/dev/null 2>&1 || true
        fi
        sleep "$SLEEP_BETWEEN"
    done
}

driver_loop &
DRIVER_PID=$!

cleanup_with_driver() {
    if kill -0 "$DRIVER_PID" 2>/dev/null; then
        kill -TERM "$DRIVER_PID" 2>/dev/null || true
    fi
    cleanup
}
trap cleanup_with_driver EXIT INT TERM

# ---------------------------------------------------------------------------
# Sampling loop (foreground)
# ---------------------------------------------------------------------------
START_TS=$(date +%s)
END_TS=$((START_TS + DURATION))

log "sampling every ${SAMPLE_INTERVAL}s for ${DURATION}s -> $REPORT_FILE"

while :; do
    NOW=$(date +%s)
    (( NOW >= END_TS )) && break
    if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
        log "daemon exited prematurely; aborting soak"
        break
    fi
    # ps output: RSS in KB, CPU as percent
    SAMPLE=$(ps -o rss=,%cpu= -p "$DAEMON_PID" 2>/dev/null | awk '{print $1","$2}')
    if [[ -n "$SAMPLE" ]]; then
        ELAPSED=$((NOW - START_TS))
        echo "$NOW,$ELAPSED,$SAMPLE" >>"$REPORT_FILE"
    fi
    sleep "$SAMPLE_INTERVAL"
done

log "sampling complete; running analyzer"

# ---------------------------------------------------------------------------
# Analyze + decide exit code
# ---------------------------------------------------------------------------
ANALYZER="$REPO_ROOT/scripts/soak-report.sh"
if [[ -x "$ANALYZER" ]]; then
    if "$ANALYZER" --input "$REPORT_FILE" --threshold "$GROWTH_THRESHOLD"; then
        exit 0
    else
        rc=$?
        # 3 = regression, anything else = analyzer error
        exit "$rc"
    fi
else
    log "analyzer not found at $ANALYZER (skipping; raw csv: $REPORT_FILE)"
    exit 0
fi
