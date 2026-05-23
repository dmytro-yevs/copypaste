#!/usr/bin/env bash
# Local ad-hoc sign + DMG build helper for beta release (worker-invoked).
# Usage: scripts/release/_sign-and-dmg.sh <version> <arch>
#   <arch> = arm64 | x86_64
#
# Builds CopyPaste.app from target/<triple>/release/ binaries, signs ad-hoc
# with hardened runtime + entitlements, then packages into a UDZO DMG +
# .sha256 alongside.
#
# Preconditions:
#   - `cargo build --release --workspace --target <triple>` already run.
#   - macOS host with codesign + hdiutil + iconutil.
set -euo pipefail

VERSION="${1:-}"
ARCH="${2:-}"
if [[ -z "$VERSION" || -z "$ARCH" ]]; then
    echo "Usage: $0 <version> <arch:arm64|x86_64>" >&2
    exit 1
fi

case "$ARCH" in
    arm64)  TRIPLE="aarch64-apple-darwin" ;;
    x86_64) TRIPLE="x86_64-apple-darwin"  ;;
    *) echo "ERROR: arch must be arm64 or x86_64 (got: $ARCH)" >&2; exit 1 ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

APP_DIR="dist/CopyPaste.app"
ENTITLEMENTS="scripts/macos/entitlements.plist"
# All release artefacts live in dist/ only — never target/release/.
mkdir -p dist
OUT_DMG="dist/CopyPaste-v${VERSION}-macos-${ARCH}.dmg"

# 1) Build .app bundle via the canonical helper (includes UI + relay + plist + icon).
echo "==> Building $APP_DIR for $TRIPLE"
bash scripts/make_app_bundle.sh "$VERSION" "$TRIPLE"

if [[ ! -d "$APP_DIR" ]]; then
    echo "ERROR: $APP_DIR missing after make_app_bundle.sh" >&2
    exit 1
fi

# 2) Ad-hoc sign with hardened runtime + entitlements.
echo "==> Ad-hoc signing $APP_DIR"
codesign --force --deep \
    --sign - \
    --options runtime \
    --entitlements "$ENTITLEMENTS" \
    --timestamp=none \
    "$APP_DIR"

echo "==> Verifying signature"
codesign --verify --deep --strict --verbose=2 "$APP_DIR"

# 3) Build DMG.
echo "==> Building DMG → $OUT_DMG"
rm -f "$OUT_DMG"
hdiutil create \
    -volname "CopyPaste v${VERSION}" \
    -srcfolder "$APP_DIR" \
    -ov -format UDZO \
    "$OUT_DMG"

# 4) SHA256 alongside.
echo "==> SHA256"
shasum -a 256 "$OUT_DMG" > "${OUT_DMG}.sha256"
cat "${OUT_DMG}.sha256"

echo
echo "Built: $OUT_DMG"
ls -lh "$OUT_DMG" "${OUT_DMG}.sha256"
