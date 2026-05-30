#!/usr/bin/env bash
# check-asset-names.sh — guard that every release artefact references the DMG
# under the SAME canonical name scheme:
#
#     CopyPaste-v${VERSION}-macos-arm64.dmg
#
# (VERSION is the bare version with no leading 'v'. The release *tag* carries a
# leading 'v'; the asset name has a single 'v' prefix, not a double 'vv'.)
#
# Sources checked:
#   - Casks/copypaste.rb            url ".../CopyPaste-v#{version}-macos-arm64.dmg"
#   - scripts/release/gen-cask.sh   DMG_NAME="CopyPaste-v${VERSION}-macos-arm64.dmg"
#   - scripts/release/build-dmg-ci.sh OUT_DMG=".../${APP_NAME}-v${VERSION}-macos-${ARCH}.dmg"
#   - scripts/release/setup-tap.sh  sync.yml URL ".../CopyPaste-v${VERSION}-macos-arm64.dmg"
#   - scripts/release/install.sh    ASSET_URL ".../${APP_NAME}-v${VER_NO_V}-macos-arm64.dmg"
#
# Exits non-zero (and prints the offending file) if any source drifts from the
# scheme. Intended for CI lint and local pre-release checks.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

CASK="$REPO_ROOT/Casks/copypaste.rb"
GEN_CASK="$REPO_ROOT/scripts/release/gen-cask.sh"
BUILD_DMG="$REPO_ROOT/scripts/release/build-dmg-ci.sh"
SETUP_TAP="$REPO_ROOT/scripts/release/setup-tap.sh"
INSTALL="$REPO_ROOT/scripts/release/install.sh"

fail=0
err() { echo "FAIL: $1" >&2; fail=1; }

# Each check greps for the canonical single-v pattern. VERSION is always the
# bare version (no leading 'v'); the asset name prefixes it with exactly one
# 'v'. The release tag is separately v-prefixed in the download path.

# 1. Cask url: single-v with the #{version} interpolation.
if ! grep -Eq 'CopyPaste-v#\{version\}-macos-arm64\.dmg' "$CASK"; then
    err "$CASK url does not match CopyPaste-v#{version}-macos-arm64.dmg"
fi

# 2. gen-cask DMG_NAME: CopyPaste-v${VERSION}-macos-arm64.dmg (VERSION is bare).
if ! grep -Eq 'DMG_NAME="CopyPaste-v\$\{VERSION\}-macos-arm64\.dmg"' "$GEN_CASK"; then
    err "$GEN_CASK DMG_NAME does not match CopyPaste-v\${VERSION}-macos-arm64.dmg"
fi

# 3. build-dmg-ci OUT_DMG: ${APP_NAME}-v${VERSION}-macos-${ARCH}.dmg (VERSION is bare).
if ! grep -Eq 'OUT_DMG="\$\{DIST_DIR\}/\$\{APP_NAME\}-v\$\{VERSION\}-macos-\$\{ARCH\}\.dmg"' "$BUILD_DMG"; then
    err "$BUILD_DMG OUT_DMG does not match \${APP_NAME}-v\${VERSION}-macos-\${ARCH}.dmg"
fi

# 4. setup-tap sync.yml URL: single-v with ${VERSION}.
if ! grep -Eq 'CopyPaste-v\\\$\{VERSION\}-macos-arm64\.dmg' "$SETUP_TAP"; then
    err "$SETUP_TAP sync.yml URL does not match CopyPaste-v\${VERSION}-macos-arm64.dmg"
fi

# 5. install.sh ASSET_URL: ${APP_NAME}-v${VER_NO_V}-macos-arm64.dmg.
if ! grep -Eq '\$\{APP_NAME\}-v\$\{VER_NO_V\}-macos-arm64\.dmg' "$INSTALL"; then
    err "$INSTALL ASSET_URL does not match \${APP_NAME}-v\${VER_NO_V}-macos-arm64.dmg"
fi

if [[ $fail -ne 0 ]]; then
    echo "==> Asset-name consistency check FAILED." >&2
    echo "    All sources must use: CopyPaste-v\${VERSION}-macos-arm64.dmg" >&2
    exit 1
fi

echo "==> Asset-name consistency check PASSED (CopyPaste-v\${VERSION}-macos-arm64.dmg)"
