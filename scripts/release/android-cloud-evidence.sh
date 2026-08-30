#!/usr/bin/env bash
set -uo pipefail

# shellcheck source=scripts/release/android-smoke-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-smoke-lib.sh"
# shellcheck source=scripts/release/android-ui-evidence-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-ui-evidence-lib.sh"
# shellcheck source=scripts/release/android-navigation-lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/android-navigation-lib.sh"
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

# The sign-in fields are addressed by their position in the form, because their
# authored labels are only in the dump from API 36. See [dump_exposes_hint].
CLOUD_FORM="Cloud account sign in"
FIELD_EMAIL="field:$CLOUD_FORM:0"
FIELD_PASSWORD="field:$CLOUD_FORM:1"
FIELD_PASSPHRASE="field:$CLOUD_FORM:2"
CLOUD_SECTION_ACTION="Cloud sync Account, encryption and cloud status"
CLOUD_SYNC_ERROR="Cloud sync failed. Check the connection and try again."

now_ms() { python3 -c 'import time; print(time.time_ns() // 1000000)'; }

cleanup() {
    [[ -n "$STUB_PID" ]] && kill "$STUB_PID" 2>/dev/null || true
    adb reverse --remove "tcp:$STUB_PORT" >/dev/null 2>&1 || true
}

# Artefacts are named, never pathed: the runner's absolute path discloses the
# account name (AGENTS.md rule 4).
evidence_name() { # <path>
    printf '%s' "${1#"$OUT/"}"
}

install_and_open() { # <apk>
    [[ -f "$1" ]] || {
        bad "the cloud evidence APK exists" "missing: $(basename "$1")"
        return 1
    }
    sh_ am force-stop "$PKG" >/dev/null 2>&1 || true
    adb uninstall "$PKG" >/dev/null 2>&1 || true
    adb install -r -g "$1" >/dev/null || {
        bad "the cloud evidence APK installs" "adb rejected $(basename "$1")"
        return 1
    }
    wake_screen
    sh_ am start -W -n "$MAIN" >/dev/null
    reach_settings_tab "$OUT/launch.xml" 90 || {
        bad "the launched app exposes its Settings tab" \
            "uiautomator did not expose Settings within 90s"
        return 1
    }
}

cloud_settings_pane_holds() { # <artifact>
    enabled_node_exists_exact "$1" "Preference sections"
}

cloud_detail_holds() { # <artifact>
    node_exists_exact "$1" "Cloud sync" \
        && enabled_action_exists_exact "$1" "Back to Preferences"
}

open_cloud() {
    tap_until_state "Settings" "$OUT/settings-nav.xml" \
        cloud_settings_pane_holds none || return 1
    tap_until_state "$CLOUD_SECTION_ACTION" "$OUT/settings-sync.xml" \
        cloud_detail_holds up
}

capture_state() { # <state>
    local dir="$OUT/$1" png
    mkdir -p "$dir"
    png="$dir/screenshot.png"
    if ! dump_hierarchy "$dir/ax.xml" || [[ ! -s "$dir/ax.xml" ]]; then
        bad "$1 accessibility evidence exists"
    fi
    if ! capture_png "$png"; then
        bad "$1 screenshot evidence is a complete PNG" \
            "$(tail -n 12 "${png%.png}-screencap.log" | tr '\n' ' ')"
    fi
}

# Cloud sync is the last row of the Sync pane, so on a phone viewport its form,
# its actions and the status they produce all start below the fold. Every
# lookup here scrolls towards the end of the pane; a plain wait reported the
# whole configured account lifecycle as missing while the app rendered it.
expect_label() { # <selector> <artifact> [timeout] [dump fn] [scroll fn]
    wait_selector_scrolling "$1" "$2" up "${3:-$WAIT_SECS}" any \
        "${4:-dump_hierarchy}" "${5:-scroll_content}" \
        && ok "cloud UI exposes $1" \
        || bad "cloud UI exposes $1" "uiautomator did not find it"
}

expect_exact_label() { # <selector> <artifact> [timeout] [dump fn] [scroll fn]
    wait_selector_scrolling "$1" "$2" up "${3:-$WAIT_SECS}" exact \
        "${4:-dump_hierarchy}" "${5:-scroll_content}" \
        && ok "cloud UI exposes $1" \
        || bad "cloud UI exposes $1" "uiautomator did not find it"
}

# A sync summary is a toast, not a row: it is in the accessibility tree only
# while it is on screen, and never in the accessibility event stream.
expect_feedback() { # <selector> <artifact>
    wait_authored_feedback "$1" "$2" "$WAIT_SECS" \
        && ok "cloud UI exposes $1" \
        || bad "cloud UI exposes $1" "uiautomator did not find it"
}

