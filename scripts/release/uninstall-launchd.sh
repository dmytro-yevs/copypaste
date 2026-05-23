#!/usr/bin/env bash
# uninstall-launchd.sh — remove the CopyPaste daemon LaunchAgent.
#
# Scope: Launch Agent only. Stops the running daemon (if loaded) and removes
# the plist from ~/Library/LaunchAgents/. Does NOT touch binaries, app bundle,
# data directories, or Keychain entries.
#
# Usage:
#   ./uninstall-launchd.sh [--dry-run] [--force] [--help]
#
# Flags:
#   --dry-run   Print every command without executing.
#   --force     Suppress non-fatal warnings, never prompt.
#   --help      Show this help and exit.
#
# Exit codes:
#   0  Agent removed (or was already absent — idempotent).
#   1  Unrecoverable error (e.g. unable to write to LaunchAgents dir).
#
# Re-usable by `uninstall.sh` and by users who want the daemon off while
# leaving the binary in place.
set -euo pipefail

# ---- config ----------------------------------------------------------------
LABEL="com.copypaste.daemon"
LAUNCH_AGENT="$HOME/Library/LaunchAgents/${LABEL}.plist"
# ----------------------------------------------------------------------------

DRY_RUN=0
FORCE=0

usage() {
    sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
}

for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=1 ;;
        --force)   FORCE=1 ;;
        --help|-h) usage; exit 0 ;;
        *)
            echo "ERROR: unknown flag: $arg" >&2
            usage >&2
            exit 1
            ;;
    esac
done

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "ERROR: launchd is macOS-only (detected: $(uname -s))" >&2
    exit 1
fi

# run — execute or echo depending on --dry-run.
# Failures are NOT propagated (idempotent removal); call site decides.
run() {
    if [[ "$DRY_RUN" -eq 1 ]]; then
        echo "DRY-RUN: $*"
    else
        eval "$@"
    fi
}

UID_NUM="$(id -u)"
DOMAIN_TARGET="gui/${UID_NUM}/${LABEL}"

# 1. Stop the running daemon (modern launchctl; ignore "not loaded" errors).
echo "==> Stopping ${LABEL} (if running)"
if [[ "$FORCE" -eq 1 ]]; then
    run "launchctl bootout '${DOMAIN_TARGET}' >/dev/null 2>&1 || true"
else
    run "launchctl bootout '${DOMAIN_TARGET}' 2>/dev/null || true"
fi

# Legacy fallback for users still on `launchctl unload` muscle memory; no-op
# if the modern bootout above already removed it.
run "launchctl unload '${LAUNCH_AGENT}' 2>/dev/null || true"

# 2. Remove the plist file.
if [[ -f "$LAUNCH_AGENT" ]]; then
    echo "==> Removing ${LAUNCH_AGENT}"
    run "rm -f '${LAUNCH_AGENT}'"
else
    echo "==> No plist at ${LAUNCH_AGENT} (already removed)"
fi

echo "==> LaunchAgent removed."
