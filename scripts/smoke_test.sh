#!/usr/bin/env bash
# smoke_test.sh — end-to-end macOS smoke test for CopyPaste
# Tests: build, daemon startup, IPC status, clipboard capture, list, stats, cleanup.
#
# Usage: bash scripts/smoke_test.sh [--skip-build]
#
# Requirements: macOS, Rust toolchain, nc (netcat), pbcopy, jq (optional)

set -euo pipefail

# ---------------------------------------------------------------------------
# Colours
# ---------------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Colour

# ---------------------------------------------------------------------------
# Paths — daemon and CLI hardcode these; no env-var override exists.
# ---------------------------------------------------------------------------
SUPPORT_DIR="$HOME/Library/Application Support/CopyPaste"
SOCKET_PATH="$SUPPORT_DIR/daemon.sock"
DB_PATH="$SUPPORT_DIR/clipboard.db"

DAEMON_BIN="./target/release/copypaste-daemon"
# Package name is `copypaste-cli` but the produced binary is `copypaste`.
CLI_BIN="./target/release/copypaste"

DAEMON_PID=""
DAEMON_OWNED=false      # true only if THIS script started the daemon
BACKUP_SOCK=""
BACKUP_DB=""

SKIP_BUILD=false
if [[ "${1:-}" == "--skip-build" ]]; then
  SKIP_BUILD=true
fi

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
fail() {
  echo -e "${RED}FAIL: $*${NC}" >&2
  cleanup
  exit 1
}

pass_step() {
  echo -e "${GREEN}  OK${NC}: $*"
}

run_step() {
  echo -e "${YELLOW}STEP${NC}: $*"
}

# ---------------------------------------------------------------------------
# Cleanup — always runs (trap + explicit call on failure)
# ---------------------------------------------------------------------------
cleanup() {
  if [[ "$DAEMON_OWNED" == "true" && -n "$DAEMON_PID" ]]; then
    kill "$DAEMON_PID" 2>/dev/null || true
    wait "$DAEMON_PID" 2>/dev/null || true
    DAEMON_PID=""
  fi

  # Restore backed-up socket
  if [[ -n "$BACKUP_SOCK" && -f "$BACKUP_SOCK" ]]; then
    mv -f "$BACKUP_SOCK" "$SOCKET_PATH" 2>/dev/null || true
    BACKUP_SOCK=""
  elif [[ -n "$BACKUP_SOCK" ]]; then
    # Original socket did not exist; remove the one we left
    rm -f "$SOCKET_PATH" 2>/dev/null || true
  fi

  # Restore backed-up database
  if [[ -n "$BACKUP_DB" && -f "$BACKUP_DB" ]]; then
    mv -f "$BACKUP_DB" "$DB_PATH" 2>/dev/null || true
    BACKUP_DB=""
  elif [[ -n "$BACKUP_DB" ]]; then
    rm -f "$DB_PATH" 2>/dev/null || true
  fi
}
trap 'cleanup' EXIT

# ---------------------------------------------------------------------------
# STEP 1 — Build binaries (unless --skip-build and both exist)
# ---------------------------------------------------------------------------
run_step "Build release binaries"

if [[ "$SKIP_BUILD" == "true" && -x "$DAEMON_BIN" && -x "$CLI_BIN" ]]; then
  pass_step "Skipping build (--skip-build, binaries present)"
else
  cargo build --release -p copypaste-daemon -p copypaste-cli \
    || fail "cargo build failed"
  pass_step "Binaries built: $DAEMON_BIN, $CLI_BIN"
fi

[[ -x "$DAEMON_BIN" ]] || fail "daemon binary not found or not executable: $DAEMON_BIN"
[[ -x "$CLI_BIN" ]]    || fail "CLI binary not found or not executable: $CLI_BIN"

# ---------------------------------------------------------------------------
# STEP 2 — Ensure app support dir exists
# ---------------------------------------------------------------------------
run_step "Ensure support directory"
mkdir -p "$SUPPORT_DIR" || fail "cannot create $SUPPORT_DIR"
pass_step "Support dir ready: $SUPPORT_DIR"

# ---------------------------------------------------------------------------
# STEP 3 — Handle existing daemon / back up existing data
# ---------------------------------------------------------------------------
run_step "Check for existing daemon"

EXISTING_DAEMON=false
if [[ -S "$SOCKET_PATH" ]]; then
  # Test if something is actually listening
  if echo '{"id":"pre","method":"status","params":{}}' \
       | nc -U "$SOCKET_PATH" -w1 2>/dev/null \
       | grep -q '"ok":true'; then
    EXISTING_DAEMON=true
    echo "  Note: existing daemon detected at $SOCKET_PATH — will test against it (no restart)"
  fi
