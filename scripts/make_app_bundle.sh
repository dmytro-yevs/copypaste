#!/usr/bin/env bash
# Creates CopyPaste.app bundle from cargo release build
set -e
APP_NAME="CopyPaste"
APP_DIR="dist/${APP_NAME}.app"
CONTENTS="$APP_DIR/Contents"

mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources"

# Copy binaries
cp target/release/copypaste-daemon "$CONTENTS/MacOS/"
cp target/release/copypaste-cli "$CONTENTS/MacOS/"

# Info.plist
cat > "$CONTENTS/Info.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>com.copypaste.app</string>
  <key>CFBundleName</key><string>CopyPaste</string>
  <key>CFBundleVersion</key><string>0.1.0-alpha.1</string>
  <key>CFBundleExecutable</key><string>copypaste-daemon</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST

echo "Created: $APP_DIR"
