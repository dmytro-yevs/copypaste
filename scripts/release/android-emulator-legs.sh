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

    printf '\n%d passed, %d failed\n' $((6 - failures)) "$failures"
    [[ $failures -eq 0 ]]
}

case "${1:-}" in
    --self-test) self_test ;;
    "")
        "$here/android-smoke.sh"; smoke=$?
        npm --prefix e2e-android run test:harness && npm --prefix e2e-android test; ui=$?
        SMOKE_OUT=artifacts/android-storage TRANSFER_REQUIRE_RUN_AS=1 \
            "$here/android-storage-transfer.sh"; storage=$?
        APK_UNCONFIGURED="$APK" CLOUD_OUT=artifacts/android-cloud-unconfigured \
            "$here/android-cloud-evidence.sh" --unconfigured; cloud=$?

        # API-36 rung: fail closed when adb/API probing fails; record explicit
        # run/not-applicable receipts. A probe that failed or returned a
        # sentence was previously treated as "not 36" and skipped with rungs=0,
        # so the rungs assertion never ran and the gate passed with no evidence
        # that the clipboard-uid claim was checked.
        rungs=0
        api_probe="$(adb shell getprop ro.build.version.sdk 2>&1 | tr -d '\r')"
        api_status=$?
        if ! api_level="$(validate_api_level "$api_probe" "$api_status")"; then
            echo "::error::$api_level"
            rungs=1
        elif [[ "$api_level" == 36 ]]; then
            APK='' SMOKE_OUT=artifacts/android-rungs "$here/android-rungs.sh"; rungs=$?
        else
            echo "  rungs: not applicable (API $api_level, target 36)"
        fi

        printf '\n== emulator legs: smoke=%s ui=%s storage=%s cloud=%s rungs=%s ==\n' \
            "$smoke" "$ui" "$storage" "$cloud" "$rungs"
        [ "$smoke" -eq 0 ] && [ "$ui" -eq 0 ] && [ "$storage" -eq 0 ] && \
            [ "$cloud" -eq 0 ] && [ "$rungs" -eq 0 ]
        ;;
    *)
        printf 'usage: %s [--self-test]\n' "$0" >&2; exit 2 ;;
esac
