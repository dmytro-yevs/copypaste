#!/usr/bin/env bash
set -u

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
cd "$repo" || exit 1

SMOKE_PROFILE="${COPYPASTE_SMOKE_PROFILE:-full}"

valid_smoke_profile() { # <profile>
    case "$1" in
        full|critical) return 0 ;;
        *) return 1 ;;
    esac
}

profile_runs_extended_release_legs() { # <profile>
    [[ "$1" == full ]]
}

run_release_leg() { # <name> <command...>
    local name="$1"
    shift
    "$@" || {
        printf '::error::%s failed; later release legs were not run\n' "$name" >&2
        return 1
    }
}

run_upgrade_leg() {
    [[ -z "${PREVIOUS_APK:-}" ]] \
        || bash scripts/release/android-install-upgrade.sh "$PREVIOUS_APK" "$APK"
}

run_release_smoke_leg() {
    ./scripts/release/android-smoke-release.sh
}

run_storage_leg() {
    SMOKE_OUT="${STORAGE_OUT:?STORAGE_OUT is required}" \
        ./scripts/release/android-storage-transfer.sh
}

run_cloud_leg() {
    APK_UNCONFIGURED="${APK_UNCONFIGURED:-$APK}" \
    CLOUD_EVIDENCE_APK="${CLOUD_EVIDENCE_APK:-$APK}" \
    CLOUD_OUT="${CLOUD_OUT:?CLOUD_OUT is required}" \
        ./scripts/release/android-cloud-evidence.sh "--${CLOUD_MODE:?CLOUD_MODE is required}"
}

run_profiled_release_legs() { # <profile> [runner]
    local profile="$1" runner="${2:-run_release_leg}"
    "$runner" "Android upgrade persistence" run_upgrade_leg || return 1
    "$runner" "Android release smoke" run_release_smoke_leg || return 1
    if profile_runs_extended_release_legs "$profile"; then
        "$runner" "Android storage transfer" run_storage_leg || return 1
        "$runner" "Android cloud evidence" run_cloud_leg || return 1
    fi
}

if ! valid_smoke_profile "$SMOKE_PROFILE"; then
    printf 'COPYPASTE_SMOKE_PROFILE must be full or critical, got %s\n' "$SMOKE_PROFILE" >&2
    exit 2
fi

if [[ "${1:-}" == "--self-test" ]]; then
    critical_legs="" failed_critical_legs=""
    successful_runner() { critical_legs+="$1 "; }
    failing_runner() {
        failed_critical_legs+="$1 "
        [[ "$1" != "Android release smoke" ]]
    }
    if valid_smoke_profile full && valid_smoke_profile critical \
        && ! valid_smoke_profile unknown \
        && run_profiled_release_legs critical successful_runner \
        && [[ "$critical_legs" == "Android upgrade persistence Android release smoke " ]] \
        && ! run_profiled_release_legs critical failing_runner \
        && [[ "$failed_critical_legs" == "Android upgrade persistence Android release smoke " ]]; then
        printf 'release emulator profile self-test passed\n'
        exit 0
    fi
    printf 'release emulator profile self-test failed\n' >&2
    exit 1
fi

if run_profiled_release_legs "$SMOKE_PROFILE"; then
    printf '\n== release emulator legs (%s) passed ==\n' "$SMOKE_PROFILE"
else
    exit 1
fi
if ! profile_runs_extended_release_legs "$SMOKE_PROFILE"; then
    printf '== critical release profile: cloud and storage-transfer legs are deferred ==\n'
fi
