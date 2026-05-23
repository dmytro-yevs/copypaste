#!/usr/bin/env bash
# restore-db.sh — Restore an encrypted SQLCipher backup of the CopyPaste DB.
#
# What it does:
#   1. Validates the supplied backup file opens with the *current* db_key
#      (a quick `PRAGMA key; SELECT count(*) FROM sqlite_master` smoke test).
#   2. If a live DB exists in the data dir, it is renamed with a timestamp
#      suffix (e.g. clipboard.db.before-restore-YYYYMMDD-HHMMSS) unless
#      --force is passed (which deletes the live DB before copying).
#   3. Copies the backup into place as `<data-dir>/clipboard.db`.
#   4. chmod 600 the new file.
#
# Caller is responsible for stopping the daemon BEFORE restore and starting
# it AFTER. We refuse to clobber a live DB without a backup-rename or --force.
#
# Usage:
#   restore-db.sh <backup-file> [flags]
#
# Flags:
#   --force      Delete the existing live DB instead of renaming it aside.
#   --dry-run    Show what would happen, change nothing on disk.
#   --help       Print this help and exit 0.
#
# Exit codes:
#   0  Success.
#   1  Generic error (key mismatch, missing file, etc).
#   2  Bad CLI args.
#
# Requirements:
#   - sqlcipher CLI on PATH (for the key-verification smoke test).
#   - <data-dir>/db_key present (the key must match the backup).
#
# See: docs/ops/backup-restore.md

set -euo pipefail

# ─── Defaults ────────────────────────────────────────────────────────────────
DRY_RUN=0
FORCE=0
BACKUP_FILE=""

DEFAULT_DATA_ROOT="${HOME:-/tmp}/Library/Application Support"
DATA_ROOT="${COPYPASTE_DATA_HOME:-$DEFAULT_DATA_ROOT}"

CANONICAL_DIR="CopyPaste"
ALIAS_DIRS=("copypaste" "Copypaste")

# ─── Helpers ─────────────────────────────────────────────────────────────────
die() { echo "ERROR: $*" >&2; exit 1; }
warn() { echo "WARN:  $*" >&2; }
info() { echo "INFO:  $*"; }

usage() {
    sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//'
}

run() {
    echo "    \$ $*"
    if [[ "$DRY_RUN" -eq 0 ]]; then
        "$@"
    fi
}

# ─── Parse args ──────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --force)   FORCE=1;   shift ;;
        --dry-run) DRY_RUN=1; shift ;;
        --help|-h) usage; exit 0 ;;
        -*)
            echo "Unknown flag: $1" >&2
            usage >&2
            exit 2
            ;;
        *)
            if [[ -z "$BACKUP_FILE" ]]; then
                BACKUP_FILE="$1"
                shift
            else
                echo "Unexpected positional arg: $1" >&2
                usage >&2
                exit 2
            fi
            ;;
    esac
done

[[ -n "$BACKUP_FILE" ]] || { echo "Missing <backup-file>" >&2; usage >&2; exit 2; }
[[ -f "$BACKUP_FILE" ]] || die "Backup file not found: $BACKUP_FILE"

# Resolve to absolute path before any cd/relative work.
BACKUP_FILE="$(cd "$(dirname "$BACKUP_FILE")" && pwd)/$(basename "$BACKUP_FILE")"

# ─── Locate data dir ─────────────────────────────────────────────────────────
DATA_DIR=""
for candidate in "$CANONICAL_DIR" "${ALIAS_DIRS[@]}"; do
    if [[ -d "$DATA_ROOT/$candidate" ]]; then
        DATA_DIR="$DATA_ROOT/$candidate"
        break
    fi
done

if [[ -z "$DATA_DIR" ]]; then
    die "No CopyPaste data dir found under: $DATA_ROOT
     (set COPYPASTE_DATA_HOME to override the root)"
fi

DB_PATH="$DATA_DIR/clipboard.db"
KEY_PATH="$DATA_DIR/db_key"

[[ -f "$KEY_PATH" ]] || die "DB key not found: $KEY_PATH
     The current key file must match the key used when the backup was made."

# ─── Validate tools ──────────────────────────────────────────────────────────
if ! command -v sqlcipher >/dev/null 2>&1; then
    if [[ "$DRY_RUN" -eq 1 ]]; then
        warn "sqlcipher CLI not found (continuing because --dry-run)."
    else
        die "sqlcipher CLI not found on PATH.
     Install:
       macOS:  brew install sqlcipher
       Linux:  apt-get install sqlcipher  (or distro equivalent)"
    fi
fi

info "Data dir : $DATA_DIR"
info "Backup   : $BACKUP_FILE"
info "Live DB  : $DB_PATH"
info "Dry run  : $([[ $DRY_RUN -eq 1 ]] && echo yes || echo no)"
info "Force    : $([[ $FORCE -eq 1 ]] && echo yes || echo no)"
echo ""

# ─── Verify key opens the backup ─────────────────────────────────────────────
info "Verifying SQLCipher key works against backup..."
if [[ "$DRY_RUN" -eq 0 ]]; then
    DB_KEY="$(tr -d '[:space:]' < "$KEY_PATH")"
    [[ -n "$DB_KEY" ]] || die "DB key file is empty: $KEY_PATH"

    SMOKE_OUT="$(sqlcipher "$BACKUP_FILE" <<SQL 2>&1 || true
PRAGMA key = "x'${DB_KEY}'";
SELECT count(*) FROM sqlite_master;
.exit
SQL
)"

    # On key mismatch sqlcipher prints "file is not a database" or
    # "file is encrypted or is not a database".
    if echo "$SMOKE_OUT" | grep -qiE "not a database|encrypted"; then
        die "Backup did not open with current db_key.
     The key in $KEY_PATH does NOT match the key used for this backup.
     Restore aborted to avoid data loss.
     Output: $SMOKE_OUT"
    fi
    info "Key verification OK."
else
    echo "    \$ sqlcipher $BACKUP_FILE (PRAGMA key + count check)"
fi

# ─── Move existing DB aside (or delete with --force) ─────────────────────────
if [[ -f "$DB_PATH" ]]; then
    if [[ "$FORCE" -eq 1 ]]; then
        info "--force given: removing existing live DB."
        run rm -f "$DB_PATH"
        # SQLite sidecars
        run rm -f "${DB_PATH}-wal" "${DB_PATH}-shm" || true
    else
        TS="$(date +%Y%m%d-%H%M%S)"
        ASIDE="${DB_PATH}.before-restore-${TS}"
        info "Renaming existing live DB aside (use --force to delete instead)."
        run mv "$DB_PATH" "$ASIDE"
        # Move WAL/SHM aside if present.
        [[ -f "${DB_PATH}-wal" ]] && run mv "${DB_PATH}-wal" "${ASIDE}-wal"
        [[ -f "${DB_PATH}-shm" ]] && run mv "${DB_PATH}-shm" "${ASIDE}-shm"
        info "Old DB saved at: $ASIDE"
    fi
fi

# ─── Copy backup into place ──────────────────────────────────────────────────
info "Copying backup to live location..."
run cp "$BACKUP_FILE" "$DB_PATH"
run chmod 600 "$DB_PATH"

echo ""
info "Restore OK. Start the daemon to resume service."
info "  launchctl bootstrap gui/\$(id -u) ~/Library/LaunchAgents/com.copypaste.daemon.plist"
