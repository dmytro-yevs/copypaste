#!/usr/bin/env bash
set -uo pipefail

# shellcheck source=scripts/release/android-smoke-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-smoke-lib.sh"
# shellcheck source=scripts/release/android-ui-evidence-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-ui-evidence-lib.sh"
# shellcheck source=scripts/release/native-cloud-evidence-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/native-cloud-evidence-lib.sh"

MODE="${1:---all}"
OUT="${CLOUD_OUT:-artifacts/native/android/cloud}"
APK_UNCONFIGURED="${APK_UNCONFIGURED:-${APK:-}}"
APK_CONFIGURED="${CLOUD_EVIDENCE_APK:-}"
MAIN="$PKG/$APP_NAMESPACE.MainActivity"
WAIT_SECS="${CLOUD_WAIT_SECS:-45}"
STUB_PORT="${CLOUD_STUB_PORT:-47800}"
LATENCIES="$OUT/latency.tsv"
STUB_PID=""

now_ms() { python3 -c 'import time; print(time.time_ns() // 1000000)'; }

cleanup() {
    [[ -n "$STUB_PID" ]] && kill "$STUB_PID" 2>/dev/null || true
    adb reverse --remove "tcp:$STUB_PORT" >/dev/null 2>&1 || true
}

install_and_open() { # <apk>
    [[ -f "$1" ]] || { bad "the cloud evidence APK exists" "missing: $1"; return 1; }
    sh_ am force-stop "$PKG" >/dev/null 2>&1 || true
    adb uninstall "$PKG" >/dev/null 2>&1 || true
    adb install -r -g "$1" >/dev/null || return 1
    wake_screen
    sh_ am start -W -n "$MAIN" >/dev/null
    wait_selector "Settings" "$OUT/launch.xml" 90
}

open_cloud() {
    tap_selector "Settings" "$OUT/settings-nav.xml" || return 1
    tap_selector "Sync" "$OUT/settings-sync.xml" || return 1
    find_scrolling "Cloud sync" "$OUT/cloud-visible.xml" up
}

capture_state() { # <state>
    local dir="$OUT/$1"
    mkdir -p "$dir"
    dump_hierarchy "$dir/ax.xml" || true
    capture_png "$dir/screenshot.png"
    [[ -s "$dir/ax.xml" ]] || bad "$1 accessibility evidence exists"
    [[ -s "$dir/screenshot.png" ]] || bad "$1 screenshot evidence exists"
}

expect_label() { # <selector> <artifact>
    wait_selector "$1" "$2" "$WAIT_SECS" \
        && ok "cloud UI exposes $1" \
        || bad "cloud UI exposes $1" "uiautomator did not find it"
}

fill_field() { # <label> <value> <artifact>
    tap_selector "$1" "$3" || return 1
    sh_ input keyevent KEYCODE_MOVE_END >/dev/null
    sh_ input text "$2" >/dev/null
    sh_ input keyevent KEYCODE_BACK >/dev/null
}

start_stub() {
    python3 scripts/cloud-stub.py --port "$STUB_PORT" --password stub-password \
        --dump "$OUT/stub-rows.json" > "$OUT/stub.log" 2>&1 &
    STUB_PID=$!
    for _ in $(seq 1 50); do
        curl -fsS -o /dev/null -X POST "http://127.0.0.1:$STUB_PORT/auth/v1/logout" && return 0
        sleep 0.2
    done
    return 1
}

seed_forged_row() {
    local stamp payload
    stamp="$(now_ms)"
    payload="[{\"item_id\":\"native-forged-$stamp\",\"ciphertext\":\"AA==\",\"nonce\":\"AA==\",\"content_type\":\"text\",\"payload_metadata\":null,\"created_at\":$stamp,\"deleted\":false,\"origin_device_id\":\"native-evidence\",\"signature\":\"\"}]"
    curl -fsS -X POST "http://127.0.0.1:$STUB_PORT/rest/v1/clipboard_items" \
        -H 'Authorization: Bearer native-evidence' -H 'Content-Type: application/json' \
        --data "$payload" >/dev/null
}

unconfigured_scenario() {
    local started elapsed
    group "Cloud UI: unconfigured build"
    started="$(now_ms)"
    install_and_open "$APK_UNCONFIGURED" || return
    open_cloud || { bad "the unconfigured cloud row is reachable"; return; }
    expect_label "Not configured" "$OUT/unconfigured-status.xml"
    elapsed=$(( $(now_ms) - started ))
    cloud_latency_record "$LATENCIES" unconfigured-status "$elapsed" 90000 \
        && ok "unconfigured cloud status meets its latency budget" \
        || bad "unconfigured cloud status meets its latency budget" "${elapsed}ms"
    capture_state unconfigured
}

