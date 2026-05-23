#!/usr/bin/env bash
# uninstall.sh — complete removal of CopyPaste from macOS.
#
# Usage:
#   ./uninstall.sh [--keep-data] [--dry-run] [--force] [--help]
#
# What it does (in order):
#   1. Stops the daemon and removes the LaunchAgent
#      (delegates to ./uninstall-launchd.sh).
#   2. Detects install method:
#        - brew cask install   → `brew uninstall --cask copypaste`
#        - manual /Applications → rm -rf /Applications/CopyPaste.app
#      Also removes legacy `/usr/local/bin/copypaste{,-daemon}` symlinks
#      that the install.sh post-install hint may have created.
#   3. Removes data directories (with confirmation unless --force/--keep-data):
#        ~/Library/Application Support/CopyPaste
#        ~/Library/Caches/CopyPaste
#        ~/Library/Logs/CopyPaste
#   4. Prints a note about Keychain entries (NOT removed automatically —
#      user must delete `copypaste-master-key` from Keychain Access manually
#      if desired; see docs/release/uninstall.md).
#
# Flags:
#   --keep-data   Skip data directory removal (binaries + LaunchAgent only).
#   --dry-run     Print every command without executing.
#   --force       Skip confirmation prompts (implies "yes" to all).
#   --help        Show this help and exit.
#
# Exit codes:
#   0  Uninstall complete (idempotent; partial-removal re-runs are safe).
#   1  Unrecoverable error.
#
# Safe to re-run: every step is idempotent — already-removed paths are skipped.
set -euo pipefail

# ---- config ----------------------------------------------------------------
APP_NAME="CopyPaste"
APP_BUNDLE="/Applications/${APP_NAME}.app"
CLI_SYMLINKS=(
    "/usr/local/bin/copypaste"
    "/usr/local/bin/copypaste-daemon"
)
DATA_DIRS=(
    "$HOME/Library/Application Support/CopyPaste"
    "$HOME/Library/Caches/CopyPaste"
    "$HOME/Library/Logs/CopyPaste"
)
BREW_CASK_NAME="copypaste"
KEYCHAIN_SERVICE="copypaste-master-key"

SCRIPT_DIR="$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
LAUNCHD_SCRIPT="${SCRIPT_DIR}/uninstall-launchd.sh"
# ----------------------------------------------------------------------------

DRY_RUN=0
FORCE=0
KEEP_DATA=0

usage() {
    sed -n '2,35p' "$0" | sed 's/^# \{0,1\}//'
}

for arg in "$@"; do
    case "$arg" in
        --keep-data) KEEP_DATA=1 ;;
        --dry-run)   DRY_RUN=1 ;;
        --force)     FORCE=1 ;;
        --help|-h)   usage; exit 0 ;;
        *)
            echo "ERROR: unknown flag: $arg" >&2
            usage >&2
            exit 1
            ;;
    esac
done

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "ERROR: this uninstaller is macOS-only (detected: $(uname -s))" >&2
    exit 1
fi

# run — execute or echo depending on --dry-run.
run() {
    if [[ "$DRY_RUN" -eq 1 ]]; then
        echo "DRY-RUN: $*"
    else
        eval "$@"
    fi
}

# confirm — prompt y/N, honour --force (always yes) and --dry-run (always yes
# so the user sees what *would* happen). Returns 0 for yes, 1 for no.
confirm() {
    local prompt="$1"
    if [[ "$FORCE" -eq 1 || "$DRY_RUN" -eq 1 ]]; then
        return 0
    fi
    read -r -p "$prompt [y/N]: " reply
    case "$reply" in
        y|Y|yes|YES) return 0 ;;
        *) return 1 ;;
    esac
}

