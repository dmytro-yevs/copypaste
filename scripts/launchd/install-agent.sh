#!/usr/bin/env bash
# install-agent.sh — Install CopyPaste daemon as per-user LaunchAgent
#
# Usage: ./install-agent.sh
#
# Installs com.copypaste.daemon.plist into ~/Library/LaunchAgents/,
# substitutes the current user's home into log paths, bootstraps into
# the user's GUI domain, enables, and kickstarts.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
SOURCE_PLIST="${REPO_ROOT}/packaging/macos/com.copypaste.daemon.plist"

LABEL="com.copypaste.daemon"
TARGET_DIR="${HOME}/Library/LaunchAgents"
TARGET_PLIST="${TARGET_DIR}/${LABEL}.plist"
LOG_DIR="${HOME}/Library/Logs/CopyPaste"
BINARY_PATH="/Applications/CopyPaste.app/Contents/MacOS/copypaste-daemon"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "error: this installer is macOS-only" >&2
    exit 1
fi

if [[ ! -f "${SOURCE_PLIST}" ]]; then
    echo "error: plist not found at ${SOURCE_PLIST}" >&2
    exit 1
fi

if [[ ! -x "${BINARY_PATH}" ]]; then
    echo "warning: ${BINARY_PATH} is not executable or missing" >&2
    echo "         install CopyPaste.app to /Applications first" >&2
fi

UID_NUM="$(id -u)"

echo "==> creating log dir ${LOG_DIR}"
mkdir -p "${LOG_DIR}"

echo "==> creating launch agents dir ${TARGET_DIR}"
mkdir -p "${TARGET_DIR}"

echo "==> installing plist to ${TARGET_PLIST}"
# Substitute @HOME@ template token with the actual user home directory.
# Using @HOME@ (not a literal username like /Users/USERNAME) avoids embedding
# any real account name in the committed plist template.
sed "s|@HOME@|${HOME}|g" "${SOURCE_PLIST}" > "${TARGET_PLIST}"
chmod 644 "${TARGET_PLIST}"

echo "==> validating plist"
plutil -lint "${TARGET_PLIST}"

# If already loaded, unload first so changes apply
if launchctl print "gui/${UID_NUM}/${LABEL}" >/dev/null 2>&1; then
    echo "==> unloading existing agent (gui/${UID_NUM}/${LABEL})"
    launchctl bootout "gui/${UID_NUM}/${LABEL}" 2>/dev/null || true
fi

# Kill any lingering daemon process so the in-use binary is not held open
# when we install the new one. Safe/idempotent: `|| true` means the script
# never fails when no process is running.
echo "==> stopping any running daemon process"
pkill -f copypaste-daemon 2>/dev/null || true
# Brief settle so the OS releases file handles before we bootstrap the new agent.
sleep 1

# Clear any persistent disabled override BEFORE bootstrap. A prior
# `launchctl unload -w` / `disable` (or even a plain `bootout`) can leave the
# label on launchd's per-user disabled list, which makes `bootstrap` fail with
# "Bootstrap failed: 5: Input/output error". `enable` is idempotent.
echo "==> enabling agent (clearing any disabled override)"
launchctl enable "gui/${UID_NUM}/${LABEL}"

echo "==> bootstrapping agent into gui/${UID_NUM}"
launchctl bootstrap "gui/${UID_NUM}" "${TARGET_PLIST}"

echo "==> kickstarting agent"
launchctl kickstart -k "gui/${UID_NUM}/${LABEL}"

echo ""
echo "installed: ${LABEL}"
echo "logs:      ${LOG_DIR}/daemon.{out,err}.log"
echo "status:    launchctl print gui/${UID_NUM}/${LABEL}"
echo "uninstall: ${SCRIPT_DIR}/uninstall-agent.sh"
