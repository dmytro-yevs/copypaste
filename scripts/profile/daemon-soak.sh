#!/usr/bin/env bash
# Does a daemon under a steady capture stream settle, or grow?
#
# Captures are driven through the fake clipboard's watched file, so the whole
# path runs: poll, change detection, detect, encrypt, insert, index, both
# sweeps. RSS is sampled every `--every` seconds; the question is whether the
# last third of the run is flat.
#
#   scripts/profile/daemon-soak.sh [minutes] [captures_per_second] [bytes]
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

MINUTES="${1:-5}"
RATE="${2:-4}"
BYTES="${3:-512}"

trap stop_daemon EXIT
build_release
require_quiet

CLIP="$(mktemp)"
export COPYPASTE_FAKE_CLIPBOARD="$CLIP"
start_daemon
# Fast enough that the writer is never the bottleneck.
"$CLI" config set --poll-interval-ms 100 >/dev/null

note "soak: ${MINUTES}m at ${RATE}/s of ${BYTES}B, load $(load_now)"
printf '%6s %10s %10s %10s %8s\n' "t(s)" "RSS(KB)" "items" "cpu(s)" "ctxt"

SAMPLE_EVERY=15
END=$(( $(date +%s) + MINUTES * 60 ))
START=$(date +%s)
FILLER=$(head -c "$BYTES" /dev/zero | tr '\0' 'x')
n=0
while [[ $(date +%s) -lt $END ]]; do
    for _ in $(seq 1 $((RATE * SAMPLE_EVERY))); do
        n=$((n + 1))
        printf 'soak %s %s\n' "$n" "$FILLER" >"$CLIP"
        sleep "$(awk -v r="$RATE" 'BEGIN { print 1 / r }')"
    done
    now=$(date +%s)
    printf '%6s %10s %10s %10s %8s\n' \
        "$((now - START))" \
        "$(rss_kb "$DAEMON_PID")" \
        "$("$CLI" --json status 2>/dev/null | tr -d ' ' | grep -o '"item_count":[0-9]*' | cut -d: -f2)" \
        "$(awk -v t="$(cpu_ticks "$DAEMON_PID")" -v hz="$CLK_TCK" 'BEGIN { printf "%.2f", t / hz }')" \
        "$(ctxt_switches "$DAEMON_PID")"
done

rm -f "$CLIP"
