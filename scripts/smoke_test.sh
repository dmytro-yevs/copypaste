#!/usr/bin/env bash
# smoke_test.sh — end-to-end macOS smoke test for CopyPaste
# Tests: build, daemon startup, IPC status, clipboard capture, list, stats, cleanup.
#
# Usage:
#   bash scripts/smoke_test.sh [--skip-build]
#   bash scripts/smoke_test.sh --from-bundle <path-to-CopyPaste.app>
#
# Modes:
#   (default)        Build/use the release binaries in target/release; runs
#                    against the user's normal Application Support socket/DB so
#                    real clipboard polling is exercised (with backup/restore).
#   --from-bundle    Run the SHIPPED daemon + CLI from inside a built
#                    CopyPaste.app (Contents/MacOS/). Uses a fully ISOLATED
#                    temp environment (own socket/DB/dirs, ephemeral key) so it
#                    never touches the real Keychain or user data — this proves
#                    the actual shipped daemon starts, binds its IPC socket,
#                    opens its DB, and round-trips a copied string. On startup
#                    failure it FAILS LOUDLY with the daemon's stderr.
#
# Requirements: macOS, Rust toolchain (unless --from-bundle/--skip-build),
#               nc (netcat with -U), pbcopy/pbpaste, base64, jq (optional)

set -euo pipefail

# ---------------------------------------------------------------------------
# Colours
# ---------------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Colour

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
SKIP_BUILD=false
FROM_BUNDLE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build) SKIP_BUILD=true; shift ;;
    --from-bundle) FROM_BUNDLE="${2:?--from-bundle needs a path to CopyPaste.app}"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

# ---------------------------------------------------------------------------
# Paths and binaries — depend on the mode.
#
# Default mode: the daemon/CLI use their built-in Application Support paths
# (real clipboard polling, backup/restore of user data).
#
# --from-bundle mode: run the SHIPPED binaries from the .app and point them at
# an ISOLATED temp environment via COPYPASTE_* env overrides (own socket/DB/
# dirs + ephemeral key), so nothing touches the real Keychain or user data.
# ---------------------------------------------------------------------------
ISOLATED=false
ISO_ROOT=""
DAEMON_STDERR=""

if [[ -n "$FROM_BUNDLE" ]]; then
  ISOLATED=true
  DAEMON_BIN="$FROM_BUNDLE/Contents/MacOS/copypaste-daemon"
  CLI_BIN="$FROM_BUNDLE/Contents/MacOS/copypaste"
  [[ -x "$DAEMON_BIN" ]] || { echo "FAIL: daemon not found in bundle: $DAEMON_BIN" >&2; exit 1; }
  [[ -x "$CLI_BIN" ]]    || { echo "FAIL: CLI not found in bundle: $CLI_BIN" >&2; exit 1; }

  ISO_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/copypaste-smoke-bundle.XXXXXX")"
  mkdir -p "$ISO_ROOT/data" "$ISO_ROOT/config" "$ISO_ROOT/cache" "$ISO_ROOT/logs"
  SOCKET_PATH="$ISO_ROOT/copypaste.sock"
  DB_PATH="$ISO_ROOT/clipboard.db"
  DAEMON_STDERR="$ISO_ROOT/daemon.stderr"

  # Export the isolation env so BOTH the daemon and the CLI agree on paths.
  export COPYPASTE_SOCKET="$SOCKET_PATH"
  export COPYPASTE_DB="$DB_PATH"
  export COPYPASTE_DATA_DIR="$ISO_ROOT/data"
  export COPYPASTE_CONFIG_DIR="$ISO_ROOT/config"
  export COPYPASTE_CACHE_DIR="$ISO_ROOT/cache"
  export COPYPASTE_LOG_DIR="$ISO_ROOT/logs"
  export COPYPASTE_DEVICE_ID_PATH="$ISO_ROOT/device_id"
  export COPYPASTE_EPHEMERAL_KEY=1
else
  SUPPORT_DIR="$HOME/Library/Application Support/CopyPaste"
  SOCKET_PATH="$SUPPORT_DIR/daemon.sock"
  DB_PATH="$SUPPORT_DIR/clipboard.db"
  DAEMON_BIN="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/target/release/copypaste-daemon"
  # Package name is `copypaste-cli` but the produced binary is `copypaste`.
  CLI_BIN="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/target/release/copypaste"
fi

DAEMON_PID=""
DAEMON_OWNED=false      # true only if THIS script started the daemon
BACKUP_SOCK=""
BACKUP_DB=""

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
fail() {
  echo -e "${RED}FAIL: $*${NC}" >&2
  # Surface the daemon's stderr so startup failures are diagnosable (bundle mode
  # always captures it; default mode inherits the terminal's stderr).
  if [[ -n "$DAEMON_STDERR" && -s "$DAEMON_STDERR" ]]; then
    echo -e "${YELLOW}--- daemon stderr ($DAEMON_STDERR) ---${NC}" >&2
    tail -n 80 "$DAEMON_STDERR" >&2 || true
  fi
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

  # Remove the isolated temp root used by --from-bundle mode.
  if [[ -n "$ISO_ROOT" && -d "$ISO_ROOT" ]]; then
    rm -rf "$ISO_ROOT" 2>/dev/null || true
    ISO_ROOT=""
  fi
}
trap 'cleanup' EXIT

