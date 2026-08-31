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

capture_route_state() { # <state> <navigation label> <heading>
  local navigation="$2" heading="$3" state_dir="$out/ui-$1"
  mkdir -p "$state_dir"
  mac_press_exact_button "$navigation" >/dev/null || return 1
  mac_wait_safe_role_label "$heading" "AXHeading" "$state_dir/heading.tsv" 30 || return 1
  mac_capture_state "$state_dir"
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
  out="$fixture_dir/ui"
  osascript() { echo "self-test attempted a native accessibility command" >&2; return 97; }
  mac_capture_state_self_test() {
    local original_ax
    original_ax="$(declare -f mac_ax)"
    local screenshot_calls=0
    mac_ax() { return 1; }
    screencapture() { screenshot_calls=$((screenshot_calls + 1)); return 97; }
    if mac_capture_state "$out/failed-ax"; then bad "failed AX dumps cannot produce artifacts"; else ok "failed AX dumps cannot produce artifacts"; fi
    [[ "$screenshot_calls" == 0 ]] \
      && ok "failed AX dumps do not invoke screenshot capture" \
      || bad "failed AX dumps do not invoke screenshot capture"
    mac_ax() { printf 'AXHeading\tLibrary\n'; }
    screencapture() { screenshot_calls=$((screenshot_calls + 1)); return 1; }
    mkdir -p "$out/stale-screenshot"
    printf 'stale ax' > "$out/stale-screenshot/ax.txt"
    printf 'stale png' > "$out/stale-screenshot/screenshot.png"
    if mac_capture_state "$out/stale-screenshot"; then bad "stale screenshots cannot mask capture failure"; else ok "stale screenshots cannot mask capture failure"; fi
    [[ "$screenshot_calls" == 1 && "$(cat "$out/stale-screenshot/ax.txt")" == "stale ax" && "$(cat "$out/stale-screenshot/screenshot.png")" == "stale png" ]] \
      && ok "failed screenshots preserve prior artifacts" \
      || bad "failed screenshots preserve prior artifacts"
    screenshot_calls=0
    screencapture() { screenshot_calls=$((screenshot_calls + 1)); :; }
    if mac_capture_state "$out/empty-png"; then bad "empty screenshots cannot produce artifacts"; else ok "empty screenshots cannot produce artifacts"; fi
    [[ "$screenshot_calls" == 1 ]] \
      && ok "empty PNG fixtures exercise the screenshot command" \
      || bad "empty PNG fixtures exercise the screenshot command"
    mac_ax() { printf 'AXHeading\tLibrary\n'; }
    screencapture() { printf 'png' > "$2"; }
    if mac_capture_state "$out/fresh-success" \
      && [[ -s "$out/fresh-success/ax.txt" && -s "$out/fresh-success/screenshot.png" ]]; then
      ok "fresh native artifacts are committed only after both commands succeed"
    else
      bad "fresh native artifacts are committed only after both commands succeed"
    fi
    unset -f mac_ax screencapture
    eval "$original_ax"
  }
  mac_capture_state_self_test
  mac_recovery_self_test() {
    local original_ax mode=delayed library_queries=0 explore_queries=0 presses=0
    original_ax="$(declare -f mac_ax)"
    mac_ax() {
      case "$1" in
        find-safe-role)
          if [[ "$2" == "Library" ]]; then
            library_queries=$((library_queries + 1))
            if [[ "$mode" == delayed && "$library_queries" -gt 1 ]]; then
              printf 'AXButton\tLibrary\n'
              return 0
            fi
          elif [[ "$2" == "Explore first" && "$mode" == delayed ]]; then
            explore_queries=$((explore_queries + 1))
            printf 'AXButton\tExplore first\n'
            return 0
          fi
          echo "no accessible element named $2" >&2
          return 1
          ;;
        press-exact)
          [[ "$2" == "Explore first" && "$mode" == delayed ]] || return 1
          presses=$((presses + 1))
          return 0
          ;;
        *) return 97 ;;
      esac
    }
    mac_recover_onboarding "$out/recovery-delayed.tsv" 2 \
      && [[ "$presses" == 1 && "$library_queries" -ge 2 && "$explore_queries" == 1 ]] \
      && ok "delayed onboarding presses Explore first once before Library appears" \
      || bad "delayed onboarding presses Explore first once before Library appears"
    mode=absent
    if mac_recover_onboarding "$out/recovery-absent.tsv" 1; then
      bad "absent onboarding controls fail by deadline"
    else
      local recovery_status=$?
      if [[ "$recovery_status" == 1 ]]; then
        ok "absent onboarding controls fail by deadline"
      else
        bad "absent onboarding controls fail by deadline"
      fi
    fi
    mode=provider-error
    mac_ax() { echo "System Events provider denied request" >&2; return 1; }
    if mac_recover_onboarding "$out/recovery-error.tsv" 1; then
      bad "provider errors do not masquerade as absent controls"
    else
      local recovery_status=$?
      if [[ "$recovery_status" == 2 ]]; then
        ok "provider errors do not masquerade as absent controls"
      else
        bad "provider errors do not masquerade as absent controls"
      fi
    fi
    unset -f mac_ax
    eval "$original_ax"
  }
  mac_recovery_self_test
  mac_press_exact_button() { [[ "$1" == "Library" || "$1" == "Explore first" ]]; }
  mac_wait_safe_role_label() {
    [[ "$1" == "Library" && "$2" == "AXHeading" ]] || return 1
    printf 'AXHeading\tLibrary\n' > "$3"
  }
  mac_capture_state() { mkdir -p "$1"; printf 'AXHeading\tLibrary\n' > "$1/ax.txt"; printf 'png' > "$1/screenshot.png"; }
  mac_recover_onboarding() { [[ "$1" == "$out/onboarding.tsv" ]] && printf 'AXButton\tExplore first\n' > "$1"; }
  mac_recover_onboarding "$out/onboarding.tsv" 2 || bad "onboarding recovery remains bounded"
  capture_route_state history "Library" "Library" \
    && [[ -s "$out/ui-history/heading.tsv" && -s "$out/ui-history/ax.txt" && -s "$out/ui-history/screenshot.png" ]] \
    && ok "route evidence requires navigation, a unique heading, and both artifacts" \
    || bad "route evidence requires navigation, a unique heading, and both artifacts"
  if capture_route_state settings "Settings" "Settings"; then
    bad "wrong routes cannot produce route evidence"
  else
    ok "wrong routes cannot produce route evidence"
  fi
  unset -f osascript mac_press_exact_button mac_wait_safe_role_label mac_capture_state mac_recover_onboarding mac_capture_state_self_test
  [[ "$FAIL" -eq 0 ]] || exit 1
  echo "macOS native accessibility self-test passed"
  exit 0
