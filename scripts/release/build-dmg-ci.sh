#!/usr/bin/env bash
# build-dmg-ci.sh — copy Tauri's .app bundle + sibling binaries into dist/, then package a .dmg.
#
# Usage: scripts/release/build-dmg-ci.sh <version> [arch]
#   <arch> defaults to host arch (arm64 on Apple Silicon, x86_64 on Intel).
#
# Preconditions:
#   - `cargo build --release -p copypaste-cli -p copypaste-daemon -p copypaste-relay` done.
#   - `cd crates/copypaste-ui && pnpm install && pnpm tauri build` done.
#     src-tauri is a workspace member, so Tauri writes its bundle to the
#     WORKSPACE-ROOT target/release/bundle/macos/ (not crate-local).
#   - macOS host with codesign + hdiutil (i.e. a real Mac runner).
#
# Output: dist/CopyPaste-v<version>-macos-<arch>.dmg + .sha256
#         (All release artefacts live in dist/ only — never target/release/.)
#
# Signing: ad-hoc (`--sign -`) with hardened runtime + entitlements.
# This is good enough for self-distribution and Homebrew Cask without
# requiring an Apple Developer ID; users still need to drop quarantine
# (handled by scripts/release/install.sh).
set -euo pipefail

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
    echo "ERROR: version required. Usage: $0 <version> [arch:arm64|x86_64]" >&2
    exit 1
fi

ARCH="${2:-}"
if [[ -z "$ARCH" ]]; then
    case "$(uname -m)" in
        arm64)  ARCH="arm64"  ;;
        x86_64) ARCH="x86_64" ;;
        *)      echo "ERROR: cannot infer arch from $(uname -m); pass arch explicitly" >&2; exit 1 ;;
    esac
fi
case "$ARCH" in
    arm64|x86_64) ;;
    *) echo "ERROR: arch must be arm64 or x86_64 (got: $ARCH)" >&2; exit 1 ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "ERROR: build-dmg-ci.sh must run on macOS (current: $(uname -s))" >&2
    exit 1
fi

# Tauri bundle output — `pnpm tauri build` writes the .app here.
# crates/copypaste-ui/src-tauri is a workspace member, so Cargo (and the Tauri
# bundler) emit into the WORKSPACE-ROOT target/, not a crate-local target/.
TAURI_BUNDLE_DIR="target/release/bundle/macos"
TAURI_APP="${TAURI_BUNDLE_DIR}/CopyPaste.app"

BIN_CLI="target/release/copypaste"
BIN_DAEMON="target/release/copypaste-daemon"
BIN_RELAY="target/release/copypaste-relay"

if [[ ! -d "$TAURI_APP" ]]; then
    echo "ERROR: $TAURI_APP not found — run 'cd crates/copypaste-ui && pnpm install && pnpm tauri build' first." >&2
    exit 1
fi
if [[ ! -f "$BIN_CLI" ]]; then
    echo "ERROR: $BIN_CLI not found — run 'cargo build --release -p copypaste-cli' first." >&2
    exit 1
fi
if [[ ! -f "$BIN_DAEMON" ]]; then
    echo "ERROR: $BIN_DAEMON not found — run 'cargo build --release -p copypaste-daemon' first." >&2
    exit 1
fi
if [[ ! -f "$BIN_RELAY" ]]; then
    echo "ERROR: $BIN_RELAY not found — run 'cargo build --release -p copypaste-relay' first." >&2
    exit 1
fi

APP_NAME="CopyPaste"
DIST_DIR="dist"
APP_DIR="${DIST_DIR}/${APP_NAME}.app"
ENTITLEMENTS="scripts/macos/entitlements.plist"
mkdir -p "$DIST_DIR"
OUT_DMG="${DIST_DIR}/${APP_NAME}-v${VERSION}-macos-${ARCH}.dmg"

if [[ ! -f "$ENTITLEMENTS" ]]; then
    echo "ERROR: entitlements file not found: $ENTITLEMENTS" >&2
    exit 1
fi

# 1) Copy Tauri-produced .app into dist/ and inject sibling binaries.
#    Tauri's bundler builds copypaste-ui (the Tauri shell). The daemon, CLI, and
#    relay are built separately above and placed in Contents/MacOS/ so the app
#    can launch them at runtime (daemon via launchd; relay as a sidecar).
echo "==> Staging Tauri .app bundle from $TAURI_APP"
rm -rf "$APP_DIR"
cp -R "$TAURI_APP" "$APP_DIR"

echo "==> Injecting sibling binaries into $APP_DIR/Contents/MacOS/"
cp "$BIN_CLI"    "$APP_DIR/Contents/MacOS/"
cp "$BIN_DAEMON" "$APP_DIR/Contents/MacOS/"
cp "$BIN_RELAY"  "$APP_DIR/Contents/MacOS/"

# LaunchAgent plist template: the Tauri setup code reads this from
# Contents/Resources/com.copypaste.daemon.plist on first launch and
# installs it into ~/Library/LaunchAgents/ (substituting USERNAME and
# the /Applications path). Without this file the daemon never starts.
LAUNCHD_PLIST_SRC="packaging/macos/com.copypaste.daemon.plist"
if [[ -f "$LAUNCHD_PLIST_SRC" ]]; then
    cp "$LAUNCHD_PLIST_SRC" "$APP_DIR/Contents/Resources/com.copypaste.daemon.plist"
else
    echo "ERROR: $LAUNCHD_PLIST_SRC missing — daemon will not start on first launch" >&2
    exit 1
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
if [[ -f "scripts/make_dmg.sh" ]]; then
    echo "==> Building DMG via scripts/make_dmg.sh $VERSION"
    bash scripts/make_dmg.sh "$VERSION"
    # make_dmg.sh writes to dist/CopyPaste-<version>.dmg; rename to canonical form.
    SRC_DMG="${DIST_DIR}/${APP_NAME}-${VERSION}.dmg"
    if [[ ! -f "$SRC_DMG" ]]; then
        echo "ERROR: expected $SRC_DMG after make_dmg.sh; not found." >&2
        exit 1
    fi
    mv -f "$SRC_DMG" "$OUT_DMG"
else
    echo "==> Building DMG via hdiutil → $OUT_DMG"
    # Two-step: stage with /Applications symlink → create writable → convert compressed.
    STAGING_DIR="$(mktemp -d)"
    trap 'rm -rf "$STAGING_DIR"' EXIT
    cp -R "$APP_DIR" "$STAGING_DIR/"
    # Strip quarantine so files baked into the DMG image carry no xattr.
    # Without this, Finder propagates com.apple.quarantine to the /Applications
    # copy on drag-install, causing Gatekeeper to block ad-hoc-signed binaries.
    xattr -cr "$STAGING_DIR/CopyPaste.app" 2>/dev/null || true
    ln -s /Applications "$STAGING_DIR/Applications"
    RW_DMG="${OUT_DMG%.dmg}-rw.dmg"
    rm -f "$OUT_DMG" "$RW_DMG"
    hdiutil create \
        -volname "${APP_NAME} v${VERSION}" \
        -srcfolder "$STAGING_DIR" \
        -ov -format UDRW \
        "$RW_DMG"
    hdiutil convert "$RW_DMG" -format UDZO -o "$OUT_DMG" -ov
    rm -f "$RW_DMG"
fi

# 5) SHA256 alongside.
echo "==> SHA256"
shasum -a 256 "$OUT_DMG" > "${OUT_DMG}.sha256"
cat "${OUT_DMG}.sha256"

echo
echo "Built: $OUT_DMG"
ls -lh "$OUT_DMG" "${OUT_DMG}.sha256"
