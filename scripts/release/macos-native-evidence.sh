#!/usr/bin/env bash
set -euo pipefail

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

osascript -e 'tell application "System Events" to tell process "CopyPaste" to get {name, role, description} of every UI element of window 1' > "$out/ax.log"
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
