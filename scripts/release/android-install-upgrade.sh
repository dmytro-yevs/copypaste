#!/usr/bin/env bash
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
metadata="$here/android-metadata.mjs"
previous="${1:?usage: android-install-upgrade.sh PREVIOUS_APK CURRENT_APK}"
current="${2:?usage: android-install-upgrade.sh PREVIOUS_APK CURRENT_APK}"

package="$(node "$metadata" --field releaseApplicationId)"
previous_version="$(node "$metadata" --previous-fixture)"
previous_code="$(COPYPASTE_ANDROID_UPGRADE_FIXTURE=1 \
    node "$metadata" --version "$previous_version" --field versionCode)"
current_code="$(node "$metadata" --field versionCode)"
((previous_code < current_code)) || {
    printf 'upgrade fixture code %s is not below current code %s\n' "$previous_code" "$current_code" >&2
    exit 1
}

adb uninstall "$package" >/dev/null 2>&1 || true
adb install "$previous"
installed="$(adb shell dumpsys package "$package" | tr -d '\r')"
grep -q "versionCode=${previous_code}\b" <<<"$installed" || {
    printf 'installed fixture did not report versionCode=%s\n' "$previous_code" >&2
    exit 1
}
adb install -r "$current"
upgraded="$(adb shell dumpsys package "$package" | tr -d '\r')"
grep -q "versionCode=${current_code}\b" <<<"$upgraded" || {
    printf 'upgraded package did not report versionCode=%s\n' "$current_code" >&2
    exit 1
}
printf 'upgrade: %s %s -> %s\n' "$package" "$previous_code" "$current_code"
