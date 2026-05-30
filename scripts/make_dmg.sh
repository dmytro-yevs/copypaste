#!/usr/bin/env bash
set -e

VERSION="${1:-0.1.0-alpha.1}"
DMG_NAME="CopyPaste-${VERSION}.dmg"

if [[ ! -d "dist/CopyPaste.app" ]]; then
    echo "ERROR: dist/CopyPaste.app not found. Run 'make bundle' first." >&2
    exit 1
fi

mkdir -p dist

# Build DMG with /Applications symlink so users can drag-install.
# Two-step: stage → create writable DMG → add symlink → convert to compressed.
STAGING_DIR="$(mktemp -d)"
trap 'rm -rf "$STAGING_DIR"' EXIT

cp -R "dist/CopyPaste.app" "$STAGING_DIR/"
# Strip quarantine from the staged bundle so files inside the DMG are
# clean.  When a user drags the .app from the mounted DMG to /Applications,
# Finder copies file-level xattrs; if com.apple.quarantine is present on the
# source files, the destination copy is also quarantined and Gatekeeper will
# block the ad-hoc-signed binaries on launch.  Stripping here ensures the
# files baked into the UDZO image carry no quarantine xattr.
xattr -cr "$STAGING_DIR/CopyPaste.app" 2>/dev/null || true
ln -s /Applications "$STAGING_DIR/Applications"

# Install instructions baked into the DMG so users see them before dragging.
# Addresses two failure modes: (1) dragging while CopyPaste is still running
# (macOS locks the bundle; copy silently fails or is partial), and (2) no
# visible guidance for ad-hoc-signed first launch.
cat > "$STAGING_DIR/READ ME — Install.txt" <<'INSTALL_README'
BEFORE INSTALLING
-----------------
If CopyPaste is already running, QUIT it fully first (click the menu-bar /
tray icon → Quit). macOS cannot replace a running app and the copy will
fail silently or produce a broken installation.

HOW TO INSTALL
--------------
1. Drag CopyPaste.app onto the Applications folder in this window.
2. First launch: right-click CopyPaste.app → Open (this build is ad-hoc
   signed; macOS requires the explicit Open step the very first time).

IF SOMETHING WENT WRONG
------------------------
If you see "Installation incomplete" or "could not locate bundled
copypaste-daemon" after launching:
  • Delete /Applications/CopyPaste.app
  • Make sure CopyPaste is fully quit, then drag it again from this DMG.
INSTALL_README

RW_DMG="dist/${DMG_NAME%.dmg}-rw.dmg"
hdiutil create -volname "CopyPaste" -srcfolder "$STAGING_DIR" \
  -ov -format UDRW "$RW_DMG"

# Best-effort Finder window styling (icon layout + window bounds).
# Skipped automatically in headless CI (CI env var set by GitHub Actions /
# most CI systems).  A failure here never aborts the build.
if [[ -z "${CI:-}" ]]; then
  MOUNT_POINT="$(mktemp -d)"
  hdiutil attach "$RW_DMG" -mountpoint "$MOUNT_POINT" -nobrowse -quiet || true
  if [[ -d "$MOUNT_POINT/CopyPaste.app" ]]; then
    osascript - "$MOUNT_POINT" <<'APPLESCRIPT' || true
on run argv
  set dmgPath to item 1 of argv
  tell application "Finder"
    tell disk (do shell script "basename " & quoted form of dmgPath)
      open
      set current view of container window to icon view
      set toolbar visible of container window to false
      set statusbar visible of container window to false
      set the bounds of container window to {200, 120, 740, 480}
      set theViewOptions to the icon view options of container window
      set arrangement of theViewOptions to not arranged
      set icon size of theViewOptions to 96
      set position of item "CopyPaste.app" of container window to {140, 180}
      set position of item "Applications" of container window to {400, 180}
      close
      open
      update without registering applications
    end tell
  end tell
end run
APPLESCRIPT
    hdiutil detach "$MOUNT_POINT" -quiet || true
  else
    hdiutil detach "$MOUNT_POINT" -quiet 2>/dev/null || true
  fi
  rmdir "$MOUNT_POINT" 2>/dev/null || true
fi

hdiutil convert "$RW_DMG" -format UDZO -o "dist/${DMG_NAME}" -ov
rm -f "$RW_DMG"

echo "Created: dist/${DMG_NAME}"
