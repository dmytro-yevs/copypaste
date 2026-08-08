#!/usr/bin/env bash
# The three emulator legs, composed in one process.
#
# This exists because reactivecircus/android-emulator-runner runs every line of
# its `script:` in a separate `sh -c`: variables set on one line are gone by the
# next, so collecting three exit statuses and combining them cannot be done
# there. It also gets us bash, which that `sh` (dash) is not.
#
# Every leg runs even when an earlier one fails — a red smoke leg should not
# hide whether the UI harness and the rungs still hold.
set -u

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
cd "$repo" || exit 1

"$here/android-smoke.sh"; smoke=$?
npm --prefix e2e-android test; ui=$?
APK='' SMOKE_OUT=artifacts/android-rungs "$here/android-rungs.sh"; rungs=$?

printf '\n== emulator legs: smoke=%s ui=%s rungs=%s ==\n' "$smoke" "$ui" "$rungs"
[ "$smoke" -eq 0 ] && [ "$ui" -eq 0 ] && [ "$rungs" -eq 0 ]
