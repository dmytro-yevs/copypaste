#!/usr/bin/env bash
# Sign the Android updater artifact and compose the one release feed.
set -euo pipefail

die() { echo "ERROR: $*" >&2; exit 1; }

sign_apk() {
  local apk=$1 output=$2
  [[ -f "$apk" ]] || die "APK is missing: $apk"
  [[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]] || die "TAURI_SIGNING_PRIVATE_KEY is required"
  [[ -n "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]] || die "TAURI_SIGNING_PRIVATE_KEY_PASSWORD is required"
  apk=$(cd "$(dirname "$apk")" && pwd)/$(basename "$apk")
  local keyfile
  keyfile=$(mktemp)
  printf '%s' "$TAURI_SIGNING_PRIVATE_KEY" >"$keyfile"
  if ! npm --prefix crates/copypaste-ui run tauri -- signer sign "$apk" \
      --private-key-path "$keyfile" \
      --password "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD"; then
    rm -f "$keyfile"
    die "Tauri could not sign the Android updater artifact"
  fi
  rm -f "$keyfile"
  [[ -s "${apk}.sig" ]] || die "Tauri did not create the APK updater signature"
  mkdir -p "$(dirname "$output")"
  local output_path
  output_path=$(cd "$(dirname "$output")" && pwd)/$(basename "$output")
  if [[ "${apk}.sig" != "$output_path" ]]; then
    cp "${apk}.sig" "$output_path"
  fi
  [[ -s "$output_path" ]] || die "APK updater signature is empty"
}

write_feed() {
  local version=$1 pub_date=$2 base_url=$3 windows=$4 windows_sig=$5 android=$6 android_sig=$7 output=$8
  VERSION="$version" PUB_DATE="$pub_date" BASE_URL="$base_url" \
    WINDOWS="$windows" WINDOWS_SIG="$windows_sig" ANDROID="$android" ANDROID_SIG="$android_sig" OUTPUT="$output" \
    python3 - <<'PY'
import json, os, re
from pathlib import Path

version = os.environ["VERSION"]
pub_date = os.environ["PUB_DATE"]
base = os.environ["BASE_URL"].rstrip("/")
windows = Path(os.environ["WINDOWS"])
windows_sig = Path(os.environ["WINDOWS_SIG"])
android = Path(os.environ["ANDROID"])
android_sig = Path(os.environ["ANDROID_SIG"])
output = Path(os.environ["OUTPUT"])

if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?", version):
    raise SystemExit("version is not a valid release version")
if not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", pub_date):
    raise SystemExit("pub_date must be UTC RFC3339")
if not base.startswith("https://") or "__" in base:
    raise SystemExit("release base URL must be an HTTPS, non-placeholder URL")
for path in (windows, windows_sig, android, android_sig):
    if not path.is_file() or not path.stat().st_size:
        raise SystemExit(f"missing release feed input: {path}")

def signature(path):
    value = path.read_text(encoding="utf-8").strip()
    if not value or "__" in value or "PLACEHOLDER" in value.upper():
        raise SystemExit(f"invalid or placeholder updater signature: {path}")
    return value

windows_name = windows.name
android_name = android.name
feed = {
    "version": version,
    "pub_date": pub_date,
    "platforms": {
        "windows-x86_64": {"signature": signature(windows_sig), "url": f"{base}/{windows_name}"},
        "android-universal": {"signature": signature(android_sig), "url": f"{base}/{android_name}"},
    },
}
output.write_text(json.dumps(feed, indent=2) + "\n", encoding="utf-8")
PY
}

render_config() {
  local template=$1 output=$2 public_key=$3 endpoint=$4
  TEMPLATE="$template" OUTPUT="$output" PUBLIC_KEY="$public_key" ENDPOINT="$endpoint" python3 - <<'PY'
import os
from pathlib import Path

template = Path(os.environ["TEMPLATE"])
output = Path(os.environ["OUTPUT"])
public_key = os.environ["PUBLIC_KEY"]
endpoint = os.environ["ENDPOINT"]
if not template.is_file():
    raise SystemExit(f"Android updater template is missing: {template}")
if not public_key or "__" in public_key or not endpoint.startswith("https://") or "__" in endpoint:
    raise SystemExit("Android updater config requires non-placeholder public key and HTTPS endpoint")
source = template.read_text(encoding="utf-8")
for marker in ("__TAURI_UPDATER_PUBLIC_KEY__", "__TAURI_UPDATER_ENDPOINT__"):
    if marker not in source:
        raise SystemExit(f"Android updater template is missing {marker}")
source = source.replace("__TAURI_UPDATER_PUBLIC_KEY__", public_key)
source = source.replace("__TAURI_UPDATER_ENDPOINT__", endpoint)
if "__" in source:
    raise SystemExit("Android updater config contains an unresolved placeholder")
output.write_text(source, encoding="utf-8")
PY
}

self_test() {
  local root
  root=$(mktemp -d)
  trap 'rm -rf "$root"' RETURN
  printf exe >"$root/windows.exe"
  printf win-signature >"$root/windows.exe.sig"
  printf apk >"$root/android.apk"
  printf android-signature >"$root/android.apk.sig"
  write_feed "2.0.0-alpha.16" "2026-08-25T00:00:00Z" \
    "https://example.test/releases/v2.0.0-alpha.16" \
    "$root/windows.exe" "$root/windows.exe.sig" "$root/android.apk" "$root/android.apk.sig" "$root/latest.json"
  python3 - "$root/latest.json" <<'PY'
import json, sys
data = json.load(open(sys.argv[1], encoding="utf-8"))
assert set(data["platforms"]) == {"windows-x86_64", "android-universal"}
assert all(item["url"].startswith("https://") for item in data["platforms"].values())
PY
  printf '{"plugins":{"updater":{"pubkey":"__TAURI_UPDATER_PUBLIC_KEY__","endpoints":["__TAURI_UPDATER_ENDPOINT__"]}}}\n' >"$root/template.json"
  render_config "$root/template.json" "$root/generated.json" self-test-public-key https://example.test/latest.json
  grep -q 'self-test-public-key' "$root/generated.json"
  if render_config "$root/template.json" "$root/rejected.json" '' https://example.test/latest.json 2>/dev/null; then
    die "self-test accepted a missing updater public key"
  fi
  if write_feed 2.0.0-alpha.16 bad-date https://example.test/releases/v2 "$root/windows.exe" "$root/windows.exe.sig" "$root/android.apk" "$root/android.apk.sig" "$root/should-not-write.json" 2>/dev/null; then
    die "self-test accepted a malformed pub_date"
  fi
  echo "PASS: Android updater signature/feed self-test"
}

case "${1:-}" in
  --self-test) self_test ;;
  --sign-apk) [[ $# -eq 3 ]] || die "usage: $0 --sign-apk APK OUTPUT_SIG"; sign_apk "$2" "$3" ;;
  --write-feed) [[ $# -eq 9 ]] || die "usage: $0 --write-feed VERSION PUB_DATE BASE_URL WINDOWS WINDOWS_SIG ANDROID ANDROID_SIG OUTPUT"; write_feed "$2" "$3" "$4" "$5" "$6" "$7" "$8" "$9" ;;
  --render-config) [[ $# -eq 5 ]] || die "usage: $0 --render-config TEMPLATE OUTPUT PUBLIC_KEY ENDPOINT"; render_config "$2" "$3" "$4" "$5" ;;
  *) die "usage: $0 --self-test | --sign-apk APK OUTPUT_SIG | --write-feed VERSION PUB_DATE BASE_URL WINDOWS WINDOWS_SIG ANDROID ANDROID_SIG OUTPUT | --render-config TEMPLATE OUTPUT PUBLIC_KEY ENDPOINT" ;;
esac