# ---------------------------------------------------------------------------
# STEP 1 — Build binaries (skipped in --from-bundle mode and with --skip-build)
# ---------------------------------------------------------------------------
run_step "Build release binaries"

if [[ "$ISOLATED" == "true" ]]; then
  pass_step "Using SHIPPED binaries from bundle (no build): $DAEMON_BIN"
elif [[ "$SKIP_BUILD" == "true" && -x "$DAEMON_BIN" && -x "$CLI_BIN" ]]; then
  pass_step "Skipping build (--skip-build, binaries present)"
else
  ( cd "$(dirname "${BASH_SOURCE[0]}")/.." && cargo build --release -p copypaste-daemon -p copypaste-cli ) \
    || fail "cargo build failed"
  pass_step "Binaries built: $DAEMON_BIN, $CLI_BIN"
fi

[[ -x "$DAEMON_BIN" ]] || fail "daemon binary not found or not executable: $DAEMON_BIN"
[[ -x "$CLI_BIN" ]]    || fail "CLI binary not found or not executable: $CLI_BIN"

# ---------------------------------------------------------------------------
# STEP 2 — Ensure app support dir exists
# ---------------------------------------------------------------------------
run_step "Ensure support directory"
if [[ "$ISOLATED" == "true" ]]; then
  pass_step "Isolated env ready: $ISO_ROOT"
else
  mkdir -p "$SUPPORT_DIR" || fail "cannot create $SUPPORT_DIR"
  pass_step "Support dir ready: $SUPPORT_DIR"
fi

# ---------------------------------------------------------------------------
# STEP 3 — Start the daemon.
#
# Bundle/isolated mode ALWAYS starts a fresh daemon from the shipped binary in
# its own temp environment (no existing-daemon reuse, no user-data backup —
# everything lives in $ISO_ROOT and is wiped on exit). Default mode reuses a
# running daemon if present, otherwise backs up user data and starts fresh.
# ---------------------------------------------------------------------------
if [[ "$ISOLATED" == "true" ]]; then
  run_step "Start fresh SHIPPED daemon (isolated env, ephemeral key)"
  "$DAEMON_BIN" >/dev/null 2>"$DAEMON_STDERR" &
  DAEMON_PID=$!
  DAEMON_OWNED=true
  # If it dies immediately, fail loudly with its stderr.
  sleep 0.3
  if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
    fail "shipped daemon exited immediately on startup"
  fi
  pass_step "Daemon started (PID=$DAEMON_PID)"
else
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
# STEP 9b — Deterministic round-trip through REAL IPC (import -> history_page).
#
# NSPasteboard polling is gated behind macOS Input Monitoring permission, which
# CI cannot grant — so the pbcopy path above is best-effort. This step instead
# pushes a unique item through the daemon's `import` IPC method (encrypt + DB
# insert) and reads it back via `history_page` (DB read), proving the daemon
# opened its DB and round-trips an item end-to-end over the real socket.
#
# We assert on the item's unique `wall_time` (= the `created_at_ms` we sent),
# NOT on the preview text: imported items are stored encrypted and their FTS
# plaintext preview is intentionally not populated by the import path (the
# list view shows a "[text — id:…]" placeholder for them), so matching the
# marker in the preview would be wrong. A unique millisecond timestamp is a
# stable, mode-independent witness that the exact row we inserted came back.
# This is asserted in ALL modes and is the load-bearing "an item round-trips
# through real IPC" check for the shipped daemon.
# ---------------------------------------------------------------------------
run_step "Round-trip an item through real IPC (import -> history_page)"

# Unique wall_time witness: epoch seconds * 1000 + a random sub-second component
# so two runs (and the two history rows from this run vs prior) never collide.
RT_WALL_TIME=$(( $(date +%s) * 1000 + RANDOM % 1000 ))
RT_MARKER="copypaste-ipc-roundtrip-${RT_WALL_TIME}"
RT_B64=$(printf '%s' "$RT_MARKER" | base64 | tr -d '\n')
RT_IMPORT_BODY="{\"id\":\"rti\",\"method\":\"import\",\"params\":{\"items\":[{\"content_type\":\"text\",\"content_bytes_b64\":\"$RT_B64\",\"created_at_ms\":$RT_WALL_TIME}]}}"

RT_IMPORT_RESP=$(printf '%s\n' "$RT_IMPORT_BODY" | nc -U "$SOCKET_PATH" -w5)
echo "$RT_IMPORT_RESP" | grep -q '"ok":true'     || fail "IPC import failed: $RT_IMPORT_RESP"
echo "$RT_IMPORT_RESP" | grep -q '"inserted":1'  || fail "IPC import did not insert item: $RT_IMPORT_RESP"

RT_HISTORY_RESP=$(echo '{"id":"rth","method":"history_page","params":{"limit":50,"offset":0}}' \
  | nc -U "$SOCKET_PATH" -w5)
echo "$RT_HISTORY_RESP" | grep -q '"ok":true' \
  || fail "history_page IPC failed (DB read): $RT_HISTORY_RESP"
echo "$RT_HISTORY_RESP" | grep -q "\"wall_time\":$RT_WALL_TIME" \
  || fail "imported item (wall_time=$RT_WALL_TIME) did not round-trip back via history_page (DB read failed): $RT_HISTORY_RESP"
pass_step "IPC round-trip OK: imported and read back item wall_time=$RT_WALL_TIME"

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