configured_scenario() {
    local started elapsed
    group "Cloud UI: configured account lifecycle"
    start_stub || { bad "the cloud evidence backend starts"; return; }
    adb reverse "tcp:$STUB_PORT" "tcp:$STUB_PORT" >/dev/null \
        || { bad "the cloud evidence endpoint is reversed to loopback"; return; }
    install_and_open "$APK_CONFIGURED" || return
    open_cloud || { bad "the configured cloud row is reachable"; return; }

    expect_label "Signed out" "$OUT/signed-out-status.xml"
    expect_label "Cloud account sign in" "$OUT/signed-out-form.xml"
    expect_label "Email" "$OUT/signed-out-email.xml"
    expect_label "Password" "$OUT/signed-out-password.xml"
    expect_label "Sync passphrase" "$OUT/signed-out-passphrase.xml"
    capture_state signed-out

    fill_field "Email" "native@example.test" "$OUT/email.xml" || bad "email can be entered"
    fill_field "Password" "stub-password" "$OUT/password.xml" || bad "password can be entered"
    fill_field "Sync passphrase" "native-evidence" "$OUT/passphrase.xml" || bad "passphrase can be entered"
    started="$(now_ms)"
    tap_selector "Sign in" "$OUT/sign-in.xml" || bad "the native sign-in action is reachable"
    expect_label "Connected" "$OUT/connected.xml"
    elapsed=$(( $(now_ms) - started ))
    cloud_latency_record "$LATENCIES" sign-in "$elapsed" 30000 \
        && ok "cloud sign-in meets its latency budget" \
        || bad "cloud sign-in meets its latency budget" "${elapsed}ms"
    expect_label "native@example.test" "$OUT/account-status.xml"
    capture_state signed-in

    seed_forged_row || bad "the skip fixture reaches the stub backend"
    started="$(now_ms)"
    tap_selector "Sync cloud now" "$OUT/sync.xml" || bad "the native cloud sync action is reachable"
    expect_label "skipped" "$OUT/sync-skips.xml"
    elapsed=$(( $(now_ms) - started ))
    cloud_latency_record "$LATENCIES" sync-with-skips "$elapsed" 30000 \
        && ok "cloud sync with skips meets its latency budget" \
        || bad "cloud sync with skips meets its latency budget" "${elapsed}ms"
    capture_state sync-with-skips

    kill "$STUB_PID" 2>/dev/null || true
    wait "$STUB_PID" 2>/dev/null || true
    STUB_PID=""
    started="$(now_ms)"
    tap_selector "Sync cloud now" "$OUT/offline-sync.xml" || bad "cloud sync remains actionable offline"
    expect_label "The last cloud sync failed" "$OUT/offline-error.xml"
    elapsed=$(( $(now_ms) - started ))
    cloud_latency_record "$LATENCIES" offline-error "$elapsed" 60000 \
        && ok "offline cloud error meets its latency budget" \
        || bad "offline cloud error meets its latency budget" "${elapsed}ms"
    capture_state offline-error

    tap_selector "Sign out" "$OUT/sign-out.xml" || bad "the native sign-out action is reachable"
    expect_label "Signed out" "$OUT/signed-out-again.xml"
    capture_state signed-out-again
}

if [[ "$MODE" == "--self-test" ]]; then
    SELF_TEST_TMP="$(mktemp -d)"
    trap 'rm -rf "$SELF_TEST_TMP"' EXIT
    android_ui_self_test
    cloud_evidence_self_test "$SELF_TEST_TMP"
    cloud_evidence_summary Android
    [[ $FAIL -eq 0 ]]
    exit
fi

mkdir -p "$OUT"
: > "$LATENCIES"
trap cleanup EXIT
command -v adb >/dev/null 2>&1 || { echo "FATAL: adb is not on PATH"; exit 1; }
command -v curl >/dev/null 2>&1 || { echo "FATAL: curl is not on PATH"; exit 1; }
adb wait-for-device

case "$MODE" in
    --unconfigured) unconfigured_scenario ;;
    --configured) configured_scenario ;;
    --all) unconfigured_scenario; configured_scenario ;;
    *) echo "usage: android-cloud-evidence.sh [--all|--unconfigured|--configured|--self-test]" >&2; exit 2 ;;
esac

cloud_latency_write "$LATENCIES" "$OUT/latency.json" android
cloud_evidence_summary Android
[[ $FAIL -eq 0 ]]
