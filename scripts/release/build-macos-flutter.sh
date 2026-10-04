#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-}"
BUILD_NUMBER="${2:-1}"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
    echo "ERROR: usage: $0 <stable-version> [build-number]" >&2
    exit 1
}
[[ "$BUILD_NUMBER" =~ ^[1-9][0-9]*$ ]] || {
    echo "ERROR: build-number must be a positive integer" >&2
    exit 1
}

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APP_ROOT="$ROOT/apps/copypaste_flutter"
SOURCE_APP="$APP_ROOT/build/macos/Build/Products/Release/CopyPaste.app"
DIST_APP="$ROOT/dist/CopyPaste.app"

[[ "$(uname -s)" == "Darwin" ]] || {
    echo "ERROR: macOS release packaging requires macOS" >&2
    exit 1
}

workspace_version="$(awk '/^\[workspace\.package\]/{inside=1; next} inside && /^version[[:space:]]*=/{gsub(/[\"[:space:]]/, "", $3); print $3; exit}' "$ROOT/Cargo.toml")"
pubspec_version="$(awk '/^version:[[:space:]]*/ {print $2; exit}' "$APP_ROOT/pubspec.yaml")"
[[ "$workspace_version" == "$VERSION" && "${pubspec_version%%+*}" == "$VERSION" ]] || {
    echo "ERROR: Cargo.toml and pubspec.yaml must both identify version $VERSION" >&2
    exit 1
}

export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
cargo build --manifest-path "$ROOT/Cargo.toml" --release --locked \
    -p copypaste-daemon -p copypaste-cli

cd "$APP_ROOT"
flutter pub get --enforce-lockfile
flutter build macos --release \
    --build-name "$VERSION" \
    --build-number "$BUILD_NUMBER"

[[ -x "$SOURCE_APP/Contents/MacOS/CopyPaste" ]] || {
    echo "ERROR: Flutter did not produce the release application" >&2
    exit 1
}
[[ -x "$SOURCE_APP/Contents/MacOS/copypaste-daemon" ]] || {
    echo "ERROR: release application does not contain copypaste-daemon" >&2
    exit 1
}
install -m 755 "$ROOT/target/release/copypaste" \
    "$SOURCE_APP/Contents/MacOS/copypaste"

install -m 755 "$ROOT/packaging/macos/selfsign.sh" \
    "$SOURCE_APP/Contents/Resources/selfsign.sh"
/usr/bin/codesign --force --sign - --timestamp=none \
    --entitlements "$APP_ROOT/macos/Runner/Daemon.entitlements" \
    "$SOURCE_APP/Contents/MacOS/copypaste-daemon"
/usr/bin/codesign --force --sign - --timestamp=none \
    "$SOURCE_APP/Contents/MacOS/copypaste"
/usr/bin/codesign --force --sign - --timestamp=none \
    --entitlements "$APP_ROOT/macos/Runner/Release.entitlements" \
    "$SOURCE_APP"
/usr/bin/codesign --verify --deep --strict --verbose=2 "$SOURCE_APP"

rm -rf "$DIST_APP"
mkdir -p "$ROOT/dist"
ditto "$SOURCE_APP" "$DIST_APP"
cd "$ROOT"
scripts/release/make-dmg.sh "$VERSION" arm64
