#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
UNIT="$SCRIPT_DIR/copypaste-daemon.service"
SYSTEMD_DIR="$HOME/.config/systemd/user"
BINARY_SRC="$(dirname "$SCRIPT_DIR")/target/release/copypaste-daemon"
BINARY_DEST="$HOME/.local/bin/copypaste-daemon"

install_service() {
    cargo build --release -p copypaste-daemon
    mkdir -p "$HOME/.local/bin" "$SYSTEMD_DIR"
    install -m 755 "$BINARY_SRC" "$BINARY_DEST"
    cp "$UNIT" "$SYSTEMD_DIR/"
    systemctl --user daemon-reload
    systemctl --user enable --now copypaste-daemon.service
    echo "Service installed. Logs: journalctl --user -u copypaste-daemon -f"
}

uninstall_service() {
    systemctl --user stop copypaste-daemon.service 2>/dev/null || true
    systemctl --user disable copypaste-daemon.service 2>/dev/null || true
    rm -f "$SYSTEMD_DIR/copypaste-daemon.service"
    rm -f "$BINARY_DEST"
    systemctl --user daemon-reload
    echo "Service uninstalled."
}

case "${1:-install}" in
    install)   install_service ;;
    uninstall) uninstall_service ;;
    *) echo "Usage: $0 [install|uninstall]" && exit 1 ;;
esac
