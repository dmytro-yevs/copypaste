#!/usr/bin/env bash
set -uo pipefail

# shellcheck source=scripts/release/macos-bundle-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/macos-bundle-lib.sh"
# shellcheck source=scripts/release/macos-ui-evidence-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/macos-ui-evidence-lib.sh"
# shellcheck source=scripts/release/native-cloud-evidence-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/native-cloud-evidence-lib.sh"

OUT="${1:-artifacts/native/macos/cloud}"
APP="${COPYPASTE_APP:-/Applications/CopyPaste.app}"
BINARY=""
STUB_PORT="${CLOUD_STUB_PORT:-47800}"
LATENCIES="$OUT/latency.tsv"
APP_PID=""
STUB_PID=""
DAEMON_SIDECAR=""

now_ms() { python3 -c 'import time; print(time.time_ns() // 1000000)'; }

cleanup() {
    [[ -n "$APP_PID" ]] && kill "$APP_PID" 2>/dev/null || true
    [[ -n "$STUB_PID" ]] && kill "$STUB_PID" 2>/dev/null || true
    "$APP/Contents/MacOS/copypaste" shutdown >/dev/null 2>&1 || true
}

seed_onboarding_complete() { # [preferences.json]
    # The welcome flow hides Settings. Seed the Tauri store the same way a
    # completed setup would, so cloud evidence can open the Sync row.
    local path="${1:-$HOME/Library/Application Support/com.copypaste.app/preferences.json}"
    mkdir -p "$(dirname "$path")"
    node --input-type=module - "$path" <<'JS'
import { readFileSync, writeFileSync } from "node:fs";
import {
  DEFAULT_PREFS,
  PREFERENCES_VERSION,
  STORAGE_KEY,
} from "./crates/copypaste-ui/src/lib/preferenceContract.ts";

const path = process.argv[2];
let store = {};
try {
  const parsed = JSON.parse(readFileSync(path, "utf8"));
  if (parsed !== null && typeof parsed === "object" && !Array.isArray(parsed)) {
    store = parsed;
  }
} catch {}
const state = {
  ...DEFAULT_PREFS,
  allowScreenshots: true,
  onboardingComplete: true,
};
store[STORAGE_KEY] = JSON.stringify({ state, version: PREFERENCES_VERSION });
writeFileSync(path, `${JSON.stringify(store, null, 2)}\n`);
JS
}

preference_seed_self_test() { # <tmp-dir>
    local path="$1/preferences.json"
    printf '{"unrelated":"kept"}\n' > "$path"
    if seed_onboarding_complete "$path" && node --input-type=module - "$path" <<'JS'
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  DEFAULT_PREFS,
  PREFERENCES_VERSION,
  STORAGE_KEY,
} from "./crates/copypaste-ui/src/lib/preferenceContract.ts";

const store = JSON.parse(readFileSync(process.argv[2], "utf8"));
const blob = JSON.parse(store[STORAGE_KEY]);
assert.equal(store.unrelated, "kept");
assert.deepEqual(Object.keys(blob).sort(), ["state", "version"]);
assert.equal(blob.version, PREFERENCES_VERSION);
assert.deepEqual(blob.state, {
  ...DEFAULT_PREFS,
  allowScreenshots: true,
  onboardingComplete: true,
});
JS
    then
        ok "onboarding seed matches the current preference contract"
    else
        bad "onboarding seed matches the current preference contract"
    fi
}

