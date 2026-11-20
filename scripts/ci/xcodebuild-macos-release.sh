#!/usr/bin/env bash
# Build the macOS Release application with one fixed xcodebuild configuration.
#
# Usage: xcodebuild-macos-release.sh DERIVED_DATA_DIR PRODUCTS_DIR [--recompile-swift]
#
# Flutter keys its assemble cache by PRODUCTS_DIR, and Cargo reuses the
# repository target directory, so a repeated build with the same directories
# reuses every output. --recompile-swift first removes the compiled Swift
# modules from DERIVED_DATA_DIR: the repeated build then recompiles every
# Swift target and nothing else, and fails unless each module is produced
# again. Compilation caching and the integrated Swift driver are disabled so
# every Swift compilation runs as a traceable swift-frontend.
set -euo pipefail

derived_data="${1:?missing derived data directory}"
products="${2:?missing products directory}"
mode="${3:-}"
app="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../apps/copypaste_flutter" && pwd)"

swift_modules=""
if [[ "$mode" == --recompile-swift ]]; then
    swift_modules="$derived_data/recompiled-swift-modules.txt"
    find "$derived_data/Build/Intermediates.noindex" -type f -name '*.swiftmodule' > "$swift_modules"
    [[ -s "$swift_modules" ]] || {
        echo "ERROR: $derived_data holds no compiled Swift modules" >&2
        exit 1
    }
    while IFS= read -r module; do
        rm "$module"
    done < "$swift_modules"
elif [[ -n "$mode" ]]; then
    echo "ERROR: unknown option $mode" >&2
    exit 1
fi

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

if [[ -n "$swift_modules" ]]; then
    while IFS= read -r module; do
        [[ -f "$module" ]] || {
            echo "ERROR: Swift module was not recompiled: $module" >&2
            exit 1
        }
    done < "$swift_modules"
    echo "Recompiled $(wc -l < "$swift_modules" | tr -d ' ') Swift modules."
fi
