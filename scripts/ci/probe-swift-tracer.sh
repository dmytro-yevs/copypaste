#!/usr/bin/env bash
# TEMPORARY: checks that a native, untraced build can trace swiftc alone.
set -uo pipefail

scripts="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dev="$(xcode-select -p)"
toolchain="$dev/Toolchains/XcodeDefault.xctoolchain/usr/bin"
sdk="$(xcrun --sdk macosx --show-sdk-path)"
work="$RUNNER_TEMP/tracer-probe"
mkdir -p "$work/package/Sources/Probe"
cd "$work" || exit 1

echo "== step shell"
echo "translated=$(sysctl -n sysctl.proc_translated) machine=$(uname -m)"
echo "DYLD_INSERT_LIBRARIES=[${DYLD_INSERT_LIBRARIES-unset}]"
printf 'runner: '
lipo -archs "$CODEQL_RUNNER"
unset DYLD_INSERT_LIBRARIES

echo "== swiftc through the CodeQL runner"
printf 'func probe_runner() -> Int { return 1 }\n' > probe_runner.swift
time "$CODEQL_RUNNER" "$toolchain/swiftc" -sdk "$sdk" -c probe_runner.swift -o runner.o
echo "exit=$?"
printf 'runner.o: '
lipo -archs runner.o

cat > package/Package.swift <<'SWIFT'
// swift-tools-version: 5.9
import PackageDescription
let package = Package(
  name: "Probe",
  products: [.library(name: "Probe", targets: ["Probe"])],
  targets: [.target(name: "Probe")]
)
SWIFT
printf 'public func probePackage() -> Int { return 1 }\n' > package/Sources/Probe/ProbePackage.swift

cat > "$work/logging-swiftc.sh" <<SH
#!/bin/bash
echo "\$(date +%T) codeql_env=\$(env | grep -c '^CODEQL_') runner=\${CODEQL_RUNNER:-unset} args=\$(printf '%s ' "\$@" | cut -c1-160)" >> "$work/swiftc-calls.log"
exec "$scripts/trace-swiftc.sh" "\$@"
SH
chmod +x "$work/logging-swiftc.sh"

cd package || exit 1
echo "== native package build with a traced Swift compiler"
export SWIFT_EXEC_MANIFEST="$work/logging-swiftc.sh"
time xcodebuild build -scheme Probe -configuration Release \
    -destination 'generic/platform=macOS' -derivedDataPath "$work/derived" \
    "SWIFT_EXEC=$work/logging-swiftc.sh" \
    CODE_SIGNING_ALLOWED=NO COMPILATION_CACHE_ENABLE_CACHING=NO \
    SWIFT_ENABLE_COMPILE_CACHE=NO SWIFT_USE_INTEGRATED_DRIVER=NO > "$work/build.log" 2>&1
echo "exit=$?"
grep -E 'BUILD SUCCEEDED|BUILD FAILED|error:' "$work/build.log" | sort | uniq -c
grep -E '^(CompileSwiftSources|CompileSwift|Ld|Resolve|ExecuteExternalTool)' "$work/build.log" | cut -c1-200
tail -n 12 "$work/build.log"
xcodebuild -showBuildSettings -scheme Probe -destination 'generic/platform=macOS' 2>/dev/null \
    | grep -E ' (NATIVE_ARCH|HOST_ARCH|ARCHS|SWIFT_EXEC) ='

echo "== Swift compiler calls"
cat "$work/swiftc-calls.log"
echo "== extracted sources"
find "$CODEQL_EXTRACTOR_SWIFT_SOURCE_ARCHIVE_DIR" -type f -name '*.swift' -print
echo "== trap files"
find "$CODEQL_EXTRACTOR_SWIFT_TRAP_DIR" -type f | wc -l
echo "== extractor invocations"
grep -E 'invocation: .*extractor' "$CODEQL_TRACER_LOG" | cut -c1-260
exit 0