# What the running image can actually be asked about the sign-in form.
#
# Its three fields carry their authored labels in `hintText` and nowhere else,
# and `uiautomator dump` emits `hint` only from API 36 — so on API 34 the
# labels are absent from the tree and a gate that waited for them spent 45 s
# per field to conclude the form was missing while the app was rendering it.
# The shape is asserted on every level; the labels are asserted where they
# exist and named as unobservable where they do not.
expect_form_fields() { # <artifact> [timeout] [dump fn] [scroll fn]
    local shape
    wait_selector_scrolling "$FIELD_EMAIL" "$1" up "${2:-$WAIT_SECS}" any \
        "${3:-dump_hierarchy}" "${4:-scroll_content}" || true
    shape="$(form_field_shape "$1" "$CLOUD_FORM")"
    [[ "$shape" == "3 2" ]] \
        && ok "the cloud sign-in form exposes three fields, two of them secret" \
        || bad "the cloud sign-in form exposes three fields, two of them secret" \
               "$(evidence_name "$1") holds '$shape'"
    if dump_exposes_hint "$1"; then
        local pair
        for pair in "Email:email" "Password:password" "Sync passphrase:passphrase"; do
            expect_label "${pair%:*}" "$OUT/signed-out-${pair##*:}.xml" "${2:-$WAIT_SECS}" \
                "${3:-dump_hierarchy}" "${4:-scroll_content}"
        done
    else
        note "the authored labels on the cloud sign-in fields" \
             "this image's uiautomator dump emits no hint attribute, which AOSP added in android-16.0.0_r1; Chromium exposes a WebView field's accessible name nowhere else, so the fields are reached by their position in the form instead"
    fi
}

fill_field() { # <label> <value> <artifact>
    tap_selector_scrolling "$1" "$3" up || return 1
    sh_ input keyevent KEYCODE_MOVE_END >/dev/null
    sh_ input text "$2" >/dev/null
    sh_ input keyevent KEYCODE_BACK >/dev/null
}

start_stub() { # [rows] [log]
    python3 scripts/cloud-stub.py --port "$STUB_PORT" --password stub-password \
        --dump "${1:-$OUT/stub-rows.json}" > "${2:-$OUT/stub.log}" 2>&1 &
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

# Every early return above recorded a verdict, but none of them proved the
# evidence exists. The debug leg could therefore install nothing, wake the
# screen and exit 0 with an empty latency table and no screenshot, which is how
# a green API 36 job shipped `latency.tsv` with zero rows. What the artefact
# must contain is asserted here, unconditionally, so an aborted scenario fails
# closed and names what it did not produce.
require_state_evidence() { # <state>
    local ax="$OUT/$1/ax.xml" png="$OUT/$1/screenshot.png"
    [[ -s "$ax" ]] \
        && ok "$1 accessibility evidence is present and non-empty" \
        || bad "$1 accessibility evidence is present and non-empty" \
               "$(evidence_name "$ax") is missing or empty"
    [[ -s "$png" ]] \
        && ok "$1 screenshot evidence is present and non-empty" \
        || bad "$1 screenshot evidence is present and non-empty" \
               "$(evidence_name "$png") is missing or empty"
}

require_latency_evidence() { # <scenario...>
    local scenario
    for scenario in "$@"; do
        grep -q "^$scenario$(printf '\t')" "$LATENCIES" 2>/dev/null \
            && ok "$scenario recorded a latency measurement" \
            || bad "$scenario recorded a latency measurement" \
                   "$(evidence_name "$LATENCIES") has no $scenario row"
    done
}

unconfigured_scenario() {
    local started elapsed
    group "Cloud UI: unconfigured build"
    started="$(now_ms)"
    if install_and_open "$APK_UNCONFIGURED"; then
        if open_cloud; then
            expect_label "Not configured" "$OUT/unconfigured-status.xml"
            expect_label "Cloud server configuration" "$OUT/unconfigured-form.xml"
            expect_label "Configure" "$OUT/unconfigured-action.xml"
            elapsed=$(( $(now_ms) - started ))
            cloud_latency_record "$LATENCIES" unconfigured-status "$elapsed" 90000 \
                && ok "unconfigured cloud status meets its latency budget" \
                || bad "unconfigured cloud status meets its latency budget" "${elapsed}ms"
            capture_state unconfigured
        else
            bad "the unconfigured cloud row is reachable" \
                "Settings never reached the Sync tab's Cloud sync row"
        fi
    fi
    require_latency_evidence unconfigured-status
    require_state_evidence unconfigured
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
    expect_label "$CLOUD_FORM" "$OUT/signed-out-form.xml"
    expect_form_fields "$OUT/signed-out-form.xml"
    capture_state signed-out

    fill_field "$FIELD_EMAIL" "native@example.test" "$OUT/email.xml" || bad "email can be entered"
    fill_field "$FIELD_PASSWORD" "stub-password" "$OUT/password.xml" || bad "password can be entered"
    fill_field "$FIELD_PASSPHRASE" "native-evidence" "$OUT/passphrase.xml" || bad "passphrase can be entered"
    started="$(now_ms)"
    tap_selector_scrolling "Sign in" "$OUT/sign-in.xml" up || bad "the native sign-in action is reachable"
    expect_label "Connected" "$OUT/connected.xml"
    elapsed=$(( $(now_ms) - started ))
    cloud_latency_record "$LATENCIES" sign-in "$elapsed" 30000 \
        && ok "cloud sign-in meets its latency budget" \
        || bad "cloud sign-in meets its latency budget" "${elapsed}ms"
    expect_label "native@example.test" "$OUT/account-status.xml"
    capture_state signed-in

    seed_forged_row || bad "the skip fixture reaches the stub backend"
    started="$(now_ms)"
    tap_selector_scrolling "Sync cloud now" "$OUT/sync.xml" up || bad "the native cloud sync action is reachable"
    expect_feedback "skipped" "$OUT/sync-skips.xml"
    elapsed=$(( $(now_ms) - started ))
    cloud_latency_record "$LATENCIES" sync-with-skips "$elapsed" 30000 \
        && ok "cloud sync with skips meets its latency budget" \
        || bad "cloud sync with skips meets its latency budget" "${elapsed}ms"
    capture_state sync-with-skips

    kill "$STUB_PID" 2>/dev/null || true
    wait "$STUB_PID" 2>/dev/null || true
    STUB_PID=""
    started="$(now_ms)"
    tap_selector_scrolling "Sync cloud now" "$OUT/offline-sync.xml" up || bad "cloud sync remains actionable offline"
    expect_exact_label "$CLOUD_SYNC_ERROR" "$OUT/offline-error.xml"
    elapsed=$(( $(now_ms) - started ))
    cloud_latency_record "$LATENCIES" offline-error "$elapsed" 60000 \
        && ok "offline cloud error meets its latency budget" \
        || bad "offline cloud error meets its latency budget" "${elapsed}ms"
    capture_state offline-error

    sign_out_scenario
}

# What the card allows a sign-out assertion to claim.
#
# `Sign out` is disabled for as long as a cloud mutation is in flight
# (`SyncTab.tsx`), and the offline probe above ends the session on its own: a
# sync against a dead backend comes back unauthorised and the card falls back to
# its sign-in form. Run 31671766432's release leg then spent 45 s looking for a
# control that was gone, called it unreachable, and read the badge it had never
# left as proof that signing out worked. A badge that was already showing proves
# nothing, so the pre-state decides what may be asserted.
sign_out_precondition() { # <artifact>
    case "$(control_state "$1" "Sign out")" in
        actionable) printf ready ;;
        # Exact: the sign-out toast reads "Signed out of cloud sync", and a
        # partial match on it would call a signed-in card signed out.
        *) signed_out_state_holds "$1" && printf restore || printf blocked ;;
    esac
}