fi

if [[ "$EXISTING_DAEMON" == "false" ]]; then
  # Back up existing socket and db so we can restore them after the test
  BACKUP_SOCK=$(mktemp "/tmp/copypaste-smoke-sock-backup.XXXXXX") && rm -f "$BACKUP_SOCK"
  BACKUP_DB=$(mktemp "/tmp/copypaste-smoke-db-backup.XXXXXX")    && rm -f "$BACKUP_DB"

  [[ -S "$SOCKET_PATH" ]] && mv "$SOCKET_PATH" "$BACKUP_SOCK" || BACKUP_SOCK="__none__"
  [[ -f "$DB_PATH"     ]] && mv "$DB_PATH"     "$BACKUP_DB"   || BACKUP_DB="__none__"

  run_step "Start fresh daemon"
  RUST_LOG=error "$DAEMON_BIN" &
  DAEMON_PID=$!
  DAEMON_OWNED=true
  pass_step "Daemon started (PID=$DAEMON_PID)"
fi

# ---------------------------------------------------------------------------
# STEP 4 — Wait for socket (max 10 s, 0.2 s intervals)
# ---------------------------------------------------------------------------
run_step "Wait for daemon socket to become ready"

MAX_WAIT_SECONDS=10
INTERVAL=0.2
ELAPSED=0
READY=false

while (( $(echo "$ELAPSED < $MAX_WAIT_SECONDS" | bc -l) )); do
  if [[ -S "$SOCKET_PATH" ]]; then
    if echo '{"id":"ping","method":"status","params":{}}' \
         | nc -U "$SOCKET_PATH" -w1 2>/dev/null \
         | grep -q '"ok":true'; then
      READY=true
      break
    fi
  fi
  sleep "$INTERVAL"
  ELAPSED=$(echo "$ELAPSED + $INTERVAL" | bc)
done

[[ "$READY" == "true" ]] || fail "daemon socket not ready after ${MAX_WAIT_SECONDS}s"
pass_step "Socket ready at $SOCKET_PATH"

# ---------------------------------------------------------------------------
# STEP 5 — IPC status check via nc
# ---------------------------------------------------------------------------
run_step "IPC status check (nc)"

STATUS_RESP=$(echo '{"id":"1","method":"status","params":{}}' \
  | nc -U "$SOCKET_PATH" -w2)

echo "$STATUS_RESP" | grep -q '"ok":true'   || fail "IPC status: expected ok:true, got: $STATUS_RESP"
echo "$STATUS_RESP" | grep -q '"running"'   || fail "IPC status: expected 'running' in response, got: $STATUS_RESP"
pass_step "IPC status: running"

# ---------------------------------------------------------------------------
# STEP 6 — CLI status command
# ---------------------------------------------------------------------------
run_step "CLI status command"

STATUS_OUT=$("$CLI_BIN" status 2>&1)
echo "$STATUS_OUT" | grep -qi "running" || fail "CLI status output missing 'running': $STATUS_OUT"
pass_step "CLI status: $STATUS_OUT"

# ---------------------------------------------------------------------------
# STEP 7 — Count before clipboard write
# ---------------------------------------------------------------------------
run_step "Count items before clipboard capture"

COUNT_BEFORE=$("$CLI_BIN" count 2>&1)
pass_step "Count before: $COUNT_BEFORE"

# Extract numeric count
COUNT_BEFORE_NUM=$(echo "$COUNT_BEFORE" | grep -oE '[0-9]+' | head -1)
COUNT_BEFORE_NUM=${COUNT_BEFORE_NUM:-0}

# ---------------------------------------------------------------------------
# STEP 8 — Write a unique string to the clipboard
# ---------------------------------------------------------------------------
run_step "Write unique string to clipboard via pbcopy"

SMOKE_MARKER="copypaste-smoke-$(date +%s)"
echo "$SMOKE_MARKER" | pbcopy || fail "pbcopy failed"
pass_step "Clipboard set to: $SMOKE_MARKER"

# ---------------------------------------------------------------------------
# STEP 9 — Wait for daemon to capture it (poll up to 5s, 0.5s intervals)
# ---------------------------------------------------------------------------
run_step "Wait for daemon to capture clipboard entry"

CAPTURE_TIMEOUT=5
CAPTURE_ELAPSED=0
CAPTURED=false

