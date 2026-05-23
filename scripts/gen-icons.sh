#!/usr/bin/env bash
# gen-icons.sh — Regenerate CopyPaste icons.
#
# Strategy:
#   1. If a vector source SVG exists and ImageMagick `convert` is available,
#      rasterize from SVG (preferred — sharper, designer-controlled).
#   2. Otherwise, fall back to the PIL placeholder generator (scripts/gen-icons.py).
#
# Source SVG (optional): crates/copypaste-ui/assets/tray-icon.svg
# Output: see scripts/gen-icons.py for the full file list.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASSETS_DIR="$REPO_ROOT/crates/copypaste-ui/assets"
ICONSET_DIR="$ASSETS_DIR/AppIcon.iconset"
SVG_SOURCE="$ASSETS_DIR/tray-icon.svg"

mkdir -p "$ASSETS_DIR" "$ICONSET_DIR"

if [[ -f "$SVG_SOURCE" ]] && command -v convert >/dev/null 2>&1; then
  echo "Found SVG source + ImageMagick — rasterizing from $SVG_SOURCE"
  for sz in 16 32 128 256 512; do
    convert -background none -resize "${sz}x${sz}" "$SVG_SOURCE" \
      "$ICONSET_DIR/icon_${sz}x${sz}.png"
  done
  convert -background none -resize 16x16 "$SVG_SOURCE" "$ASSETS_DIR/tray-icon-16.png"
  convert -background none -resize 32x32 "$SVG_SOURCE" "$ASSETS_DIR/tray-icon-32.png"
  convert -background none -resize 32x32 "$SVG_SOURCE" "$ASSETS_DIR/tray-icon-active.png"
  convert -background none -resize 32x32 -colorspace Gray "$SVG_SOURCE" \
    "$ASSETS_DIR/tray-icon-idle.png"
  echo "SVG rasterization complete."
else
  if [[ ! -f "$SVG_SOURCE" ]]; then
    echo "No SVG source at $SVG_SOURCE — using PIL placeholder generator."
  fi
  if ! command -v convert >/dev/null 2>&1; then
    echo "ImageMagick 'convert' not installed — using PIL placeholder generator."
  fi
  python3 "$REPO_ROOT/scripts/gen-icons.py"
fi

# Optional: build .icns from iconset on macOS
if command -v iconutil >/dev/null 2>&1; then
  echo "Building AppIcon.icns from iconset..."
  iconutil -c icns "$ICONSET_DIR" -o "$ASSETS_DIR/AppIcon.icns" || \
    echo "iconutil failed — .icns not generated (non-fatal)."
fi

echo "Done."