cloud_card_state() { # <artifact>
    local badge=absent state
    for state in Connected "Signed out" "Not configured" Unavailable; do
        if [[ -n "$(node_center_exact "$1" "$state")" ]]; then
            badge="$state"
            break
        fi
    done
    printf 'the cloud card reads %s and Sign out is %s' \
        "$badge" "$(control_state "$1" "Sign out")"
}

# A zero-sized badge is still useful semantic state when the WebView has
# clipped its header. Require the complete visible form and reject signed-in
# card state anywhere in XML; a blank form's submit action may be disabled.
signed_out_form_holds() { # <artifact>
    [[ "$(form_field_shape "$1" "$CLOUD_FORM")" == "3 2" ]] || return 1
    python3 - "$1" "$CLOUD_FORM" <<'PY'
import re
import sys
import xml.etree.ElementTree as ET

root = ET.parse(sys.argv[1]).getroot()
label = sys.argv[2].casefold()

def exact(node, value):
    return any((node.get(name) or "").casefold() == value
               for name in ("text", "content-desc"))

def rect(node):
    match = re.fullmatch(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]", node.get("bounds", ""))
    return tuple(map(int, match.groups())) if match else None

form = next((node for node in root.iter("node") if exact(node, label)), None)
if form is None or form.get("enabled") != "true":
    raise SystemExit(1)
form_rect = rect(form)
if form_rect is None or form_rect[2] - form_rect[0] < 8 or form_rect[3] - form_rect[1] < 8:
    raise SystemExit(1)
fields = [node for node in form.iter("node")
          if (node.get("class") or "").endswith("EditText")]
if len(fields) != 3 or sum(node.get("password") == "true" for node in fields) != 2:
    raise SystemExit(1)
left, top, right, bottom = form_rect
for field in fields:
    bounds = rect(field)
    if bounds is None or field.get("enabled") != "true":
        raise SystemExit(1)
    if not (left <= bounds[0] <= bounds[2] <= right and top <= bounds[1] <= bounds[3] <= bottom):
        raise SystemExit(1)
actions = [node for node in form.iter("node")
           if exact(node, "sign in") or exact(node, "create account")]
if not any(node.get("clickable") == "true" and rect(node)
           and rect(node)[2] - rect(node)[0] >= 8
           and rect(node)[3] - rect(node)[1] >= 8
           and left <= rect(node)[0] <= rect(node)[2] <= right
           and top <= rect(node)[1] <= rect(node)[3] <= bottom
           for node in actions):
    raise SystemExit(1)
PY
}

signed_out_state_holds() { # <artifact>
    enabled_node_exists_exact "$1" "Signed out" \
        && signed_out_form_holds "$1" \
        && ! node_exists_exact "$1" "Sign out" \
        && ! node_exists_exact "$1" "Connected"
}

