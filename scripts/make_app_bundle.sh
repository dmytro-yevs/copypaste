#!/usr/bin/env bash
# Creates CopyPaste.app bundle from cargo release build
set -e

VERSION="${1:-0.1.0-alpha.1}"
APP_NAME="CopyPaste"
APP_DIR="dist/${APP_NAME}.app"
CONTENTS="$APP_DIR/Contents"

# Verify binaries exist before proceeding
if [[ ! -f "target/release/copypaste-daemon" ]]; then
    echo "ERROR: target/release/copypaste-daemon not found. Run 'cargo build --release' first." >&2
    exit 1
fi
if [[ ! -f "target/release/copypaste" ]]; then
    echo "ERROR: target/release/copypaste not found. Run 'cargo build --release' first." >&2
    exit 1
fi

mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources"

# Copy binaries
# copypaste-daemon crate: [[bin]] name = "copypaste-daemon"
cp target/release/copypaste-daemon "$CONTENTS/MacOS/"
# copypaste-cli crate: [[bin]] name = "copypaste" (not "copypaste-cli")
cp target/release/copypaste "$CONTENTS/MacOS/"

# Info.plist
cat > "$CONTENTS/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>com.copypaste.app</string>
  <key>CFBundleName</key><string>CopyPaste</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>CFBundleExecutable</key><string>copypaste-daemon</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST

echo "Created: $APP_DIR"
