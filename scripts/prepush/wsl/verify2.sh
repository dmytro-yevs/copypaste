#!/usr/bin/env bash
# Second half of the local gate run, against the tree you name: the demo
# scripts (release binaries) and the browser-layer e2e suite (debug binaries,
# tauri-driver, Xvfb).
set -uo pipefail
COPYPASTE_GATE_NAME="verify2.sh"
. "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/gate-lib.sh"

tree="$(copypaste_gate_tree "${1:-}")" || exit $?
cd "$tree" || exit 2
copypaste_gate_banner "$tree"

export RUSTUP_TOOLCHAIN=1.96

copypaste_gate_run "build release daemon+cli" cargo build --release --locked -p copypaste-daemon -p copypaste-cli
copypaste_gate_run "demo.sh"                  ./scripts/demo.sh
copypaste_gate_run "demo-p2p.sh"              ./scripts/demo-p2p.sh

command -v tauri-driver >/dev/null 2>&1 || \
    copypaste_gate_run "install tauri-driver 2.0.6" cargo install tauri-driver --version 2.0.6 --locked

copypaste_gate_run "ui npm ci"             bash -c 'cd crates/copypaste-ui && npm ci'
copypaste_gate_run "ui npm run build"      bash -c 'cd crates/copypaste-ui && npm run build'
copypaste_gate_run "ui npm test"           bash -c 'cd crates/copypaste-ui && npm test'
copypaste_gate_run "design npm ci + check" bash -c 'cd design && npm ci && npm run rebuild'
# Debug, not release: a Tauri debug build loads devUrl and the harness starts
# Vite to serve it (browser-webkitgtk.yml).
copypaste_gate_run "build debug ui+daemon+cli" cargo build -p copypaste-ui -p copypaste-daemon -p copypaste-cli --locked
copypaste_gate_run "e2e npm ci"                bash -c 'cd e2e && npm ci'
copypaste_gate_run "e2e npm test"              bash -c 'cd e2e && npm test'

copypaste_gate_summary "$tree"
