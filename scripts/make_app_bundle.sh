#!/usr/bin/env bash
# Creates CopyPaste.app bundle from cargo release build.
#
# Usage: scripts/make_app_bundle.sh <version> [target-triple]
#   <version>       e.g. 0.2.0-beta.1
#   [target-triple] aarch64-apple-darwin (default) or x86_64-apple-darwin
#
# Reads from: target/<triple>/release/{copypaste,copypaste-daemon,copypaste-ui,copypaste-relay}
#   (falls back to target/release/ when --target was not used)
# Writes to:  dist/CopyPaste.app
#
# Bundle layout (canonical for beta):
#   Contents/MacOS/copypaste-ui       (CFBundleExecutable — what `open` runs)
#   Contents/MacOS/copypaste-daemon   (launched via launchd plist)
#   Contents/MacOS/copypaste          (CLI wrapper)
#   Contents/MacOS/copypaste-relay    (HTTP relay server)
#   Contents/Resources/AppIcon.icns
#   Contents/Resources/com.copypaste.daemon.plist  (LaunchAgent template — USERNAME substituted at install time)
#   Contents/Info.plist               (CFBundleExecutable = copypaste-ui; no LSUIElement — runtime .accessory policy)
set -euo pipefail

VERSION="${1:-0.2.0-beta.1}"
TRIPLE="${2:-}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# Resolve target binary dir: prefer per-triple build when triple given.
if [[ -n "$TRIPLE" ]]; then
    BIN_DIR="target/${TRIPLE}/release"
else
    BIN_DIR="target/release"
fi

REQUIRED_BINS=(copypaste-daemon copypaste copypaste-ui copypaste-relay)
for bin in "${REQUIRED_BINS[@]}"; do
    if [[ ! -f "$BIN_DIR/$bin" ]]; then
        echo "ERROR: $BIN_DIR/$bin not found. Run 'cargo build --release --workspace${TRIPLE:+ --target $TRIPLE}' first." >&2
        exit 1
    fi
done

APP_NAME="CopyPaste"
APP_DIR="dist/${APP_NAME}.app"
CONTENTS="$APP_DIR/Contents"

# Wipe + recreate so stale binaries from previous builds don't ride along.
rm -rf "$APP_DIR"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources"

# Binaries
for bin in "${REQUIRED_BINS[@]}"; do
    cp "$BIN_DIR/$bin" "$CONTENTS/MacOS/"
done

# Icon — generate .icns from iconset if iconutil is available.
ICONSET="crates/copypaste-ui/assets/AppIcon.iconset"
if [[ -d "$ICONSET" ]] && command -v iconutil >/dev/null 2>&1; then
    iconutil -c icns "$ICONSET" -o "$CONTENTS/Resources/AppIcon.icns"
else
    echo "warning: iconutil or iconset missing — bundle will ship without AppIcon.icns" >&2
fi

# Tray icons — bundle the menu-bar PNGs into Contents/Resources/icons/.
# The tray host (daemon on beta) looks for `tray-icon.png` next to the binary
# or under `../Resources/icons/`. Without these files the tray fell back to a
# 22×22 grey placeholder in v0.2.0-beta.1. Fixed for v0.2.0-beta.2+.
TRAY_SRC_DIR="crates/copypaste-ui/assets"
if [[ -d "$TRAY_SRC_DIR" ]] && ls "$TRAY_SRC_DIR"/tray-icon-*.png >/dev/null 2>&1; then
    mkdir -p "$CONTENTS/Resources/icons"
    cp "$TRAY_SRC_DIR"/tray-icon-*.png "$CONTENTS/Resources/icons/"
    # Canonical filename the loader probes first. Use the idle variant as the
    # default (active variant is swapped in at runtime when the daemon is busy).
    if [[ -f "$TRAY_SRC_DIR/tray-icon-idle.png" ]]; then
        cp "$TRAY_SRC_DIR/tray-icon-idle.png" "$CONTENTS/Resources/icons/tray-icon.png"
    fi
else
    echo "warning: $TRAY_SRC_DIR/tray-icon-*.png missing — tray will render a grey placeholder" >&2
fi

# LaunchAgent plist template (USERNAME placeholder substituted at install time
# by `copypaste daemon install` or `scripts/launchd/install-agent.sh`).
PLIST_SRC="packaging/macos/com.copypaste.daemon.plist"
if [[ -f "$PLIST_SRC" ]]; then
    cp "$PLIST_SRC" "$CONTENTS/Resources/com.copypaste.daemon.plist"
else
    echo "warning: $PLIST_SRC missing — autostart on first launch will fail" >&2
fi

# Info.plist — CFBundleExecutable points at the UI binary so `open CopyPaste.app`
# launches the Slint window, which then autostarts the daemon via launchd.
# NOTE: LSUIElement is intentionally absent — the app calls setActivationPolicy(.accessory)
# at runtime to start hidden. This allows runtime flipping to .regular (cmd-tab visible)
# when the user opens a window, which LSUIElement=true would permanently prevent.
cat > "$CONTENTS/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>com.copypaste.app</string>
  <key>CFBundleName</key><string>CopyPaste</string>
  <key>CFBundleDisplayName</key><string>CopyPaste</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleExecutable</key><string>copypaste-ui</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>© 2026 CopyPaste contributors</string>
</dict></plist>
PLIST

echo "Created: $APP_DIR (version: $VERSION, triple: ${TRIPLE:-host})"
ls -la "$CONTENTS/MacOS/" "$CONTENTS/Resources/"
