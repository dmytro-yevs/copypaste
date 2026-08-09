#!/usr/bin/env bash
set -euo pipefail

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
  echo "macOS native accessibility self-test passed"
  exit 0
fi

out="${1:?usage: macos-native-evidence.sh OUTPUT_DIRECTORY}"
mkdir -p "$out"

app="/Applications/CopyPaste.app"
[[ -d "$app" ]] || { echo "CopyPaste.app is not installed" >&2; exit 1; }

start_ms="$(python3 -c 'import time; print(time.time_ns() // 1000000)')"
open -a "$app"
for _ in $(seq 1 100); do
  pgrep -x CopyPaste >/dev/null && break
  sleep 0.1
done
pgrep -x CopyPaste >/dev/null || { echo "CopyPaste did not launch" >&2; exit 1; }
ready_ms="$(python3 -c 'import time; print(time.time_ns() // 1000000)')"

if ! osascript - "CopyPaste" > "$out/ax.log" 2> "$out/ax.err" <<'APPLESCRIPT'
on run argv
    set appName to item 1 of argv
    tell application "System Events"
        if UI elements enabled is false then error "Accessibility permission is unavailable"
        if not (exists process appName) then error "CopyPaste process is unavailable"
        tell process appName
            set outputLines to {}
            repeat with elementRef in entire contents
                set roleText to ""
                set nameText to ""
                try
                    set roleText to role of elementRef as text
                end try
                try
                    set nameText to name of elementRef as text
                end try
                set end of outputLines to roleText & tab & nameText
            end repeat
            set AppleScript's text item delimiters to linefeed
            return outputLines as text
        end tell
    end tell
end run
APPLESCRIPT
then
  echo "native accessibility observation unavailable: $(tr '\n' ' ' < "$out/ax.err")" >&2
  exit 1
fi
check_accessibility_surface "$out/ax.log"
screencapture -x "$out/screenshot.png"
python3 - "$out/latency.json" "$((ready_ms - start_ms))" <<'PY'
import json, pathlib, sys
pathlib.Path(sys.argv[1]).write_text(json.dumps({"scenario": "native-launch", "latency_ms": int(sys.argv[2]), "budget_ms": 3000}) + "\n")
if int(sys.argv[2]) > 3000:
    raise SystemExit("native launch exceeded 3000 ms")
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
  --scenario native-launch \
  --elapsed-ms "$((ready_ms - start_ms))" \
  --budget-ms 3000 \
  --assertion "installed app launched" \
  --assertion "native accessibility tree is non-empty" \
  --assertion "native accessibility surface exposes a menu bar and named elements" \
  --artifact screenshot=screenshot.png \
  --artifact accessibility=ax.log \
  --artifact measurement=latency.json