wait_signed_out_state() { # <artifact> [timeout] [dump fn] [scroll fn]
    local artifact="$1" timeout="${2:-$WAIT_SECS}"
    local dump="${3:-dump_hierarchy}" scroll="${4:-scroll_content}"
    local started="$SECONDS"
    while (( SECONDS - started < timeout )); do
        if "$dump" "$artifact" && signed_out_state_holds "$artifact"; then
            return 0
        fi
        "$scroll" up
        sleep 1
    done
    return 1
}

restore_session() { # <artifact>
    start_stub "$OUT/restore-rows.json" "$OUT/restore-stub.log" || return 1
    fill_field "$FIELD_EMAIL" "native@example.test" "$OUT/restore-email.xml" || return 1
    fill_field "$FIELD_PASSWORD" "stub-password" "$OUT/restore-password.xml" || return 1
    fill_field "$FIELD_PASSPHRASE" "native-evidence" "$OUT/restore-passphrase.xml" || return 1
    tap_selector_scrolling "Sign in" "$OUT/restore-sign-in.xml" up || return 1
    # Exact: the service status line reads "Background service connected", and
    # it is on every screen this scenario ever dumps.
    wait_selector_scrolling "Connected" "$1" up "$WAIT_SECS" exact
}

# The card at the end of the leg, whatever it turned out to be: the state this
# publishes is the one a failed sign-out is read from.
sign_out_scenario() {
    sign_out_lifecycle
    capture_state signed-out-again
}

# Waiting for `Sign out` to appear spends the whole timeout whenever the answer
# is that it never will: the offline probe had already ended the session, and
# every release run paid 45 s to rediscover that. Only `blocked` — the card is
# there and the control is disabled mid-mutation — is worth another sample.
settle_sign_out_card() { # <artifact> [timeout] [dump fn] [scroll fn] [pace fn]
    local artifact="$1" timeout="${2:-$WAIT_SECS}"
    local dump="${3:-dump_hierarchy}" scroll="${4:-scroll_content}" pace="${5:-settle_pace}"
    local started="$SECONDS"
    while (( SECONDS - started < timeout )); do
        if "$dump" "$artifact"; then
            [[ "$(sign_out_precondition "$artifact")" != blocked ]] && return 0
            dismiss_covering_feedback "$artifact" "Sign out" action && continue
        fi
        "$scroll" up
        "$pace"
    done
    return 1
}

sign_out_lifecycle() {
    local pre="$OUT/sign-out.xml"
    settle_sign_out_card "$pre"
    if [[ "$(sign_out_precondition "$pre")" == restore ]]; then
        if restore_session "$OUT/restore-connected.xml"; then
            ok "a signed-in account is restored for the sign-out assertion"
        else
            bad "a signed-in account is restored for the sign-out assertion" \
                "$(cloud_card_state "$pre")"
            return
        fi
        settle_sign_out_card "$pre"
    fi
    if [[ "$(sign_out_precondition "$pre")" != ready ]]; then
        bad "the native sign-out action is reachable" "$(cloud_card_state "$pre")"
        return
    fi
    ok "the native sign-out action is reachable"

    # The same dump the tap is aimed from, so the state before it is the state
    # the assertion below is measured against.
    tap_found_action "Sign out" "$pre" || {
        bad "the native sign-out tap reaches the app"
        return
    }
    wait_signed_out_state "$OUT/signed-out-again.xml" \
        && ok "signing out returns the account to Signed out" \
        || bad "signing out returns the account to Signed out" \
               "$(cloud_card_state "$OUT/signed-out-again.xml")"
}

# Asserting a fail-closed helper means running it and reading the verdict it
# recorded, so the surrounding counters are restored around each call.
requirement_fails() { # <function> <args...>
    local saved_pass="$PASS" saved_fail="$FAIL" observed
    PASS=0
    FAIL=0
    "$@" >/dev/null
    (( FAIL > 0 )) && observed=0 || observed=1
    PASS="$saved_pass"
    FAIL="$saved_fail"
    return "$observed"
}

unconfigured_evidence_self_test() { # <temp>
    local OUT="$1" LATENCIES="$1/latency.tsv"
    mkdir -p "$OUT/unconfigured"
    : > "$LATENCIES"
    requirement_fails require_latency_evidence unconfigured-status \
        && ok "an empty latency table cannot pass the unconfigured leg" \
        || bad "an empty latency table cannot pass the unconfigured leg"
    requirement_fails require_state_evidence unconfigured \
        && ok "absent screenshot and accessibility evidence fails closed" \
        || bad "absent screenshot and accessibility evidence fails closed"

    printf 'unconfigured-status\t120\t90000\n' > "$LATENCIES"
    printf '<hierarchy/>\n' > "$OUT/unconfigured/ax.xml"
    : > "$OUT/unconfigured/screenshot.png"
    requirement_fails require_state_evidence unconfigured \
        && ok "a zero-byte screenshot is not screenshot evidence" \
        || bad "a zero-byte screenshot is not screenshot evidence"
    requirement_fails require_latency_evidence sign-in \
        && ok "another scenario's latency row does not satisfy this one" \
        || bad "another scenario's latency row does not satisfy this one"

    printf 'PNG\n' > "$OUT/unconfigured/screenshot.png"
    requirement_fails require_latency_evidence unconfigured-status \
        && bad "a recorded latency row satisfies its requirement" \
        || ok "a recorded latency row satisfies its requirement"
    requirement_fails require_state_evidence unconfigured \
        && bad "complete unconfigured evidence satisfies its requirement" \
        || ok "complete unconfigured evidence satisfies its requirement"
    # The empty table is the shipped defect: it wrote no latency.json and the
    # caller ignored the refusal, so the leg summarised as passed.
    : > "$LATENCIES"
    rm -f "$OUT/latency.json"
    cloud_latency_write "$LATENCIES" "$OUT/latency.json" android >/dev/null 2>&1 \
        && bad "an empty latency table refuses to write structured evidence" \
        || ok "an empty latency table refuses to write structured evidence"
    [[ ! -e "$OUT/latency.json" ]] \
        && ok "a refused write leaves no structured latency evidence behind" \
        || bad "a refused write leaves no structured latency evidence behind"
}