fi

out="${1:?usage: macos-native-evidence.sh OUTPUT_DIRECTORY QUALIFIED_ARTIFACT QUALIFIED_ARTIFACT_IDENTITY}"
qualified_artifact="${2:?usage: macos-native-evidence.sh OUTPUT_DIRECTORY QUALIFIED_ARTIFACT QUALIFIED_ARTIFACT_IDENTITY}"
qualified_artifact_identity="${3:?usage: macos-native-evidence.sh OUTPUT_DIRECTORY QUALIFIED_ARTIFACT QUALIFIED_ARTIFACT_IDENTITY}"
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

# These exact-DMG observations are deliberately not receipt feature states:
# root owns the ledger bindings. Each route requires its source-confirmed
# navigation control, its own AXHeading, and both native artifacts.
mac_recover_onboarding "$out/onboarding.tsv" 30 || {
  echo "Onboarding recovery could not reach the exact Explore first control" >&2
  exit 1
}
capture_route_state history "Library" "Library" || {
  echo "History route did not expose its Library heading and artifacts" >&2
  exit 1
}
capture_route_state devices "Devices" "Devices" || {
  echo "Devices route did not expose its Devices heading and artifacts" >&2
  exit 1
}
capture_route_state settings "Settings" "Settings" || {
  echo "Settings route did not expose its Settings heading and artifacts" >&2
  exit 1
}

python3 scripts/release/write-native-evidence.py \
  --output "$out/native-evidence.json" \
  --platform macos \
  --environment hosted-runner \
  --os-version "$(sw_vers -productVersion)" \
  --architecture "$(uname -m)" \
  --commit "${GITHUB_SHA:-$(git rev-parse HEAD)}" \
  --run-id "${GITHUB_RUN_ID:-local-$(git rev-parse --short HEAD)}" \
  --elapsed-ms "$((ready_ms - start_ms))" \
  --qualified-artifact "$qualified_artifact" \
  --qualified-artifact-identity "$qualified_artifact_identity" \
  --feature-state devices=native-shell,screenshot=screenshot.png,accessibility=ax.log \
  --artifact screenshot=screenshot.png \
  --artifact accessibility=ax.log \
  --artifact measurement=latency.json
