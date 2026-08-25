#!/usr/bin/env bash
# The CI gates a Linux host can run, against the tree you name.
# Mirrors .github/workflows/ci.yml.
set -uo pipefail
COPYPASTE_GATE_NAME="verify.sh"
. "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/gate-lib.sh"

tree="$(copypaste_gate_tree "${1:-}")" || exit $?
cd "$tree" || exit 2
copypaste_gate_banner "$tree"

copypaste_gate_run "portable CI gates" \
    python3 scripts/ci/run-gates.py --profile linux-ci-mirror

copypaste_gate_summary "$tree"
