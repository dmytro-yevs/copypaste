#!/usr/bin/env bash
set -euo pipefail

# Pairs two booted emulators and watches a clip cross in each direction.
#
# Each emulator sits behind a separate user-mode NAT. A composed ADB forward
# and reverse tunnel makes the creator's listener available on the joiner's
# loopback; only the joiner can start a session, so creator waits for sync.

APK="${APK:-}"
TEST_APK="${TEST_APK:-}"
FIRST_DEVICE="${FIRST_DEVICE:-}"
SECOND_DEVICE="${SECOND_DEVICE:-}"
OUT="${ANDROID_TWO_DEVICE_OUT:-artifacts/android-two-device-e2e}"
PKG="com.copypaste.app"
RUNNER="androidx.test.runner.AndroidJUnitRunner"
HANDOFF="files/e2e-pairing.txt"
CLAIMED="files/e2e-claimed"
PAIRED="files/e2e-paired"
PEER_READY="files/e2e-peer-ready"
CREATOR_LIVE_READY="files/e2e-creator-live-ready"
LIVE="files/e2e-live"
SYNC_READY="files/e2e-sync-ready"
JOINER_DONE="files/e2e-joiner-done"
mkdir -p "$OUT/first" "$OUT/second"

collect() {
    local serial="$1" target="$2"
    adb -s "$serial" logcat -d -v threadtime > "$target/logcat.txt" 2>&1 || true
    adb -s "$serial" exec-out run-as "$PKG" cat sync-cursors.json > "$target/sync-cursors.json" 2>/dev/null || true
    adb -s "$serial" exec-out run-as "$PKG" sh -c 'cat logs/app.*.log' > "$target/app.log" 2>/dev/null || true
    adb -s "$serial" shell uiautomator dump /sdcard/window.xml >/dev/null 2>&1 || true
    adb -s "$serial" exec-out cat /sdcard/window.xml > "$target/window.xml" 2>/dev/null || true
    adb -s "$serial" exec-out screencap -p > "$target/final.png" 2>/dev/null || true
    adb -s "$serial" shell dumpsys package "$PKG" > "$target/package.txt" 2>&1 || true
}
finish() {
    collect "$FIRST_DEVICE" "$OUT/first"
    collect "$SECOND_DEVICE" "$OUT/second"
    # The handoff carries a live pairing secret; it must not outlive the run or
    # reach the retained evidence.
    adb -s "$FIRST_DEVICE" shell run-as "$PKG" rm -f "$HANDOFF" "$CLAIMED" "$PEER_READY" "$LIVE" "$SYNC_READY" "$JOINER_DONE" >/dev/null 2>&1 || true
    adb -s "$SECOND_DEVICE" shell run-as "$PKG" rm -f "$PAIRED" "$CREATOR_LIVE_READY" "$LIVE" "$SYNC_READY" "$JOINER_DONE" >/dev/null 2>&1 || true
    [[ -n "${host_port:-}" ]] && adb -s "$FIRST_DEVICE" forward --remove "tcp:$host_port" >/dev/null 2>&1
    [[ -n "${join_port:-}" ]] && adb -s "$SECOND_DEVICE" reverse --remove "tcp:$join_port" >/dev/null 2>&1
    true
}
trap finish EXIT

[[ -f "$APK" ]] || { echo "APK is missing: ${APK:-<unset>}"; exit 1; }
[[ -f "$TEST_APK" ]] || { echo "instrumentation APK is missing: ${TEST_APK:-<unset>}"; exit 1; }
[[ -n "$FIRST_DEVICE" && -n "$SECOND_DEVICE" && "$FIRST_DEVICE" != "$SECOND_DEVICE" ]] || {
    echo "two distinct booted devices are required"; exit 1;
}

for serial in "$FIRST_DEVICE" "$SECOND_DEVICE"; do
    adb -s "$serial" wait-for-device
    adb -s "$serial" uninstall "$PKG" >/dev/null 2>&1 || true
    adb -s "$serial" install -r -g "$APK"
    adb -s "$serial" install -r "$TEST_APK"
    adb -s "$serial" logcat -c
done
adb -s "$FIRST_DEVICE" shell run-as "$PKG" rm -f "$HANDOFF" "$CLAIMED" "$PEER_READY" "$LIVE" "$SYNC_READY" >/dev/null 2>&1 || true
adb -s "$SECOND_DEVICE" shell run-as "$PKG" rm -f "$PAIRED" "$CREATOR_LIVE_READY" "$LIVE" "$SYNC_READY" >/dev/null 2>&1 || true

first_canary="e2e-first-canary"
second_canary="e2e-second-canary"
test_class="$PKG.PairingDeviceTest"

adb -s "$FIRST_DEVICE" shell am instrument -w -r \
    -e class "$test_class" -e peerRole creator \
    -e ownCanary "$first_canary" -e remoteCanary "$second_canary" \
    "$PKG.test/$RUNNER" > "$OUT/first/instrumentation.txt" 2>&1 &
