#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/release/macos-bundle-lib.sh
. "$REPO_ROOT/scripts/release/macos-bundle-lib.sh"
# shellcheck source=scripts/release/macos-ui-evidence-lib.sh
. "$REPO_ROOT/scripts/release/macos-ui-evidence-lib.sh"

check_accessibility_surface() {
  python3 - "$1" <<'PY'
import csv
import pathlib
import sys

try:
    with pathlib.Path(sys.argv[1]).open(encoding="utf-8", newline="") as source:
        rows = [row for row in csv.reader(source, delimiter="\t") if row]
except OSError as error:
    raise SystemExit(f"native accessibility observation unavailable: {error}")
if not rows:
    raise SystemExit("native accessibility surface is empty")
roles = {row[0] for row in rows}
named = [row for row in rows if len(row) > 1 and row[1].strip()]
if "AXMenuBar" not in roles:
    raise SystemExit("VoiceOver surface exposes no menu bar")
if not named:
    raise SystemExit("VoiceOver surface exposes no named elements")
print(f"VoiceOver accessibility surface: {len(rows)} elements, {len(named)} named")
PY
}

if [[ "${1:-}" == "--self-test" ]]; then
  fixture_dir="$(mktemp -d)"
  trap 'rm -rf "$fixture_dir"' EXIT
  PASS=0
  FAIL=0
  ok() { PASS=$((PASS + 1)); }
  bad() { FAIL=$((FAIL + 1)); echo "self-test failed: $1" >&2; }
  printf 'AXMenuBar\tCopyPaste\nAXMenuBarItem\tCopyPaste\n' > "$fixture_dir/good.tsv"
  printf 'AXWindow\tCopyPaste\n' > "$fixture_dir/no-menu.tsv"
  printf 'AXMenuBar\t\n' > "$fixture_dir/unnamed.tsv"
  check_accessibility_surface "$fixture_dir/good.tsv" >/dev/null
  if check_accessibility_surface "$fixture_dir/no-menu.tsv" >/dev/null 2>&1; then
    echo "self-test failed: surface without a menu bar passed" >&2
    exit 1
  fi
  if check_accessibility_surface "$fixture_dir/unnamed.tsv" >/dev/null 2>&1; then
    echo "self-test failed: surface without a name passed" >&2
    exit 1
  fi
  mac_ui_self_test "$fixture_dir"
  [[ "$FAIL" -eq 0 ]] || exit 1
  echo "macOS native accessibility self-test passed"
  exit 0
fi

out="${1:?usage: macos-native-evidence.sh OUTPUT_DIRECTORY}"
mkdir -p "$out"

app="${COPYPASTE_APP:-/Applications/CopyPaste.app}"
[[ -d "$app" ]] || { echo "CopyPaste.app is not installed" >&2; exit 1; }
app_executable="$(mac_evidence_executable "$app")"
cli="$app/Contents/MacOS/copypaste"
app_pid=""

cleanup() {
  [[ -z "$app_pid" ]] || kill "$app_pid" 2>/dev/null || true
  "$cli" shutdown >/dev/null 2>&1 || true
}
trap cleanup EXIT

mac_stop_executable "$app_executable" || {
  echo "the installed CopyPaste process did not stop" >&2
  exit 1
}
"$cli" shutdown >/dev/null 2>&1 || true

start_ms="$(python3 -c 'import time; print(time.time_ns() // 1000000)')"
open -n -a "$app"
app_pid="$(mac_wait_executable_pid "$app_executable" 30)" || {
  echo "CopyPaste did not launch from its bundle executable" >&2
  exit 1
}
mac_set_app_pid "$app_pid"

surface_ready="no"
surface_started="$SECONDS"
while (( SECONDS - surface_started < 30 )); do
  if mac_ax ready > /dev/null 2> "$out/ax.err"; then
    surface_ready="yes"
    break
  fi
  sleep 0.1
done
if [[ "$surface_ready" != "yes" ]]; then
  echo "native accessibility observation unavailable: $(tr '\n' ' ' < "$out/ax.err")" >&2
  exit 1
fi
ready_ms="$(python3 -c 'import time; print(time.time_ns() // 1000000)')"
scenario="$(python3 scripts/release/native_evidence_policy.py value --platform macos --field scenario)"
budget_ms="$(python3 scripts/release/native_evidence_policy.py value --platform macos --field budget_ms)"
mac_ax surface > "$out/ax.log" 2> "$out/ax.err"
check_accessibility_surface "$out/ax.log"
screencapture -x "$out/screenshot.png"
python3 - "$out/latency.json" "$scenario" "$((ready_ms - start_ms))" "$budget_ms" <<'PY'
import json, pathlib, sys
pathlib.Path(sys.argv[1]).write_text(json.dumps({"scenario": sys.argv[2], "latency_ms": int(sys.argv[3]), "budget_ms": int(sys.argv[4])}) + "\n")
if int(sys.argv[3]) > int(sys.argv[4]):
    raise SystemExit("native launch exceeded its policy budget")
PY

test -s "$out/ax.log"
test -s "$out/screenshot.png"
test -s "$out/latency.json"

python3 scripts/release/write-native-evidence.py \
  --output "$out/native-evidence.json" \
  --platform macos \
  --environment hosted-runner \
  --os-version "$(sw_vers -productVersion)" \
  --architecture "$(uname -m)" \
  --commit "${GITHUB_SHA:-$(git rev-parse HEAD)}" \
  --run-id "${GITHUB_RUN_ID:-local-$(git rev-parse --short HEAD)}" \
  --elapsed-ms "$((ready_ms - start_ms))" \
  --feature-state devices=native-shell,screenshot=screenshot.png,accessibility=ax.log \
  --artifact screenshot=screenshot.png \
  --artifact accessibility=ax.log \
  --artifact measurement=latency.json