ensure_cloud_evidence_daemon() {
    # Evidence-only daemon: never write into the production target/release path.
    local provided="${COPYPASTE_CLOUD_EVIDENCE_DAEMON:-}"
    local target_dir="${COPYPASTE_CLOUD_EVIDENCE_TARGET_DIR:-$PWD/target/cloud-evidence-daemon}"
    local built="$target_dir/release/copypaste-daemon"
    local candidate=""
    local log="/dev/null"
    [[ -n "${OUT:-}" ]] && log="$OUT/sidecar-build.log"

    if [[ -n "$provided" ]]; then
        candidate="$provided"
    else
        if [[ ! -x "$built" ]]; then
            cargo build --release --locked -p copypaste-daemon --features cloud-evidence \
                --target-dir "$target_dir" >"$log" 2>&1 || return 1
        fi
        candidate="$built"
    fi
    [[ -n "$candidate" && -x "$candidate" ]] || return 1
    DAEMON_SIDECAR="$(cd "$(dirname "$candidate")" && pwd)/$(basename "$candidate")"
    [[ "$DAEMON_SIDECAR" == /* && -x "$DAEMON_SIDECAR" ]]
}

open_cloud_evidence_app() { # <unconfigured|configured>
    if [[ "$1" == configured ]]; then
        [[ "${DAEMON_SIDECAR:-}" == /* && -x "$DAEMON_SIDECAR" ]] || return 1
        open -n -a "$APP" \
            --env "COPYPASTE_EVIDENCE_AX=1" \
            --env "COPYPASTE_CLOUD_URL=http://127.0.0.1:$STUB_PORT" \
            --env "COPYPASTE_CLOUD_ANON_KEY=native-evidence" \
            --env "COPYPASTE_DAEMON_BIN=$DAEMON_SIDECAR"
    else
        env -u COPYPASTE_CLOUD_URL -u COPYPASTE_CLOUD_ANON_KEY -u COPYPASTE_DAEMON_BIN \
            open -n -a "$APP" \
            --env "COPYPASTE_EVIDENCE_AX=1"
    fi
}

launch_app() { # <unconfigured|configured>
    mac_stop_executable "$BINARY" || return 1
    "$APP/Contents/MacOS/copypaste" shutdown >/dev/null 2>&1 || true
    pkill -f "$APP/Contents/MacOS/copypaste-daemon" 2>/dev/null || true
    seed_onboarding_complete
    # Launch Services registration matches macos-native-evidence; a raw binary
    # background job is not reliably addressable by System Events.
    # COPYPASTE_EVIDENCE_AX asks the app to publish the WKWebView AX tree.
    open_cloud_evidence_app "$1" > "$OUT/app-$1-open.log" 2>&1 || return 1
    APP_PID="$(mac_wait_executable_pid "$BINARY" 30)" || return 1
    mac_set_app_pid "$APP_PID"
    local ready_started="$SECONDS"
    while (( SECONDS - ready_started < 30 )); do
        mac_ax ready >/dev/null 2>"$OUT/app-$1-ax.err" && break
        sleep 0.1
    done
    if ! mac_reach_settings "$OUT/app-$1-ready.txt" 45; then
        mac_ax dump > "$OUT/app-$1-ax-fail.txt" 2>&1 || true
        mac_ax surface > "$OUT/app-$1-ax-surface.txt" 2>&1 || true
        return 1
    fi
}

open_cloud() {
    mac_press_exact_role "Cloud sync" "AXRadioButton" >/dev/null || return 1
    mac_wait_unique_safe_role_label "Cloud sync" "AXHeading" "$OUT/cloud.txt" 15
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
    launch_app unconfigured || { bad "the unconfigured app exposes accessibility state"; return; }
    open_cloud || { bad "the unconfigured cloud row is reachable"; return; }
    started="$(now_ms)"
    expect_label "Not configured" "$OUT/unconfigured-status.txt"
    expect_label "Cloud server configuration" "$OUT/unconfigured-form.txt"
    expect_label "Server URL" "$OUT/unconfigured-url.txt"
    expect_label "Publishable key" "$OUT/unconfigured-key.txt"
    expect_label "Configure" "$OUT/unconfigured-action.txt"
    elapsed=$(( $(now_ms) - started ))
    cloud_latency_record "$LATENCIES" unconfigured-status "$elapsed" 30000 \
        && ok "unconfigured cloud status meets its latency budget" \
        || bad "unconfigured cloud status meets its latency budget" "${elapsed}ms"
    capture_state unconfigured
}

configured_scenario() {
    local started elapsed
    group "Cloud UI: configured macOS account lifecycle"
    ensure_cloud_evidence_daemon || { bad "the cloud-evidence daemon sidecar is present"; return; }
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

cloud_panel_selector_self_test() { # <tmp-dir>
    local selected=no presses=0 saved_out="$OUT" radio_rows heading_rows
    OUT="$1/cloud-panel"
    mkdir -p "$OUT"

    mac_ax() {
        case "$1" in
            find-exact-role-candidates)
                [[ "$2" == "Cloud sync" ]] || return 1
                case "$3" in
                    AXRadioButton) printf '%s' "$radio_rows" ;;
                    AXHeading) printf '%s' "$heading_rows" ;;
                    *) return 1 ;;
                esac
                ;;
            press-exact-role)
                [[ "$2" == "Cloud sync" && "$3" == "AXRadioButton" ]] || return 1
                ((presses += 1))
                selected=yes
                printf 'ok\n'
                ;;
            press) [[ "$2" == "Sync now" ]] && printf 'ok\n' ;;
            *) return 1 ;;
        esac
    }

    radio_rows=""
    if mac_press_exact_role "Cloud sync" "AXRadioButton" >/dev/null 2>&1 || (( presses != 0 )); then
        bad "Cloud sync selector rejects zero matching radios"
    else
        ok "Cloud sync selector rejects zero matching radios"
    fi
    radio_rows=$'AXRadioButton\tCloud sync\nAXRadioButton\tCloud sync\n'
    if mac_press_exact_role "Cloud sync" "AXRadioButton" >/dev/null 2>&1 || (( presses != 0 )); then
        bad "Cloud sync selector rejects duplicate matching radios"
    else
        ok "Cloud sync selector rejects duplicate matching radios"
    fi
    radio_rows=$'AXButton\tCloud sync\n'
    if mac_press_exact_role "Cloud sync" "AXRadioButton" >/dev/null 2>&1 || (( presses != 0 )); then
        bad "Cloud sync selector rejects wrong-role-only matches without pressing"
    else
        ok "Cloud sync selector rejects wrong-role-only matches without pressing"
    fi
    radio_rows=$'AXRadioButton\tCloud sync\n'
    heading_rows=""
    if mac_press_exact_role "Cloud sync" "AXRadioButton" >/dev/null \
        && [[ "$selected" == yes && "$presses" == 1 ]] \
        && ! mac_find_unique_exact_role_label "Cloud sync" "AXHeading" > "$OUT/zero-heading.txt" 2>&1; then
        ok "Cloud sync selector rejects zero headings after selection"
    else
        bad "Cloud sync selector rejects zero headings after selection"
    fi
    heading_rows=$'AXHeading\tCloud sync\nAXHeading\tCloud sync\n'
    if ! mac_find_unique_exact_role_label "Cloud sync" "AXHeading" > "$OUT/duplicate-heading.txt" 2>&1; then
        ok "Cloud sync selector rejects duplicate headings after selection"
    else
        bad "Cloud sync selector rejects duplicate headings after selection"
    fi
    heading_rows=$'AXButton\tCloud sync\n'
    if ! mac_find_unique_exact_role_label "Cloud sync" "AXHeading" > "$OUT/wrong-role-heading.txt" 2>&1; then
        ok "Cloud sync selector rejects wrong-role-only headings after selection"
    else
        bad "Cloud sync selector rejects wrong-role-only headings after selection"
    fi
    selected=no
    presses=0
    heading_rows=$'AXHeading\tSync now\n'
    if mac_ax press "Sync now" >/dev/null \
        && [[ "$selected" == no && "$presses" == 0 ]] \
        && ! mac_find_unique_exact_role_label "Cloud sync" "AXHeading" > "$OUT/pre-select.txt" 2>&1; then
        ok "Sync now cannot satisfy Cloud sync selection"
    else
        bad "Sync now cannot satisfy Cloud sync selection"
    fi
    radio_rows=$'AXRadioButton\tCloud sync\n'
    heading_rows=$'AXHeading\tCloud sync\n'
    if open_cloud && [[ "$selected" == yes ]] \
        && [[ "$presses" == 1 && "$(cat "$OUT/cloud.txt")" == $'AXHeading\tCloud sync' ]]; then
        ok "Cloud sync selector activates and reacquires the Cloud panel"
    else
        bad "Cloud sync selector activates and reacquires the Cloud panel"
    fi
    unset -f mac_ax
    OUT="$saved_out"
}

sidecar_launch_self_test() { # <tmp-dir>
    local sidecar="$1/copypaste-daemon" configured="" unconfigured=""
    printf '#!/bin/sh\n' > "$sidecar"
    chmod +x "$sidecar"
    open() { printf 'OPEN'; printf '\t%s' "$@"; printf '\n'; }
    env() { printf 'ENV'; printf '\t%s' "$@"; printf '\n'; }

    APP="/Applications/CopyPaste.app"
    STUB_PORT=47800
    DAEMON_SIDECAR=""
    if open_cloud_evidence_app configured >/dev/null 2>&1; then
        bad "configured launch fails closed without a sidecar"
    else
        ok "configured launch fails closed without a sidecar"
    fi
    COPYPASTE_CLOUD_EVIDENCE_DAEMON="$1/missing-daemon"
    if ensure_cloud_evidence_daemon; then
        bad "a missing required sidecar fails closed"
    else
        ok "a missing required sidecar fails closed"
    fi
    unset COPYPASTE_CLOUD_EVIDENCE_DAEMON
    DAEMON_SIDECAR="relative/copypaste-daemon"
    if open_cloud_evidence_app configured >/dev/null 2>&1; then
        bad "configured launch rejects a relative daemon override"
    else
        ok "configured launch rejects a relative daemon override"
    fi

    DAEMON_SIDECAR="$sidecar"
    configured="$(open_cloud_evidence_app configured)"
    if [[ "$configured" == OPEN$'\t'* \
        && "$configured" == *$'\t--env\tCOPYPASTE_CLOUD_URL=http://127.0.0.1:47800'* \
        && "$configured" == *$'\t--env\tCOPYPASTE_CLOUD_ANON_KEY=native-evidence'* \
        && "$configured" == *$'\t--env\tCOPYPASTE_DAEMON_BIN='"$sidecar"* \
        && "$configured" != ENV* ]]; then
        ok "configured launch passes an absolute sidecar and loopback endpoint"
    else
        bad "configured launch passes an absolute sidecar and loopback endpoint"
    fi
    unconfigured="$(open_cloud_evidence_app unconfigured)"
    if [[ "$unconfigured" == ENV$'\t-u\tCOPYPASTE_CLOUD_URL\t-u\tCOPYPASTE_CLOUD_ANON_KEY\t-u\tCOPYPASTE_DAEMON_BIN\topen\t'* \
        && "$unconfigured" == *$'\t--env\tCOPYPASTE_EVIDENCE_AX=1' \
        && "$unconfigured" != *COPYPASTE_DAEMON_BIN=/* \
        && "$unconfigured" != *COPYPASTE_CLOUD_URL=http* ]]; then
        ok "unconfigured launch uses the bundled daemon and no override"
    else
        bad "unconfigured launch uses the bundled daemon and no override"
    fi
    unset -f open env
}

unconfigured_latency_self_test() { # <tmp-dir>
    local saved_pass="$PASS" saved_fail="$FAIL" t=10000
    local recorded="" recorded_scenario="" recorded_budget="" labels=""
    PASS=0
    FAIL=0
    now_ms() { printf '%s\n' "$t"; }
    launch_app() { t=$((t + 8000)); return 0; }
    open_cloud() { t=$((t + 4000)); return 0; }
    expect_label() { labels+="$1"$'\n'; t=$((t + 10)); return 0; }
    capture_state() { return 0; }
    cloud_latency_record() {
        recorded_scenario="$2"
        recorded="$3"
        recorded_budget="$4"
        (( $3 <= $4 ))
    }
    LATENCIES="$1/latency-probe.tsv"
    : > "$LATENCIES"
    unconfigured_scenario >/dev/null
    unset -f now_ms launch_app open_cloud expect_label capture_state cloud_latency_record
    PASS="$saved_pass"
    FAIL="$saved_fail"
    if [[ "$recorded_scenario" == unconfigured-status \
        && "$recorded_budget" == 30000 \
        && -n "$recorded" && "$recorded" -lt 1000 \
        && "$labels" == $'Not configured\nCloud server configuration\nServer URL\nPublishable key\nConfigure\n' ]]; then
        ok "unconfigured status latency excludes launch delay"
    else
        bad "unconfigured status latency excludes launch delay" \
            "scenario=${recorded_scenario:-unset} budget=${recorded_budget:-unset} ms=${recorded:-unset}"
    fi
}

configured_assertions_self_test() {
    local body
    body="$(type configured_scenario 2>/dev/null)"
    if [[ "$body" == *"expect_label \"Signed out\""* \
        && "$body" == *"expect_label \"Cloud account sign in\""* \
        && "$body" == *"expect_label \"Email\""* \
        && "$body" == *"expect_label \"Password\""* \
        && "$body" == *"expect_label \"Sync passphrase\""* \
        && "$body" == *"expect_label \"Connected\""* \
        && "$body" == *"expect_label \"native@example.test\""* \
        && "$body" == *"expect_label \"skipped\""* \
        && "$body" == *"expect_label \"The last cloud sync failed\""* \
        && "$body" == *"expect_label \"Signed out\" \"\$OUT/signed-out-again.txt\""* \
        && "$body" == *'sign-in "$elapsed" 30000'* \
        && "$body" == *'sync-with-skips "$elapsed" 30000'* \
        && "$body" == *'offline-error "$elapsed" 60000'* ]]; then
        ok "configured scenario keeps exact lifecycle assertions and timeouts"
    else
        bad "configured scenario keeps exact lifecycle assertions and timeouts"
    fi
}

if [[ "${1:-}" == "--self-test" ]]; then
    SELF_TEST_TMP="$(mktemp -d)"
    trap 'rm -rf "$SELF_TEST_TMP"' EXIT
    preference_seed_self_test "$SELF_TEST_TMP"
    mac_ui_self_test "$SELF_TEST_TMP"
    cloud_panel_selector_self_test "$SELF_TEST_TMP"
    cloud_evidence_self_test "$SELF_TEST_TMP"
    sidecar_launch_self_test "$SELF_TEST_TMP"
    unconfigured_latency_self_test "$SELF_TEST_TMP"
    configured_assertions_self_test
    cloud_evidence_summary macOS
    [[ $FAIL -eq 0 ]]
    exit
fi

[[ "$(uname -s)" == Darwin ]] || { echo "ERROR: must run on macOS" >&2; exit 2; }
BINARY="$(mac_evidence_executable "$APP")" || exit 2
mkdir -p "$OUT"
: > "$LATENCIES"
trap cleanup EXIT
unconfigured_scenario
configured_scenario
cloud_latency_write "$LATENCIES" "$OUT/latency.json" macos
cloud_evidence_summary macOS
[[ $FAIL -eq 0 ]]
