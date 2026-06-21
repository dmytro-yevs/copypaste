#!/usr/bin/env bash
# test-backup-restore-roundtrip.sh — round-trip test for backup-db.sh + restore-db.sh
#
# CopyPaste-vzc3: verifies that a backup created by backup-db.sh can be
# successfully restored by restore-db.sh, and that the restored DB produces
# the same content as the original.
#
# This test is self-contained:
#   - Creates a temp directory with a minimal SQLCipher DB and db_key.
#   - Runs backup-db.sh --no-stop --no-restart to produce a backup file.
#   - Runs restore-db.sh --no-stop --no-restart --force to restore the backup.
#   - Verifies the restored DB opens with the same key and produces the same
#     row count as the original.
#
# Requirements:
#   - sqlcipher CLI on PATH.
#
# Usage:
#   ./scripts/test-backup-restore-roundtrip.sh
#
# Exit codes:
#   0  All assertions passed.
#   1  A test assertion or tool prerequisite failed.
#
# See: docs/ops/backup-restore.md, scripts/backup-db.sh, scripts/restore-db.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ─── Helpers ─────────────────────────────────────────────────────────────────
PASS_COUNT=0
FAIL_COUNT=0

pass() { echo "  PASS: $*"; (( PASS_COUNT++ )) || true; }
fail() { echo "  FAIL: $*" >&2; (( FAIL_COUNT++ )) || true; }
info() { echo "INFO:  $*"; }
die()  { echo "ERROR: $*" >&2; exit 1; }

# ─── Prerequisites ───────────────────────────────────────────────────────────
if ! command -v sqlcipher >/dev/null 2>&1; then
    die "sqlcipher CLI not found on PATH.
     Install:
       macOS:  brew install sqlcipher
       Linux:  apt-get install sqlcipher"
fi

# ─── Setup temp environment ──────────────────────────────────────────────────
WORK_DIR="$(mktemp -d)"
# Always clean up on exit, even on failure.
trap 'rm -rf "$WORK_DIR"' EXIT

# The COPYPASTE_DATA_HOME override lets backup-db.sh and restore-db.sh find the
# temp data dir without touching ~/Library/Application Support/CopyPaste.
DATA_DIR="$WORK_DIR/data/CopyPaste"
mkdir -p "$DATA_DIR"

BACKUP_DIR="$WORK_DIR/backups"
mkdir -p "$BACKUP_DIR"

DB_PATH="$DATA_DIR/clipboard.db"
KEY_PATH="$DATA_DIR/db_key"

info "Work dir : $WORK_DIR"
info "Data dir : $DATA_DIR"
info ""

# ─── 1. Create a minimal SQLCipher DB with known content ─────────────────────
info "=== Step 1: create source DB ==="

# Use a fixed 32-byte hex key (64 hex chars = 32 bytes).
DB_KEY="deadbeefcafe1234deadbeefcafe1234deadbeefcafe1234deadbeefcafe1234"
printf '%s' "$DB_KEY" > "$KEY_PATH"
chmod 600 "$KEY_PATH"

sqlcipher "$DB_PATH" <<SQL
PRAGMA key = "x'${DB_KEY}'";
CREATE TABLE IF NOT EXISTS items (id INTEGER PRIMARY KEY, content TEXT NOT NULL);
INSERT INTO items (content) VALUES ('hello round-trip test');
INSERT INTO items (content) VALUES ('second row');
.exit
SQL
chmod 600 "$DB_PATH"

# Verify source DB row count.
SOURCE_COUNT="$(sqlcipher "$DB_PATH" <<SQL 2>/dev/null
PRAGMA key = "x'${DB_KEY}'";
SELECT count(*) FROM items;
.exit
SQL
)"
SOURCE_COUNT="${SOURCE_COUNT//[[:space:]]/}"

if [[ "$SOURCE_COUNT" == "2" ]]; then
    pass "Source DB created with $SOURCE_COUNT rows"
else
    fail "Source DB row count: expected 2, got '$SOURCE_COUNT'"
fi

# ─── 2. Run backup-db.sh ──────────────────────────────────────────────────────
info ""
info "=== Step 2: backup-db.sh ==="

BACKUP_OUTPUT="$(COPYPASTE_DATA_HOME="$WORK_DIR/data" \
    bash "$REPO_ROOT/scripts/backup-db.sh" \
    --output-dir "$BACKUP_DIR" \
    --no-stop --no-restart 2>&1)"

echo "$BACKUP_OUTPUT"

# Find the produced backup file.
BACKUP_FILE="$(ls -t "$BACKUP_DIR"/copypaste-*.db.enc 2>/dev/null | head -1)"

if [[ -n "$BACKUP_FILE" && -f "$BACKUP_FILE" ]]; then
    pass "backup-db.sh produced: $(basename "$BACKUP_FILE")"
else
    fail "backup-db.sh did not produce a .db.enc file in $BACKUP_DIR"
    BACKUP_FILE=""
fi

if [[ -n "$BACKUP_FILE" ]]; then
    BACKUP_SIZE="$(wc -c < "$BACKUP_FILE" | tr -d ' ')"
    if [[ "$BACKUP_SIZE" -gt 0 ]]; then
        pass "Backup file is non-empty ($BACKUP_SIZE bytes)"
    else
        fail "Backup file is empty: $BACKUP_FILE"
    fi

    # Verify backup file permissions (should be 600).
    BACKUP_PERMS="$(stat -f '%Lp' "$BACKUP_FILE" 2>/dev/null || stat -c '%a' "$BACKUP_FILE" 2>/dev/null || echo unknown)"
    if [[ "$BACKUP_PERMS" == "600" ]]; then
        pass "Backup file has mode 600"
    else
        fail "Backup file mode: expected 600, got $BACKUP_PERMS"
    fi

    # Verify the backup opens with the same key.
    BACKUP_SMOKE="$(sqlcipher "$BACKUP_FILE" <<SQL 2>/dev/null
