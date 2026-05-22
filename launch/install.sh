#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY="$SCRIPT_DIR/../target/release/copypaste-daemon"
PLIST="$SCRIPT_DIR/com.copypaste.daemon.plist"
AGENTS_DIR="$HOME/Library/LaunchAgents"
DEST_PLIST="$AGENTS_DIR/com.copypaste.daemon.plist"
DEST_BINARY="/usr/local/bin/copypaste-daemon"

install_daemon() {
    echo "Building release binary..."
    cargo build --release -p copypaste-daemon

    echo "Installing binary to $DEST_BINARY..."
    sudo install -m 755 "$BINARY" "$DEST_BINARY"

    echo "Installing launchd plist..."
    mkdir -p "$AGENTS_DIR"
    cp "$PLIST" "$DEST_PLIST"
    launchctl load -w "$DEST_PLIST"

    echo "CopyPaste daemon installed and started."
    echo "Logs: tail -f /tmp/copypaste-daemon.log"
}

uninstall_daemon() {
    echo "Unloading launchd agent..."
    launchctl unload -w "$DEST_PLIST" 2>/dev/null || true
    rm -f "$DEST_PLIST"
    sudo rm -f "$DEST_BINARY"
    echo "CopyPaste daemon uninstalled."
}

case "${1:-install}" in
    install)   install_daemon ;;
    uninstall) uninstall_daemon ;;
    *) echo "Usage: $0 [install|uninstall]" && exit 1 ;;
esac
