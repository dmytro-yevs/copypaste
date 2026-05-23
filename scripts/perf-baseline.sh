#!/usr/bin/env bash
# perf-baseline.sh — Run copypaste-bench harness, consolidate criterion output
# into reports/perf/baseline-<git-rev>.json, and compare against
# reports/perf/baseline-main.json (if present), flagging >10% regressions.
#
# Bench harness lives in crates/copypaste-bench (commit 1eecfd4) and is
# treated as read-only by this script.
#
# Usage:
#   bash scripts/perf-baseline.sh                  # bench + consolidate + diff
#   bash scripts/perf-baseline.sh --threshold 15   # flag >15% regressions
#   bash scripts/perf-baseline.sh --update-baseline
#                                                  # refresh baseline-main.json
#   bash scripts/perf-baseline.sh --dry-run        # show plan, run nothing
#   bash scripts/perf-baseline.sh --help
#
# Exit codes:
#   0  success, no regression
#   1  usage / setup error
#   2  bench failed
#   3  regression detected (>= threshold)

set -euo pipefail

# ---------------------------------------------------------------------------
# Colours
# ---------------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
THRESHOLD="10"
DRY_RUN="0"
UPDATE_BASELINE="0"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PERF_DIR="${REPO_ROOT}/reports/perf"
CRITERION_DIR="${REPO_ROOT}/target/criterion"
BENCH_CRATE="copypaste-bench"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
log()  { printf '%b\n' "$*" >&2; }
info() { log "${GREEN}[info]${NC} $*"; }
warn() { log "${YELLOW}[warn]${NC} $*"; }
err()  { log "${RED}[err ]${NC} $*"; }

usage() {
  sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'
}

require() {
  command -v "$1" >/dev/null 2>&1 || { err "missing required tool: $1"; exit 1; }
}

# ---------------------------------------------------------------------------
# Arg parsing
# ---------------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
  case "$1" in
    --threshold)
      [[ $# -ge 2 ]] || { err "--threshold requires a value"; exit 1; }
      THRESHOLD="$2"; shift 2 ;;
    --threshold=*)
      THRESHOLD="${1#*=}"; shift ;;
    --update-baseline)
      UPDATE_BASELINE="1"; shift ;;
    --dry-run)
      DRY_RUN="1"; shift ;;
    -h|--help)
      usage; exit 0 ;;
    *)
      err "unknown arg: $1"; usage; exit 1 ;;
  esac
done

case "$THRESHOLD" in
  ''|*[!0-9.]*) err "--threshold must be numeric: $THRESHOLD"; exit 1 ;;
esac

# ---------------------------------------------------------------------------
# Pre-flight
# ---------------------------------------------------------------------------
require cargo
require git
require python3

mkdir -p "$PERF_DIR"

GIT_REV="$(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
GIT_BRANCH="$(git -C "$REPO_ROOT" rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
OUT_FILE="${PERF_DIR}/baseline-${GIT_REV}.json"
BASELINE_MAIN="${PERF_DIR}/baseline-main.json"

info "repo:      $REPO_ROOT"
info "rev:       $GIT_REV ($GIT_BRANCH)"
info "threshold: ${THRESHOLD}%"
info "out:       $OUT_FILE"

if [[ "$DRY_RUN" == "1" ]]; then
  warn "dry-run — skipping cargo bench, consolidation, and diff"
  info "would run: cargo bench -p ${BENCH_CRATE}"
  info "would parse: ${CRITERION_DIR}/<group>/<id>/new/estimates.json"
  info "would write: $OUT_FILE"
  [[ -f "$BASELINE_MAIN" ]] && info "would compare against: $BASELINE_MAIN" \
                            || warn "no baseline-main.json yet — would skip diff"
  exit 0
fi

# ---------------------------------------------------------------------------
# Run benches (harness from crates/copypaste-bench)
# ---------------------------------------------------------------------------
info "running cargo bench -p ${BENCH_CRATE} ..."
if ! cargo bench -p "$BENCH_CRATE" 2>&1 | tee /tmp/perf-baseline-cargo.log; then
  err "cargo bench failed — see /tmp/perf-baseline-cargo.log"
  exit 2
fi

if [[ ! -d "$CRITERION_DIR" ]]; then
  err "criterion output dir missing: $CRITERION_DIR"
  exit 2
fi

# ---------------------------------------------------------------------------
# Consolidate criterion JSON
# ---------------------------------------------------------------------------
info "consolidating criterion estimates into $OUT_FILE"
python3 - "$CRITERION_DIR" "$OUT_FILE" "$GIT_REV" "$GIT_BRANCH" "$TIMESTAMP" <<'PY'
import json, os, sys

criterion_dir, out_file, rev, branch, ts = sys.argv[1:6]
results = {}

for root, _dirs, files in os.walk(criterion_dir):
    if "estimates.json" not in files:
        continue
    # Only collect the "new" estimates (most recent run).
    if os.path.basename(root) != "new":
        continue
    # bench_id = path between criterion_dir and "/new"
    rel = os.path.relpath(root, criterion_dir)
    parts = rel.split(os.sep)
    if parts[-1] != "new" or len(parts) < 2:
        continue
    bench_id = "/".join(parts[:-1])
    try:
        with open(os.path.join(root, "estimates.json")) as fh:
            est = json.load(fh)
    except Exception as exc:
        print(f"skip {bench_id}: {exc}", file=sys.stderr)
        continue
    # criterion estimates have: mean, median, std_dev, slope (each {point_estimate, ...})
    mean = est.get("mean", {}).get("point_estimate")
    median = est.get("median", {}).get("point_estimate")
    stddev = est.get("std_dev", {}).get("point_estimate")
    if mean is None:
        continue
    results[bench_id] = {
        "mean_ns": mean,
        "median_ns": median,
        "std_dev_ns": stddev,
    }

payload = {
    "schema": "copypaste-perf-baseline/v1",
    "git_rev": rev,
    "git_branch": branch,
    "timestamp_utc": ts,
    "benches": results,
}
with open(out_file, "w") as fh:
    json.dump(payload, fh, indent=2, sort_keys=True)
print(f"wrote {len(results)} bench results", file=sys.stderr)
PY

# ---------------------------------------------------------------------------
# Optionally refresh baseline-main.json
# ---------------------------------------------------------------------------
if [[ "$UPDATE_BASELINE" == "1" ]]; then
  cp "$OUT_FILE" "$BASELINE_MAIN"
  info "updated $BASELINE_MAIN from $OUT_FILE"
fi

# ---------------------------------------------------------------------------
# Compare against baseline-main.json if present
# ---------------------------------------------------------------------------
if [[ ! -f "$BASELINE_MAIN" ]]; then
  warn "no baseline-main.json — skipping regression diff"
  warn "run with --update-baseline once on a known-good rev to seed it"
  exit 0
fi

info "diffing against $BASELINE_MAIN (threshold ${THRESHOLD}%)"
set +e
bash "${REPO_ROOT}/scripts/perf-compare.sh" \
  "$BASELINE_MAIN" "$OUT_FILE" --threshold "$THRESHOLD"
DIFF_EXIT=$?
set -e

if [[ $DIFF_EXIT -eq 3 ]]; then
  err "regression detected (>= ${THRESHOLD}%)"
  exit 3
fi

info "no regressions >= ${THRESHOLD}%"
exit 0
