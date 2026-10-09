#!/usr/bin/env bash
# Run the GNOME bridge transport fixture against an isolated session bus.
set -euo pipefail

export COPYPASTE_GNOME_FIXTURE=1
dbus-run-session -- cargo test -p copypaste-daemon live_gnome_dbus_fixture -- --nocapture
