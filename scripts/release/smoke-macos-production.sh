#!/usr/bin/env bash
set -euo pipefail

DMG="${1:-}"
VERSION="${2:-}"
[[ -f "$DMG" && "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
    echo "ERROR: usage: $0 <dmg> <stable-version>" >&2
    exit 1
}
[[ "$(uname -s)" == Darwin ]] || {
    echo "ERROR: macOS smoke requires macOS" >&2
    exit 1
}
if [[ "${COPYPASTE_KEYCHAIN_TEST:-}" != 1 || -z "${COPYPASTE_KEYCHAIN_TEST_PATH:-}" ]]; then
    echo "ERROR: production smoke requires a disposable test Keychain." >&2
    exit 2
fi
expected_keychain="$COPYPASTE_KEYCHAIN_TEST_PATH"
default_keychain="$(security default-keychain -d user | sed 's/^[[:space:]]*"//; s/"[[:space:]]*$//')"
search_count="$(security list-keychains -d user | sed '/^[[:space:]]*$/d' | wc -l | tr -d ' ')"
search_keychain="$(security list-keychains -d user | sed -n '1s/^[[:space:]]*"//; 1s/"[[:space:]]*$//; 1p')"
if [[ "$default_keychain" != "$expected_keychain" || "$search_count" != 1 || "$search_keychain" != "$expected_keychain" ]]; then
    echo "ERROR: production smoke refuses a login or mixed Keychain search list." >&2
    exit 2
fi

root="$(mktemp -d)"
mount="$root/mount"
installed="$root/CopyPaste.app"
app_pid=""
cleanup() {
    if [[ -n "$app_pid" ]]; then kill "$app_pid" >/dev/null 2>&1 || true; fi
    hdiutil detach "$mount" -quiet >/dev/null 2>&1 || true
    rm -rf "$root"
}
trap cleanup EXIT

mkdir -p "$mount"
hdiutil attach "$DMG" -nobrowse -readonly -mountpoint "$mount" >/dev/null
[[ -d "$mount/CopyPaste.app" ]]
ditto "$mount/CopyPaste.app" "$installed"
/bin/bash "$installed/Contents/Resources/selfsign.sh" "$installed"
/usr/bin/codesign --verify --deep --strict "$installed"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$installed/Contents/Info.plist")" == "$VERSION" ]]

"$installed/Contents/MacOS/CopyPaste" >/dev/null 2>&1 &
app_pid=$!
for _ in $(seq 1 60); do
    daemon_pid="$(ps -axo pid=,ppid=,args= | awk -v parent="$app_pid" '$2 == parent && $0 ~ /\/copypaste-daemon( |$)/ && !found { print $1; found = 1 }')"
    if [[ -n "$daemon_pid" ]]; then break; fi
    kill -0 "$app_pid" 2>/dev/null
    sleep 0.5
done
[[ -n "${daemon_pid:-}" ]]
daemon_path="$(ps -p "$daemon_pid" -o command=)"
[[ "$daemon_path" == "$installed/Contents/MacOS/copypaste-daemon"* ]]
cli="$installed/Contents/MacOS/copypaste-cli"
[[ -x "$cli" ]]
socket="${TMPDIR:-/tmp}/cp-${app_pid}.sock"
status_file="$root/status.json"
for _ in $(seq 1 20); do
    if COPYPASTE_SOCKET="$socket" "$cli" --json status >"$status_file" 2>/dev/null; then
        break
    fi
    sleep 0.25
done
python3 - "$status_file" "$VERSION" <<'PY'
import json
import sys

response = json.load(open(sys.argv[1], encoding="utf-8"))
status = response.get("data", {}).get("status", {})
if not response.get("ok") or status.get("version") != sys.argv[2]:
    raise SystemExit("production daemon did not report ready at the release version")
PY

kill "$app_pid"
wait "$app_pid" 2>/dev/null || true
app_pid=""
for _ in $(seq 1 20); do
    kill -0 "$daemon_pid" 2>/dev/null || break
    sleep 0.25
done
! kill -0 "$daemon_pid" 2>/dev/null
echo "verified macOS production bundle and app-owned daemon lifecycle"