# ---- 1. LaunchAgent --------------------------------------------------------
echo "==> Step 1/4: Stopping daemon and removing LaunchAgent"
if [[ -x "$LAUNCHD_SCRIPT" ]]; then
    LAUNCHD_FLAGS=()
    [[ "$DRY_RUN" -eq 1 ]] && LAUNCHD_FLAGS+=("--dry-run")
    [[ "$FORCE"   -eq 1 ]] && LAUNCHD_FLAGS+=("--force")
    if [[ ${#LAUNCHD_FLAGS[@]} -gt 0 ]]; then
        "$LAUNCHD_SCRIPT" "${LAUNCHD_FLAGS[@]}" || {
            echo "WARN: uninstall-launchd.sh returned non-zero (continuing)" >&2
        }
    else
        "$LAUNCHD_SCRIPT" || {
            echo "WARN: uninstall-launchd.sh returned non-zero (continuing)" >&2
        }
    fi
else
    echo "WARN: ${LAUNCHD_SCRIPT} not found or not executable — falling back to inline removal" >&2
    run "launchctl bootout 'gui/$(id -u)/com.copypaste.daemon' 2>/dev/null || true"
    run "rm -f '$HOME/Library/LaunchAgents/com.copypaste.daemon.plist'"
fi

# ---- 2. Application bundle + CLI symlinks ----------------------------------
echo "==> Step 2/4: Removing application bundle"

# Detect brew install first; brew cask owns the bundle and should be the
# uninstall path when present (otherwise brew thinks the cask is still there).
USED_BREW=0
if command -v brew >/dev/null 2>&1; then
    if brew list --cask 2>/dev/null | grep -qx "$BREW_CASK_NAME"; then
        echo "    Detected Homebrew cask install: ${BREW_CASK_NAME}"
        run "brew uninstall --cask '${BREW_CASK_NAME}'"
        USED_BREW=1
    fi
fi

if [[ "$USED_BREW" -eq 0 ]]; then
    if [[ -d "$APP_BUNDLE" ]]; then
        echo "    Removing manual install at ${APP_BUNDLE}"
        run "rm -rf '${APP_BUNDLE}'"
    else
        echo "    No app bundle at ${APP_BUNDLE} (already removed)"
    fi
fi

# Manual CLI symlinks the user may have created post-install (per install.sh
# hint). Brew cask doesn't drop these, so we always check them — even after
# `brew uninstall` succeeded.
for link in "${CLI_SYMLINKS[@]}"; do
    if [[ -L "$link" || -e "$link" ]]; then
        # Only remove if it points into a CopyPaste path or no longer resolves.
        target="$(readlink "$link" 2>/dev/null || true)"
        if [[ -z "$target" ]] || [[ "$target" == *"${APP_NAME}"* ]] || [[ ! -e "$link" ]]; then
            echo "    Removing CLI symlink ${link}"
            # sudo only if not writable by current user (Homebrew prefix on
            # Apple Silicon is /opt/homebrew → user-owned; /usr/local/bin on
            # Intel may require sudo).
            if [[ -w "$(dirname "$link")" ]]; then
                run "rm -f '${link}'"
            else
                run "sudo rm -f '${link}'"
            fi
        else
            echo "    Skipping ${link} (does not point into ${APP_NAME})"
        fi
    fi
done

# ---- 3. Data directories ---------------------------------------------------
echo "==> Step 3/4: Data directories"
if [[ "$KEEP_DATA" -eq 1 ]]; then
    echo "    --keep-data set; leaving the following intact:"
    for dir in "${DATA_DIRS[@]}"; do
        echo "      $dir"
    done
else
    echo "    The following directories contain your clipboard history, settings, and logs:"
    for dir in "${DATA_DIRS[@]}"; do
        if [[ -d "$dir" ]]; then
            echo "      $dir"
        fi
    done
    if confirm "    Remove these directories?"; then
        for dir in "${DATA_DIRS[@]}"; do
            if [[ -d "$dir" ]]; then
                echo "    Removing $dir"
                run "rm -rf '${dir}'"
            fi
        done
    else
        echo "    Skipping data directory removal."
    fi
fi

# ---- 4. Keychain note ------------------------------------------------------
echo "==> Step 4/4: Keychain"
echo "    CopyPaste stores a master encryption key in your login Keychain."
echo "    For safety, this uninstaller does NOT delete it automatically."
echo
echo "    To remove it manually:"
echo "      1. Open Keychain Access.app"
echo "      2. Search for: ${KEYCHAIN_SERVICE}"
echo "      3. Delete entries owned by 'CopyPaste' / 'com.copypaste.daemon'"
echo
echo "    Or via CLI:"
echo "      security delete-generic-password -s '${KEYCHAIN_SERVICE}'"

echo
echo "==> Uninstall complete."
if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "    (--dry-run: no changes were made)"
fi
