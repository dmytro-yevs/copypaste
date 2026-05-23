#!/usr/bin/env bash
# perf-compare.sh — diff two perf baseline JSONs and print a table.
#
# Inputs are produced by scripts/perf-baseline.sh (schema:
# copypaste-perf-baseline/v1). Each baseline is a JSON map of
# bench_id -> {mean_ns, median_ns, std_dev_ns}.
#
# Usage:
#   bash scripts/perf-compare.sh <baseline.json> <candidate.json> [--threshold N]
#   bash scripts/perf-compare.sh --help
#
# Output: markdown-style table on stdout with delta %.
# Exit codes:
#   0  no regression
#   1  usage / setup error
#   3  at least one bench regressed by >= threshold

set -euo pipefail

THRESHOLD="10"
BASELINE=""
CANDIDATE=""

usage() {
  sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --threshold)
      [[ $# -ge 2 ]] || { echo "--threshold needs value" >&2; exit 1; }
      THRESHOLD="$2"; shift 2 ;;
    --threshold=*)
      THRESHOLD="${1#*=}"; shift ;;
    -h|--help)
      usage; exit 0 ;;
    -*)
      echo "unknown flag: $1" >&2; usage >&2; exit 1 ;;
    *)
      if [[ -z "$BASELINE" ]]; then BASELINE="$1"
      elif [[ -z "$CANDIDATE" ]]; then CANDIDATE="$1"
      else echo "extra positional: $1" >&2; exit 1
      fi
      shift ;;
  esac
done

[[ -n "$BASELINE"  ]] || { echo "missing <baseline.json>"  >&2; usage >&2; exit 1; }
[[ -n "$CANDIDATE" ]] || { echo "missing <candidate.json>" >&2; usage >&2; exit 1; }
[[ -f "$BASELINE"  ]] || { echo "not found: $BASELINE"   >&2; exit 1; }
[[ -f "$CANDIDATE" ]] || { echo "not found: $CANDIDATE"  >&2; exit 1; }

case "$THRESHOLD" in
  ''|*[!0-9.]*) echo "--threshold must be numeric: $THRESHOLD" >&2; exit 1 ;;
esac

command -v python3 >/dev/null 2>&1 || { echo "missing python3" >&2; exit 1; }

python3 - "$BASELINE" "$CANDIDATE" "$THRESHOLD" <<'PY'
import json, sys

baseline_path, candidate_path, threshold_s = sys.argv[1:4]
threshold = float(threshold_s)

with open(baseline_path)  as fh: base = json.load(fh)
with open(candidate_path) as fh: cand = json.load(fh)

base_b = base.get("benches", {})
cand_b = cand.get("benches", {})

all_ids = sorted(set(base_b) | set(cand_b))
if not all_ids:
    print("no benches to compare", file=sys.stderr)
    sys.exit(0)

print(f"# perf compare")
print(f"- baseline:  `{base.get('git_rev','?')}` ({base.get('git_branch','?')})  {base.get('timestamp_utc','')}")
print(f"- candidate: `{cand.get('git_rev','?')}` ({cand.get('git_branch','?')})  {cand.get('timestamp_utc','')}")
print(f"- threshold: {threshold}%")
print()
print("| bench | baseline (ns) | candidate (ns) | delta % | status |")
print("|---|---:|---:|---:|:---:|")

regressed = 0
for bid in all_ids:
    b = base_b.get(bid, {}).get("mean_ns")
    c = cand_b.get(bid, {}).get("mean_ns")
    if b is None and c is None:
        continue
    if b is None:
        print(f"| `{bid}` | – | {c:.1f} | – | NEW |")
        continue
    if c is None:
        print(f"| `{bid}` | {b:.1f} | – | – | MISSING |")
        continue
    delta = (c - b) / b * 100.0
    if delta >= threshold:
        status = "REGRESSION"
        regressed += 1
    elif delta <= -threshold:
        status = "IMPROVED"
    else:
        status = "ok"
    print(f"| `{bid}` | {b:.1f} | {c:.1f} | {delta:+.2f}% | {status} |")

print()
if regressed:
    print(f"**{regressed} bench(es) regressed >= {threshold}%**")
    sys.exit(3)
print(f"no regressions >= {threshold}%")
PY
