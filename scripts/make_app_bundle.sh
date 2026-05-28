#!/usr/bin/env bash
# Creates dist/CopyPaste.app by copying the Tauri-produced bundle and injecting
# sibling binaries (daemon, CLI, relay) into it.
#
# Usage: scripts/make_app_bundle.sh <version> [target-triple]
#   <version>       e.g. 0.4.1
#   [target-triple] aarch64-apple-darwin (default) or x86_64-apple-darwin
#
# Preconditions:
#   - `cd crates/copypaste-ui && pnpm install && pnpm tauri build` already ran.
#     Tauri writes its .app to crates/copypaste-ui/src-tauri/target/release/bundle/macos/
#   - `cargo build --release -p copypaste-cli -p copypaste-daemon -p copypaste-relay`
#     already ran (or supply cross-compiled bins via target-triple).
#
# Bundle layout (after this script):
#   Contents/MacOS/CopyPaste          (Tauri shell — CFBundleExecutable; what `open` runs)
#   Contents/MacOS/copypaste-daemon   (launched via launchd plist)
#   Contents/MacOS/copypaste          (CLI wrapper)
#   Contents/MacOS/copypaste-relay    (HTTP relay server)
#   Contents/Resources/               (icons, launchd plist — populated by Tauri bundler)
#   Contents/Info.plist               (written by Tauri bundler; CFBundleExecutable = CopyPaste)
set -euo pipefail

VERSION="${1:-0.4.1}"
TRIPLE="${2:-}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# Tauri bundle output path (relative to repo root).
TAURI_BUNDLE_DIR="crates/copypaste-ui/src-tauri/target/release/bundle/macos"
TAURI_APP="${TAURI_BUNDLE_DIR}/CopyPaste.app"

if [[ ! -d "$TAURI_APP" ]]; then
    echo "ERROR: Tauri bundle not found at $TAURI_APP." >&2
    echo "       Run: cd crates/copypaste-ui && pnpm install && pnpm tauri build" >&2
    exit 1
fi

# Resolve sibling binary dir: prefer per-triple build when triple given.
if [[ -n "$TRIPLE" ]]; then
    BIN_DIR="target/${TRIPLE}/release"
else
    BIN_DIR="target/release"
fi

SIBLING_BINS=(copypaste-daemon copypaste copypaste-relay)
for bin in "${SIBLING_BINS[@]}"; do
    if [[ ! -f "$BIN_DIR/$bin" ]]; then
        echo "ERROR: $BIN_DIR/$bin not found. Run 'cargo build --release${TRIPLE:+ --target $TRIPLE} -p copypaste-daemon -p copypaste-cli -p copypaste-relay' first." >&2
        exit 1
    fi
done

APP_NAME="CopyPaste"
APP_DIR="dist/${APP_NAME}.app"
CONTENTS="$APP_DIR/Contents"

# Copy the Tauri-produced bundle into dist/, wipe any stale copy first.
rm -rf "$APP_DIR"
echo "==> Copying Tauri bundle from $TAURI_APP"
cp -R "$TAURI_APP" "$APP_DIR"

# Inject sibling binaries into the bundle.
echo "==> Injecting sibling binaries into $CONTENTS/MacOS/"
for bin in "${SIBLING_BINS[@]}"; do
    cp "$BIN_DIR/$bin" "$CONTENTS/MacOS/"
done

# LaunchAgent plist template (USERNAME placeholder substituted at install time
# by `copypaste daemon install` or `scripts/launchd/install-agent.sh`).
PLIST_SRC="packaging/macos/com.copypaste.daemon.plist"
if [[ -f "$PLIST_SRC" ]]; then
    cp "$PLIST_SRC" "$CONTENTS/Resources/com.copypaste.daemon.plist"
else
    echo "warning: $PLIST_SRC missing — autostart on first launch will fail" >&2
fi

echo "Created: $APP_DIR (version: $VERSION, triple: ${TRIPLE:-host})"
ls -la "$CONTENTS/MacOS/" "$CONTENTS/Resources/"
