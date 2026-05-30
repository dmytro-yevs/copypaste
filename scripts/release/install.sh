#!/usr/bin/env bash
# install.sh — curl-piped end-user installer for CopyPaste on macOS.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/dmytro-yevs/copypaste/main/scripts/release/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/dmytro-yevs/copypaste/main/scripts/release/install.sh | bash -s -- 0.5.1
#
# What it does:
#   1. Downloads CopyPaste-v<version>-macos-arm64.dmg from the GitHub release.
#   2. Mounts, copies CopyPaste.app to /Applications, drops quarantine attr
#      (ad-hoc signed builds would otherwise trip Gatekeeper on first launch).
#   3. Loads ~/Library/LaunchAgents/com.copypaste.daemon.plist if present.
#
# Override the repo via COPYPASTE_REPO env for forks.
set -euo pipefail

# ---- config ----------------------------------------------------------------
REPO="${COPYPASTE_REPO:-dmytro-yevs/copypaste}"   # override via env for forks
VERSION="${1:-latest}"
APP_NAME="CopyPaste"
APP_BUNDLE="${APP_NAME}.app"
LAUNCH_AGENT="$HOME/Library/LaunchAgents/com.copypaste.daemon.plist"
# ----------------------------------------------------------------------------

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "ERROR: this installer is macOS-only (detected: $(uname -s))" >&2
    exit 1
fi

# Resolve "latest" by querying the GitHub API for the newest release tag, then
# build the canonical asset URL. The published asset name embeds the bare
# version with a single-v prefix: CopyPaste-v<version>-macos-arm64.dmg (the
# release tag carries a leading 'v'; the asset name does not double it).
if [[ "$VERSION" == "latest" ]]; then
    TAG="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
    if [[ -z "$TAG" ]]; then
        echo "ERROR: could not resolve latest release tag from GitHub API." >&2
        echo "       Pass an explicit version, e.g. bash -s -- 0.5.1" >&2
        exit 1
    fi
    VER_NO_V="${TAG#v}"
    DISPLAY_VERSION="v${VER_NO_V} (latest)"
else
    # Accept both "0.2.0-beta.1" and "v0.2.0-beta.1"
    VER_NO_V="${VERSION#v}"
    DISPLAY_VERSION="v${VER_NO_V}"
fi
ASSET_URL="https://github.com/${REPO}/releases/download/v${VER_NO_V}/${APP_NAME}-v${VER_NO_V}-macos-arm64.dmg"

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

# App-owned daemon lifecycle (ADR-014): the CopyPaste app starts/stops the
# daemon itself as a child process. We therefore do NOT bootstrap an always-on
# LaunchAgent here — that would fight the app (launchd would relaunch the daemon
# after the app quits). If a leftover agent from an older install is loaded, the
# app boots it out on launch; we also boot out any stale one now so a freshly
# installed app never races a launchd-managed daemon on the socket.
if [[ -f "$LAUNCH_AGENT" ]]; then
    echo "==> Booting out leftover launchd agent (app now owns the daemon)"
    UID_NUM="$(id -u)"
    launchctl bootout "gui/${UID_NUM}/com.copypaste.daemon" 2>/dev/null || true
    echo "    The app will start the daemon on launch. To run a headless,"
    echo "    CLI-managed daemon WITHOUT the app, see: copypaste daemon install"
else
    echo "==> Daemon is app-managed; just launch CopyPaste.app to start it."
fi

CLI_PATH="/Applications/${APP_BUNDLE}/Contents/MacOS/copypaste"
echo
echo "Installed ${APP_NAME} ${DISPLAY_VERSION}."
echo "  CLI binary: $CLI_PATH"
echo "  Try:        $CLI_PATH --help"
echo
echo "To expose 'copypaste' on your PATH, symlink it:"
echo "  sudo ln -sf '$CLI_PATH' /usr/local/bin/copypaste"
