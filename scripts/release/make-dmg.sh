#!/usr/bin/env bash
# make-dmg.sh — package dist/CopyPaste.app into a compressed DMG.
#
#   Usage: scripts/release/make-dmg.sh <version> [arch]
#
# Output: dist/CopyPaste-v<version>-macos-<arch>.dmg
#         dist/CopyPaste-v<version>-macos-<arch>.dmg.sha256
#
# macOS only (hdiutil). Never executed on a Mac by its author.
#
# Why hdiutil by hand rather than a packaging crate or Tauri's own DMG bundler:
# Tauri's bundler is where tauri-apps/tauri#13804 fails intermittently under an
# ad-hoc identity, and we need the extra step below (stripping xattrs from the
# staged copy) that no bundler exposes. This is roughly twenty lines of
# hdiutil, is the documented Apple path, and does not pull a dependency in to
# call the same two commands.
set -euo pipefail

VERSION="${1:-}"
[[ -n "$VERSION" ]] || { echo "ERROR: version required. Usage: $0 <version> [arch]" >&2; exit 1; }
VERSION="${VERSION#v}"

ARCH="${2:-}"
if [[ -z "$ARCH" ]]; then
    case "$(uname -m)" in
        arm64)  ARCH="arm64"  ;;
        x86_64) ARCH="x86_64" ;;
        *) echo "ERROR: cannot infer arch from $(uname -m)" >&2; exit 1 ;;
    esac
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

[[ "$(uname -s)" == "Darwin" ]] || { echo "ERROR: must run on macOS" >&2; exit 1; }

APP_NAME="CopyPaste"
DIST="dist"
APP="${DIST}/${APP_NAME}.app"
OUT="${DIST}/${APP_NAME}-v${VERSION}-macos-${ARCH}.dmg"

[[ -d "$APP" ]] || {
    echo "ERROR: $APP not found — run scripts/release/build-macos-app.sh first." >&2
    exit 1
}

STAGE="$(mktemp -d)"
RW="${OUT%.dmg}-rw.dmg"
cleanup() { rm -rf "$STAGE" "$RW"; }
trap cleanup EXIT

echo "==> Staging disk image contents"
cp -R "$APP" "$STAGE/"

# Strip every extended attribute from the copy that goes *into* the image.
#
# This is not the same thing as the cask's postflight and does not replace it.
# This one stops attributes picked up during the build from being baked into
# the image, so a user who mounts the DMG and drags the app across gets a clean
# bundle. The cask's postflight handles the attribute Homebrew itself applies
# to the download, which happens later and cannot be pre-empted from here.
#
# Order matters: xattr -cr can invalidate a signature that seals resource
# attributes, so verify afterwards rather than assuming.
xattr -cr "${STAGE}/${APP_NAME}.app" 2>/dev/null || true
codesign --verify --strict "${STAGE}/${APP_NAME}.app" || {
    echo "ERROR: stripping xattrs invalidated the signature." >&2
    echo "       Re-sign after the strip instead: codesign --force --sign - <app>" >&2
    exit 1
}

# The /Applications symlink is what makes the drag-to-install window work for
# anyone who downloads the DMG directly instead of using the tap.
ln -s /Applications "$STAGE/Applications"

echo "==> Creating image"
rm -f "$OUT" "$RW"
# Two steps because UDZO (compressed) cannot be created directly from a folder
# in a way that is reliable across macOS versions: create read/write, convert.
hdiutil create \
    -volname "${APP_NAME} ${VERSION}" \
    -srcfolder "$STAGE" \
    -fs HFS+ \
    -format UDRW -ov \
    "$RW"
hdiutil convert "$RW" -format UDZO -imagekey zlib-level=9 -o "$OUT" -ov

echo "==> Checksum"
# Bare `shasum -a 256 <file>` embeds the full path, which then appears in the
# published .sha256 and discloses the runner layout. cd into the directory so
# the recorded name is just the filename. (CLAUDE.md rule 4: no paths in
# anything a user sees.)
( cd "$DIST" && shasum -a 256 "$(basename "$OUT")" > "$(basename "$OUT").sha256" )
cat "${OUT}.sha256"

echo
ls -lh "$OUT"
