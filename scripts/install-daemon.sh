#!/usr/bin/env bash
# install-daemon.sh — Install copypaste-daemon binary and LaunchAgent plist
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINARY_SRC="$REPO_ROOT/target/release/copypaste-daemon"
PLIST_SRC="$REPO_ROOT/launch/com.copypaste.daemon.plist"
AGENTS_DIR="$HOME/Library/LaunchAgents"
DEST_PLIST="$AGENTS_DIR/com.copypaste.daemon.plist"
DEST_BINARY="/usr/local/bin/copypaste-daemon"

# ── Guard: must NOT be run as root (LaunchAgent lives in user's ~/Library) ──
if [[ "$EUID" -eq 0 ]]; then
    echo "ERROR: Do not run this script as root. Run as the target user." >&2
    exit 1
fi

echo "=== CopyPaste Daemon Installer ==="
echo "User    : $(whoami)"
echo "Binary  : $DEST_BINARY"
echo "Plist   : $DEST_PLIST"
echo ""

# ── Validate sources ──────────────────────────────────────────────────────────
if [[ ! -f "$BINARY_SRC" ]]; then
    echo "ERROR: Release binary not found at $BINARY_SRC" >&2
    echo "       Run 'cargo build --release -p copypaste-daemon' first." >&2
    exit 1
fi

if [[ ! -f "$PLIST_SRC" ]]; then
    echo "ERROR: Plist not found at $PLIST_SRC" >&2
    exit 1
fi

# ── Install binary ────────────────────────────────────────────────────────────
echo "Installing binary..."
sudo install -m 755 "$BINARY_SRC" "$DEST_BINARY"
echo "  ✓ $DEST_BINARY"

# ── Install plist ─────────────────────────────────────────────────────────────
echo "Installing LaunchAgent plist..."
mkdir -p "$AGENTS_DIR"
cp "$PLIST_SRC" "$DEST_PLIST"
echo "  ✓ $DEST_PLIST"

# ── Unload stale instance if already loaded ───────────────────────────────────
if launchctl list | grep -q "com.copypaste.daemon" 2>/dev/null; then
    echo "Unloading existing agent..."
    launchctl unload "$DEST_PLIST" 2>/dev/null || true
fi

# ── Load the agent ────────────────────────────────────────────────────────────
echo "Loading LaunchAgent..."
launchctl load -w "$DEST_PLIST"
echo "  ✓ Agent loaded"

echo ""
echo "=== Installation complete ==="
echo "Status : launchctl list com.copypaste.daemon"
echo "Logs   : tail -f /tmp/copypaste-daemon.log"
