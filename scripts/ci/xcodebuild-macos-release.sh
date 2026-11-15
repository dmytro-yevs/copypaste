#!/usr/bin/env bash
# Build the macOS Release application with one fixed xcodebuild configuration.
#
# Usage: xcodebuild-macos-release.sh DERIVED_DATA_DIR PRODUCTS_DIR
#
# A new DERIVED_DATA_DIR recompiles every Xcode target. Flutter keys its
# assemble cache by PRODUCTS_DIR, and Cargo reuses the repository target
# directory, so a repeated build with the same PRODUCTS_DIR reuses the Dart
# and Rust outputs. Compilation caching and the integrated Swift driver are
# disabled so every Swift compilation runs as a traceable swift-frontend.
set -euo pipefail

derived_data="${1:?missing derived data directory}"
products="${2:?missing products directory}"
app="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../apps/copypaste_flutter" && pwd)"

cd "$app"
xcodebuild build \
    -workspace macos/Runner.xcworkspace \
    -scheme Runner \
    -configuration Release \
    -destination 'generic/platform=macOS' \
    -derivedDataPath "$derived_data" \
    -clonedSourcePackagesDirPath "$app/build/macos/SourcePackages" \
    "OBJROOT=$derived_data/Build/Intermediates.noindex" \
    "SYMROOT=$products" \
    CODE_SIGNING_REQUIRED=NO \
    CODE_SIGNING_ALLOWED=NO \
    COMPILATION_CACHE_ENABLE_CACHING=NO \
    SWIFT_ENABLE_COMPILE_CACHE=NO \
    SWIFT_USE_INTEGRATED_DRIVER=NO
