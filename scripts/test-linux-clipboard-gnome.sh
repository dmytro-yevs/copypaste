#!/usr/bin/env bash
# Run the GNOME bridge transport fixture against an isolated session bus.
set -euo pipefail

export COPYPASTE_GNOME_FIXTURE=1
fixture_log=$(mktemp)
trap 'rm -f "$fixture_log"' EXIT
dbus-run-session -- cargo test --locked -p copypaste-daemon live_gnome_dbus_fixture -- --nocapture | tee "$fixture_log"
grep -q 'test result: ok. 1 passed' "$fixture_log"
