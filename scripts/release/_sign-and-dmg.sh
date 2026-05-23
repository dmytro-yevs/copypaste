#!/usr/bin/env bash
# Local ad-hoc sign + DMG build helper for beta release (worker-invoked).
# Usage: scripts/release/_sign-and-dmg.sh <version> <arch>
#   <arch> = arm64 | x86_64
set -euo pipefail

VERSION="${1:-}"
ARCH="${2:-}"
if [[ -z "$VERSION" || -z "$ARCH" ]]; then
    echo "Usage: $0 <version> <arch:arm64|x86_64>" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

APP_DIR="dist/CopyPaste.app"
ENTITLEMENTS="scripts/macos/entitlements.plist"
OUT_DMG="dist/CopyPaste-v${VERSION}-macos-${ARCH}.dmg"

if [[ ! -d "$APP_DIR" ]]; then
    echo "ERROR: $APP_DIR missing" >&2
    exit 1
fi

echo "==> Ad-hoc signing $APP_DIR"
codesign --force --deep \
    --sign - \
    --options runtime \
    --entitlements "$ENTITLEMENTS" \
    --timestamp=none \
    "$APP_DIR"

echo "==> Verifying signature"
codesign --verify --deep --strict --verbose=2 "$APP_DIR"

echo "==> Building DMG → $OUT_DMG"
rm -f "$OUT_DMG"
hdiutil create \
    -volname "CopyPaste v${VERSION}" \
    -srcfolder "$APP_DIR" \
    -ov -format UDZO \
    "$OUT_DMG"

echo "==> SHA256"
shasum -a 256 "$OUT_DMG" > "${OUT_DMG}.sha256"
cat "${OUT_DMG}.sha256"

echo
echo "Built: $OUT_DMG"
ls -lh "$OUT_DMG"
