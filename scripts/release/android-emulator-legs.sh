#!/usr/bin/env bash
# Compose the debug emulator assertions in one process because the runner
# action starts each line of `script:` in a separate shell.
set -uo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
cd "$repo" || exit 1

# Validate a raw API level probe. Fails closed: an empty, non-numeric, or
# failed probe is never accepted as a level, so every downstream decision
# (which rungs run, which clipboard codes to expect) rests on a real number.
#
# A previous probe folded stderr into stdout via `sh_`, so an offline device
# produced a sentence that every level comparison silently treated as
# "not 36", and the rungs were skipped with exit 0.
validate_api_level() {
    local probe="$1" status="$2"
    if [[ $status -ne 0 ]]; then
        printf 'the API probe failed (exit %s): %s\n' "$status" "${probe:-adb said nothing}" >&2
        return 1
    fi
    probe="$(printf '%s' "$probe" | tr -d '\r' | head -n 1)"
    probe="${probe#"${probe%%[![:space:]]*}"}"
    probe="${probe%"${probe##*[![:space:]]}"}"
    if [[ ! "$probe" =~ ^[0-9]+$ ]]; then
        printf 'the API probe returned no level: %s\n' "${probe:-nothing at all}" >&2
        return 1
    fi
    printf '%s\n' "$probe"
}

run_required_leg() { # <name> <command...>
    local name="$1" status
    shift
    "$@" && return 0
    status=$?
    printf '::error::%s failed (exit %s); later emulator legs were not run\n' "$name" "$status" >&2
    return "$status"
}

self_test() {
    local failures=0

    group() { printf '\n== %s\n' "$1"; }
    ok()   { printf '  ok    %s\n' "$1"; }
    bad()  { printf '  FAIL  %s\n' "$1"; [[ -n "${2:-}" ]] && printf '        %s\n' "$2"; failures=$((failures + 1)); }

    group "API probe validation"

    local probe status result
    probe="$(echo "error: device offline" | tr -d '\r')"
    status=1
    result="$(validate_api_level "$probe" "$status" 2>&1)"
    if [[ $? -ne 0 && "$result" == *"failed"* ]]; then
        ok "adb failure fails closed"
    else
        bad "adb failure fails closed" "result: $result"
    fi

    probe="emulator-5554"
    status=1
    result="$(validate_api_level "$probe" "$status" 2>&1)"
    if [[ $? -ne 0 && "$result" == *"failed"* ]]; then
        ok "device-name probe fails closed"
    else
        bad "device-name probe fails closed" "result: $result"
    fi

    probe=""
    status=0
    result="$(validate_api_level "$probe" "$status" 2>&1)"
    if [[ $? -ne 0 && "$result" == *"no level"* ]]; then
        ok "empty probe fails closed"
    else
        bad "empty probe fails closed" "result: $result"
    fi

    probe="not a number"
    status=0
    result="$(validate_api_level "$probe" "$status" 2>&1)"
    if [[ $? -ne 0 && "$result" == *"no level"* ]]; then
        ok "non-numeric probe fails closed"
    else
        bad "non-numeric probe fails closed" "result: $result"
    fi

    probe="  24  "
    status=0
    result="$(validate_api_level "$probe" "$status" 2>&1)"
    if [[ $? -eq 0 && "$result" == "24" ]]; then
        ok "whitespace-padded level is read"
    else
        bad "whitespace-padded level is read" "result: $result"
    fi

    probe="36"
    status=0
    result="$(validate_api_level "$probe" "$status" 2>&1)"
    if [[ $? -eq 0 && "$result" == "36" ]]; then
        ok "API 36 is recognized"
    else
        bad "API 36 is recognized" "result: $result"
    fi

    local observed=""
    fixture_leg() {
        observed+="$1 "
        [[ "$1" != broken ]]
    }
    if run_required_leg first fixture_leg first >/dev/null 2>&1 \
        && run_required_leg broken fixture_leg broken >/dev/null 2>&1 \
        && run_required_leg unreachable fixture_leg unreachable >/dev/null 2>&1; then
        bad "a failed emulator leg stops the sequence"
    elif [[ "$observed" == "first broken " ]]; then
        ok "a failed emulator leg stops the sequence"
    else
        bad "a failed emulator leg stops the sequence" "observed: $observed"
    fi

    printf '\n%d passed, %d failed\n' $((7 - failures)) "$failures"
    [[ $failures -eq 0 ]]
}

run_emulator_legs() {
    run_required_leg "Android smoke" "$here/android-smoke.sh" || return 1
    run_required_leg "Android harness unit tests" npm --prefix e2e-android run test:harness || return 1
    run_required_leg "Android WebView E2E" npm --prefix e2e-android test || return 1
    SMOKE_OUT=artifacts/android-storage TRANSFER_REQUIRE_RUN_AS=1 \
        run_required_leg "Android storage transfer" "$here/android-storage-transfer.sh" || return 1
    APK_UNCONFIGURED="$APK" CLOUD_OUT=artifacts/android-cloud-unconfigured \
        run_required_leg "Android unconfigured cloud evidence" \
            "$here/android-cloud-evidence.sh" --unconfigured || return 1

    # A failed adb probe once became a sentence that compared unequal to 36,
    # silently skipping the API-36 rung. Keep status and output separate.
    local api_probe api_status api_level
    if api_probe="$(adb shell getprop ro.build.version.sdk 2>&1 | tr -d '\r')"; then
        api_status=0
    else
        api_status=$?
    fi
    if ! api_level="$(validate_api_level "$api_probe" "$api_status")"; then
        printf '::error::%s\n' "$api_level" >&2
        return 1
    fi
    if [[ "$api_level" == 36 ]]; then
        APK='' SMOKE_OUT=artifacts/android-rungs \
            run_required_leg "Android API 36 clipboard rung" \
                "$here/android-rungs.sh" || return 1
    else
        printf '  rungs: not applicable (API %s, target 36)\n' "$api_level"
    fi
    printf '\n== all required emulator legs passed ==\n'
}

case "${1:-}" in
    --self-test) self_test ;;
    "") run_emulator_legs ;;
    *)
        printf 'usage: %s [--self-test]\n' "$0" >&2; exit 2 ;;
esac
