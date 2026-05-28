#!/usr/bin/env bash
# uninstall-daemon.sh — DEPRECATED shim.
#
# The legacy `launchctl unload -w` uninstall flow is gone: `unload -w` writes a
# persistent *disable* override that prevented a later re-install from
# restarting the daemon (the v0.4 startup bug). This shim now delegates to the
# canonical per-user LaunchAgent uninstaller, which uses a non-disabling
# `bootout` and removes the single source-of-truth plist.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

exec "${REPO_ROOT}/scripts/launchd/uninstall-agent.sh"
