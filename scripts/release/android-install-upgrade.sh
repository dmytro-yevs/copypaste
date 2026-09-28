#!/usr/bin/env bash
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
metadata="$here/android-metadata.mjs"
previous="${1:?usage: android-install-upgrade.sh PREVIOUS_APK CURRENT_APK}"
current="${2:?usage: android-install-upgrade.sh PREVIOUS_APK CURRENT_APK}"

# Keep the upgrade receipt beside the signed-artifact smoke evidence. These
# helpers drive the same visible Library surface as the release smoke does.
SMOKE_OUT="${UPGRADE_SMOKE_OUT:-${SMOKE_OUT:-artifacts/android-upgrade}}"
# shellcheck source=scripts/release/android-smoke-lib.sh
. "$here/android-smoke-lib.sh"
# shellcheck source=scripts/release/android-ui-evidence-lib.sh
. "$here/android-ui-evidence-lib.sh"
# shellcheck source=scripts/release/android-navigation-lib.sh
. "$here/android-navigation-lib.sh"

package="$PKG"
# A publishing qualification supplies the latest published release. Manual and
# nightly callers retain the synthetic immediate predecessor fixture.
previous_version="${PREVIOUS_VERSION:-$(node "$metadata" --previous-fixture)}"
previous_code="$(COPYPASTE_ANDROID_UPGRADE_FIXTURE=1 \
    node "$metadata" --version "$previous_version" --field versionCode)"
current_code="$(node "$metadata" --field versionCode)"
activity="$package/$APP_NAMESPACE.MainActivity"
canary="CopyPasteUpgradeCanary$(date +%s)$RANDOM"
((previous_code < current_code)) || {
    printf 'upgrade fixture code %s is not below current code %s\n' "$previous_code" "$current_code" >&2
    exit 1
}
mkdir -p "$OUT"

history_ready_holds() { # <accessibility artifact> <unused>
    enabled_node_exists_exact "$1" "Search clipboard history, default|Search clipboard history, active"
}

history_canary_holds() { # <accessibility artifact> <unused>
    history_ready_holds "$1" \
        && node_exists_exact "$1" "$canary"
}

history_canary_visible() { # <artifact prefix>
    local prefix="$1" xml="$OUT/$1-history.xml"
    android_recover_onboarding "$OUT/$prefix-onboarding.xml" 30 \
        && tap_until_state "Library" "$xml" history_canary_holds none
}

launch_current_package() { # <label>
    local label="$1" start_out pid
    adb logcat -c || true
    start_out="$(sh_ am start -W -n "$activity")"
    grep -q '^Status: ok' <<<"$start_out" || {
        printf '%s package did not launch: %s\n' "$label" "$start_out" >&2
        return 1
    }
    wait_for 60 has_pid || true
    pid="$(app_pid)"
    [[ -n "$pid" ]] || {
        printf '%s package did not create a process\n' "$label" >&2
        return 1
    }
    sleep "${SETTLE_SECS:-25}"
    dump_logcat "$label-launch"
}

capture_before_upgrade() {
    local send_out
    send_out="$(sh_ am start -a android.intent.action.SEND -t text/plain \
        --es android.intent.extra.TEXT "$canary" -n "$package/$APP_NAMESPACE.IntakeActivity")"
    grep -q 'Error' <<<"$send_out" && {
        printf 'previous fixture did not accept ACTION_SEND: %s\n' "$send_out" >&2
        return 1
    }
    sleep 10
    history_canary_visible upgrade-before || {
        printf 'previous fixture did not expose the upgrade canary in Library\n' >&2
        return 1
    }
    capture_png "$OUT/upgrade-before-history.png"
}

assert_persisted_history() { # <label>
    local label="$1"
    history_canary_visible "$label" || {
        printf '%s package did not retain the upgrade canary in Library\n' "$label" >&2
        return 1
    }
    capture_png "$OUT/$label-history.png"
}

adb uninstall "$package" >/dev/null 2>&1 || true
# The prior release is a provisioned upgrade fixture: runtime permission
# dialogs must not block the pre-upgrade Library/canary assertion.
adb install -g "$previous"
installed="$(adb shell dumpsys package "$package" | tr -d '\r')"
grep -q "versionCode=${previous_code}\b" <<<"$installed" || {
    printf 'installed fixture did not report versionCode=%s\n' "$previous_code" >&2
    exit 1
}
launch_current_package previous-upgrade
android_recover_onboarding "$OUT/upgrade-ready-onboarding.xml" 30 \
    && tap_until_state "Library" "$OUT/upgrade-ready-history.xml" history_ready_holds none || {
    printf 'previous fixture did not expose Library before seeding the upgrade canary\n' >&2
    exit 1
}
capture_before_upgrade

adb install -r "$current"
upgraded="$(adb shell dumpsys package "$package" | tr -d '\r')"
grep -q "versionCode=${current_code}\b" <<<"$upgraded" || {
    printf 'upgraded package did not report versionCode=%s\n' "$current_code" >&2
    exit 1
}
launch_current_package upgraded
assert_persisted_history upgrade-after

adb shell am force-stop "$package"
launch_current_package upgraded-restart
assert_persisted_history upgrade-restart
printf 'upgrade: %s %s -> %s retained visible history\n' "$package" "$previous_code" "$current_code"
