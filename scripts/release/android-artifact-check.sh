#!/usr/bin/env bash
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
metadata="$here/android-metadata.mjs"

fail() {
    printf 'android artifact: %s\n' "$*" >&2
    return 1
}

check_artifact() {
    local apk="$1" build_type="$2" aapt2="$3" apksigner="$4"
    local expected_cert="${5:-}" version_override="${6:-}"
    local meta_args=(--format properties)
    [[ -z "$version_override" ]] || meta_args+=(--version "$version_override")

    local properties
    properties="$(node "$metadata" "${meta_args[@]}")" || return 1
    local release_id debug_id version_name version_code expected_id
    release_id="$(sed -n 's/^releaseApplicationId=//p' <<<"$properties")"
    debug_id="$(sed -n 's/^debugApplicationId=//p' <<<"$properties")"
    version_name="$(sed -n 's/^versionName=//p' <<<"$properties")"
    version_code="$(sed -n 's/^versionCode=//p' <<<"$properties")"
    [[ "$debug_id" != "$release_id" ]] || fail "debug and release application ids collide" || return 1

    case "$build_type" in
        debug) expected_id="$debug_id" ;;
        release) expected_id="$release_id" ;;
        *) fail "build type must be debug or release"; return 1 ;;
    esac
    [[ -f "$apk" ]] || fail "APK not found: $apk" || return 1
    [[ -x "$aapt2" ]] || fail "aapt2 is not executable: $aapt2" || return 1
    [[ -x "$apksigner" ]] || fail "apksigner is not executable: $apksigner" || return 1

    local badging package actual_name actual_code
    badging="$("$aapt2" dump badging "$apk")" || fail "aapt2 could not inspect $apk" || return 1
    package="$(sed -n "s/^package: name='\([^']*\)'.*/\1/p" <<<"$badging")"
    actual_name="$(sed -n "s/^package: .*versionName='\([^']*\)'.*/\1/p" <<<"$badging")"
    actual_code="$(sed -n "s/^package: .*versionCode='\([^']*\)'.*/\1/p" <<<"$badging")"
    [[ "$package" == "$expected_id" ]] || fail "$build_type package is '${package:-<unset>}', expected '$expected_id'" || return 1
    [[ "$actual_name" == "$version_name" ]] || fail "versionName is '${actual_name:-<unset>}', expected '$version_name'" || return 1
    [[ "$actual_code" == "$version_code" ]] || fail "versionCode is '${actual_code:-<unset>}', expected '$version_code'" || return 1
    if [[ "$build_type" == debug ]]; then
        grep -q '^application-debuggable' <<<"$badging" || fail "debug APK is not debuggable" || return 1
    else
        ! grep -q '^application-debuggable' <<<"$badging" || fail "release APK is debuggable" || return 1
    fi

    local signer_report certs actual_cert
    signer_report="$("$apksigner" verify --verbose --print-certs "$apk")" || fail "APK signature verification failed" || return 1
    certs="$(sed -n 's/^Signer #[0-9][0-9]* certificate SHA-256 digest: //p' <<<"$signer_report" | tr -d ':' | tr '[:upper:]' '[:lower:]')"
    [[ "$(wc -l <<<"$certs" | tr -d ' ')" == 1 ]] || fail "APK must contain exactly one signer" || return 1
    actual_cert="$certs"
    [[ "$actual_cert" =~ ^[0-9a-f]{64}$ ]] || fail "apksigner reported an invalid SHA-256 fingerprint" || return 1
    if [[ -n "$expected_cert" ]]; then
        local pinned
        pinned="$(tr -d '[:space:]:' <<<"$expected_cert" | tr '[:upper:]' '[:lower:]')"
        [[ "$pinned" =~ ^[0-9a-f]{64}$ ]] || fail "expected signer fingerprint is invalid" || return 1
        [[ "$actual_cert" == "$pinned" ]] || fail "APK signer fingerprint does not match Cargo.toml release metadata" || return 1
    fi
    printf 'package=%s versionName=%s versionCode=%s signer=%s\n' \
        "$package" "$actual_name" "$actual_code" "$actual_cert"
}

self_test() {
    local temp
    temp="$(mktemp -d)"
    trap 'rm -rf -- "$temp"' RETURN
    : > "$temp/app.apk"
    cat > "$temp/aapt2" <<'SH'
#!/usr/bin/env bash
printf "package: name='%s' versionCode='%s' versionName='%s'\n" "$MOCK_PACKAGE" "$MOCK_CODE" "$MOCK_NAME"
[[ "${MOCK_DEBUG:-0}" == 1 ]] && printf 'application-debuggable\n'
exit 0
SH
    cat > "$temp/apksigner" <<'SH'
#!/usr/bin/env bash
printf 'Signer #1 certificate SHA-256 digest: %064d\n' 0
SH
    chmod +x "$temp/aapt2" "$temp/apksigner"

    local current code debug release
    current="$(node "$metadata" --field versionName)" || return 1
    code="$(node "$metadata" --field versionCode)" || return 1
    debug="$(node "$metadata" --field debugApplicationId)" || return 1
    release="$(node "$metadata" --field releaseApplicationId)" || return 1
    MOCK_PACKAGE="$debug" MOCK_NAME="$current" MOCK_CODE="$code" MOCK_DEBUG=1 \
        check_artifact "$temp/app.apk" debug "$temp/aapt2" "$temp/apksigner" >/dev/null
    if MOCK_PACKAGE="$release" MOCK_NAME="$current" MOCK_CODE="$code" MOCK_DEBUG=1 \
        check_artifact "$temp/app.apk" debug "$temp/aapt2" "$temp/apksigner" >/dev/null 2>&1; then
        fail "debug-over-release identity collision was accepted"
        return 1
    fi
    MOCK_PACKAGE="$release" MOCK_NAME="$current" MOCK_CODE="$code" MOCK_DEBUG=0 \
        check_artifact "$temp/app.apk" release "$temp/aapt2" "$temp/apksigner" "$(printf '%064d' 0)" >/dev/null
    if MOCK_PACKAGE="$release" MOCK_NAME="$current" MOCK_CODE="$code" MOCK_DEBUG=0 \
        check_artifact "$temp/app.apk" release "$temp/aapt2" "$temp/apksigner" "$(printf '%064d' 1)" >/dev/null 2>&1; then
        fail "release signer mismatch was accepted"
        return 1
    fi
    printf 'android artifact self-test: ok\n'
}

if [[ "${1:-}" == --self-test ]]; then
    self_test
    exit $?
fi

apk='' build_type='' aapt2='' apksigner='' expected_cert='' version_override=''
while (($#)); do
    case "$1" in
        --apk) apk="$2"; shift 2 ;;
        --build-type) build_type="$2"; shift 2 ;;
        --aapt2) aapt2="$2"; shift 2 ;;
        --apksigner) apksigner="$2"; shift 2 ;;
        --expected-cert) expected_cert="$2"; shift 2 ;;
        --version) version_override="$2"; shift 2 ;;
        *) fail "unknown argument '$1'"; exit 1 ;;
    esac
done
check_artifact "$apk" "$build_type" "$aapt2" "$apksigner" "$expected_cert" "$version_override"