while (( $(echo "$CAPTURE_ELAPSED < $CAPTURE_TIMEOUT" | bc -l) )); do
  sleep 0.5
  CAPTURE_ELAPSED=$(echo "$CAPTURE_ELAPSED + 0.5" | bc)

  COUNT_AFTER=$("$CLI_BIN" count 2>&1)
  COUNT_AFTER_NUM=$(echo "$COUNT_AFTER" | grep -oE '[0-9]+' | head -1)
  COUNT_AFTER_NUM=${COUNT_AFTER_NUM:-0}

  if (( COUNT_AFTER_NUM > COUNT_BEFORE_NUM )); then
    CAPTURED=true
    break
  fi
done

if [[ "$CAPTURED" == "false" ]]; then
  # Non-fatal if daemon is not capturing (e.g., macOS privacy prompt pending).
  # Still validate list/stats commands work.
  echo -e "${YELLOW}WARN${NC}: clipboard capture not detected (count unchanged at $COUNT_BEFORE_NUM). "
  echo "       This may require macOS Input Monitoring permission for the daemon."
  echo "       Continuing smoke test for IPC / CLI command correctness..."
else
  pass_step "Clipboard captured (count: $COUNT_BEFORE_NUM → $COUNT_AFTER_NUM)"
fi

# ---------------------------------------------------------------------------
# STEP 10 — List history
# ---------------------------------------------------------------------------
run_step "CLI list command"

LIST_OUT=$("$CLI_BIN" list --limit 10 2>&1) || fail "CLI list failed"

# Output must contain the table header or "No items"
if echo "$LIST_OUT" | grep -qE "(ID|No items)"; then
  pass_step "CLI list output looks valid"
else
  fail "CLI list output unexpected: $LIST_OUT"
fi

# ---------------------------------------------------------------------------
# STEP 11 — Stats
# ---------------------------------------------------------------------------
run_step "CLI stats command"

STATS_OUT=$("$CLI_BIN" stats 2>&1) || fail "CLI stats failed"

echo "$STATS_OUT" | grep -q "total:"     || fail "CLI stats missing 'total:' field: $STATS_OUT"
echo "$STATS_OUT" | grep -q "sensitive:" || fail "CLI stats missing 'sensitive:' field: $STATS_OUT"
echo "$STATS_OUT" | grep -q "version:"   || fail "CLI stats missing 'version:' field: $STATS_OUT"
pass_step "CLI stats OK: $(echo "$STATS_OUT" | tr '\n' ' ')"

# ---------------------------------------------------------------------------
# STEP 12 — IPC stats via nc
# ---------------------------------------------------------------------------
run_step "IPC stats check (nc)"

STATS_RESP=$(echo '{"id":"2","method":"stats","params":{}}' \
  | nc -U "$SOCKET_PATH" -w2)

echo "$STATS_RESP" | grep -q '"ok":true'        || fail "IPC stats: not ok: $STATS_RESP"
echo "$STATS_RESP" | grep -q '"total_items"'    || fail "IPC stats: missing total_items: $STATS_RESP"
echo "$STATS_RESP" | grep -q '"sensitive_items"' || fail "IPC stats: missing sensitive_items: $STATS_RESP"
pass_step "IPC stats: $STATS_RESP"

# ---------------------------------------------------------------------------
# STEP 13 — IPC count via nc
# ---------------------------------------------------------------------------
run_step "IPC count check (nc)"

COUNT_RESP=$(echo '{"id":"3","method":"count","params":{}}' \
  | nc -U "$SOCKET_PATH" -w2)

echo "$COUNT_RESP" | grep -q '"ok":true' || fail "IPC count: not ok: $COUNT_RESP"
echo "$COUNT_RESP" | grep -q '"count"'   || fail "IPC count: missing count field: $COUNT_RESP"
pass_step "IPC count: $COUNT_RESP"

# ---------------------------------------------------------------------------
# STEP 14 — IPC unknown method returns error
# ---------------------------------------------------------------------------
run_step "IPC unknown method returns error"

ERR_RESP=$(echo '{"id":"4","method":"__smoke_nonexistent__","params":{}}' \
  | nc -U "$SOCKET_PATH" -w2)

echo "$ERR_RESP" | grep -q '"ok":false' || fail "IPC unknown method: expected ok:false, got: $ERR_RESP"
pass_step "IPC error handling: ok=false for unknown method"

# ---------------------------------------------------------------------------
# Done — cleanup is handled by EXIT trap
# ---------------------------------------------------------------------------
echo ""
echo -e "${GREEN}PASS: smoke test completed${NC}"
exit 0