cloud_form_self_test() { # <temp>
    local OUT="$1" observed
    android_ui_form_fixtures "$OUT"

    ui_fixtures "$OUT/form-api34.xml"
    observed="$(expect_form_fields "$OUT/observed.xml" 5 ui_fixture_dump ui_fixture_scroll)"
    [[ "$observed" == *"ok    the cloud sign-in form exposes three fields, two of them secret"* \
       && "$observed" == *"NOT ASSERTED  the authored labels on the cloud sign-in fields"* ]] \
        && ok "an image without hint asserts the form's shape and names what it cannot read" \
        || bad "an image without hint asserts the form's shape and names what it cannot read" \
               "$(tr '\n' ' ' <<<"$observed")"

    # One sample per lookup is the fast path, not the contract: a fixture list
    # that is exactly long enough turns any extra sample into a missing label.
    ui_fixtures "$OUT/form-api36.xml" "$OUT/form-api36.xml" "$OUT/form-api36.xml" \
                "$OUT/form-api36.xml" "$OUT/form-api36.xml" "$OUT/form-api36.xml" \
                "$OUT/form-api36.xml" "$OUT/form-api36.xml"
    observed="$(expect_form_fields "$OUT/observed.xml" 5 ui_fixture_dump ui_fixture_scroll)"
    [[ "$observed" == *"ok    cloud UI exposes Sync passphrase"* \
       && "$observed" != *"NOT ASSERTED"* ]] \
        && ok "an image with hint also asserts the authored labels" \
        || bad "an image with hint also asserts the authored labels" \
               "$(tr '\n' ' ' <<<"$observed")"

    # The gate this replaces could not fail: it waited for a label no dump
    # below API 36 carries, so a form that was not there and one the harness
    # could not read looked identical. A form with a field missing must FAIL on
    # every level.
    python3 - "$OUT/form-api34.xml" "$OUT/form-short.xml" <<'PY'
import sys, xml.etree.ElementTree as ET
tree = ET.parse(sys.argv[1])
form = next(n for n in tree.getroot().iter("node") if n.get("text") == "Cloud account sign in")
form.remove(next(n for n in form if n.get("resource-id") == "passphrase"))
tree.write(sys.argv[2])
PY
    ui_fixtures "$OUT/form-short.xml" "$OUT/form-short.xml" "$OUT/form-short.xml" "$OUT/form-short.xml"
    requirement_fails expect_form_fields "$OUT/observed.xml" 3 ui_fixture_dump ui_fixture_scroll \
        && ok "a form missing a field fails without hint to read" \
        || bad "a form missing a field fails without hint to read"
}

cloud_navigation_fixture_holds() { # <temp>
    (
        local OUT="$1" primary settings prefs_hidden prefs_visible detail
        primary='<node text="Primary" bounds="[49,572][271,628]"><node text="Settings" bounds="[196,577][266,623]" enabled="true" clickable="true"/></node>'
        settings='<node text="Preference sections" bounds="[12,184][308,640]" enabled="true"/>'
        prefs_hidden="${settings}<node text=\"$CLOUD_SECTION_ACTION\" enabled=\"true\" clickable=\"true\"/>"
        prefs_visible="${settings}<node text=\"$CLOUD_SECTION_ACTION\" bounds=\"[13,444][307,510]\" enabled=\"true\" clickable=\"true\"/>"
        detail='<node text="Cloud sync" bounds="[12,12][308,640]" enabled="true"><node text="Back to Preferences" bounds="[12,29][56,74]" enabled="true" clickable="true"/></node>'
        printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$primary</hierarchy>" > "$OUT/cloud-library.xml"
        printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$primary$settings</hierarchy>" > "$OUT/cloud-preferences.xml"
        printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$primary$prefs_hidden</hierarchy>" > "$OUT/cloud-row-hidden.xml"
        printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$primary$prefs_visible</hierarchy>" > "$OUT/cloud-row-visible.xml"
        printf '%s\n' "<?xml version=\"1.0\"?><hierarchy>$primary$detail</hierarchy>" > "$OUT/cloud-detail.xml"
        ui_fixtures "$OUT/cloud-library.xml" "$OUT/cloud-library.xml" \
            "$OUT/cloud-preferences.xml" "$OUT/cloud-row-hidden.xml" \
            "$OUT/cloud-row-visible.xml" "$OUT/cloud-detail.xml"
        dump_hierarchy() { ui_fixture_dump "$@"; }
        scroll_content() { navigation_fixture_scroll "$@"; }
        tap_transition_point() { navigation_fixture_tap "$@"; }
        settle_pace() { ui_fixture_pace; }
        NAVIGATION_FIXTURE_DIRECTION=""
        open_cloud \
            && [[ $UI_FIXTURE_INDEX -eq 6 && $UI_FIXTURE_TAPS -eq 3 \
                  && $UI_FIXTURE_SCROLLS -eq 1 \
                  && "$NAVIGATION_FIXTURE_DIRECTION" == up ]] \
            && cloud_settings_pane_holds "$OUT/settings-nav.xml" \
            && cloud_detail_holds "$OUT/settings-sync.xml"
    )
}

