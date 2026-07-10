#!/usr/bin/env bash
# backup-db.sh — Create an encrypted SQLCipher backup of the CopyPaste database.
#
# What it does (safe to re-run; produces a new timestamped file each call):
#   1. Locates the data dir (~/Library/Application Support/CopyPaste, override
#      via COPYPASTE_DATA_HOME).
#   2. Optionally stops the daemon (launchctl bootout, falls back to pkill) so
#      the SQLite write lock is released; skipped if --no-stop.
#   3. Runs `sqlcipher ... .backup <path>` to produce a hot, consistent copy
#      that remains encrypted with the same key as the source DB.
#   4. Writes the backup to ./backups/copypaste-{YYYYMMDD-HHMMSS}.db.enc
#      (or --output-dir <path>).
#   5. Optionally restarts the daemon afterwards; skipped if --no-restart.
#
# Flags:
#   --output-dir <path>   Directory to write the backup file into.
#                         Default: <repo>/backups
#   --no-stop             Do not attempt to stop the daemon. Use when the
#                         caller is already managing daemon lifecycle.
#   --no-restart          Do not attempt to restart the daemon after backup.
#   --dry-run             Show what would happen, change nothing on disk.
#   --help                Print this help and exit 0.
#
# Exit codes:
#   0  Success.
#   1  Generic error (missing tools, sqlcipher failure, etc).
#   2  Bad CLI args.
#
# Requirements:
#   - sqlcipher CLI on PATH. Install hint:
#       macOS:  brew install sqlcipher
#       Linux:  apt-get install sqlcipher  (or distro equivalent)
#   - The db_key file inside the data dir (managed by the daemon).
#
# See: docs/ops/backup-restore.md

set -euo pipefail

# ─── Defaults ────────────────────────────────────────────────────────────────
DRY_RUN=0
NO_STOP=0
NO_RESTART=0
OUTPUT_DIR=""

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEFAULT_OUTPUT_DIR="$REPO_ROOT/backups"

# shellcheck source=lib/release-identity.sh
source "$REPO_ROOT/scripts/lib/release-identity.sh"   # sets DAEMON_LABEL

DEFAULT_DATA_ROOT="${HOME:-/tmp}/Library/Application Support"
DATA_ROOT="${COPYPASTE_DATA_HOME:-$DEFAULT_DATA_ROOT}"

CANONICAL_DIR="CopyPaste"
ALIAS_DIRS=("copypaste" "Copypaste")

DAEMON_PLIST="$HOME/Library/LaunchAgents/${DAEMON_LABEL}.plist"
DAEMON_PROC="copypaste-daemon"

# ─── Helpers ─────────────────────────────────────────────────────────────────
die() { echo "ERROR: $*" >&2; exit 1; }
warn() { echo "WARN:  $*" >&2; }
info() { echo "INFO:  $*"; }

usage() {
    sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//'
}

run() {
    # Echo + run, honor DRY_RUN.
    echo "    \$ $*"
    if [[ "$DRY_RUN" -eq 0 ]]; then
        "$@"
    fi
}

# ─── Parse args ──────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --output-dir)
            [[ $# -ge 2 ]] || { usage >&2; exit 2; }
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --no-stop)    NO_STOP=1;    shift ;;
        --no-restart) NO_RESTART=1; shift ;;
        --dry-run)    DRY_RUN=1;    shift ;;
        --help|-h)    usage; exit 0 ;;
        *)
            echo "Unknown arg: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

OUTPUT_DIR="${OUTPUT_DIR:-$DEFAULT_OUTPUT_DIR}"

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

[[ -f "$DB_PATH" ]] || die "Database not found: $DB_PATH"
[[ -f "$KEY_PATH" ]] || die "DB key not found: $KEY_PATH
     The daemon manages this file; backup is meaningless without it."

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

# ─── Compute output path ─────────────────────────────────────────────────────
TS="$(date +%Y%m%d-%H%M%S)"
OUTPUT_FILE="$OUTPUT_DIR/copypaste-${TS}.db.enc"

info "Data dir   : $DATA_DIR"
info "Source DB  : $DB_PATH"
info "Output     : $OUTPUT_FILE"
info "Dry run    : $([[ $DRY_RUN -eq 1 ]] && echo yes || echo no)"
info "Stop daemon: $([[ $NO_STOP -eq 1 ]] && echo no || echo yes)"
echo ""

# ─── Stop daemon (optional) ──────────────────────────────────────────────────
DAEMON_WAS_STOPPED=0
if [[ "$NO_STOP" -eq 0 ]]; then
    info "Stopping daemon to release SQLite locks..."
    if [[ -f "$DAEMON_PLIST" ]]; then
        # launchctl bootout returns non-zero if not loaded; tolerate that.
        run launchctl bootout "gui/$(id -u)/${DAEMON_LABEL}" || \
            warn "launchctl bootout failed (daemon may not be loaded)"
        DAEMON_WAS_STOPPED=1
    elif pgrep -x "$DAEMON_PROC" >/dev/null 2>&1; then
        run pkill -x "$DAEMON_PROC" || warn "pkill failed"
        DAEMON_WAS_STOPPED=1
    else
        info "Daemon not running; nothing to stop."
    fi
else
    info "Skipping daemon stop (--no-stop)."
fi

# ─── Prepare output dir ──────────────────────────────────────────────────────
run mkdir -p "$OUTPUT_DIR"

# ─── Read key (hex format expected by daemon) ────────────────────────────────
if [[ "$DRY_RUN" -eq 0 ]]; then
    DB_KEY="$(tr -d '[:space:]' < "$KEY_PATH")"
    [[ -n "$DB_KEY" ]] || die "DB key file is empty: $KEY_PATH"
else
    DB_KEY="<redacted>"
fi

# ─── Run sqlcipher .backup ───────────────────────────────────────────────────
info "Running SQLCipher .backup..."
if [[ "$DRY_RUN" -eq 0 ]]; then
    # PRAGMA key MUST be the first statement after opening; .backup produces a
    # consistent copy encrypted with the same key.
    sqlcipher "$DB_PATH" <<SQL
PRAGMA key = "x'${DB_KEY}'";
.backup '${OUTPUT_FILE}'
.exit
SQL
    [[ -s "$OUTPUT_FILE" ]] || die "Backup file is empty or missing: $OUTPUT_FILE"
    chmod 600 "$OUTPUT_FILE"
    info "Backup OK: $OUTPUT_FILE ($(wc -c <"$OUTPUT_FILE") bytes)"
else
    echo "    \$ sqlcipher $DB_PATH (PRAGMA key + .backup '$OUTPUT_FILE')"
fi

# ─── Restart daemon (optional) ───────────────────────────────────────────────
if [[ "$NO_RESTART" -eq 0 && "$DAEMON_WAS_STOPPED" -eq 1 ]]; then
    if [[ -f "$DAEMON_PLIST" ]]; then
        info "Restarting daemon..."
        run launchctl bootstrap "gui/$(id -u)" "$DAEMON_PLIST" || \
            warn "launchctl bootstrap failed; restart manually"
    else
        info "No LaunchAgent plist installed; skipping restart."
    fi
elif [[ "$NO_RESTART" -eq 1 ]]; then
    info "Skipping daemon restart (--no-restart)."
fi

echo ""
info "Done."
