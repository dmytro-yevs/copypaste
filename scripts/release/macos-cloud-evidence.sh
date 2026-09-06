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
ISOLATION_ROOT=""
ISOLATION_DATA_DIR=""
ISOLATION_SOCKET=""
ISOLATION_KEYCHAIN=""
ISOLATION_PASSWORD=""
ISOLATION_SAVED_DEFAULT=""
ISOLATION_SAVED_SEARCH=""
ISOLATION_SETUP_REASON=""
ISOLATION_ACTIVE=0
DARWIN_SUN_PATH=104
ISOLATION_PROBE_SERVICE="copypaste-cloud-evidence-probe"
ISOLATION_PROBE_ACCOUNT="probe"

now_ms() { python3 -c 'import time; print(time.time_ns() // 1000000)'; }

default_cloud_shutdown() {
    env -u COPYPASTE_DATA_DIR -u COPYPASTE_SOCKET \
        "$APP/Contents/MacOS/copypaste" shutdown >/dev/null 2>&1 || true
}

cleanup() {
    [[ -n "$APP_PID" ]] && kill "$APP_PID" 2>/dev/null || true
    [[ -n "$STUB_PID" ]] && kill "$STUB_PID" 2>/dev/null || true
    if [[ -n "${ISOLATION_SOCKET:-}" ]]; then
        COPYPASTE_SOCKET="$ISOLATION_SOCKET" \
            COPYPASTE_DATA_DIR="${ISOLATION_DATA_DIR:-}" \
            "$APP/Contents/MacOS/copypaste" shutdown >/dev/null 2>&1 || true
    fi
    default_cloud_shutdown
    configured_isolation_cleanup
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
        [[ "${ISOLATION_DATA_DIR:-}" == /* && "${ISOLATION_SOCKET:-}" == /* ]] || return 1
        open -n -a "$APP" \
            --env "COPYPASTE_EVIDENCE_AX=1" \
            --env "COPYPASTE_CLOUD_URL=http://127.0.0.1:$STUB_PORT" \
            --env "COPYPASTE_CLOUD_ANON_KEY=native-evidence" \
            --env "COPYPASTE_DAEMON_BIN=$DAEMON_SIDECAR" \
            --env "COPYPASTE_DATA_DIR=$ISOLATION_DATA_DIR" \
            --env "COPYPASTE_SOCKET=$ISOLATION_SOCKET"
    else
        env -u COPYPASTE_CLOUD_URL -u COPYPASTE_CLOUD_ANON_KEY -u COPYPASTE_DAEMON_BIN \
            -u COPYPASTE_DATA_DIR -u COPYPASTE_SOCKET \
            open -n -a "$APP" \
            --env "COPYPASTE_EVIDENCE_AX=1"
    fi
}

launch_app() { # <unconfigured|configured>
    mac_stop_executable "$BINARY" || return 1
    default_cloud_shutdown
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

PROBE_CLI_SECS=5
PROBE_LOG_LINES=40
PROBE_KEYCHAIN_SERVICE="com.copypaste.daemon"
PROBE_KEYCHAIN_ACCOUNT="device-secret-key"

probe_run_bounded() { # <seconds> <argv...>
    local secs="$1"
    shift
    python3 -c '
import subprocess, sys
try:
    result = subprocess.run(sys.argv[2:], timeout=float(sys.argv[1]), capture_output=True, text=True)
except subprocess.TimeoutExpired as error:
    sys.stdout.write(error.stdout or "")
    sys.stderr.write(error.stderr or "")
    raise SystemExit(124)
sys.stdout.write(result.stdout or "")
sys.stderr.write(result.stderr or "")
raise SystemExit(result.returncode)
' "$secs" "$@"
}

probe_redact_text() {
    python3 -c '
import re, sys
text = sys.stdin.read()
text = re.sub(r"https?://\S+", "<url>", text)
for secret in (
    "native-evidence",
    "stub-password",
    "native@example.test",
    "COPYPASTE_CLOUD_ANON_KEY",
):
    text = text.replace(secret, "<redacted>")
text = re.sub(r"(?im)^.*\bpassword:.*$", "password: <redacted>", text)
text = re.sub(r"(?i)(?:file://)?(?:/Users|/home)/[^\n\"]+", "<path>", text)
text = re.sub(r"(?i)~(?:/[^\n\"]*)?", "<path>", text)
text = re.sub(r"(?i)/(?:var/folders|private/var|tmp|Library)[^\n\"]*", "<path>", text)
text = re.sub(r"(?i)\S+\.sock", "<socket>", text)
text = re.sub(r"(?i)[A-Za-z]:\\\S+", "<path>", text)
text = re.sub(r"(?<![A-Za-z:<])(/\S+)", "<path>", text)
sys.stdout.write(text)
'
}

probe_classify_cli() { # <text>
    case "$1" in
        *"the key store could not be read"*) printf 'KEY_LOCKED\n' ;;
        *"this device's key is present and cannot be used"*) printf 'UNUSABLE\n' ;;
        *"the cloud endpoint must use HTTPS"*|*"PlaintextEndpoint"*) printf 'PLAINTEXT\n' ;;
        *"cannot reach the CopyPaste daemon"*) printf 'UNREACHABLE\n' ;;
        *"configured    yes"*|*"configured   yes"*|*"configured  yes"*) printf 'configured\n' ;;
        *) printf 'unknown\n' ;;
    esac
}

probe_classify_cli_pair() { # <status-text> <cloud-text>
    local status_class cloud_class class
    status_class="$(probe_classify_cli "$1")"
    cloud_class="$(probe_classify_cli "$2")"
    for class in KEY_LOCKED UNUSABLE PLAINTEXT UNREACHABLE configured; do
        if [[ "$status_class" == "$class" || "$cloud_class" == "$class" ]]; then
            printf '%s\n' "$class"
            return
        fi
    done
    printf 'unknown\n'
}

probe_decide() { # <path_class> <cli_class> <keychain_item> <db>
    local path_class="$1" cli_class="$2" item="$3" db="$4"
    if [[ "$item" == unknown ]]; then
        printf 'fail\tunknown-keychain\n'
        return
    fi
    if [[ "$item" == absent && "$db" == present ]]; then
        printf 'fail\tabsent-item-existing-db\n'
        return
    fi
    if [[ "$cli_class" == PLAINTEXT ]]; then
        printf 'fail\tplaintext-endpoint\n'
        return
    fi
    if [[ "$path_class" == bundled ]]; then
        printf 'fail\tbundled-daemon\n'
        return
    fi
    if [[ "$path_class" == none ]]; then
        printf 'fail\tno-daemon\n'
        return
    fi
    if [[ "$path_class" == sidecar && "$cli_class" == KEY_LOCKED ]]; then
        printf 'fail\tsidecar-key-locked\n'
        return
    fi
    if [[ "$path_class" == sidecar && "$cli_class" == UNUSABLE ]]; then
        printf 'fail\tsidecar-key-unusable\n'
        return
    fi
    if [[ "$path_class" == sidecar && "$cli_class" == configured && (
            "$item" == present || ( "$item" == absent && "$db" == absent )
        ) ]]; then
        printf 'pass\tsidecar-configured\n'
        return
    fi
    printf 'fail\tunclassified\n'
}

probe_realpath() { # <path>
    python3 -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' "$1" 2>/dev/null || true
}

probe_codesign_identity() { # <path>
    probe_run_bounded 5 codesign -dvvv "$1" 2>&1 \
        | grep -E '^(Identifier|Format|Signature|Authority|TeamIdentifier|CDHash)=' \
        | probe_redact_text || true
}

probe_keychain_item() {
    local raw
    local -a kc=()
    [[ -n "${ISOLATION_KEYCHAIN:-}" ]] && kc=("$ISOLATION_KEYCHAIN")
    raw="$(probe_run_bounded 5 security find-generic-password -s "$PROBE_KEYCHAIN_SERVICE" -a "$PROBE_KEYCHAIN_ACCOUNT" "${kc[@]}" 2>&1)" || true
    case "$raw" in
        *"could not be found"*) printf 'absent\n' ;;
        *)
            if printf '%s' "$raw" | grep -Eq 'svce|acct|genp|com.copypaste.daemon'; then
                printf 'present\n'
            else
                printf 'unknown\n'
            fi
            ;;
    esac
}

probe_db_state() {
    local db="${ISOLATION_DATA_DIR:-$HOME/Library/Application Support/com.copypaste.CopyPaste}/copypaste-v2.db"
    if [[ -f "$db" ]]; then
        printf 'present\n'
    else
        printf 'absent\n'
    fi
}

probe_proc_exe() { # <pid>
    probe_run_bounded 3 lsof -a -nP -p "$1" -d txt -Fn 2>/dev/null \
        | python3 -c '
import sys
for line in sys.stdin:
    if line.startswith("n/") or line.startswith("n~"):
        sys.stdout.write(line[1:])
        break
' || true
}

probe_live_daemon() { # writes path_class pid ppid exe_class argv socket_owner
    local bundled sidecar cand_pid="" pid="" ppid="" exe="" live_real="" side_real="" bund_real=""
    local path_class=none socket_owner=unknown argv="" cand_class="" cand_ppid="" cand_argv=""
    local sock="${ISOLATION_SOCKET:-$HOME/Library/Application Support/com.copypaste.CopyPaste/daemon.sock}"
    bundled="$APP/Contents/MacOS/copypaste-daemon"
    sidecar="${DAEMON_SIDECAR:-}"
    [[ -n "$sidecar" ]] && side_real="$(probe_realpath "$sidecar")"
    [[ -x "$bundled" ]] && bund_real="$(probe_realpath "$bundled")"
    while read -r cand_pid; do
        [[ -n "$cand_pid" ]] || continue
        exe="$(probe_proc_exe "$cand_pid")"
        live_real="$(probe_realpath "$exe")"
        cand_ppid="$(probe_run_bounded 3 ps -p "$cand_pid" -o ppid= 2>/dev/null | tr -d '[:space:]')"
        cand_argv="$(probe_run_bounded 3 ps -p "$cand_pid" -www -o command= 2>/dev/null || true)"
        cand_class=other
        if [[ -n "$side_real" && -n "$live_real" && "$live_real" == "$side_real" ]]; then
            cand_class=sidecar
        elif [[ -n "$bund_real" && -n "$live_real" && "$live_real" == "$bund_real" ]]; then
            cand_class=bundled
        fi
        if [[ "$cand_class" == sidecar || "$path_class" == none || "$path_class" == other ]]; then
            path_class="$cand_class"
            pid="$cand_pid"
            ppid="$cand_ppid"
            argv="$cand_argv"
        fi
        [[ "$path_class" == sidecar ]] && break
    done < <(probe_run_bounded 3 pgrep -x copypaste-daemon 2>/dev/null || true)
    if [[ -S "$sock" ]]; then
        if [[ "$(stat -f '%u' "$sock" 2>/dev/null || true)" == "$(id -u)" ]]; then
            socket_owner=same-user
        else
            socket_owner=other
        fi
    elif [[ "$path_class" == none ]]; then
        socket_owner=absent
    fi
    printf '%s\n' "$path_class" "${pid:-}" "${ppid:-}" "$( [[ "$path_class" == none ]] && printf 'none' || printf '%s' "$path_class" )"
    printf '%s\n' "$(printf '%s' "$argv" | probe_redact_text)"
    printf '%s\n' "$socket_owner"
}

probe_cli_pair() {
    local cli="$APP/Contents/MacOS/copypaste"
    local status_text cloud_text
    local -a prefix=()
    if [[ -n "${ISOLATION_SOCKET:-}" && -n "${ISOLATION_DATA_DIR:-}" ]]; then
        prefix=(env COPYPASTE_SOCKET="$ISOLATION_SOCKET" COPYPASTE_DATA_DIR="$ISOLATION_DATA_DIR")
    fi
    status_text="$(probe_run_bounded "$PROBE_CLI_SECS" "${prefix[@]}" "$cli" status 2>&1 || true)"
    cloud_text="$(probe_run_bounded "$PROBE_CLI_SECS" "${prefix[@]}" "$cli" cloud status 2>&1 || true)"
    printf '%s\n' "$(probe_classify_cli_pair "$status_text" "$cloud_text")"
    printf '%s\n' "$(printf '%s\n%s\n' "$status_text" "$cloud_text" | probe_redact_text)"
}

probe_runtime_log() {
    local logdir="${ISOLATION_DATA_DIR:-$HOME/Library/Application Support/com.copypaste.CopyPaste}/logs"
    local latest=""
    latest="$(find "$logdir" -maxdepth 1 -name 'daemon*.log' -type f 2>/dev/null | sort | tail -n 1 || true)"
    if [[ -z "$latest" || ! -f "$latest" ]]; then
        return 0
    fi
    tail -n "$PROBE_LOG_LINES" "$latest" 2>/dev/null | probe_redact_text || true
}

probe_write() { # <file> <text>
    printf '%s\n' "$2" | probe_redact_text > "$1"
}

configured_isolation_owned_root() { # <path>
    local path="$1"
    [[ "$path" == /* ]] || return 1
    [[ "$path" != *..* ]] || return 1
    [[ "$path" != *login.keychain* ]] || return 1
    [[ "$path" != *"/Library/Keychains"* ]] || return 1
    [[ "$path" != *"Application Support"* ]] || return 1
    [[ "$(basename "$path")" == cpc.* ]]
}

configured_isolation_owned_leaf() { # <path>
    local path="$1"
    [[ "$path" == /* ]] || return 1
    [[ "$path" != *..* ]] || return 1
    [[ "$path" != *login.keychain* ]] || return 1
    [[ "$path" != *"/Library/Keychains"* ]] || return 1
    [[ "$path" != *"Application Support"* ]] || return 1
    [[ "$path" == */cpc.*/* ]]
}

configured_isolation_owned_keychain() { # <path>
    configured_isolation_owned_leaf "$1" || return 1
    [[ "$(basename "$1")" == k.keychain-db ]]
}

configured_isolation_socket_fits() { # <socket>
    local socket="$1" staged="${1}.new/s"
    [[ "$socket" == /* ]] || return 1
    [[ "$socket" != *..* ]] || return 1
    (( ${#socket} < DARWIN_SUN_PATH && ${#staged} < DARWIN_SUN_PATH ))
}

configured_isolation_parse_keychain_lines() {
    sed -e 's/^[[:space:]]*"//' -e 's/"[[:space:]]*$//'
}

# Run 34010217484: default-keychain -d user can return errSecNoDefaultKeychain
# (-25307) while login remains on the search list. Keep that stderr visible.
configured_isolation_current_default() {
    local raw line
    raw="$(security default-keychain -d user)" || return $?
    line="$(printf '%s\n' "$raw" | configured_isolation_parse_keychain_lines)"
    line="${line%%$'\n'*}"
    [[ -n "$line" ]] || return 1
    printf '%s\n' "$line"
}

configured_isolation_current_search() {
    local raw
    raw="$(security list-keychains -d user)" || return $?
    printf '%s\n' "$raw" | configured_isolation_parse_keychain_lines
}

configured_isolation_choose_restore_default() { # <search-lines>
    local search="$1" login="$HOME/Library/Keychains/login.keychain-db" line found=0
    while IFS= read -r line; do
        if [[ "$line" == "$login" ]]; then
            found=1
            break
        fi
    done <<EOF
$search
EOF
    if ((found)) && [[ -e "$login" ]]; then
        printf '%s\n' "$login"
        return 0
    fi
    while IFS= read -r line; do
        [[ -n "$line" ]] || continue
        [[ "$line" == /* ]] || continue
        [[ "$line" != *..* ]] || continue
        [[ -e "$line" ]] || continue
        printf '%s\n' "$line"
        return 0
    done <<EOF
$search
EOF
    return 1
}

configured_isolation_restore() {
    local default="${ISOLATION_SAVED_DEFAULT:-}" line
    local -a search=()
    if [[ -n "$default" ]]; then
        security default-keychain -d user -s "$default" || true
    fi
    if [[ -n "${ISOLATION_SAVED_SEARCH:-}" ]]; then
        while IFS= read -r line; do
            [[ -n "$line" ]] && search+=("$line")
        done <<EOF
$ISOLATION_SAVED_SEARCH
EOF
        if ((${#search[@]} > 0)); then
            security list-keychains -d user -s "${search[@]}" || true
        fi
    fi
}

configured_isolation_delete_owned() {
    local keychain="${ISOLATION_KEYCHAIN:-}" root="${ISOLATION_ROOT:-}"
    if [[ -n "$keychain" ]] && configured_isolation_owned_keychain "$keychain"; then
        security delete-keychain "$keychain" >/dev/null 2>&1 || true
    fi
    if [[ -n "$root" ]] && configured_isolation_owned_root "$root"; then
        rm -rf "$root" || true
    fi
}

configured_isolation_cleanup() {
    # Restore first so a failed delete cannot leave the throwaway as default.
    configured_isolation_restore
    configured_isolation_delete_owned || true
    ISOLATION_PASSWORD=""
    ISOLATION_ROOT=""
    ISOLATION_DATA_DIR=""
    ISOLATION_SOCKET=""
    ISOLATION_KEYCHAIN=""
    ISOLATION_SAVED_DEFAULT=""
    ISOLATION_SAVED_SEARCH=""
    ISOLATION_ACTIVE=0
}

configured_isolation_mint_keychain() {
    local keychain="$ISOLATION_ROOT/k.keychain-db" xtrace_restore=""
    [[ -o xtrace ]] && xtrace_restore="set -x"
    { set +x; } 2>/dev/null
    ISOLATION_PASSWORD="$(openssl rand -base64 24)" || {
        eval "$xtrace_restore"
        return 1
    }
    if ! security create-keychain -p "$ISOLATION_PASSWORD" "$keychain"; then
        ISOLATION_PASSWORD=""
        eval "$xtrace_restore"
        return 1
    fi
    ISOLATION_KEYCHAIN="$keychain"
    if ! security set-keychain-settings -lut 21600 "$ISOLATION_KEYCHAIN" \
        || ! security unlock-keychain -p "$ISOLATION_PASSWORD" "$ISOLATION_KEYCHAIN"; then
        ISOLATION_PASSWORD=""
        eval "$xtrace_restore"
        return 1
    fi
    eval "$xtrace_restore"
}

configured_isolation_roundtrip() {
    local found
    security add-generic-password \
        -s "$ISOLATION_PROBE_SERVICE" -a "$ISOLATION_PROBE_ACCOUNT" \
        -w "cloud-evidence-roundtrip" "$ISOLATION_KEYCHAIN" || return 1
    found="$(security find-generic-password \
        -s "$ISOLATION_PROBE_SERVICE" -a "$ISOLATION_PROBE_ACCOUNT" \
        -w "$ISOLATION_KEYCHAIN")" || return 1
    [[ "$found" == "cloud-evidence-roundtrip" ]] || return 1
    security delete-generic-password \
        -s "$ISOLATION_PROBE_SERVICE" -a "$ISOLATION_PROBE_ACCOUNT" \
        "$ISOLATION_KEYCHAIN" >/dev/null
}

configured_isolation_setup() {
    # Isolation mutates the user keychain search list. Refuse outside CI.
    local root data socket saved_default saved_search
    ISOLATION_SETUP_REASON="ci"
    [[ -n "${GITHUB_ACTIONS:-}" ]] || return 1
    ISOLATION_SETUP_REASON="root"
    root="$(mktemp -d /tmp/cpc.XXXXXX)" || return 1
    if ! configured_isolation_owned_root "$root"; then
        rm -rf "$root"
        return 1
    fi
    data="$root/d"
    mkdir -p "$data" || { rm -rf "$root"; return 1; }
    socket="$data/daemon.sock"
    if ! configured_isolation_socket_fits "$socket"; then
        rm -rf "$root"
        return 1
    fi
    ISOLATION_SETUP_REASON="search-list"
    saved_search="$(configured_isolation_current_search)" || { rm -rf "$root"; return 1; }
    if [[ -z "$saved_search" ]]; then
        rm -rf "$root"
        return 1
    fi
    ISOLATION_SETUP_REASON="default-keychain-read"
    saved_default="$(configured_isolation_current_default)" || saved_default=""
    if [[ -z "$saved_default" ]]; then
        saved_default="$(configured_isolation_choose_restore_default "$saved_search")" || {
            rm -rf "$root"
            return 1
        }
    fi
    ISOLATION_ROOT="$root"
    ISOLATION_DATA_DIR="$data"
    ISOLATION_SOCKET="$socket"
    ISOLATION_SAVED_DEFAULT="$saved_default"
    ISOLATION_SAVED_SEARCH="$saved_search"
    ISOLATION_SETUP_REASON="mint"
    if ! configured_isolation_mint_keychain \
        || ! configured_isolation_owned_keychain "$ISOLATION_KEYCHAIN"; then
        configured_isolation_cleanup
        return 1
    fi
    # Run 34007760276: login stayed on the search list, so the sidecar
    # classified KEY_LOCKED against the I-10 item. Isolation uses the
    # throwaway keychain alone and never names that item.
    ISOLATION_SETUP_REASON="switch"
    if ! security list-keychains -d user -s "$ISOLATION_KEYCHAIN" \
        || ! security default-keychain -d user -s "$ISOLATION_KEYCHAIN"; then
        configured_isolation_cleanup
        return 1
    fi
    ISOLATION_SETUP_REASON="roundtrip"
    if ! configured_isolation_roundtrip; then
        configured_isolation_cleanup
        return 1
    fi
    ISOLATION_SETUP_REASON=""
    ISOLATION_ACTIVE=1
}

# Run 34004833224: configured AX waits burned the job after status could not load.
probe_configured_sidecar_path() {
    local dir="$OUT/sidecar-probe"
    local path_class=none cli_class=unknown item=unknown db=absent
    local pid="" ppid="" exe_class="" argv="" socket_owner=unknown
    local side_ident="" bund_ident="" verdict reason
    local status_blob=""
    if [[ "${ISOLATION_ACTIVE:-0}" != 1 ]] \
        || ! configured_isolation_owned_root "$ISOLATION_ROOT" \
        || ! configured_isolation_owned_keychain "$ISOLATION_KEYCHAIN" \
        || ! configured_isolation_owned_leaf "$ISOLATION_DATA_DIR" \
        || ! configured_isolation_socket_fits "$ISOLATION_SOCKET"; then
        bad "the configured sidecar path is live and ready" "isolation-inactive"
        return 1
    fi
    mkdir -p "$dir"
    {
        read -r path_class
        read -r pid
        read -r ppid
        read -r exe_class
        read -r argv
        read -r socket_owner
    } < <(probe_live_daemon)
    item="$(probe_keychain_item)"
    db="$(probe_db_state)"
    {
        read -r cli_class
        status_blob="$(cat)"
    } < <(probe_cli_pair)
    if [[ "${DAEMON_SIDECAR:-}" == /* ]]; then
        side_ident="$(probe_codesign_identity "$DAEMON_SIDECAR")"
    else
        side_ident=""
    fi
    bund_ident="$(probe_codesign_identity "$APP/Contents/MacOS/copypaste-daemon")"
    read -r verdict reason < <(probe_decide "$path_class" "$cli_class" "$item" "$db")
    probe_write "$dir/summary.txt" "$(printf 'path_class=%s\ncli_class=%s\nkeychain_item=%s\ndb=%s\npid=%s\nppid=%s\nexe_class=%s\nsocket_owner=%s\nverdict=%s\nreason=%s\n' \
        "$path_class" "$cli_class" "$item" "$db" "${pid:-none}" "${ppid:-none}" "${exe_class:-none}" "$socket_owner" "$verdict" "$reason")"
    probe_write "$dir/identity.txt" "$(printf 'sidecar_override=%s\nlive_path_class=%s\n\n# sidecar\n%s\n\n# bundled\n%s\n' \
        "$( [[ "${DAEMON_SIDECAR:-}" == /* ]] && printf present || printf missing )" \
        "$path_class" "$side_ident" "$bund_ident")"
    probe_write "$dir/keychain.txt" "$(printf 'service=%s\naccount=%s\nitem=%s\nmethod=attribute-only\n' \
        "$PROBE_KEYCHAIN_SERVICE" "$PROBE_KEYCHAIN_ACCOUNT" "$item")"
    probe_write "$dir/process.txt" "$(printf 'pid=%s\nppid=%s\nexe_class=%s\nsocket_owner=%s\nargv=%s\n' \
        "${pid:-none}" "${ppid:-none}" "${exe_class:-none}" "$socket_owner" "$argv")"
    probe_write "$dir/cli-class.txt" "$(printf 'class=%s\n%s\n' "$cli_class" "$status_blob")"
    probe_runtime_log > "$dir/runtime-log.txt" || true
    if [[ "$verdict" == pass ]]; then
        ok "the configured sidecar path is live and ready"
        return 0
    fi
    bad "the configured sidecar path is live and ready" "$reason"
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
    configured_isolation_setup || { bad "configured evidence isolation is armed" "${ISOLATION_SETUP_REASON:-}"; return; }
    launch_app configured || { bad "the configured app exposes accessibility state"; return; }
    probe_configured_sidecar_path || return
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

    mac_set_exact_role "Email" "AXTextField" "native@example.test" >/dev/null || bad "email can be entered"
    mac_set_exact_role "Password" "AXTextField" "stub-password" >/dev/null || bad "password can be entered"
    mac_set_exact_role "Sync passphrase" "AXTextField" "native-evidence" >/dev/null || bad "passphrase can be entered"
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
    ISOLATION_DATA_DIR=""
    ISOLATION_SOCKET=""
    if open_cloud_evidence_app configured >/dev/null 2>&1; then
        bad "configured launch fails closed without isolated paths"
    else
        ok "configured launch fails closed without isolated paths"
    fi
    ISOLATION_DATA_DIR="$1/cpc.fixture/d"
    ISOLATION_SOCKET="$1/cpc.fixture/d/daemon.sock"
    configured="$(open_cloud_evidence_app configured)"
    if [[ "$configured" == OPEN$'\t'* \
        && "$configured" == *$'\t--env\tCOPYPASTE_CLOUD_URL=http://127.0.0.1:47800'* \
        && "$configured" == *$'\t--env\tCOPYPASTE_CLOUD_ANON_KEY=native-evidence'* \
        && "$configured" == *$'\t--env\tCOPYPASTE_DAEMON_BIN='"$sidecar"* \
        && "$configured" == *$'\t--env\tCOPYPASTE_DATA_DIR='"$ISOLATION_DATA_DIR"* \
        && "$configured" == *$'\t--env\tCOPYPASTE_SOCKET='"$ISOLATION_SOCKET"* \
        && "$configured" != ENV* ]]; then
        ok "configured launch passes an absolute sidecar and loopback endpoint"
    else
        bad "configured launch passes an absolute sidecar and loopback endpoint"
    fi
    unconfigured="$(open_cloud_evidence_app unconfigured)"
    if [[ "$unconfigured" == ENV$'\t-u\tCOPYPASTE_CLOUD_URL\t-u\tCOPYPASTE_CLOUD_ANON_KEY\t-u\tCOPYPASTE_DAEMON_BIN\t-u\tCOPYPASTE_DATA_DIR\t-u\tCOPYPASTE_SOCKET\topen\t'* \
        && "$unconfigured" == *$'\t--env\tCOPYPASTE_EVIDENCE_AX=1' \
        && "$unconfigured" != *COPYPASTE_DAEMON_BIN=/* \
        && "$unconfigured" != *COPYPASTE_CLOUD_URL=http* \
        && "$unconfigured" != *COPYPASTE_DATA_DIR=/* \
        && "$unconfigured" != *COPYPASTE_SOCKET=/* ]]; then
        ok "unconfigured launch uses the bundled daemon and no override"
    else
        bad "unconfigured launch uses the bundled daemon and no override"
    fi
    ISOLATION_DATA_DIR=""
    ISOLATION_SOCKET=""
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
        && "$body" == *'offline-error "$elapsed" 60000'* \
        && "$body" == *"configured_isolation_setup"* \
        && "$body" == *'bad "configured evidence isolation is armed" "${ISOLATION_SETUP_REASON:-}"'* \
        && "$body" == *"configured_isolation_setup"*"launch_app configured"* \
        && "$body" == *"probe_configured_sidecar_path || return"* \
        && "$body" == *"probe_configured_sidecar_path"*"open_cloud"* \
        && "$body" == *"probe_configured_sidecar_path"*"expect_label \"Signed out\""* ]]; then
        ok "configured scenario keeps exact lifecycle assertions and timeouts"
    else
        bad "configured scenario keeps exact lifecycle assertions and timeouts"
    fi
    if [[ "$body" == *'mac_set_exact_role "Email" "AXTextField" "native@example.test"'* \
        && "$body" == *'mac_set_exact_role "Password" "AXTextField" "stub-password"'* \
        && "$body" == *'mac_set_exact_role "Sync passphrase" "AXTextField" "native-evidence"'* \
        && "$body" != *'mac_ax set '* \
        && "$body" == *'mac_ax press "Sign in"'* \
        && "$body" == *'mac_ax press "Sync cloud now"'* \
        && "$body" == *'mac_ax press "Sign out"'* ]]; then
        ok "configured scenario fills unique exact AXTextField values only"
    else
        bad "configured scenario fills unique exact AXTextField values only"
    fi
}

probe_decision_self_test() {
    local got
    got="$(probe_decide sidecar KEY_LOCKED present present)"
    [[ "$got" == $'fail\tsidecar-key-locked' ]] \
        && ok "sidecar plus KEY_LOCKED fails closed" \
        || bad "sidecar plus KEY_LOCKED fails closed"
    got="$(probe_decide sidecar UNUSABLE present present)"
    [[ "$got" == $'fail\tsidecar-key-unusable' ]] \
        && ok "sidecar plus UNUSABLE fails closed" \
        || bad "sidecar plus UNUSABLE fails closed"
    got="$(probe_decide sidecar configured present present)"
    [[ "$got" == $'pass\tsidecar-configured' ]] \
        && ok "sidecar plus configured lets the probe pass" \
        || bad "sidecar plus configured lets the probe pass"
    got="$(probe_decide bundled configured present present)"
    [[ "$got" == $'fail\tbundled-daemon' ]] \
        && ok "a bundled live daemon fails closed" \
        || bad "a bundled live daemon fails closed"
    got="$(probe_decide none UNREACHABLE present present)"
    [[ "$got" == $'fail\tno-daemon' ]] \
        && ok "no live daemon fails closed" \
        || bad "no live daemon fails closed"
    got="$(probe_decide sidecar PLAINTEXT present present)"
    [[ "$got" == $'fail\tplaintext-endpoint' ]] \
        && ok "PlaintextEndpoint fails closed" \
        || bad "PlaintextEndpoint fails closed"
    got="$(probe_decide bundled PLAINTEXT present present)"
    [[ "$got" == $'fail\tplaintext-endpoint' ]] \
        && ok "bundled PlaintextEndpoint fails closed" \
        || bad "bundled PlaintextEndpoint fails closed"
    got="$(probe_decide sidecar configured absent present)"
    [[ "$got" == $'fail\tabsent-item-existing-db' ]] \
        && ok "an absent keychain item plus an existing database fails closed" \
        || bad "an absent keychain item plus an existing database fails closed"
    got="$(probe_decide sidecar unknown present present)"
    [[ "$got" == $'fail\tunclassified' ]] \
        && ok "an unclassified sidecar path fails closed" \
        || bad "an unclassified sidecar path fails closed"
    got="$(probe_decide sidecar configured absent absent)"
    [[ "$got" == $'pass\tsidecar-configured' ]] \
        && ok "an absent keychain item without a database can still pass" \
        || bad "an absent keychain item without a database can still pass"
    got="$(probe_decide sidecar configured unknown present)"
    [[ "$got" == $'fail\tunknown-keychain' ]] \
        && ok "an unknown keychain item plus an existing database fails closed" \
        || bad "an unknown keychain item plus an existing database fails closed"
    got="$(probe_decide sidecar configured unknown absent)"
    [[ "$got" == $'fail\tunknown-keychain' ]] \
        && ok "an unknown keychain item without a database fails closed" \
        || bad "an unknown keychain item without a database fails closed"
}

probe_classify_self_test() {
    local got
    got="$(probe_classify_cli "the key store could not be read, so this history could not be unlocked; it is worth trying again once the key store is available")"
    [[ "$got" == KEY_LOCKED ]] \
        && ok "CLI KEY_LOCKED maps to a fixed refusal class" \
        || bad "CLI KEY_LOCKED maps to a fixed refusal class"
    got="$(probe_classify_cli "this device's key is present and cannot be used, so the history encrypted with it cannot be read by anything; trying again will not change that")"
    [[ "$got" == UNUSABLE ]] \
        && ok "CLI UNUSABLE maps to a fixed refusal class" \
        || bad "CLI UNUSABLE maps to a fixed refusal class"
    got="$(probe_classify_cli "cannot reach the CopyPaste daemon. Start it with \`copypaste-daemon\`, then run this command again.")"
    [[ "$got" == UNREACHABLE ]] \
        && ok "CLI unreachable maps to a fixed class" \
        || bad "CLI unreachable maps to a fixed class"
    got="$(probe_classify_cli $'configured    yes\naccount      signed out\n')"
    [[ "$got" == configured ]] \
        && ok "CLI configured maps to a fixed class" \
        || bad "CLI configured maps to a fixed class"
    got="$(probe_classify_cli "the cloud endpoint must use HTTPS or WSS")"
    [[ "$got" == PLAINTEXT ]] \
        && ok "CLI PlaintextEndpoint maps to a fixed class" \
        || bad "CLI PlaintextEndpoint maps to a fixed class"
    got="$(probe_classify_cli_pair "daemon       running" $'configured    yes\n')"
    [[ "$got" == configured ]] \
        && ok "a running daemon plus configured cloud is configured" \
        || bad "a running daemon plus configured cloud is configured"
}

probe_sanitize_self_test() {
    local out url_path url_key
    out="$(printf '%s\n' \
        'keychain: "/Users/dmytro/Library/Keychains/login.keychain-db"' \
        'password: "super-secret-value"' \
        'socket /Users/dmytro/Library/Application Support/com.copypaste.CopyPaste/daemon.sock' \
        'COPYPASTE_CLOUD_ANON_KEY=native-evidence' \
        'orphan.sock' \
        'http://127.0.0.1:47800 stays' \
        | probe_redact_text)"
    if [[ "$out" != *dmytro* \
        && "$out" != *super-secret-value* \
        && "$out" != *native-evidence* \
        && "$out" != *login.keychain-db* \
        && "$out" != *daemon.sock* \
        && "$out" != *orphan.sock* \
        && "$out" != *"/Users/"* \
        && "$out" != *"Application Support"* \
        && "$out" != *'http://'* \
        && "$out" != *'https://'* \
        && "$out" == *'<path>'* \
        && "$out" == *'<socket>'* \
        && "$out" == *'<redacted>'* \
        && "$out" == *'<url> stays'* ]]; then
        ok "sidecar probe artifacts redact secrets and filesystem paths"
    else
        bad "sidecar probe artifacts redact secrets and filesystem paths"
    fi
    url_path="$(printf '%s\n' 'http://127.0.0.1:47800/Users/dmytro/Library/Keychains/login.keychain-db' | probe_redact_text)"
    if [[ "$url_path" != *dmytro* \
        && "$url_path" != *"/Users/"* \
        && "$url_path" != *login.keychain-db* \
        && "$url_path" != *'http://'* \
        && "$url_path" == *'<url>'* ]]; then
        ok "sidecar probe artifacts redact filesystem paths inside URLs"
    else
        bad "sidecar probe artifacts redact filesystem paths inside URLs"
    fi
    url_key="$(printf '%s\n' 'https://example.test/callback?apikey=native-evidence' | probe_redact_text)"
    if [[ "$url_key" != *native-evidence* \
        && "$url_key" != *apikey* \
        && "$url_key" != *'https://'* \
        && "$url_key" == *'<url>'* ]]; then
        ok "sidecar probe artifacts redact query secrets inside URLs"
    else
        bad "sidecar probe artifacts redact query secrets inside URLs"
    fi
}

probe_override_self_test() {
    local body
    body="$(type open_cloud_evidence_app 2>/dev/null)$(type ensure_cloud_evidence_daemon 2>/dev/null)$(type default_cloud_shutdown 2>/dev/null)$(type launch_app 2>/dev/null)"
    if [[ "$body" == *"COPYPASTE_DAEMON_BIN=\$DAEMON_SIDECAR"* \
        && "$body" == *"--features cloud-evidence"* \
        && "$body" == *"-u COPYPASTE_DAEMON_BIN"* \
        && "$body" == *"COPYPASTE_DATA_DIR=\$ISOLATION_DATA_DIR"* \
        && "$body" == *"COPYPASTE_SOCKET=\$ISOLATION_SOCKET"* \
        && "$body" == *"-u COPYPASTE_DATA_DIR"* \
        && "$body" == *"-u COPYPASTE_SOCKET"* \
        && "$body" == *"default_cloud_shutdown"* ]]; then
        ok "configured launch still passes the exact sidecar override"
    else
        bad "configured launch still passes the exact sidecar override"
    fi
}

probe_bounded_runtime_self_test() {
    local started ended elapsed
    started="$(python3 -c 'import time; print(int(time.time() * 1000))')"
    probe_run_bounded 1 python3 -c 'import time; time.sleep(30)' >/dev/null 2>&1 || true
    ended="$(python3 -c 'import time; print(int(time.time() * 1000))')"
    elapsed=$((ended - started))
    if (( elapsed < 5000 )); then
        ok "sidecar probe commands stay bounded"
    else
        bad "sidecar probe commands stay bounded" "${elapsed}ms"
    fi
}

configured_isolation_self_test() { # <tmp-dir>
    local fx="$1/iso" sec_log="$1/iso-security.log" cli_log="$1/iso-cli.log"
    local saved_ga="${GITHUB_ACTIONS-}" out="" got="" home_saved="$HOME"
    local setup_body restore_body cleanup_body delete_body probe_region
    mkdir -p "$fx/cpc.AAAAAA/d" "$fx/home"

    if configured_isolation_owned_root "$fx/cpc.AAAAAA" \
        && ! configured_isolation_owned_root "cpc.rel" \
        && ! configured_isolation_owned_root "$fx/cpc.AAAAAA/../cpc.BBBBBB" \
        && ! configured_isolation_owned_root "$HOME/Library/Keychains/cpc.x" \
        && ! configured_isolation_owned_root "$HOME/Library/Application Support/cpc.x" \
        && ! configured_isolation_owned_root "$fx/other.AAAAAA"; then
        ok "owned-root validation rejects relative, .., login, Application Support, and non-cpc paths"
    else
        bad "owned-root validation rejects relative, .., login, Application Support, and non-cpc paths"
    fi
    if configured_isolation_owned_keychain "$fx/cpc.AAAAAA/k.keychain-db" \
        && ! configured_isolation_owned_keychain "$HOME/Library/Keychains/login.keychain-db" \
        && ! configured_isolation_owned_keychain "$fx/cpc.AAAAAA/login.keychain-db" \
        && ! configured_isolation_owned_keychain "$fx/other/k.keychain-db"; then
        ok "owned-keychain validation rejects login and non-cpc paths"
    else
        bad "owned-keychain validation rejects login and non-cpc paths"
    fi
    if configured_isolation_socket_fits "$fx/cpc.AAAAAA/d/daemon.sock" \
        && ! configured_isolation_socket_fits "/$(python3 -c 'print("x" * 120)')"; then
        ok "Darwin socket length guard accepts short isolated sockets"
    else
        bad "Darwin socket length guard accepts short isolated sockets"
    fi

    unset GITHUB_ACTIONS
    mktemp() { printf 'MKTEMP\n' >> "$sec_log"; return 1; }
    openssl() { printf 'OPENSSL\n' >> "$sec_log"; return 1; }
    security() { printf 'SECURITY\n' >> "$sec_log"; return 1; }
    : > "$sec_log"
    if configured_isolation_setup; then
        bad "configured isolation setup refuses outside CI"
    else
        ok "configured isolation setup refuses outside CI"
    fi
    if [[ -s "$sec_log" ]]; then
        bad "configured isolation setup does not mutate outside CI"
    else
        ok "configured isolation setup does not mutate outside CI"
    fi
    unset -f mktemp openssl security
    if [[ -n "$saved_ga" ]]; then
        export GITHUB_ACTIONS="$saved_ga"
    fi

    export GITHUB_ACTIONS=1
    : > "$sec_log"
    mktemp() { mkdir -p "$fx/cpc.AAAAAA"; printf '%s\n' "$fx/cpc.AAAAAA"; }
    openssl() { printf '%s\n' 'SECRET_PASSWORD_TOKEN'; }
    security() {
        printf '%s\n' "$*" >> "$sec_log"
        case "$1" in
            default-keychain)
                if [[ "${4:-}" == -s ]]; then
                    return 0
                fi
                printf '    "/Users/fixture/Library/Keychains/login.keychain-db"\n'
                ;;
            list-keychains)
                if [[ "${4:-}" == -s ]]; then
                    return 0
                fi
                printf '    "/Users/fixture/Library/Keychains/login.keychain-db"\n'
                ;;
            create-keychain)
                touch "${!#}"
                ;;
            find-generic-password)
                if [[ " $* " == *" -w "* ]]; then
                    printf 'cloud-evidence-roundtrip\n'
                else
                    printf 'could not be found\n'
                fi
                ;;
            *)
                return 0
                ;;
        esac
    }
    got=1
    { set -x
      configured_isolation_setup
      got=$?
      set +x
    } >"$fx/setup.out" 2>"$fx/setup.err"
    out="$(cat "$fx/setup.out" "$fx/setup.err")"
    if [[ "$got" -eq 0 && "$ISOLATION_ACTIVE" == 1 \
        && "$ISOLATION_DATA_DIR" == "$fx/cpc.AAAAAA/d" \
        && "$ISOLATION_SOCKET" == "$fx/cpc.AAAAAA/d/daemon.sock" \
        && "$ISOLATION_KEYCHAIN" == "$fx/cpc.AAAAAA/k.keychain-db" \
        && "$ISOLATION_SAVED_DEFAULT" == "/Users/fixture/Library/Keychains/login.keychain-db" \
        && -z "$ISOLATION_SETUP_REASON" \
        && "$out" != *SECRET_PASSWORD_TOKEN* \
        && "$(cat "$sec_log")" != *com.copypaste.daemon* \
        && "$(cat "$sec_log")" != *device-secret-key* ]]; then
        ok "configured isolation setup arms throwaway paths without logging the password"
    else
        bad "configured isolation setup arms throwaway paths without logging the password"
    fi
    if grep -q "create-keychain -p SECRET_PASSWORD_TOKEN $fx/cpc.AAAAAA/k.keychain-db" "$sec_log" \
        && ! grep -q "create-keychain -p SECRET_PASSWORD_TOKEN $fx/cpc.AAAAAA/k$" "$sec_log" \
        && ! grep -q '\.keychain-db\.keychain-db' "$sec_log"; then
        ok "create-keychain uses the owned k.keychain-db path without suffix guessing"
    else
        bad "create-keychain uses the owned k.keychain-db path without suffix guessing"
    fi
    if awk '
        $1 == "create-keychain" { if (!create) create = NR }
        $1 == "list-keychains" && / -s / { listed = 1 }
        $1 == "default-keychain" && $0 !~ / -s / { if (!def) def = NR }
        $1 == "list-keychains" && $0 !~ / -s / { if (!search) search = NR }
        END { exit !(search && def && create && listed && search < def && def < create) }
    ' "$sec_log" \
        && ! grep -E 'list-keychains -d user -s .*(login\.keychain|existing)' "$sec_log" \
        && grep -q "list-keychains -d user -s $fx/cpc.AAAAAA/k.keychain-db" "$sec_log"; then
        ok "setup saves the search list, then switches it to the throwaway only"
    else
        bad "setup saves the search list, then switches it to the throwaway only"
    fi

    : > "$sec_log"
    configured_isolation_restore
    if grep -qx "default-keychain -d user -s /Users/fixture/Library/Keychains/login.keychain-db" "$sec_log" \
        && grep -qx "list-keychains -d user -s /Users/fixture/Library/Keychains/login.keychain-db" "$sec_log"; then
        ok "successful quoted default restores the exact unsuffixed path"
    else
        bad "successful quoted default restores the exact unsuffixed path"
    fi

    configured_isolation_cleanup
    HOME="$fx/home"
    mkdir -p "$HOME/Library/Keychains"
    : > "$HOME/Library/Keychains/login.keychain-db"
    : > "$sec_log"
    security() {
        printf '%s\n' "$*" >> "$sec_log"
        case "$1" in
            default-keychain)
                if [[ "${4:-}" == -s ]]; then
                    return 0
                fi
                printf 'security: SecKeychainCopyDefault: A default keychain could not be found. (-25307)\n' >&2
                return 1
                ;;
            list-keychains)
                if [[ "${4:-}" == -s ]]; then
                    return 0
                fi
                printf '    "%s"\n' "$HOME/Library/Keychains/login.keychain-db"
                ;;
            create-keychain)
                touch "${!#}"
                ;;
            find-generic-password)
                if [[ " $* " == *" -w "* ]]; then
                    printf 'cloud-evidence-roundtrip\n'
                else
                    printf 'could not be found\n'
                fi
                ;;
            *)
                return 0
                ;;
        esac
    }
    got=1
    configured_isolation_setup
    got=$?
    if [[ "$got" -eq 0 && "$ISOLATION_ACTIVE" == 1 \
        && "$ISOLATION_SAVED_DEFAULT" == "$HOME/Library/Keychains/login.keychain-db" \
        && -z "$ISOLATION_SETUP_REASON" ]]; then
        ok "errSecNoDefaultKeychain plus on-list login arms and saves login restore"
    else
        bad "errSecNoDefaultKeychain plus on-list login arms and saves login restore"
    fi
    configured_isolation_cleanup
    HOME="$home_saved"

    isolation_fail_named() { # <reason> <security-fn>
        local expect="$1" impl="$2" got=0
        ISOLATION_SETUP_REASON=""
        ISOLATION_ACTIVE=0
        : > "$sec_log"
        security() { "$impl" "$@"; }
        configured_isolation_setup || got=$?
        if [[ "$got" -ne 0 && "$ISOLATION_ACTIVE" == 0 \
            && "$ISOLATION_SETUP_REASON" == "$expect" \
            && "$ISOLATION_SETUP_REASON" != /* \
            && "$ISOLATION_SETUP_REASON" != *SECRET_PASSWORD_TOKEN* ]]; then
            ok "named isolation failure is $expect"
        else
            bad "named isolation failure is $expect"
        fi
        unset -f security
        configured_isolation_cleanup
    }
    isolation_base_security() {
        printf '%s\n' "$*" >> "$sec_log"
        case "$1" in
            default-keychain)
                if [[ "${4:-}" == -s ]]; then
                    return 0
                fi
                printf '    "/Users/fixture/Library/Keychains/login.keychain-db"\n'
                ;;
            list-keychains)
                if [[ "${4:-}" == -s ]]; then
                    return 0
                fi
                printf '    "/Users/fixture/Library/Keychains/login.keychain-db"\n'
                ;;
            create-keychain)
                touch "${!#}"
                ;;
            find-generic-password)
                if [[ " $* " == *" -w "* ]]; then
                    printf 'cloud-evidence-roundtrip\n'
                else
                    printf 'could not be found\n'
                fi
                ;;
            *)
                return 0
                ;;
        esac
    }
    isolation_missing_default_security() {
        printf '%s\n' "$*" >> "$sec_log"
        case "$1" in
            default-keychain)
                if [[ "${4:-}" == -s ]]; then
                    return 0
                fi
                printf 'security: SecKeychainCopyDefault: A default keychain could not be found. (-25307)\n' >&2
                return 1
                ;;
            list-keychains)
                if [[ "${4:-}" == -s ]]; then
                    return 0
                fi
                printf '    "/missing/not-a-keychain"\n'
                ;;
            *)
                return 0
                ;;
        esac
    }
    isolation_mint_fail_security() {
        isolation_base_security "$@"
        [[ "$1" == create-keychain ]] && return 1
        return 0
    }
    isolation_switch_fail_security() {
        isolation_base_security "$@" || true
        if [[ "$1" == list-keychains && "${4:-}" == -s ]]; then
            return 1
        fi
        return 0
    }
    isolation_roundtrip_fail_security() {
        isolation_base_security "$@" || true
        if [[ "$1" == add-generic-password ]]; then
            return 1
        fi
        return 0
    }
    isolation_fail_named default-keychain-read isolation_missing_default_security
    isolation_fail_named mint isolation_mint_fail_security
    isolation_fail_named switch isolation_switch_fail_security
    isolation_fail_named roundtrip isolation_roundtrip_fail_security
    unset -f isolation_fail_named isolation_base_security isolation_missing_default_security \
        isolation_mint_fail_security isolation_switch_fail_security isolation_roundtrip_fail_security

    : > "$sec_log"
    ISOLATION_SAVED_DEFAULT="/Users/fixture/Library/Keychains/login.keychain-db"
    ISOLATION_SAVED_SEARCH="/Users/fixture/Library/Keychains/login.keychain-db"
    security() {
        printf '%s\n' "$*" >> "$sec_log"
        if [[ "$1" == delete-keychain ]]; then
            return 1
        fi
        return 0
    }
    rm() {
        printf 'RM %s\n' "$*" >> "$sec_log"
        return 1
    }
    configured_isolation_cleanup
    if awk '
        /default-keychain -d user -s / { restore = NR }
        /list-keychains -d user -s / { search = NR }
        /delete-keychain/ { del = NR }
        /RM / { rm = NR }
        END { exit !(restore && search && restore < (del ? del : 9999) && search < (rm ? rm : 9999)) }
    ' "$sec_log" \
        && [[ "$ISOLATION_ACTIVE" == 0 && -z "$ISOLATION_PASSWORD" ]]; then
        ok "cleanup restores default and search list before a failing delete"
    else
        bad "cleanup restores default and search list before a failing delete"
    fi
    unset -f rm

    : > "$sec_log"
    ISOLATION_KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"
    ISOLATION_ROOT="$HOME/Library/Application Support/com.copypaste.CopyPaste"
    ISOLATION_SAVED_DEFAULT=""
    ISOLATION_SAVED_SEARCH=""
    security() {
        printf '%s\n' "$*" >> "$sec_log"
        return 0
    }
    rm() {
        printf 'RM %s\n' "$*" >> "$sec_log"
        return 0
    }
    configured_isolation_delete_owned
    if [[ -s "$sec_log" ]]; then
        bad "owned-only delete refuses login and Application Support paths"
    else
        ok "owned-only delete refuses login and Application Support paths"
    fi
    unset -f rm security mktemp openssl

    HOME="$fx/home"
    mkdir -p "$HOME/Library/Application Support/com.copypaste.CopyPaste/logs"
    touch "$HOME/Library/Application Support/com.copypaste.CopyPaste/copypaste-v2.db"
    ISOLATION_DATA_DIR="$fx/cpc.AAAAAA/d"
    ISOLATION_SOCKET="$fx/cpc.AAAAAA/d/daemon.sock"
    ISOLATION_KEYCHAIN="$fx/cpc.AAAAAA/k.keychain-db"
    mkdir -p "$ISOLATION_DATA_DIR/logs"
    if [[ "$(probe_db_state)" == absent ]]; then
        ok "probe db inspection uses the isolated data dir"
    else
        bad "probe db inspection uses the isolated data dir"
    fi
    touch "$ISOLATION_DATA_DIR/copypaste-v2.db"
    if [[ "$(probe_db_state)" == present ]]; then
        ok "probe db inspection sees only the isolated database"
    else
        bad "probe db inspection sees only the isolated database"
    fi
    : > "$sec_log"
    security() {
        printf '%s\n' "$*" >> "$sec_log"
        printf 'could not be found\n'
    }
    probe_run_bounded() { shift; "$@"; }
    got="$(probe_keychain_item)"
    if [[ "$got" == absent && "$(cat "$sec_log")" == *"$ISOLATION_KEYCHAIN"* \
        && "$(cat "$sec_log")" != *login.keychain* ]]; then
        ok "probe keychain inspection uses the isolated keychain"
    else
        bad "probe keychain inspection uses the isolated keychain"
    fi
    : > "$cli_log"
    probe_run_bounded() {
        shift
        printf '%s\n' "$*" >> "$cli_log"
        printf 'configured    yes\n'
    }
    APP="$fx/CopyPaste.app"
    mkdir -p "$APP/Contents/MacOS"
    : > "$APP/Contents/MacOS/copypaste"
    chmod +x "$APP/Contents/MacOS/copypaste"
    probe_cli_pair >/dev/null
    if grep -q "COPYPASTE_SOCKET=$ISOLATION_SOCKET" "$cli_log" \
        && grep -q "COPYPASTE_DATA_DIR=$ISOLATION_DATA_DIR" "$cli_log"; then
        ok "probe CLI inspection uses isolated socket and data dir"
    else
        bad "probe CLI inspection uses isolated socket and data dir"
    fi
    HOME="$home_saved"
    unset -f security probe_run_bounded

    setup_body="$(type configured_isolation_setup 2>/dev/null)$(type configured_isolation_mint_keychain 2>/dev/null)$(type configured_isolation_roundtrip 2>/dev/null)"
    helper_body="$(type configured_isolation_current_default 2>/dev/null)$(type configured_isolation_current_search 2>/dev/null)$(type configured_isolation_choose_restore_default 2>/dev/null)$(type configured_isolation_parse_keychain_lines 2>/dev/null)"
    restore_body="$(type configured_isolation_restore 2>/dev/null)"
    cleanup_body="$(type configured_isolation_cleanup 2>/dev/null)$(type cleanup 2>/dev/null)"
    delete_body="$(type configured_isolation_delete_owned 2>/dev/null)"
    probe_region="$(type probe_keychain_item 2>/dev/null)$(type probe_db_state 2>/dev/null)$(type probe_cli_pair 2>/dev/null)$(type probe_live_daemon 2>/dev/null)$(type probe_runtime_log 2>/dev/null)$(type probe_configured_sidecar_path 2>/dev/null)"
    if [[ "$setup_body" != *existing* \
        && "$setup_body" != *login.keychain* \
        && "$setup_body" != *COPYPASTE_EPHEMERAL_KEY* \
        && "$setup_body" != *COPYPASTE_KEYCHAIN_TEST* \
        && "$setup_body" == *"list-keychains -d user -s \"\$ISOLATION_KEYCHAIN\""* ]]; then
        ok "setup does not keep login on the search list or use product key escapes"
    else
        bad "setup does not keep login on the search list or use product key escapes"
    fi
    if [[ "$helper_body" != *'2>/dev/null'* \
        && "$helper_body" != *'| head'* \
        && "$helper_body" != *'head -'* \
        && "$helper_body" == *login.keychain-db* ]]; then
        ok "isolation keychain helpers avoid 2>/dev/null and head"
    else
        bad "isolation keychain helpers avoid 2>/dev/null and head"
    fi
    if [[ "$probe_region" != *delete-generic-password* \
        && "$probe_region" != *add-generic-password* \
        && "$probe_region" != *create-keychain* \
        && "$probe_region" != *unlock-keychain* \
        && "$probe_region" != *delete-keychain* \
        && "$probe_region" != *COPYPASTE_EPHEMERAL_KEY* \
        && "$probe_region" == *ISOLATION_KEYCHAIN* \
        && "$probe_region" == *ISOLATION_DATA_DIR* \
        && "$probe_region" == *ISOLATION_SOCKET* ]]; then
        ok "sidecar probe stays attribute-only on isolated paths"
    else
        bad "sidecar probe stays attribute-only on isolated paths"
    fi
    if [[ "$cleanup_body" == *configured_isolation_restore* \
        && "$cleanup_body" == *configured_isolation_delete_owned* \
        && "$cleanup_body" == *configured_isolation_cleanup* \
        && "$cleanup_body" == *default_cloud_shutdown* \
        && "$restore_body" == *"default-keychain -d user -s"* \
        && "$delete_body" == *configured_isolation_owned_keychain* \
        && "$delete_body" == *configured_isolation_owned_root* ]] \
        && grep -q '^trap cleanup EXIT$' "${BASH_SOURCE[0]}"; then
        ok "cleanup restores isolation on EXIT and validates owned deletes"
    else
        bad "cleanup restores isolation on EXIT and validates owned deletes"
    fi

    ISOLATION_ROOT=""
    ISOLATION_DATA_DIR=""
    ISOLATION_SOCKET=""
    ISOLATION_KEYCHAIN=""
    ISOLATION_PASSWORD=""
    ISOLATION_SAVED_DEFAULT=""
    ISOLATION_SAVED_SEARCH=""
    ISOLATION_SETUP_REASON=""
    ISOLATION_ACTIVE=0
    unset GITHUB_ACTIONS
    if [[ -n "$saved_ga" ]]; then
        export GITHUB_ACTIONS="$saved_ga"
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
    probe_decision_self_test
    probe_classify_self_test
    probe_sanitize_self_test
    probe_override_self_test
    probe_bounded_runtime_self_test
    configured_isolation_self_test "$SELF_TEST_TMP"
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