cloud_navigation_self_test() { # <temp>
    cloud_navigation_fixture_holds "$1" \
        && ok "Cloud settings retries a swallowed tab tap and verifies its detail pane" \
        || bad "Cloud settings retries a swallowed tab tap and verifies its detail pane"
}

# Cards taken from run 31671766432's published dumps: the release leg signed in,
# lost the session to the offline probe, and then asserted a sign-out against
# the sign-in form that came back.
sign_out_self_test() { # <temp>
    local temp="$1" primary card badge form fields toast
    primary='<node text="Primary" bounds="[0,570][320,640]"><node text="Library" bounds="[17,583][113,635]" enabled="true" clickable="true"/></node>'
    card='<node text="Cloud sync" bounds="[24,411][93,427]"/><node text="native@example.test" bounds="[184,479][296,493]"/>'
    fields='<node class="android.widget.EditText" password="false" bounds="[24,360][296,404]" enabled="true"/><node class="android.widget.EditText" password="true" bounds="[24,414][296,458]" enabled="true"/><node class="android.widget.EditText" password="true" bounds="[24,462][296,506]" enabled="true"/>'
    form='<node text="Cloud account sign in" bounds="[24,346][296,546]" enabled="true">'$fields'<node text="Sign in" bounds="[24,502][296,546]" enabled="false" clickable="true"/></node>'
    badge='<node text="Connected" bounds="[109,412][168,426]"/>'
    toast='<node text="Signed out of cloud sync" bounds="[49,543][263,563]"/>'
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$primary$card$badge<node text=\"Sign out\" bounds=\"[200,502][296,546]\" enabled=\"true\" clickable=\"true\"/></node></hierarchy>" > "$temp/connected.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$primary$card$badge<node text=\"Sign out\" bounds=\"[200,538][296,582]\" enabled=\"false\" clickable=\"true\"/><node text=\"The last cloud sync failed. Try again or sign in again.\" bounds=\"[24,470][296,502]\"/></node></hierarchy>" > "$temp/busy.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$primary$card<node text=\"Signed out\" bounds=\"[109,280][168,294]\" enabled=\"true\"/>$form</node></hierarchy>" > "$temp/signed-out.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$primary$card$badge<node text=\"Sign out\" bounds=\"[200,502][296,546]\" enabled=\"true\" clickable=\"true\"/>$toast</node></hierarchy>" > "$temp/toasted.xml"

    [[ "$(control_state "$temp/connected.xml" "Sign out")" == actionable ]] \
        && ok "a signed-in card exposes an actionable sign-out" \
        || bad "a signed-in card exposes an actionable sign-out" \
               "$(control_state "$temp/connected.xml" "Sign out")"
    # Disabled and absent were one verdict, and the leg that reported "Sign out
    # unreachable" could not say which one it had seen.
    [[ "$(control_state "$temp/busy.xml" "Sign out")" == disabled ]] \
        && ok "a card with a sync in flight reports sign-out disabled, not missing" \
        || bad "a card with a sync in flight reports sign-out disabled, not missing" \
               "$(control_state "$temp/busy.xml" "Sign out")"

    [[ "$(sign_out_precondition "$temp/connected.xml")" == ready ]] \
        && ok "a signed-in card may be signed out" \
        || bad "a signed-in card may be signed out" \
               "$(sign_out_precondition "$temp/connected.xml")"
    [[ "$(sign_out_precondition "$temp/busy.xml")" == blocked ]] \
        && ok "a busy card is not a session that ended by itself" \
        || bad "a busy card is not a session that ended by itself" \
               "$(sign_out_precondition "$temp/busy.xml")"
    [[ "$(sign_out_precondition "$temp/signed-out.xml")" == restore ]] \
        && ok "a session the offline probe ended is restored, not asserted against" \
        || bad "a session the offline probe ended is restored, not asserted against" \
               "$(sign_out_precondition "$temp/signed-out.xml")"
    # The false pass this scenario shipped: the badge was read from a partial
    # match, and the toast carries the words.
    [[ "$(sign_out_precondition "$temp/toasted.xml")" == ready ]] \
        && ok "the sign-out toast is not read as the signed-out badge" \
        || bad "the sign-out toast is not read as the signed-out badge" \
               "$(sign_out_precondition "$temp/toasted.xml")"

    [[ "$(cloud_card_state "$temp/signed-out.xml")" == "the cloud card reads Signed out and Sign out is absent" ]] \
        && ok "a refused sign-out names the card it read" \
        || bad "a refused sign-out names the card it read" \
               "$(cloud_card_state "$temp/signed-out.xml")"
    signed_out_state_holds "$temp/signed-out.xml" \
        && ok "a visible sign-in form corroborates signed-out semantics" \
        || bad "a visible sign-in form corroborates signed-out semantics"

    # The 45 s every release run spent rediscovering a session that had already
    # ended. One dump answers it, and the sample count is the assertion.
    #
    # Three samples off a fixture list is the most any case below asks for, and
    # none of them waits for a device, so the ceiling is a regression alarm
    # rather than a budget. A live-device number here just makes a broken case
    # take minutes to say so.
    local settle_secs=5
    ui_fixtures "$temp/signed-out.xml"
    settle_sign_out_card "$temp/observed.xml" "$settle_secs" ui_fixture_dump ui_fixture_scroll \
        ui_fixture_pace \
        && [[ "$UI_FIXTURE_INDEX" == 1 && "$UI_FIXTURE_SCROLLS" == 0 ]] \
        && ok "an ended session settles on its first sample" \
        || bad "an ended session settles on its first sample" \
               "$UI_FIXTURE_INDEX samples, $UI_FIXTURE_SCROLLS scrolls"
    ui_fixtures "$temp/connected.xml"
    settle_sign_out_card "$temp/observed.xml" "$settle_secs" ui_fixture_dump ui_fixture_scroll \
        ui_fixture_pace \
        && [[ "$UI_FIXTURE_INDEX" == 1 ]] \
        && ok "an actionable sign-out settles on its first sample" \
        || bad "an actionable sign-out settles on its first sample" "$UI_FIXTURE_INDEX samples"
    # Disabled mid-mutation is the one state worth another sample, so this one
    # does wait — and then reports rather than claiming the card was ready.
    ui_fixtures "$temp/busy.xml" "$temp/busy.xml" "$temp/connected.xml"
    settle_sign_out_card "$temp/observed.xml" "$settle_secs" ui_fixture_dump ui_fixture_scroll \
        ui_fixture_pace \
        && [[ "$UI_FIXTURE_INDEX" == 3 && "$UI_FIXTURE_PACES" == 2 ]] \
        && ok "a card still mutating is sampled again" \
        || bad "a card still mutating is sampled again" \
               "$UI_FIXTURE_INDEX samples, $UI_FIXTURE_PACES paces"
    # The one case that has to reach its ceiling, so the ceiling is the budget.
    ui_fixtures "$temp/busy.xml" "$temp/busy.xml"
    settle_sign_out_card "$temp/observed.xml" 2 ui_fixture_dump ui_fixture_scroll \
        && bad "a card that never settles times out" \
        || ok "a card that never settles times out"
}

