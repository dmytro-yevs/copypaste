#!/usr/bin/env bash
set -u

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
cd "$repo" || exit 1
upgrade=0
if [[ -n "${PREVIOUS_APK:-}" ]]; then
    bash scripts/release/android-install-upgrade.sh "$PREVIOUS_APK" "$APK"; upgrade=$?
fi
./scripts/release/android-smoke-release.sh; smoke=$?
SMOKE_OUT="${STORAGE_OUT:?STORAGE_OUT is required}" \
    ./scripts/release/android-storage-transfer.sh; storage=$?
APK_UNCONFIGURED="${APK_UNCONFIGURED:-$APK}" \
CLOUD_EVIDENCE_APK="${CLOUD_EVIDENCE_APK:-$APK}" \
CLOUD_OUT="${CLOUD_OUT:?CLOUD_OUT is required}" \
    ./scripts/release/android-cloud-evidence.sh "--${CLOUD_MODE:?CLOUD_MODE is required}"; cloud=$?

printf '\n== release emulator legs: upgrade=%s smoke=%s storage=%s cloud=%s ==\n' \
    "$upgrade" "$smoke" "$storage" "$cloud"
[[ $upgrade -eq 0 && $smoke -eq 0 && $storage -eq 0 && $cloud -eq 0 ]]
