#!/usr/bin/env bash
# Coverage runner for CopyPaste workspace.
# Uses cargo-llvm-cov to produce HTML, LCOV, and JSON reports.
#
# Usage:
#   scripts/coverage.sh                       # Run all formats (HTML + LCOV)
#   scripts/coverage.sh --html-only           # Only HTML report
#   scripts/coverage.sh --lcov-only           # Only LCOV file (for CI/codecov)
#   scripts/coverage.sh --json                # Also emit JSON summary
#   scripts/coverage.sh --threshold 70        # Fail if line coverage < 70%
#   scripts/coverage.sh --help                # Show this help
#
# Outputs:
#   coverage/html/index.html  — HTML report
#   coverage/lcov.info        — LCOV file (codecov-compatible)
#   coverage/summary.json     — JSON summary (if --json)

set -euo pipefail

# -----------------------------------------------------------------------------
# Defaults
# -----------------------------------------------------------------------------
HTML=1
LCOV=1
JSON=0
THRESHOLD=""
OUTPUT_DIR="coverage"

# -----------------------------------------------------------------------------
# Helpers
# -----------------------------------------------------------------------------
usage() {
  sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'
  exit 0
}

err() {
  echo "ERROR: $*" >&2
  exit 1
}

info() {
  echo "[coverage] $*"
}

# -----------------------------------------------------------------------------
# Parse args
# -----------------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
  case "$1" in
    --html-only)
      HTML=1; LCOV=0; JSON=0
      shift
      ;;
    --lcov-only)
      HTML=0; LCOV=1; JSON=0
      shift
      ;;
    --json)
      JSON=1
      shift
      ;;
    --threshold)
      [[ $# -ge 2 ]] || err "--threshold requires a numeric value"
      THRESHOLD="$2"
      shift 2
      ;;
    --threshold=*)
      THRESHOLD="${1#*=}"
      shift
      ;;
    -h|--help)
      usage
      ;;
    *)
      err "Unknown argument: $1 (use --help)"
      ;;
  esac
done

# -----------------------------------------------------------------------------
# Repo root
# -----------------------------------------------------------------------------
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

[[ -f Cargo.toml ]] || err "Cargo.toml not found at $REPO_ROOT — not a Rust workspace?"

# -----------------------------------------------------------------------------
# Ensure cargo-llvm-cov installed
# -----------------------------------------------------------------------------
if ! command -v cargo-llvm-cov >/dev/null 2>&1 && ! cargo llvm-cov --version >/dev/null 2>&1; then
  info "cargo-llvm-cov not installed."
  if [[ -n "${CI:-}" || -n "${COVERAGE_AUTO_INSTALL:-}" ]]; then
    info "CI/COVERAGE_AUTO_INSTALL detected — installing cargo-llvm-cov..."
    cargo install cargo-llvm-cov --locked
  else
    printf "Install cargo-llvm-cov now? [y/N] "
    read -r ans
    case "$ans" in
      y|Y|yes|YES)
        cargo install cargo-llvm-cov --locked
        ;;
      *)
        err "cargo-llvm-cov is required. Install with: cargo install cargo-llvm-cov --locked"
        ;;
    esac
  fi
fi

# -----------------------------------------------------------------------------
# Prepare output directory
# -----------------------------------------------------------------------------
mkdir -p "$OUTPUT_DIR"

# Clean previous coverage artifacts (instrumented .profraw files).
info "Cleaning previous coverage data..."
cargo llvm-cov clean --workspace >/dev/null 2>&1 || true

# -----------------------------------------------------------------------------
# Run coverage
# -----------------------------------------------------------------------------
COMMON_FLAGS=(--workspace)

if [[ $HTML -eq 1 ]]; then
  info "Generating HTML report -> $OUTPUT_DIR/html/"
  cargo llvm-cov "${COMMON_FLAGS[@]}" --html --output-dir "$OUTPUT_DIR/html"
fi

if [[ $LCOV -eq 1 ]]; then
  info "Generating LCOV report -> $OUTPUT_DIR/lcov.info"
  # --no-clean: reuse instrumentation if HTML already ran
  if [[ $HTML -eq 1 ]]; then
    cargo llvm-cov "${COMMON_FLAGS[@]}" --no-run --lcov --output-path "$OUTPUT_DIR/lcov.info" || \
      cargo llvm-cov "${COMMON_FLAGS[@]}" --lcov --output-path "$OUTPUT_DIR/lcov.info"
  else
    cargo llvm-cov "${COMMON_FLAGS[@]}" --lcov --output-path "$OUTPUT_DIR/lcov.info"
  fi
fi

if [[ $JSON -eq 1 ]]; then
  info "Generating JSON summary -> $OUTPUT_DIR/summary.json"
  cargo llvm-cov "${COMMON_FLAGS[@]}" --no-run --json --summary-only \
    --output-path "$OUTPUT_DIR/summary.json" || \
    cargo llvm-cov "${COMMON_FLAGS[@]}" --json --summary-only \
      --output-path "$OUTPUT_DIR/summary.json"
fi

# -----------------------------------------------------------------------------
# Threshold check
# -----------------------------------------------------------------------------
if [[ -n "$THRESHOLD" ]]; then
  info "Checking line coverage against threshold ${THRESHOLD}%..."
  # Re-run summary capture (lightweight, reuses prior instrumentation when possible)
  SUMMARY_OUT="$(cargo llvm-cov "${COMMON_FLAGS[@]}" --no-run --summary-only 2>/dev/null \
    || cargo llvm-cov "${COMMON_FLAGS[@]}" --summary-only 2>/dev/null \
    || true)"

  # Extract the TOTAL line coverage percentage. cargo-llvm-cov prints a row like:
  #   TOTAL    123    45     63.41%    ...    72.10%    ...
  # We pick the FIRST percentage on the TOTAL line, which is region/line coverage.
  LINE_PCT="$(echo "$SUMMARY_OUT" \
    | awk '/^TOTAL/ { for (i=1;i<=NF;i++) if ($i ~ /%$/) { sub("%","",$i); print $i; exit } }')"

  if [[ -z "$LINE_PCT" ]]; then
    err "Could not parse coverage summary for threshold check."
  fi

  info "Line coverage: ${LINE_PCT}% (threshold ${THRESHOLD}%)"

  awk -v cov="$LINE_PCT" -v thr="$THRESHOLD" 'BEGIN { exit (cov + 0 >= thr + 0) ? 0 : 1 }' \
    || err "Coverage ${LINE_PCT}% is below threshold ${THRESHOLD}%"
fi

info "Done."
[[ $HTML -eq 1 ]] && info "Open: $OUTPUT_DIR/html/index.html"
[[ $LCOV -eq 1 ]] && info "LCOV:  $OUTPUT_DIR/lcov.info"
[[ $JSON -eq 1 ]] && info "JSON:  $OUTPUT_DIR/summary.json"