cloud_lifecycle_self_test() { # <temp>
    local temp="$1" primary form status action fields missing_fields wrong_passwords
    primary='<node text="Primary" bounds="[0,570][320,640]"/>'
    status='<node text="Signed out" bounds="[0,0][0,0]" enabled="true"/>'
    action='<node text="Sign in" bounds="[24,482][296,527]" enabled="false" clickable="true"/>'
    fields='<node class="android.widget.EditText" password="false" bounds="[24,114][296,159]" enabled="true"/><node class="android.widget.EditText" password="true" bounds="[24,188][296,233]" enabled="true"/><node class="android.widget.EditText" password="true" bounds="[24,262][296,307]" enabled="true"/>'
    form="<node text=\"Cloud account sign in\" bounds=\"[24,96][296,527]\" enabled=\"true\">$fields$action</node>"
    missing_fields="<node class=\"android.widget.EditText\" password=\"false\" bounds=\"[24,114][296,159]\" enabled=\"true\"/><node class=\"android.widget.EditText\" password=\"true\" bounds=\"[24,188][296,233]\" enabled=\"true\"/>"
    wrong_passwords="<node class=\"android.widget.EditText\" password=\"false\" bounds=\"[24,114][296,159]\" enabled=\"true\"/><node class=\"android.widget.EditText\" password=\"false\" bounds=\"[24,188][296,233]\" enabled=\"true\"/><node class=\"android.widget.EditText\" password=\"false\" bounds=\"[24,262][296,307]\" enabled=\"true\"/>"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$primary$status$form</node></hierarchy>" > "$temp/signed-out-zero.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$primary<node text=\"Connected\" bounds=\"[24,280][168,294]\" enabled=\"true\"/><node text=\"Sign out\" bounds=\"[24,502][296,546]\" enabled=\"true\" clickable=\"true\"/></node></hierarchy>" > "$temp/still-signed-in.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$primary$status<node text=\"Cloud account sign in\" bounds=\"[24,96][296,527]\" enabled=\"false\">$fields$action</node></node></hierarchy>" > "$temp/disabled-form.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$primary$status</node></hierarchy>" > "$temp/missing-form.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$primary$status<node text=\"Cloud account\" bounds=\"[24,346][296,546]\" enabled=\"true\"/></node></hierarchy>" > "$temp/unrelated-label.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$primary$status<node text=\"Cloud account sign in\" bounds=\"[24,96][296,527]\" enabled=\"true\">$missing_fields$action</node></node></hierarchy>" > "$temp/missing-field.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$primary$status<node text=\"Cloud account sign in\" bounds=\"[24,96][296,527]\" enabled=\"true\">$wrong_passwords$action</node></node></hierarchy>" > "$temp/wrong-password-count.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$primary$status<node text=\"Cloud account sign in\" bounds=\"[24,96][296,527]\" enabled=\"true\">$fields</node><node text=\"Sign in\" bounds=\"[24,502][296,546]\" enabled=\"false\" clickable=\"true\"/></node></hierarchy>" > "$temp/action-outside-form.xml"

    signed_out_state_holds "$temp/signed-out-zero.xml" \
        && ok "zero-sized Signed out semantics need visible signed-out UI" \
        || bad "zero-sized Signed out semantics need visible signed-out UI"
    signed_out_state_holds "$temp/still-signed-in.xml" \
        && bad "a still-connected card cannot pass as signed out" \
        || ok "a still-connected card cannot pass as signed out"
    signed_out_state_holds "$temp/disabled-form.xml" \
        && bad "a disabled signed-out form cannot corroborate state" \
        || ok "a disabled signed-out form cannot corroborate state"
    signed_out_state_holds "$temp/missing-form.xml" \
        && bad "a missing signed-out form cannot corroborate state" \
        || ok "a missing signed-out form cannot corroborate state"
    signed_out_state_holds "$temp/unrelated-label.xml" \
        && bad "an unrelated label cannot corroborate signed-out state" \
        || ok "an unrelated label cannot corroborate signed-out state"
    signed_out_state_holds "$temp/missing-field.xml" \
        && bad "a form missing a field cannot corroborate signed-out state" \
        || ok "a form missing a field cannot corroborate signed-out state"
    signed_out_state_holds "$temp/wrong-password-count.xml" \
        && bad "a form with the wrong password count cannot corroborate signed-out state" \
        || ok "a form with the wrong password count cannot corroborate signed-out state"
    signed_out_state_holds "$temp/action-outside-form.xml" \
        && bad "an action outside the sign-in form cannot corroborate state" \
        || ok "an action outside the sign-in form cannot corroborate state"

    local actual='<node text="Cloud sync failed. Check the connection and try again." bounds="[77,92][295,121]" enabled="true"/>'
    local wrong='<node text="The last cloud sync failed. Try again or sign in again." bounds="[77,92][295,121]" enabled="true"/>'
    local unrelated='<node text="Cloud sync finished" bounds="[77,92][295,121]" enabled="true"/>'
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$actual</node></hierarchy>" > "$temp/sync-error.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$wrong</node></hierarchy>" > "$temp/wrong-sync-error.xml"
    printf '%s\n' "<?xml version=\"1.0\"?><hierarchy><node>$unrelated</node></hierarchy>" > "$temp/unrelated-sync-label.xml"
    ui_fixtures "$temp/sync-error.xml"
    expect_exact_label "$CLOUD_SYNC_ERROR" "$temp/observed-sync-error.xml" 1 \
        ui_fixture_dump ui_fixture_scroll
    ui_fixtures "$temp/wrong-sync-error.xml"
    requirement_fails expect_exact_label "$CLOUD_SYNC_ERROR" \
        "$temp/observed-wrong-sync-error.xml" 1 ui_fixture_dump ui_fixture_scroll \
        && ok "the persisted cloud error is not mistaken for the immediate error" \
        || bad "the persisted cloud error is not mistaken for the immediate error"
    ui_fixtures "$temp/unrelated-sync-label.xml"
    requirement_fails expect_exact_label "$CLOUD_SYNC_ERROR" \
        "$temp/observed-unrelated-sync-label.xml" 1 ui_fixture_dump ui_fixture_scroll \
        && ok "an unrelated cloud label is not a sync error" \
        || bad "an unrelated cloud label is not a sync error"
}

if [[ "$MODE" == "--self-test" ]]; then
    SELF_TEST_TMP="$(mktemp -d)"
    trap 'rm -rf "$SELF_TEST_TMP"' EXIT
    android_ui_self_test
    android_navigation_self_test "$SELF_TEST_TMP"
    cloud_navigation_self_test "$SELF_TEST_TMP"
    cloud_form_self_test "$SELF_TEST_TMP"
    sign_out_self_test "$SELF_TEST_TMP"
    cloud_lifecycle_self_test "$SELF_TEST_TMP"
    cloud_evidence_self_test "$SELF_TEST_TMP"
    unconfigured_evidence_self_test "$SELF_TEST_TMP/unconfigured-leg"
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

cloud_latency_write "$LATENCIES" "$OUT/latency.json" android \
    && ok "structured latency evidence is written" \
    || bad "structured latency evidence is written" \
           "no measurement reached $(evidence_name "$LATENCIES")"
cloud_evidence_summary Android
[[ $FAIL -eq 0 ]]
