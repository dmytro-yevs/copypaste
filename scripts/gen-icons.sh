#!/usr/bin/env bash
# gen-icons.sh — Regenerate CopyPaste icons.
#
# Strategy:
#   1. If a vector source SVG exists and ImageMagick `convert` is available,
#      rasterize from SVG (preferred — sharper, designer-controlled).
#      SVG search order:
#        a) crates/copypaste-ui/assets/tray-icon.svg  (canonical location)
#        b) assets/logo/tray.svg                      (design-assets fallback)
#   2. Otherwise, fall back to the PIL placeholder generator (scripts/gen-icons.py).
#
# Output: see scripts/gen-icons.py for the full file list.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASSETS_DIR="$REPO_ROOT/crates/copypaste-ui/assets"
ICONSET_DIR="$ASSETS_DIR/AppIcon.iconset"
# Canonical SVG location; fall back to design-assets copy if absent.
SVG_SOURCE="$ASSETS_DIR/tray-icon.svg"
if [[ ! -f "$SVG_SOURCE" && -f "$REPO_ROOT/assets/logo/tray.svg" ]]; then
  SVG_SOURCE="$REPO_ROOT/assets/logo/tray.svg"
fi

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
    echo "No SVG source found (checked assets/tray-icon.svg and assets/logo/tray.svg) — using PIL placeholder generator."
  fi
  if ! command -v convert >/dev/null 2>&1; then
    echo "ImageMagick 'convert' not installed — using PIL placeholder generator."
  fi
  python3 "$REPO_ROOT/scripts/gen-icons.py"
fi

# Build .icns from iconset on macOS (required for Tauri bundle).
# Canonical output: $ASSETS_DIR/AppIcon.icns (generated from iconset source).
# Tauri references $TAURI_ICONS_DIR/icon.icns — keep them in sync by copying.
TAURI_ICONS_DIR="$REPO_ROOT/crates/copypaste-ui/src-tauri/icons"
if command -v iconutil >/dev/null 2>&1; then
  echo "Building AppIcon.icns from iconset..."
  iconutil -c icns "$ICONSET_DIR" -o "$ASSETS_DIR/AppIcon.icns" && {
    # CopyPaste-5917.95: copy to src-tauri/icons/icon.icns (Tauri bundle path from
    # tauri.conf.json:bundle.icon) so both files are always derived from the same
    # iconset and cannot drift. The assets/ copy is the canonical generated artifact;
    # src-tauri/icons/icon.icns is a build-time consumer copy.
    mkdir -p "$TAURI_ICONS_DIR"
    cp "$ASSETS_DIR/AppIcon.icns" "$TAURI_ICONS_DIR/icon.icns"
    echo "  → copied to $TAURI_ICONS_DIR/icon.icns (Tauri bundle reference)"
  } || echo "iconutil failed — .icns not generated (non-fatal)."
else
  echo "iconutil not available — skipping .icns generation (macOS only)."
  echo "  If you have final PNGs and need an .icns, run this script on macOS."
fi

# CopyPaste-5917.100: Full platform icon generation (macOS icns + Android mipmaps).
# To regenerate ALL platform icons from the canonical SVG source, run:
#   tauri icon assets/logo/copypaste.svg
# This requires the Tauri CLI (cargo install tauri-cli) and the SVG at assets/logo/copypaste.svg.
# It writes macOS icns, iOS PNG, Android mipmap PNGs, and Windows ico — all sizes.
# gen-icons.sh handles the macOS tray icon only; tauri-icon handles the app bundle icons.

echo "Done."