first_pid=$!

# The creator writes the ceremony's own "Copy pairing details" URI here.
uri=""
for _ in $(seq 1 120); do
    uri="$(adb -s "$FIRST_DEVICE" exec-out run-as "$PKG" cat "$HANDOFF" 2>/dev/null | tr -d '\r\n')"
    [[ "$uri" == copypaste://pair* ]] && break
    kill -0 "$first_pid" 2>/dev/null || { echo "the creator exited before publishing a pairing URI"; exit 1; }
    uri=""
    sleep 2
done
[[ -n "$uri" ]] || { echo "the creator never published a pairing URI"; exit 1; }

field() { sed -n "s/.*[?&]$1=\([^&]*\).*/\1/p" <<<"$uri"; }
code="$(field code)"
pairing_id="$(field id)"
listen_addr="$(python3 -c 'import sys,urllib.parse; print(urllib.parse.unquote(sys.argv[1]))' "$(field addr)")"
security="$(tr '[:lower:]' '[:upper:]' <<<"${pairing_id:0:6}")"
[[ -n "$code" && -n "$security" && -n "$listen_addr" ]] || {
    echo "the pairing URI was missing a field"; exit 1;
}

guest_port="${listen_addr##*:}"
host_port=$((20000 + guest_port % 20000))
join_port=$((40000 + guest_port % 20000))
adb -s "$FIRST_DEVICE" forward "tcp:$host_port" "tcp:$guest_port"
adb -s "$SECOND_DEVICE" reverse "tcp:$join_port" "tcp:$host_port"
adb -s "$FIRST_DEVICE" shell run-as "$PKG" touch "$CLAIMED"

(
    for _ in $(seq 1 150); do
        if adb -s "$SECOND_DEVICE" shell run-as "$PKG" ls "$PAIRED" 2>/dev/null | grep -q "$PAIRED"; then
            adb -s "$FIRST_DEVICE" shell run-as "$PKG" touch "$PEER_READY"
            exit 0
        fi
        sleep 2
    done
    exit 1
) &
ready_pid=$!

(
    for _ in $(seq 1 150); do
        first_live="$(adb -s "$FIRST_DEVICE" shell run-as "$PKG" ls "$LIVE" 2>/dev/null || true)"
        if [[ "$first_live" == *"$LIVE"* ]]; then
            adb -s "$SECOND_DEVICE" shell run-as "$PKG" touch "$CREATOR_LIVE_READY"
        fi
        second_live="$(adb -s "$SECOND_DEVICE" shell run-as "$PKG" ls "$LIVE" 2>/dev/null || true)"
        if [[ "$first_live" == *"$LIVE"* && "$second_live" == *"$LIVE"* ]]; then
            adb -s "$FIRST_DEVICE" shell run-as "$PKG" touch "$SYNC_READY"
            adb -s "$SECOND_DEVICE" shell run-as "$PKG" touch "$SYNC_READY"
            exit 0
        fi
        sleep 2
    done
    exit 1
) &
sync_pid=$!

(
    for _ in $(seq 1 180); do
        if adb -s "$SECOND_DEVICE" shell run-as "$PKG" ls "$JOINER_DONE" 2>/dev/null | grep -q "$JOINER_DONE"; then
            adb -s "$FIRST_DEVICE" shell run-as "$PKG" touch "$JOINER_DONE"
            exit 0
        fi
        sleep 2
    done
    exit 1
) &
done_pid=$!

set +e
adb -s "$SECOND_DEVICE" shell am instrument -w -r \
    -e class "$test_class" -e peerRole joiner \
    -e ownCanary "$second_canary" -e remoteCanary "$first_canary" \
    -e pairCode "$code" \
    -e pairAddr "127.0.0.1:$join_port" -e securityCode "$security" \
    "$PKG.test/$RUNNER" | tee "$OUT/second/instrumentation.txt"
second_status=${PIPESTATUS[0]}
if [[ $second_status -ne 0 ]]; then
    kill "$first_pid" "$ready_pid" "$sync_pid" "$done_pid" 2>/dev/null || true
    exit "$second_status"
fi
wait "$first_pid"
first_status=$?
wait "$ready_pid"
ready_status=$?
wait "$sync_pid"
sync_status=$?
wait "$done_pid"
done_status=$?
set -e
cat "$OUT/first/instrumentation.txt"

for result in "$OUT/first/instrumentation.txt" "$OUT/second/instrumentation.txt"; do
    grep -q '^OK (' "$result" || { echo "instrumentation failed: $result"; exit 1; }
    grep -Eq 'FAILURES|INSTRUMENTATION_FAILED|Process crashed|shortMsg=' "$result" && exit 1
done
[[ $first_status -eq 0 && $second_status -eq 0 && $ready_status -eq 0 && $sync_status -eq 0 && $done_status -eq 0 ]]
