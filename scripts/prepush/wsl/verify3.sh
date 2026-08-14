#!/usr/bin/env bash
# The Node-side gates, against the tree you name. Needs ~/.copypaste-env to
# have made npm visible to non-interactive shells.
set -uo pipefail
COPYPASTE_GATE_NAME="verify3.sh"
. "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/gate-lib.sh"

tree="$(copypaste_gate_tree "${1:-}")" || exit $?
cd "$tree" || exit 2
copypaste_gate_banner "$tree"

export RUSTUP_TOOLCHAIN=1.96

copypaste_gate_run "ui npm ci"        bash -c 'cd crates/copypaste-ui && npm ci'
copypaste_gate_run "ui npm audit"     bash -c 'cd crates/copypaste-ui && npm audit'
copypaste_gate_run "ui npm run build" bash -c 'cd crates/copypaste-ui && npm run build'
copypaste_gate_run "ui npm test"      bash -c 'cd crates/copypaste-ui && npm test'
copypaste_gate_run "design npm ci"    bash -c 'cd design && npm ci'
copypaste_gate_run "design npm audit" bash -c 'cd design && npm audit'
copypaste_gate_run "design rebuild"   bash -c 'cd design && npm run rebuild'
copypaste_gate_run "e2e npm ci"       bash -c 'cd e2e && npm ci'
copypaste_gate_run "e2e npm audit"    bash -c 'cd e2e && npm audit'
copypaste_gate_run "e2e npm test"     bash -c 'cd e2e && npm test'

copypaste_gate_summary "$tree"
