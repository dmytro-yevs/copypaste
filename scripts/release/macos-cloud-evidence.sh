#!/usr/bin/env bash
set -uo pipefail

# shellcheck source=scripts/release/macos-ui-evidence-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/macos-ui-evidence-lib.sh"
# shellcheck source=scripts/release/native-cloud-evidence-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/native-cloud-evidence-lib.sh"

OUT="${1:-artifacts/native/macos/cloud}"
APP="${COPYPASTE_APP:-/Applications/CopyPaste.app}"
BINARY="$APP/Contents/MacOS/CopyPaste"
STUB_PORT="${CLOUD_STUB_PORT:-47800}"
LATENCIES="$OUT/latency.tsv"
APP_PID=""
STUB_PID=""

now_ms() { python3 -c 'import time; print(time.time_ns() // 1000000)'; }

cleanup() {
    [[ -n "$APP_PID" ]] && kill "$APP_PID" 2>/dev/null || true
    [[ -n "$STUB_PID" ]] && kill "$STUB_PID" 2>/dev/null || true
}

launch_app() { # <unconfigured|configured>
    pkill -x CopyPaste 2>/dev/null || true
    if [[ "$1" == configured ]]; then
        COPYPASTE_CLOUD_URL="http://127.0.0.1:$STUB_PORT" \
        COPYPASTE_CLOUD_ANON_KEY="native-evidence" "$BINARY" > "$OUT/app-$1.log" 2>&1 &
    else
        env -u COPYPASTE_CLOUD_URL -u COPYPASTE_CLOUD_ANON_KEY \
            "$BINARY" > "$OUT/app-$1.log" 2>&1 &
    fi
    APP_PID=$!
    mac_wait_label "Settings" "$OUT/app-$1-ready.txt" 30
}

open_cloud() {
    mac_ax press "Settings" >/dev/null || return 1
    mac_wait_label "Sync" "$OUT/settings.txt" 15 || return 1
    mac_ax press "Sync" >/dev/null || return 1
    mac_wait_label "Cloud sync" "$OUT/cloud.txt" 15
}

expect_label() { # <label> <artifact>
    mac_wait_label "$1" "$2" 30 \
        && ok "cloud UI exposes $1" \
        || bad "cloud UI exposes $1" "the macOS accessibility tree did not find it"
}

capture_state() { # <state>
    mac_capture_state "$OUT/$1" \
        && ok "$1 accessibility and screenshot evidence exists" \
        || bad "$1 accessibility and screenshot evidence exists"
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
    group "Cloud UI: unconfigured macOS app"
    started="$(now_ms)"
    launch_app unconfigured || { bad "the unconfigured app exposes accessibility state"; return; }
    open_cloud || { bad "the unconfigured cloud row is reachable"; return; }
    expect_label "Not configured" "$OUT/unconfigured-status.txt"
    elapsed=$(( $(now_ms) - started ))
    cloud_latency_record "$LATENCIES" unconfigured-status "$elapsed" 30000 \
        && ok "unconfigured cloud status meets its latency budget" \
        || bad "unconfigured cloud status meets its latency budget" "${elapsed}ms"
    capture_state unconfigured
}

configured_scenario() {
    local started elapsed
    group "Cloud UI: configured macOS account lifecycle"
    start_stub || { bad "the cloud evidence backend starts"; return; }
    launch_app configured || { bad "the configured app exposes accessibility state"; return; }
    open_cloud || { bad "the configured cloud row is reachable"; return; }
    if mac_ax_contains "$OUT/cloud.txt" "Connected"; then
        mac_ax press "Sign out" >/dev/null || true
    fi
    expect_label "Signed out" "$OUT/signed-out-status.txt"
    expect_label "Cloud account sign in" "$OUT/signed-out-form.txt"
    expect_label "Email" "$OUT/signed-out-email.txt"
    expect_label "Password" "$OUT/signed-out-password.txt"
    expect_label "Sync passphrase" "$OUT/signed-out-passphrase.txt"
    capture_state signed-out

    mac_ax set "Email" "native@example.test" >/dev/null || bad "email can be entered"
    mac_ax set "Password" "stub-password" >/dev/null || bad "password can be entered"
    mac_ax set "Sync passphrase" "native-evidence" >/dev/null || bad "passphrase can be entered"
    started="$(now_ms)"
    mac_ax press "Sign in" >/dev/null || bad "the native sign-in action is reachable"
    expect_label "Connected" "$OUT/connected.txt"
    elapsed=$(( $(now_ms) - started ))
    cloud_latency_record "$LATENCIES" sign-in "$elapsed" 30000 \
        && ok "cloud sign-in meets its latency budget" \
        || bad "cloud sign-in meets its latency budget" "${elapsed}ms"
    expect_label "native@example.test" "$OUT/account-status.txt"
    capture_state signed-in

    seed_forged_row || bad "the skip fixture reaches the stub backend"
    started="$(now_ms)"
    mac_ax press "Sync cloud now" >/dev/null || bad "the native cloud sync action is reachable"
    expect_label "skipped" "$OUT/sync-skips.txt"
    elapsed=$(( $(now_ms) - started ))
    cloud_latency_record "$LATENCIES" sync-with-skips "$elapsed" 30000 \
        && ok "cloud sync with skips meets its latency budget" \
        || bad "cloud sync with skips meets its latency budget" "${elapsed}ms"
    capture_state sync-with-skips

    kill "$STUB_PID" 2>/dev/null || true
    wait "$STUB_PID" 2>/dev/null || true
    STUB_PID=""
    started="$(now_ms)"
    mac_ax press "Sync cloud now" >/dev/null || bad "cloud sync remains actionable offline"
    expect_label "The last cloud sync failed" "$OUT/offline-error.txt"
    elapsed=$(( $(now_ms) - started ))
    cloud_latency_record "$LATENCIES" offline-error "$elapsed" 60000 \
        && ok "offline cloud error meets its latency budget" \
        || bad "offline cloud error meets its latency budget" "${elapsed}ms"
    capture_state offline-error

    mac_ax press "Sign out" >/dev/null || bad "the native sign-out action is reachable"
    expect_label "Signed out" "$OUT/signed-out-again.txt"
    capture_state signed-out-again
}

if [[ "${1:-}" == "--self-test" ]]; then
    SELF_TEST_TMP="$(mktemp -d)"
    trap 'rm -rf "$SELF_TEST_TMP"' EXIT
    mac_ui_self_test "$SELF_TEST_TMP"
    cloud_evidence_self_test "$SELF_TEST_TMP"
    cloud_evidence_summary macOS
    [[ $FAIL -eq 0 ]]
    exit
fi

[[ "$(uname -s)" == Darwin ]] || { echo "ERROR: must run on macOS" >&2; exit 2; }
[[ -x "$BINARY" ]] || { echo "ERROR: installed CopyPaste executable is missing" >&2; exit 2; }
mkdir -p "$OUT"
: > "$LATENCIES"
trap cleanup EXIT
unconfigured_scenario
configured_scenario
cloud_latency_write "$LATENCIES" "$OUT/latency.json" macos
cloud_evidence_summary macOS
[[ $FAIL -eq 0 ]]
