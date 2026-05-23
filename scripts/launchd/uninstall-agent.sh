#!/usr/bin/env bash
# uninstall-agent.sh — Remove CopyPaste daemon LaunchAgent
#
# Usage: ./uninstall-agent.sh
#
# Boots out the agent from the user GUI domain and removes the plist.
# Log files are left untouched.

set -euo pipefail

LABEL="com.copypaste.daemon"
TARGET_PLIST="${HOME}/Library/LaunchAgents/${LABEL}.plist"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "error: this uninstaller is macOS-only" >&2
    exit 1
fi

UID_NUM="$(id -u)"

if launchctl print "gui/${UID_NUM}/${LABEL}" >/dev/null 2>&1; then
    echo "==> booting out gui/${UID_NUM}/${LABEL}"
    launchctl bootout "gui/${UID_NUM}/${LABEL}" || true
else
    echo "==> agent not loaded (skipping bootout)"
fi

if [[ -f "${TARGET_PLIST}" ]]; then
    echo "==> removing ${TARGET_PLIST}"
    rm -f "${TARGET_PLIST}"
else
    echo "==> plist already absent at ${TARGET_PLIST}"
fi

echo ""
echo "uninstalled: ${LABEL}"
echo "note: log files in ~/Library/Logs/CopyPaste/ were preserved"
