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
# Normalize to a BARE version (strip any leading 'v'). The CI caller passes the
# git tag (e.g. v0.5.1) while manual callers may pass a bare version; the DMG
# filename template below always re-adds a single 'v' prefix. Stripping here
# guarantees the canonical single-'v' name CopyPaste-v<version>-... and avoids
# the historical double-'v' (CopyPaste-vv0.5.1-...) bug.
VERSION="${VERSION#v}"

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

# Verify the Tauri bundle is well-formed: Info.plist must exist and the
# CFBundleExecutable binary must be present inside the bundle.
# A missing or misnamed executable is the leading cause of
# "App source '/Applications/CopyPaste.app' is not there" failures during
# brew upgrade — Homebrew mounts the DMG, finds the .app, copies it to
# /Applications, but the OS refuses to open it, postflight fails, Homebrew
# rolls back, and the purge step can't find the (already-removed) app.
TAURI_INFO_PLIST="${TAURI_APP}/Contents/Info.plist"
if [[ ! -f "$TAURI_INFO_PLIST" ]]; then
    echo "ERROR: $TAURI_INFO_PLIST missing — Tauri bundle is malformed." >&2
    exit 1
fi
# Extract CFBundleExecutable using PlistBuddy (always available on macOS).
BUNDLE_EXECUTABLE="$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$TAURI_INFO_PLIST" 2>/dev/null || true)"
if [[ -z "$BUNDLE_EXECUTABLE" ]]; then
    echo "ERROR: CFBundleExecutable not set in $TAURI_INFO_PLIST." >&2
    exit 1
fi
BUNDLE_EXECUTABLE_PATH="${TAURI_APP}/Contents/MacOS/${BUNDLE_EXECUTABLE}"
if [[ ! -f "$BUNDLE_EXECUTABLE_PATH" ]]; then
    echo "ERROR: CFBundleExecutable '$BUNDLE_EXECUTABLE' not found at $BUNDLE_EXECUTABLE_PATH." >&2
    echo "       Tauri bundle is incomplete — re-run 'pnpm tauri build'." >&2
    exit 1
fi
echo "==> Preflight: Tauri bundle OK (CFBundleExecutable=$BUNDLE_EXECUTABLE)"

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

# Post-staging verification: confirm the assembled bundle is complete before
# signing. This catches e.g. a Tauri build that produced a bundle with a
# different CFBundleExecutable name than 'CopyPaste'.
DIST_INFO_PLIST="${APP_DIR}/Contents/Info.plist"
DIST_BUNDLE_EXECUTABLE="$(/usr/libexec/PlistBuddy -c "Print :CFBundleExecutable" "$DIST_INFO_PLIST" 2>/dev/null || true)"
if [[ ! -f "${APP_DIR}/Contents/MacOS/${DIST_BUNDLE_EXECUTABLE}" ]]; then
    echo "ERROR: CFBundleExecutable '$DIST_BUNDLE_EXECUTABLE' not found in staged bundle $APP_DIR." >&2
    exit 1
fi
# Verify the three sibling binaries were injected successfully.
for sibling in copypaste copypaste-daemon copypaste-relay; do
    if [[ ! -f "${APP_DIR}/Contents/MacOS/${sibling}" ]]; then
        echo "ERROR: sibling binary '${sibling}' missing from $APP_DIR/Contents/MacOS/" >&2
        exit 1
    fi
done
echo "==> Post-staging: bundle OK (executable=$DIST_BUNDLE_EXECUTABLE, siblings: cli/daemon/relay)"

# 2) Sign with hardened runtime + entitlements.
#
# Signing identity: defaults to ad-hoc (`--sign -`). Set MACOS_SIGN_IDENTITY to
# a Developer ID Application identity (e.g. "Developer ID Application: Name
# (TEAMID)") to produce a properly-signed, notarisable build. A Developer-ID
# signature has a STABLE designated requirement (real Team Identifier), so its
# Keychain ACL survives app updates and the daemon prefers the Keychain key
# store. The default ad-hoc build instead uses the non-prompting 0600 file key
# store (see crates/copypaste-daemon/src/keychain/file_store.rs).
SIGN_IDENTITY="${MACOS_SIGN_IDENTITY:--}"

# Inner Mach-O binaries get an EXPLICIT, STABLE -i identifier each. Without
# this, ad-hoc signing derives the identifier from the binary name PLUS a hash
# (e.g. `copypaste-daemon-<hash>`), and that identifier changes on every
# rebuild. Pinning the identifier stops the identifier from rotating.
#
# NOTE: under ad-hoc signing the cdhash STILL changes on every rebuild, so the
# *designated requirement* (`cdhash H"…"`) — and therefore any cdhash-pinned
# Keychain ACL — still rotates. The stable identifier alone does NOT stop the
# recurring Keychain password prompt; the real remedy is the non-prompting
# file key store referenced above. The stable identifier is kept because it
# makes the launchd label / item attributes deterministic and is required if a
# Developer ID identity is later supplied.
echo "==> Signing inner binaries with stable identifiers (identity: $SIGN_IDENTITY)"
# NOTE: macOS ships bash 3.2, which has NO associative arrays (`declare -A` is
# bash 4+). Use a case statement so this runs on the stock macOS interpreter as
# well as the Linux CI runner — `declare -A` here fails with "unbound variable".
for bin in copypaste-daemon copypaste copypaste-relay; do
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
done

# Sign the bundle itself last (--deep re-seals nested code already signed above).
echo "==> Signing $APP_DIR (hardened runtime, entitlements: $ENTITLEMENTS)"
codesign --force --deep \
    --sign "$SIGN_IDENTITY" \
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
