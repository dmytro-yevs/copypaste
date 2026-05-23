#!/usr/bin/env bash
# build-dmg-ci.sh — package release binaries into a signed .app, then a .dmg.
#
# Usage: scripts/release/build-dmg-ci.sh <version>
#
# Preconditions:
#   - `cargo build --release --workspace` has already been run.
#   - macOS host with codesign + hdiutil (i.e. a real Mac runner).
#
# Output: target/release/CopyPaste-<version>.dmg
#
# Signing: ad-hoc (`--sign -`) with hardened runtime + entitlements.
# This is good enough for self-distribution and Homebrew Cask without
# requiring an Apple Developer ID; users still need to drop quarantine
# (handled by scripts/release/install.sh).
set -euo pipefail

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
    echo "ERROR: version required. Usage: $0 <version>" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "ERROR: build-dmg-ci.sh must run on macOS (current: $(uname -s))" >&2
    exit 1
fi

BIN_CLI="target/release/copypaste"
BIN_DAEMON="target/release/copypaste-daemon"

if [[ ! -f "$BIN_CLI" ]]; then
    echo "ERROR: $BIN_CLI not found — run 'cargo build --release --workspace' first." >&2
    exit 1
fi
if [[ ! -f "$BIN_DAEMON" ]]; then
    echo "ERROR: $BIN_DAEMON not found — run 'cargo build --release --workspace' first." >&2
    exit 1
fi

APP_NAME="CopyPaste"
DIST_DIR="dist"
APP_DIR="${DIST_DIR}/${APP_NAME}.app"
ENTITLEMENTS="scripts/macos/entitlements.plist"
OUT_DMG="target/release/${APP_NAME}-${VERSION}.dmg"

if [[ ! -f "$ENTITLEMENTS" ]]; then
    echo "ERROR: entitlements file not found: $ENTITLEMENTS" >&2
    exit 1
fi

# 1) Build .app bundle. Reuse existing make_app_bundle.sh when present.
if [[ -x "scripts/make_app_bundle.sh" ]]; then
    echo "==> Building .app via scripts/make_app_bundle.sh $VERSION"
    bash scripts/make_app_bundle.sh "$VERSION"
else
    echo "==> Building minimal .app bundle in $APP_DIR"
    rm -rf "$APP_DIR"
    mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"
    cp "$BIN_DAEMON" "$APP_DIR/Contents/MacOS/"
    cp "$BIN_CLI"    "$APP_DIR/Contents/MacOS/"
    cat > "$APP_DIR/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>com.copypaste.app</string>
  <key>CFBundleName</key><string>CopyPaste</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleExecutable</key><string>copypaste-daemon</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
fi

if [[ ! -d "$APP_DIR" ]]; then
    echo "ERROR: expected .app bundle at $APP_DIR (bundle step produced nothing)" >&2
    exit 1
fi

# 2) Ad-hoc sign with hardened runtime + entitlements.
echo "==> Ad-hoc signing $APP_DIR (hardened runtime, entitlements: $ENTITLEMENTS)"
codesign --force --deep \
    --sign - \
    --options runtime \
    --entitlements "$ENTITLEMENTS" \
    --timestamp=none \
    "$APP_DIR"

# 3) Verify signature integrity (does not check notarisation).
echo "==> Verifying signature"
codesign --verify --deep --strict --verbose=2 "$APP_DIR"

# 4) Build DMG. Reuse make_dmg.sh when present (with same VERSION).
mkdir -p "$(dirname "$OUT_DMG")"
if [[ -x "scripts/make_dmg.sh" ]]; then
    echo "==> Building DMG via scripts/make_dmg.sh $VERSION"
    bash scripts/make_dmg.sh "$VERSION"
    # make_dmg.sh writes to dist/CopyPaste-<version>.dmg; relocate to target/release/.
    SRC_DMG="${DIST_DIR}/${APP_NAME}-${VERSION}.dmg"
    if [[ ! -f "$SRC_DMG" ]]; then
        echo "ERROR: expected $SRC_DMG after make_dmg.sh; not found." >&2
        exit 1
    fi
    mv -f "$SRC_DMG" "$OUT_DMG"
else
    echo "==> Building DMG via hdiutil → $OUT_DMG"
    rm -f "$OUT_DMG"
    hdiutil create \
        -volname "${APP_NAME}" \
        -srcfolder "$APP_DIR" \
        -ov -format UDZO \
        "$OUT_DMG"
fi

echo
echo "Built: $OUT_DMG"
ls -lh "$OUT_DMG"
