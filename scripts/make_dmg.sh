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

RW_DMG="dist/${DMG_NAME%.dmg}-rw.dmg"
hdiutil create -volname "CopyPaste" -srcfolder "$STAGING_DIR" \
  -ov -format UDRW "$RW_DMG"
hdiutil convert "$RW_DMG" -format UDZO -o "dist/${DMG_NAME}" -ov
rm -f "$RW_DMG"

echo "Created: dist/${DMG_NAME}"
