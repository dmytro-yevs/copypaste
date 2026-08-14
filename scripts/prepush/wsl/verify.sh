#!/usr/bin/env bash
# The CI gates a Linux host can run, against the tree you name.
# Mirrors .github/workflows/ci.yml.
set -uo pipefail
COPYPASTE_GATE_NAME="verify.sh"
. "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/gate-lib.sh"

tree="$(copypaste_gate_tree "${1:-}")" || exit $?
cd "$tree" || exit 2
copypaste_gate_banner "$tree"

copypaste_gate_run "cargo fmt --check"       cargo +1.96 fmt --all --check
copypaste_gate_run "clippy --workspace"      cargo +1.96 clippy --workspace --all-targets --locked -- -D warnings
copypaste_gate_run "clippy embedded-backend" cargo +1.96 clippy -p copypaste-ui --features embedded-backend --all-targets --locked -- -D warnings
copypaste_gate_run "macOS types self-test"   ./scripts/check-macos-types.sh --self-test
copypaste_gate_run "ipc bindings current"    ./crates/copypaste-ui/scripts/generate-ipc-bindings.sh --check
copypaste_gate_run "comment budget"          ./scripts/check-comments.sh
copypaste_gate_run "file-size budget"        ./scripts/check-file-size.sh 500
copypaste_gate_run "release pipeline"        ./scripts/release/check.sh

copypaste_gate_summary "$tree"