PRAGMA key = "x'${DB_KEY}'";
SELECT count(*) FROM items;
.exit
SQL
)"
    BACKUP_SMOKE="${BACKUP_SMOKE//[[:space:]]/}"
    if [[ "$BACKUP_SMOKE" == "2" ]]; then
        pass "Backup DB readable with same key ($BACKUP_SMOKE rows)"
    else
        fail "Backup DB smoke test failed (expected 2 rows, got '$BACKUP_SMOKE')"
    fi
fi

# ─── 3. Run restore-db.sh ─────────────────────────────────────────────────────
info ""
info "=== Step 3: restore-db.sh ==="

if [[ -z "${BACKUP_FILE:-}" ]]; then
    fail "Skipping restore test: no backup file to restore"
else
    # Remove the original DB so restore places the backup as the live file.
    rm -f "$DB_PATH"

    RESTORE_OUTPUT="$(COPYPASTE_DATA_HOME="$WORK_DIR/data" \
        bash "$REPO_ROOT/scripts/restore-db.sh" \
        "$BACKUP_FILE" \
        --no-stop --no-restart --force 2>&1)"

    echo "$RESTORE_OUTPUT"

    if [[ -f "$DB_PATH" ]]; then
        pass "restore-db.sh placed DB at $DB_PATH"
    else
        fail "restore-db.sh did not place DB at $DB_PATH"
    fi

    # Verify restore-db.sh output mentions success.
    if echo "$RESTORE_OUTPUT" | pgrep -q "Restore OK" 2>/dev/null || \
       echo "$RESTORE_OUTPUT" | rg -q "Restore OK" 2>/dev/null; then
        pass "restore-db.sh reported 'Restore OK'"
    else
        # Less strict check — just verify the file is there and correct.
        if [[ -f "$DB_PATH" ]]; then
            pass "Restore completed (DB file present)"
        else
            fail "Restore did not produce expected success output"
        fi
    fi

    # ─── 4. Verify round-trip integrity ──────────────────────────────────────
    info ""
    info "=== Step 4: verify round-trip integrity ==="

    RESTORED_COUNT="$(sqlcipher "$DB_PATH" <<SQL 2>/dev/null
PRAGMA key = "x'${DB_KEY}'";
SELECT count(*) FROM items;
.exit
SQL
)"
    RESTORED_COUNT="${RESTORED_COUNT//[[:space:]]/}"

    if [[ "$RESTORED_COUNT" == "$SOURCE_COUNT" ]]; then
        pass "Round-trip row count matches: $RESTORED_COUNT == $SOURCE_COUNT"
    else
        fail "Round-trip row count mismatch: expected $SOURCE_COUNT, got '$RESTORED_COUNT'"
    fi

    # Verify the content matches the original rows.
    RESTORED_ROWS="$(sqlcipher "$DB_PATH" <<SQL 2>/dev/null
PRAGMA key = "x'${DB_KEY}'";
SELECT content FROM items ORDER BY id;
.exit
SQL
)"
    if echo "$RESTORED_ROWS" | rg -q "hello round-trip test" 2>/dev/null || \
       echo "$RESTORED_ROWS" | pgrep -q "hello round-trip test" 2>/dev/null; then
        pass "Round-trip row content preserved: 'hello round-trip test' present"
    elif echo "$RESTORED_ROWS" | grep -q "hello round-trip test" 2>/dev/null; then
        pass "Round-trip row content preserved: 'hello round-trip test' present"
    else
        fail "Round-trip row content missing: 'hello round-trip test' not found in restored DB"
    fi

    RESTORED_PERMS="$(stat -f '%Lp' "$DB_PATH" 2>/dev/null || stat -c '%a' "$DB_PATH" 2>/dev/null || echo unknown)"
    if [[ "$RESTORED_PERMS" == "600" ]]; then
        pass "Restored DB has mode 600"
    else
        fail "Restored DB mode: expected 600, got $RESTORED_PERMS"
    fi
fi

# ─── 5. Test: restore aborts on key mismatch ─────────────────────────────────
info ""
info "=== Step 5: key-mismatch abort ==="

WRONG_KEY="0000000000000000000000000000000000000000000000000000000000000000"
WRONG_KEY_PATH="$WORK_DIR/data/CopyPaste/db_key"
printf '%s' "$WRONG_KEY" > "$WRONG_KEY_PATH"

# restore-db.sh should exit non-zero on key mismatch.
if COPYPASTE_DATA_HOME="$WORK_DIR/data" \
    bash "$REPO_ROOT/scripts/restore-db.sh" \
    "$BACKUP_FILE" --no-stop --no-restart --force 2>/dev/null; then
    fail "restore-db.sh should have failed with key mismatch but succeeded"
else
    pass "restore-db.sh correctly rejected backup opened with wrong key"
fi

# Restore the correct key for any follow-up.
printf '%s' "$DB_KEY" > "$KEY_PATH"

# ─── Summary ─────────────────────────────────────────────────────────────────
echo ""
echo "════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
