#!/usr/bin/env bash
# soak-report.sh — Analyze a soak CSV produced by scripts/soak-daemon.sh.
#
# Prints:
#   - peak / mean / p99 RSS (KB)
#   - first / last RSS and percent growth
#   - ASCII memory curve (RSS over elapsed time)
#   - mean / peak CPU
#
# Exit codes:
#   0  no regression
#   1  usage / file error
#   3  RSS growth >= threshold (likely leak)
#
# Usage:
#   bash scripts/soak-report.sh --input <csv> [--threshold 10] [--width 60]
#   bash scripts/soak-report.sh --help
#
# CSV format (header required):
#   epoch_secs,elapsed_secs,rss_kb,cpu_percent

set -euo pipefail

INPUT=""
THRESHOLD=10
WIDTH=60

usage() {
    sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
}

die() {
    echo "soak-report: $*" >&2
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --input)     INPUT="$2"; shift 2 ;;
        --threshold) THRESHOLD="$2"; shift 2 ;;
        --width)     WIDTH="$2"; shift 2 ;;
        -h|--help)   usage; exit 0 ;;
        *) die "unknown arg: $1 (try --help)" ;;
    esac
done

[[ -n "$INPUT" ]]    || die "missing --input <csv>"
[[ -f "$INPUT" ]]    || die "csv not found: $INPUT"
[[ "$THRESHOLD" =~ ^[0-9]+$ ]] || die "--threshold must be integer percent"
[[ "$WIDTH" =~ ^[0-9]+$ ]]     || die "--width must be integer"
(( WIDTH >= 10 )) || die "--width must be >= 10"

command -v awk >/dev/null || die "missing awk"

# Drop header, ensure at least 2 data rows
ROWS=$(awk -F, 'NR>1 && NF>=4 {n++} END{print n+0}' "$INPUT")
(( ROWS >= 2 )) || die "need at least 2 data rows in $INPUT (got $ROWS)"

awk -F, -v th="$THRESHOLD" -v w="$WIDTH" -v file="$INPUT" '
NR==1 { next }                                      # skip header
NF<4  { next }
{
    n++
    rss_list[n]     = $3 + 0
    cpu_list[n]     = $4 + 0
    elapsed_list[n] = $2 + 0
    if (rss_list[n] > peak) peak = rss_list[n]
    if (cpu_list[n] > peak_cpu) peak_cpu = cpu_list[n]
    sum_rss += rss_list[n]
    sum_cpu += cpu_list[n]
}
END {
    if (n < 2) { print "not enough samples"; exit 1 }

    # mean
    mean_rss = sum_rss / n
    mean_cpu = sum_cpu / n

    # p99 — sort copy of rss_list
    for (i=1; i<=n; i++) sorted[i] = rss_list[i]
    # simple insertion sort (n is small for soak samples)
    for (i=2; i<=n; i++) {
        key = sorted[i]; j = i - 1
        while (j >= 1 && sorted[j] > key) { sorted[j+1] = sorted[j]; j-- }
        sorted[j+1] = key
    }
    p99_idx = int(n * 0.99); if (p99_idx < 1) p99_idx = 1
    p99 = sorted[p99_idx]

    # first stable = 2nd sample (skip startup spike)
    first_stable = rss_list[(n>=2)?2:1]
    last = rss_list[n]
    growth_pct = (first_stable > 0) ? ((last - first_stable) * 100.0 / first_stable) : 0

    # find min for plotting
    min = sorted[1]
    range = peak - min; if (range < 1) range = 1

    printf "soak report — %s\n", file
    bar_eq = ""
    flen = length(file)
    for (k=0; k<flen; k++) bar_eq = bar_eq "="
    printf "==============%s\n", bar_eq
    printf "  samples         : %d\n", n
    printf "  duration        : %ds\n", elapsed_list[n]
    printf "  peak rss        : %d KB (%.1f MB)\n", peak, peak/1024
    printf "  mean rss        : %.0f KB (%.1f MB)\n", mean_rss, mean_rss/1024
    printf "  p99  rss        : %d KB (%.1f MB)\n", p99, p99/1024
    printf "  first-stable rss: %d KB\n", first_stable
    printf "  last rss        : %d KB\n", last
    printf "  growth          : %+.2f%% (threshold %d%%)\n", growth_pct, th
    printf "  mean cpu        : %.2f%%\n", mean_cpu
    printf "  peak cpu        : %.2f%%\n", peak_cpu
    printf "\nrss curve (%d cols, min=%d KB peak=%d KB):\n", w, min, peak

    # ASCII plot: one row per sample, width-w bar scaled to (min..peak)
    for (i=1; i<=n; i++) {
        v = rss_list[i]
        bar_len = int((v - min) * w / range)
        bar = ""
        for (k=0; k<bar_len; k++) bar = bar "#"
        printf "  %5ds | %-*s | %d KB\n", elapsed_list[i], w, bar, v
    }

    if (growth_pct >= th) {
        printf "\nREGRESSION: rss grew %.2f%% (>= %d%%) — investigate for leaks\n", growth_pct, th
        exit 3
    } else {
        printf "\nOK: rss growth within threshold\n"
        exit 0
    }
}
' "$INPUT"
