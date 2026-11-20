#!/usr/bin/env bash
# TEMPORARY: checks that a native, untraced build can trace swiftc alone.
set -uo pipefail

dev="$(xcode-select -p)"
toolchain="$dev/Toolchains/XcodeDefault.xctoolchain/usr/bin"
sdk="$(xcrun --sdk macosx --show-sdk-path)"
lib="$SEMMLE_PRELOAD_libtrace"
work="$RUNNER_TEMP/tracer-probe"
mkdir -p "$work"
cd "$work" || exit 1

echo "== step shell"
echo "translated=$(sysctl -n sysctl.proc_translated) machine=$(uname -m)"
unset DYLD_INSERT_LIBRARIES

report() {
    echo "-- extracted so far:"
    find "$CODEQL_EXTRACTOR_SWIFT_SOURCE_ARCHIVE_DIR" -type f -name '*.swift' -print 2>/dev/null | sed 's#.*/tracer-probe/##'
    echo "-- extractor invocations: $(grep -c 'invocation: .*extractor' "$CODEQL_TRACER_LOG" 2>/dev/null)"
}

for name in native_runner x86_runner no_runner; do
    printf 'func probe_%s() -> Int { return 1 }\n' "$name" > "probe_$name.swift"
done
echo "== swiftc through the runner, native start"
time DYLD_INSERT_LIBRARIES="$lib" "$CODEQL_RUNNER" "$toolchain/swiftc" -sdk "$sdk" -c probe_native_runner.swift -o native_runner.o
echo "exit=$?"
report
echo "== swiftc through the runner, x86_64 start"
time arch -x86_64 -e DYLD_INSERT_LIBRARIES="$lib" "$CODEQL_RUNNER" "$toolchain/swiftc" -sdk "$sdk" -c probe_x86_runner.swift -o x86_runner.o
echo "exit=$?"
report
echo "== swiftc without the runner"
time DYLD_INSERT_LIBRARIES="$lib" "$toolchain/swiftc" -sdk "$sdk" -c probe_no_runner.swift -o no_runner.o
echo "exit=$?"
report
for object in *.o; do
    printf '%s: ' "$object"
    lipo -archs "$object"
done

build_package() {
    local label="$1" launcher="$2"
    local package="$work/package_$label" wrapper="$work/swiftc_$label"
    mkdir -p "$package/Sources/Probe_$label"
    cat > "$package/Package.swift" <<SWIFT
// swift-tools-version: 5.9
import PackageDescription
let package = Package(
  name: "Probe_$label",
  products: [.library(name: "Probe_$label", targets: ["Probe_$label"])],
  targets: [.target(name: "Probe_$label")]
)
SWIFT
    printf 'public func probePackage() -> Int { return 1 }\n' > "$package/Sources/Probe_$label/Module_$label.swift"
    {
        echo '#!/bin/bash'
        printf 'echo "$(date +%%T) $(uname -m) $(printf "%%s " "$@" | cut -c1-110)" >> %q\n' "$work/calls_$label.log"
        while IFS= read -r name; do
            printf 'export %s=%q\n' "$name" "${!name}"
        done < <(compgen -e | grep -E '^(CODEQL|SEMMLE)_')
        printf '%s %q "$@"\n' "$launcher" "$toolchain/swiftc"
    } > "$wrapper"
    chmod +x "$wrapper"
    echo "== package build, $label"
    (
        cd "$package" || exit 1
        export SWIFT_EXEC_MANIFEST="$wrapper"
        time xcodebuild build -scheme "Probe_$label" -configuration Release \
            -destination 'generic/platform=macOS' -derivedDataPath "$work/derived_$label" \
            "SWIFT_EXEC=$wrapper" \
            CODE_SIGNING_ALLOWED=NO COMPILATION_CACHE_ENABLE_CACHING=NO \
            SWIFT_ENABLE_COMPILE_CACHE=NO SWIFT_USE_INTEGRATED_DRIVER=NO > "$work/build_$label.log" 2>&1
        echo "exit=$?"
    )
    grep -E 'BUILD SUCCEEDED|BUILD FAILED|error:' "$work/build_$label.log" | sort | uniq -c
    grep -E '^(CompileSwiftSources|CompileSwift |Ld |Resolve)' "$work/build_$label.log" | cut -c1-150
    echo "-- compiler calls:"
    cat "$work/calls_$label.log"
    report
}

build_package native "exec env DYLD_INSERT_LIBRARIES=$(printf %q "$lib") $(printf %q "$CODEQL_RUNNER")"
build_package x86 "exec arch -x86_64 -e DYLD_INSERT_LIBRARIES=$(printf %q "$lib") $(printf %q "$CODEQL_RUNNER")"

echo "== trap files"
find "$CODEQL_EXTRACTOR_SWIFT_TRAP_DIR" -type f 2>/dev/null | wc -l
echo "== extractor invocations"
grep -E 'invocation: .*extractor' "$CODEQL_TRACER_LOG" | cut -c1-300
exit 0
