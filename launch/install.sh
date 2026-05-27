#!/usr/bin/env bash
# launch/install.sh — DEPRECATED shim.
#
# The legacy /usr/local/bin + /tmp + `launchctl load -w` install flow is gone:
# `load -w` writes a persistent *disable* override that prevented the daemon
# from ever restarting (the v0.4 startup bug). This shim now delegates to the
# canonical per-user LaunchAgent installer, which installs the single
# source-of-truth plist (packaging/macos/com.copypaste.daemon.plist) and uses
# the non-disabling `bootstrap` / `enable` flow.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

case "${1:-install}" in
    install)   exec "${REPO_ROOT}/scripts/launchd/install-agent.sh" ;;
    uninstall) exec "${REPO_ROOT}/scripts/launchd/uninstall-agent.sh" ;;
    *) echo "Usage: $0 [install|uninstall]" >&2; exit 1 ;;
esac
