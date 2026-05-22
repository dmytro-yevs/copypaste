#!/usr/bin/env bash
# uninstall-daemon.sh — Unload LaunchAgent and remove copypaste-daemon
set -euo pipefail

AGENTS_DIR="$HOME/Library/LaunchAgents"
DEST_PLIST="$AGENTS_DIR/com.copypaste.daemon.plist"
DEST_BINARY="/usr/local/bin/copypaste-daemon"

# ── Guard: must NOT be run as root ────────────────────────────────────────────
if [[ "$EUID" -eq 0 ]]; then
    echo "ERROR: Do not run this script as root. Run as the target user." >&2
    exit 1
fi

echo "=== CopyPaste Daemon Uninstaller ==="
echo "User   : $(whoami)"
echo ""

# ── Unload the LaunchAgent ────────────────────────────────────────────────────
if [[ -f "$DEST_PLIST" ]]; then
    echo "Unloading LaunchAgent..."
    launchctl unload -w "$DEST_PLIST" 2>/dev/null || true
    echo "  ✓ Agent unloaded"
else
    echo "  (plist not found — skipping unload)"
fi

# ── Remove plist ──────────────────────────────────────────────────────────────
if [[ -f "$DEST_PLIST" ]]; then
    rm -f "$DEST_PLIST"
    echo "  ✓ Removed $DEST_PLIST"
fi

# ── Optionally remove binary ──────────────────────────────────────────────────
REMOVE_BINARY="${REMOVE_BINARY:-}"
if [[ -f "$DEST_BINARY" ]]; then
    if [[ -n "$REMOVE_BINARY" ]]; then
        echo "Removing binary..."
        sudo rm -f "$DEST_BINARY"
        echo "  ✓ Removed $DEST_BINARY"
    else
        echo "  Binary kept at $DEST_BINARY (set REMOVE_BINARY=1 to also remove it)"
    fi
fi

echo ""
echo "=== Uninstall complete ==="
