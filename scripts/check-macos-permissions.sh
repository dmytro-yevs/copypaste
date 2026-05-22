#!/usr/bin/env bash
# Check macOS permissions required for copypaste-daemon

set -euo pipefail

echo "=== CopyPaste macOS Permissions Check ==="
echo ""

PASS="✅"
FAIL="❌"
WARN="⚠️"

# 1. Accessibility (required for clipboard monitoring on some macOS versions)
echo "1. Accessibility permission:"
if sqlite3 "/Library/Application Support/com.apple.TCC/TCC.db" \
  "SELECT allowed FROM access WHERE service='kTCCServiceAccessibility' AND client='com.copypaste.daemon'" 2>/dev/null | grep -q "^1$"; then
    echo "   $PASS Granted"
else
    echo "   $WARN Not granted (may not be required for clipboard polling)"
    echo "   To grant: System Preferences → Privacy & Security → Accessibility"
fi

# 2. Keychain access (check if our service exists)
echo ""
echo "2. Keychain key (com.copypaste.daemon / device-secret-key):"
if security find-generic-password -s "com.copypaste.daemon" -a "device-secret-key" >/dev/null 2>&1; then
    echo "   $PASS Key exists in Keychain"
else
    echo "   $WARN Key not found — will be created on first daemon start"
fi

# 3. Daemon socket
SOCKET_PATH="$HOME/Library/Application Support/CopyPaste/daemon.sock"
echo ""
echo "3. Daemon socket ($SOCKET_PATH):"
if [ -S "$SOCKET_PATH" ]; then
    echo "   $PASS Socket exists — daemon is running"
    # Quick ping
    if echo '{"id":"1","method":"status"}' | nc -U -w 2 "$SOCKET_PATH" 2>/dev/null | grep -q '"ok":true'; then
        echo "   $PASS Daemon responds to IPC"
    else
        echo "   $WARN Socket exists but daemon not responding"
    fi
else
    echo "   $FAIL Socket not found — daemon not running"
    echo "   Start: bash launch/install.sh"
fi

# 4. launchd service
echo ""
echo "4. launchd service (com.copypaste.daemon):"
if launchctl list | grep -q "com.copypaste.daemon"; then
    echo "   $PASS Service loaded"
else
    echo "   $WARN Service not loaded"
    echo "   Install: bash launch/install.sh"
fi

# 5. Binary
echo ""
echo "5. Daemon binary (/usr/local/bin/copypaste-daemon):"
if [ -x "/usr/local/bin/copypaste-daemon" ]; then
    echo "   $PASS Binary exists"
else
    echo "   $FAIL Binary not found"
    echo "   Build: cargo build --release -p copypaste-daemon"
fi

echo ""
echo "=== Done ==="
