#!/usr/bin/env bash
# Smoke test: start daemon, copy text, verify history, paste, stop daemon.
set -e
SOCKET=$(mktemp -t copypaste.XXXXXX)
DB=$(mktemp -t copypaste.XXXXXX.db)
export COPYPASTE_SOCKET="$SOCKET" COPYPASTE_DB="$DB"

echo "Starting daemon..."
./target/release/copypaste-daemon &
DAEMON_PID=$!
sleep 1

echo "Copying test text..."
echo "hello smoke test" | pbcopy  # macOS
sleep 0.5

echo "Checking history..."
HISTORY=$(./target/release/copypaste-cli list --limit 1)
echo "$HISTORY" | grep -q "hello smoke test" || { echo "FAIL: item not in history"; kill $DAEMON_PID; exit 1; }

echo "Pasting item 1..."
./target/release/copypaste-cli copy 1

echo "PASS: smoke test OK"
kill $DAEMON_PID
rm -f "$SOCKET" "$DB"
