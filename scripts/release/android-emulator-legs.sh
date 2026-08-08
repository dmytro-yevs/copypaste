#!/usr/bin/env bash
# Compose the debug emulator assertions in one process because the runner
# action starts each line of `script:` in a separate shell.
set -u

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
cd "$repo" || exit 1

"$here/android-smoke.sh"; smoke=$?
npm --prefix e2e-android test; ui=$?
SMOKE_OUT=artifacts/android-storage TRANSFER_REQUIRE_RUN_AS=1 \
    "$here/android-storage-transfer.sh"; storage=$?

rungs=0
api_level="$(adb shell getprop ro.build.version.sdk 2>/dev/null | tr -d '\r')"
if [ "$api_level" = 36 ]; then
    APK='' SMOKE_OUT=artifacts/android-rungs "$here/android-rungs.sh"; rungs=$?
fi

printf '\n== emulator legs: smoke=%s ui=%s storage=%s rungs=%s ==\n' \
    "$smoke" "$ui" "$storage" "$rungs"
[ "$smoke" -eq 0 ] && [ "$ui" -eq 0 ] && [ "$storage" -eq 0 ] && [ "$rungs" -eq 0 ]
