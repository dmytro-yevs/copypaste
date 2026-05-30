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

# 2) Sign with hardened runtime + entitlements.
#
# Identity defaults to ad-hoc (`--sign -`); set MACOS_SIGN_IDENTITY to a
# Developer ID Application identity for a notarisable build. See the longer
# note in scripts/release/build-dmg-ci.sh. Inner binaries get a STABLE -i
# identifier so the identifier stops rotating across rebuilds — but under
# ad-hoc the cdhash (designated requirement) still changes every build, so the
# real fix for the recurring Keychain prompt is the non-prompting 0600 file key
# store (crates/copypaste-daemon/src/keychain/file_store.rs).
SIGN_IDENTITY="${MACOS_SIGN_IDENTITY:--}"

echo "==> Signing inner binaries with stable identifiers (identity: $SIGN_IDENTITY)"
# macOS ships bash 3.2 (no associative arrays / `declare -A`); use a case stmt.
for bin in copypaste-daemon copypaste copypaste-relay; do
    if [[ -f "$APP_DIR/Contents/MacOS/$bin" ]]; then
        case "$bin" in
            copypaste-daemon) bin_id="com.copypaste.daemon" ;;
            copypaste)        bin_id="com.copypaste.cli" ;;
            copypaste-relay)  bin_id="com.copypaste.relay" ;;
            *)                bin_id="com.copypaste.$bin" ;;
        esac
        codesign --force \
            --sign "$SIGN_IDENTITY" \
            --identifier "$bin_id" \
            --options runtime \
            --timestamp=none \
            "$APP_DIR/Contents/MacOS/$bin"
    fi
done

echo "==> Signing $APP_DIR"
codesign --force --deep \
    --sign "$SIGN_IDENTITY" \
    --options runtime \
    --entitlements "$ENTITLEMENTS" \
    --timestamp=none \
    "$APP_DIR"

echo "==> Verifying signature"
codesign --verify --deep --strict --verbose=2 "$APP_DIR"

# 3) Build DMG with /Applications symlink for drag-install.
# Two-step: stage into temp dir → create writable DMG → add symlink → convert.
echo "==> Building DMG → $OUT_DMG"
STAGING_DIR="$(mktemp -d)"
trap 'rm -rf "$STAGING_DIR"' EXIT

cp -R "$APP_DIR" "$STAGING_DIR/"
ln -s /Applications "$STAGING_DIR/Applications"

RW_DMG="${OUT_DMG%.dmg}-rw.dmg"
rm -f "$OUT_DMG" "$RW_DMG"
hdiutil create \
    -volname "CopyPaste v${VERSION}" \
    -srcfolder "$STAGING_DIR" \
    -ov -format UDRW \
    "$RW_DMG"
hdiutil convert "$RW_DMG" -format UDZO -o "$OUT_DMG" -ov
rm -f "$RW_DMG"

# 4) SHA256 alongside.
echo "==> SHA256"
shasum -a 256 "$OUT_DMG" > "${OUT_DMG}.sha256"
cat "${OUT_DMG}.sha256"

echo
echo "Built: $OUT_DMG"
ls -lh "$OUT_DMG" "${OUT_DMG}.sha256"
