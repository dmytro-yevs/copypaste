#!/usr/bin/env bash
# Build the macOS Release application with only the Swift compiler traced.
#
# Usage: xcodebuild-macos-release.sh DERIVED_DATA_DIR PRODUCTS_DIR
#
# CodeQL traces macOS builds as x86_64 processes under Rosetta, which costs
# many times more than the same work run natively. Start this script after
# CodeQL init with an empty DYLD_INSERT_LIBRARIES: the build then runs native
# and untraced, and Xcode and SwiftPM compile every Swift module and package
# manifest through a generated wrapper that starts swiftc under the tracer.
# Compilation caching and the integrated Swift driver are disabled so each
# compilation runs as a traceable swift-frontend.
set -euo pipefail

derived_data="${1:?missing derived data directory}"
products="${2:?missing products directory}"
app="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../apps/copypaste_flutter" && pwd)"

[[ -z "${DYLD_INSERT_LIBRARIES:-}" ]] || {
    echo "ERROR: start this build outside the CodeQL tracer" >&2
    exit 1
}
unset DYLD_INSERT_LIBRARIES

# Xcode and SwiftPM start compilers with a reduced environment, so the
# wrapper carries the tracer's variables itself.
traced_swiftc="$derived_data/traced-swiftc"
mkdir -p "$derived_data"
{
    echo '#!/bin/bash'
    while IFS= read -r name; do
        printf 'export %s=%q\n' "$name" "${!name}"
    done < <(compgen -e | grep -E '^(CODEQL|SEMMLE)_')
    printf 'export DYLD_INSERT_LIBRARIES=%q\n' "${SEMMLE_PRELOAD_libtrace:?CodeQL tracing is not initialized}"
    printf 'exec %q %q "$@"\n' "${CODEQL_RUNNER:?CodeQL tracing is not initialized}" "$(xcrun --find swiftc)"
} > "$traced_swiftc"
chmod +x "$traced_swiftc"
export SWIFT_EXEC_MANIFEST="$traced_swiftc"

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
    "SWIFT_EXEC=$traced_swiftc" \
    CODE_SIGNING_REQUIRED=NO \
    CODE_SIGNING_ALLOWED=NO \
    COMPILATION_CACHE_ENABLE_CACHING=NO \
    SWIFT_ENABLE_COMPILE_CACHE=NO \
    SWIFT_USE_INTEGRATED_DRIVER=NO
