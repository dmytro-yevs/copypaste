#!/usr/bin/env bash
# acceptance.sh — pre-release gate: real-artifact acceptance harness.
#
# Runs, in order, against the ACTUAL built binaries (not in-process mocks):
#   1. build release binaries (copypaste-daemon + copypaste-cli)
#   2. scripts/smoke_test.sh   — daemon startup / IPC / DB / round-trip
#   3. scripts/p2p_smoke_test.sh — two-process P2P clipboard sync end-to-end
#
# Prints a clear PASS/FAIL summary and exits non-zero on ANY failure. This is
# the gate that catches build-skew, daemon-can't-start, real socket/DB, and
# cross-device sync regressions that unit tests miss.
#
# Usage:
#   bash scripts/acceptance.sh [--from-bundle <CopyPaste.app>]
#
# Env:
#   COPYPASTE_EPHEMERAL_KEY=1  recommended in CI so the daemon never touches the
#                              macOS Keychain (the p2p script forces this anyway).
#
# Requirements: macOS or Linux, Rust toolchain, nc (netcat with -U), base64.

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

FROM_BUNDLE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --from-bundle) FROM_BUNDLE="${2:?--from-bundle needs a path}"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

# Track results for the final summary.
RESULTS=()
OVERALL=0

record() {
  # record <name> <rc>
  local name="$1" rc="$2"
  if [[ "$rc" -eq 0 ]]; then
    RESULTS+=("PASS  $name")
  else
    RESULTS+=("FAIL  $name (rc=$rc)")
    OVERALL=1
  fi
}

section() {
  echo ""
  echo -e "${BOLD}==================================================================${NC}"
  echo -e "${BOLD}  $*${NC}"
  echo -e "${BOLD}==================================================================${NC}"
}

# ---------------------------------------------------------------------------
# Phase 1 — Build (skipped for bundle mode; bundle ships its own binaries).
# ---------------------------------------------------------------------------
if [[ -z "$FROM_BUNDLE" ]]; then
  section "Phase 1/3: Build release binaries"
  if ( cd "$REPO_ROOT" && cargo build --release -p copypaste-daemon -p copypaste-cli ); then
    record "build (release)" 0
  else
    record "build (release)" $?
    # A failed build poisons everything downstream — report and bail early.
    echo -e "${RED}Build failed; skipping smoke and P2P phases.${NC}" >&2
    printf '\n'
    section "ACCEPTANCE SUMMARY"
    for line in "${RESULTS[@]}"; do echo "  $line"; done
    echo -e "${RED}${BOLD}ACCEPTANCE: FAIL${NC}"
    exit 1
  fi
fi

# Args passed to the child scripts. With a build already done above we let the
# children rebuild-skip; bundle mode forwards --from-bundle.
SMOKE_ARGS=()
P2P_ARGS=()
if [[ -n "$FROM_BUNDLE" ]]; then
  SMOKE_ARGS+=(--from-bundle "$FROM_BUNDLE")
  P2P_ARGS+=(--from-bundle "$FROM_BUNDLE")
else
  SMOKE_ARGS+=(--skip-build)
  P2P_ARGS+=(--skip-build)
fi

# ---------------------------------------------------------------------------
# Phase 2 — Single-daemon smoke (startup / IPC / DB / round-trip).
# ---------------------------------------------------------------------------
section "Phase 2/3: Single-daemon smoke (scripts/smoke_test.sh)"
if bash "$SCRIPT_DIR/smoke_test.sh" "${SMOKE_ARGS[@]}"; then
  record "smoke_test.sh" 0
else
  record "smoke_test.sh" $?
fi

# ---------------------------------------------------------------------------
# Phase 3 — Two-process P2P clipboard sync.
# ---------------------------------------------------------------------------
section "Phase 3/3: Two-process P2P sync (scripts/p2p_smoke_test.sh)"
if bash "$SCRIPT_DIR/p2p_smoke_test.sh" "${P2P_ARGS[@]}"; then
  record "p2p_smoke_test.sh" 0
else
  record "p2p_smoke_test.sh" $?
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
section "ACCEPTANCE SUMMARY"
for line in "${RESULTS[@]}"; do
  case "$line" in
    PASS*) echo -e "  ${GREEN}${line}${NC}" ;;
    FAIL*) echo -e "  ${RED}${line}${NC}" ;;
    *)     echo "  $line" ;;
  esac
done
echo ""
if [[ "$OVERALL" -eq 0 ]]; then
  echo -e "${GREEN}${BOLD}ACCEPTANCE: PASS${NC}"
else
  echo -e "${RED}${BOLD}ACCEPTANCE: FAIL${NC}"
fi
exit "$OVERALL"
