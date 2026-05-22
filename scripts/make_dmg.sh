#!/usr/bin/env bash
set -e

VERSION="${1:-0.1.0-alpha.1}"
DMG_NAME="CopyPaste-${VERSION}.dmg"

if [[ ! -d "dist/CopyPaste.app" ]]; then
    echo "ERROR: dist/CopyPaste.app not found. Run 'make bundle' first." >&2
    exit 1
fi

mkdir -p dist
hdiutil create -volname "CopyPaste" -srcfolder "dist/CopyPaste.app" \
  -ov -format UDZO "dist/${DMG_NAME}"
echo "Created: dist/${DMG_NAME}"
