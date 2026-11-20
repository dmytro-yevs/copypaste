#!/usr/bin/env bash
# TEMPORARY: measures how the CodeQL tracer treats process architecture.
set -uo pipefail

dev="$(xcode-select -p)"
toolchain="$dev/Toolchains/XcodeDefault.xctoolchain/usr/bin"
py="$dev/usr/bin/python3"
sdk="$(xcrun --sdk macosx --show-sdk-path)"

echo "== slices"
for file in "$SEMMLE_PRELOAD_libtrace" "$dev/usr/bin/xcodebuild" "$toolchain/swiftc" \
    "$toolchain/swift-frontend" "$toolchain/clang" "$py" /bin/bash /usr/bin/arch \
    "$CODEQL_EXTRACTOR_SWIFT_ROOT"/tools/*/extractor*; do
    printf '%s: ' "$file"
    lipo -archs "$file" 2>&1
done

echo "== step shell"
echo "translated=$(sysctl -n sysctl.proc_translated) machine=$(uname -m)"
echo "DYLD_INSERT_LIBRARIES=${DYLD_INSERT_LIBRARIES:-unset}"

machine='import platform, subprocess, sys
print("self", platform.machine())
subprocess.run([sys.executable, "-c", "import platform; print(\"child\", platform.machine())"])'
echo "== universal binary, default spawn"
"$py" -c "$machine"
echo "exit=$?"
echo "== universal binary, arch -arm64"
arch -arm64 "$py" -c "$machine"
echo "exit=$?"

work="$RUNNER_TEMP/tracer-probe"
mkdir -p "$work/package/Sources/Probe"
cd "$work" || exit 1
for name in translated native native_child; do
    printf 'func probe_%s() -> Int { return 1 }\n' "$name" > "probe_$name.swift"
done
echo "== swiftc, default spawn"
time "$toolchain/swiftc" -sdk "$sdk" -c probe_translated.swift -o translated.o
echo "exit=$?"
echo "== swiftc, arch -arm64"
time arch -arm64 "$toolchain/swiftc" -sdk "$sdk" -c probe_native.swift -o native.o
echo "exit=$?"
echo "== swiftc, child of a native process"
time arch -arm64 "$py" -c 'import subprocess, sys; sys.exit(subprocess.run(sys.argv[1:]).returncode)' \
    "$toolchain/swiftc" -sdk "$sdk" -c probe_native_child.swift -o native_child.o
echo "exit=$?"
for object in *.o; do
    printf '%s: ' "$object"
    lipo -archs "$object"
done

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
cd package || exit 1
echo "== package build, arch -arm64"
time arch -arm64 "$dev/usr/bin/xcodebuild" build -scheme Probe -configuration Release \
    -destination 'generic/platform=macOS' -derivedDataPath "$work/derived" \
    CODE_SIGNING_ALLOWED=NO COMPILATION_CACHE_ENABLE_CACHING=NO \
    SWIFT_ENABLE_COMPILE_CACHE=NO SWIFT_USE_INTEGRATED_DRIVER=NO > "$work/build.log" 2>&1
echo "exit=$?"
grep -E 'BUILD SUCCEEDED|BUILD FAILED|error:' "$work/build.log" | sort | uniq -c
tail -n 15 "$work/build.log"
arch -arm64 "$dev/usr/bin/xcodebuild" -showBuildSettings -scheme Probe \
    -destination 'generic/platform=macOS' 2>/dev/null | grep -E ' (NATIVE_ARCH|HOST_ARCH|ARCHS) ='

echo "== extracted sources"
find "$CODEQL_EXTRACTOR_SWIFT_SOURCE_ARCHIVE_DIR" -type f \
    \( -name 'probe_*' -o -name 'ProbePackage*' -o -name 'Package.swift' \) -print
echo "== trap files"
find "$CODEQL_EXTRACTOR_SWIFT_TRAP_DIR" -type f | wc -l
echo "== tracer log"
grep -c . "$CODEQL_TRACER_LOG"
grep -E 'probe_|ProbePackage' "$CODEQL_TRACER_LOG" | cut -c1-400 | head -40
exit 0
