#!/usr/bin/env bash
# install.sh — curl-piped end-user installer for CopyPaste on macOS.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/USER/CopyPaste/main/scripts/release/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/USER/CopyPaste/main/scripts/release/install.sh | bash -s -- 0.2.0-beta.1
#
# What it does:
#   1. Downloads CopyPaste-<version>.dmg from the GitHub release.
#   2. Mounts, copies CopyPaste.app to /Applications, drops quarantine attr
#      (ad-hoc signed builds would otherwise trip Gatekeeper on first launch).
#   3. Loads ~/Library/LaunchAgents/com.copypaste.daemon.plist if present.
#
# Replace USER/CopyPaste below with the actual GitHub <owner>/<repo> once the
# repo lives at its final URL. Tracked as a TODO until W1.5 lands the cask.
set -euo pipefail

# ---- config ----------------------------------------------------------------
REPO="${COPYPASTE_REPO:-USER/CopyPaste}"   # override via env for forks
VERSION="${1:-latest}"
APP_NAME="CopyPaste"
APP_BUNDLE="${APP_NAME}.app"
LAUNCH_AGENT="$HOME/Library/LaunchAgents/com.copypaste.daemon.plist"
# ----------------------------------------------------------------------------

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "ERROR: this installer is macOS-only (detected: $(uname -s))" >&2
    exit 1
fi

# Resolve "latest" via GitHub API redirect; otherwise build the asset URL directly.
if [[ "$VERSION" == "latest" ]]; then
    ASSET_URL="https://github.com/${REPO}/releases/latest/download/${APP_NAME}.dmg"
    DISPLAY_VERSION="latest"
else
    # Accept both "0.2.0-beta.1" and "v0.2.0-beta.1"
    VER_NO_V="${VERSION#v}"
    ASSET_URL="https://github.com/${REPO}/releases/download/v${VER_NO_V}/${APP_NAME}-${VER_NO_V}.dmg"
    DISPLAY_VERSION="v${VER_NO_V}"
fi

TMP="$(mktemp -d)"
DMG="${TMP}/${APP_NAME}.dmg"
MOUNT_POINT="/Volumes/${APP_NAME}"

cleanup() {
    # Detach the volume if we left it mounted, then drop temp files.
    if [[ -d "$MOUNT_POINT" ]]; then
        hdiutil detach "$MOUNT_POINT" -quiet >/dev/null 2>&1 || true
    fi
    rm -rf "$TMP"
}
trap cleanup EXIT

echo "==> Downloading ${APP_NAME} ${DISPLAY_VERSION}"
echo "    $ASSET_URL"
if ! curl -fSL --retry 3 --retry-delay 2 "$ASSET_URL" -o "$DMG"; then
    echo "ERROR: download failed. Check version exists at:" >&2
    echo "       https://github.com/${REPO}/releases" >&2
    exit 1
fi

echo "==> Mounting DMG"
hdiutil attach "$DMG" -nobrowse -quiet

if [[ ! -d "${MOUNT_POINT}/${APP_BUNDLE}" ]]; then
    echo "ERROR: expected ${MOUNT_POINT}/${APP_BUNDLE} inside DMG; not found." >&2
    exit 1
fi

echo "==> Installing to /Applications/${APP_BUNDLE}"
# Remove old install first; -R copy preserves attributes.
rm -rf "/Applications/${APP_BUNDLE}"
cp -R "${MOUNT_POINT}/${APP_BUNDLE}" /Applications/

echo "==> Removing quarantine attribute (ad-hoc signed build)"
# -dr = recursive delete; ignore failure if the attr isn't set.
xattr -dr com.apple.quarantine "/Applications/${APP_BUNDLE}" 2>/dev/null || true

echo "==> Unmounting"
hdiutil detach "$MOUNT_POINT" -quiet
trap - EXIT
rm -rf "$TMP"

# Optional: (re)load launchd agent if user already has one configured.
# Use the modern bootout → enable → bootstrap flow. We deliberately avoid
# `launchctl unload -w` / `load -w`: the `-w` flag writes a *persistent
# disable override* that prevents the daemon from ever restarting (the v0.4
# startup bug). `enable` is idempotent and clears any pre-existing override.
if [[ -f "$LAUNCH_AGENT" ]]; then
    echo "==> (Re)loading launchd agent at $LAUNCH_AGENT"
    UID_NUM="$(id -u)"
    launchctl bootout "gui/${UID_NUM}/com.copypaste.daemon" 2>/dev/null || true
    launchctl enable "gui/${UID_NUM}/com.copypaste.daemon" 2>/dev/null || true
    launchctl bootstrap "gui/${UID_NUM}" "$LAUNCH_AGENT" 2>/dev/null || true
else
    echo "==> No launchd agent at $LAUNCH_AGENT (skipping autostart wiring)"
    echo "    To enable autostart later, run: copypaste daemon install"
fi

CLI_PATH="/Applications/${APP_BUNDLE}/Contents/MacOS/copypaste"
echo
echo "Installed ${APP_NAME} ${DISPLAY_VERSION}."
echo "  CLI binary: $CLI_PATH"
echo "  Try:        $CLI_PATH --help"
echo
echo "To expose 'copypaste' on your PATH, symlink it:"
echo "  sudo ln -sf '$CLI_PATH' /usr/local/bin/copypaste"
