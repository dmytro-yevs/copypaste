#!/usr/bin/env bash
# migrate-alpha-to-beta.sh — Migrate CopyPaste v0.1.0-alpha data to v0.2.0-beta.
#
# What it does (idempotent, safe to re-run):
#   1. Locates the alpha data directory (~/Library/Application Support/{copypaste,CopyPaste}).
#   2. Snapshots it to a timestamped .bak/ sibling (unless --no-backup).
#   3. Applies beta SQLite schema migrations on `clipboard.db`:
#        - PRAGMA user_version: 1 -> 2
#        - ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT
#        - CREATE INDEX idx_clipboard_content_hash
#      Each step is a no-op if already present (idempotent).
#   4. Preserves `device_id`, `config.toml`, `db_key` (if present) untouched.
#   5. If the alpha dir is lowercase `copypaste`, rename to canonical `CopyPaste`.
#   6. Prints a hint to restart the daemon.
#
# Flags:
#   --dry-run    Show what would happen, change nothing on disk.
#   --no-backup  Skip the .bak snapshot (advanced, not recommended).
#   --help       Print this help and exit 0.
#
# Exit codes:
#   0  Success (including "nothing to do").
#   1  Generic error (filesystem, sqlite, etc).
#   2  Bad CLI args.
#
# Author: CopyPaste maintainers. See docs/migrations/alpha-to-beta.md.

set -euo pipefail

# Defaults
DRY_RUN=0
NO_BACKUP=0

# Allow override via COPYPASTE_DATA_HOME for tests; otherwise canonical mac path.
DEFAULT_ROOT="${HOME:-/tmp}/Library/Application Support"
DATA_ROOT="${COPYPASTE_DATA_HOME:-$DEFAULT_ROOT}"

# Canonical (beta) dir name + known alpha aliases.
CANONICAL_DIR="CopyPaste"
ALIAS_DIRS=("copypaste" "Copypaste")

# Helpers
log()  { printf '[migrate] %s\n' "$*"; }
warn() { printf '[migrate] WARN: %s\n' "$*" >&2; }
fail() { printf '[migrate] ERROR: %s\n' "$*" >&2; }

usage() {
    sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'
}

run() {
    if [ "$DRY_RUN" -eq 1 ]; then
        printf '[dry-run] %s\n' "$*"
    else
        "$@"
    fi
}

sqlite_run() {
    local db="$1"; shift
    local sql="$1"; shift
    if [ "$DRY_RUN" -eq 1 ]; then
        printf '[dry-run] sqlite3 %s "%s"\n' "$db" "$sql"
        return 0
    fi
    sqlite3 "$db" "$sql"
}

# Arg parsing
while [ "$#" -gt 0 ]; do
    case "$1" in
        --dry-run)    DRY_RUN=1 ;;
        --no-backup)  NO_BACKUP=1 ;;
        -h|--help)    usage; exit 0 ;;
        *)
            fail "Unknown argument: $1"
            usage
            exit 2
            ;;
    esac
    shift
done

# 1. Detect alpha data dir
find_data_dir() {
    local cand
    for cand in "$CANONICAL_DIR" "${ALIAS_DIRS[@]}"; do
        local p="$DATA_ROOT/$cand"
        if [ -d "$p" ]; then
            printf '%s\n' "$p"
            return 0
        fi
    done
    return 1
}

DATA_DIR=""
if DATA_DIR="$(find_data_dir)"; then
    log "Found CopyPaste data dir: $DATA_DIR"
else
    log "No CopyPaste data directory found under $DATA_ROOT — nothing to migrate."
    log "(This is normal for a fresh beta install.)"
    exit 0
fi

DB_FILE="$DATA_DIR/clipboard.db"
DEVICE_ID_FILE="$DATA_DIR/device_id"
CONFIG_FILE="$DATA_DIR/config.toml"
DB_KEY_FILE="$DATA_DIR/db_key"

# 2. Backup
BACKUP_DIR=""
if [ "$NO_BACKUP" -eq 0 ]; then
    TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
    BACKUP_DIR="${DATA_DIR}.bak.${TIMESTAMP}"
    log "Creating backup: $BACKUP_DIR"
    if [ "$DRY_RUN" -eq 1 ]; then
        printf '[dry-run] cp -Rp %s %s\n' "$DATA_DIR" "$BACKUP_DIR"
    else
        cp -Rp "$DATA_DIR" "$BACKUP_DIR"
    fi
else
    warn "--no-backup specified; skipping snapshot."
fi

# 3. Preserve sensitive files (sanity check; never overwrite)
for f in "$DEVICE_ID_FILE" "$CONFIG_FILE" "$DB_KEY_FILE"; do
    if [ -e "$f" ]; then
        log "Preserving: $(basename "$f")"
    fi
done

# 4. SQLite schema migration
if [ ! -f "$DB_FILE" ]; then
    log "No clipboard.db found in $DATA_DIR — skipping schema migration."
else
    if ! command -v sqlite3 >/dev/null 2>&1; then
        fail "sqlite3 CLI not found; install via 'brew install sqlite' or skip with --dry-run."
        exit 1
    fi

    CURRENT_VERSION="0"
    if [ "$DRY_RUN" -eq 0 ]; then
        CURRENT_VERSION="$(sqlite3 "$DB_FILE" 'PRAGMA user_version;' 2>/dev/null || echo 0)"
        CURRENT_VERSION="${CURRENT_VERSION:-0}"
    fi
    log "DB user_version: $CURRENT_VERSION (beta expects 2)"

    if [ "$CURRENT_VERSION" -ge 2 ] 2>/dev/null; then
        log "Schema already at v2 — no migration needed."
    else
        log "Applying schema v2 migration (add content_hash column + index)."

        HAS_COL=0
        if [ "$DRY_RUN" -eq 0 ]; then
            if sqlite3 "$DB_FILE" "PRAGMA table_info(clipboard_items);" \
                 | awk -F'|' '{print $2}' | grep -qx 'content_hash'; then
                HAS_COL=1
            fi
        fi

        if [ "$HAS_COL" -eq 0 ]; then
            sqlite_run "$DB_FILE" \
                "ALTER TABLE clipboard_items ADD COLUMN content_hash TEXT;"
        else
            log "content_hash column already present — skipping ALTER TABLE."
        fi

        sqlite_run "$DB_FILE" \
            "CREATE INDEX IF NOT EXISTS idx_clipboard_content_hash ON clipboard_items(content_hash) WHERE content_hash IS NOT NULL;"

        sqlite_run "$DB_FILE" "PRAGMA user_version = 2;"

        log "Schema migration complete."
    fi
fi

# 5. Rename alpha-lowercase dir -> canonical
BASENAME="$(basename "$DATA_DIR")"
if [ "$BASENAME" != "$CANONICAL_DIR" ]; then
    TARGET="$DATA_ROOT/$CANONICAL_DIR"
    if [ -e "$TARGET" ]; then
        warn "Cannot rename '$BASENAME' -> '$CANONICAL_DIR': target already exists."
        warn "Manual merge required. See docs/migrations/alpha-to-beta.md."
    else
        log "Renaming '$BASENAME' -> '$CANONICAL_DIR' (canonical beta path)."
        run mv "$DATA_DIR" "$TARGET"
    fi
fi

# 6. Final hint
log "Migration complete."
log "Next steps:"
log "  1. Restart the CopyPaste daemon (relaunch the app, or 'launchctl kickstart')."
log "  2. Verify with: 'copypaste history --limit 5'"
log "  3. Backup retained at: ${BACKUP_DIR:-<skipped>}"

exit 0
