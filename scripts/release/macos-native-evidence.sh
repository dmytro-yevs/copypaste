#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=scripts/release/macos-bundle-lib.sh
. "$REPO_ROOT/scripts/release/macos-bundle-lib.sh"
# shellcheck source=scripts/release/macos-ui-evidence-lib.sh
. "$REPO_ROOT/scripts/release/macos-ui-evidence-lib.sh"

SMOKE_PROFILE="${COPYPASTE_SMOKE_PROFILE:-full}"
case "$SMOKE_PROFILE" in
  full|critical) ;;
  *) echo "COPYPASTE_SMOKE_PROFILE must be full or critical" >&2; exit 2 ;;
esac

should_capture_route_evidence() {
  [[ "$SMOKE_PROFILE" == "full" ]]
}

set_route_feature_state_args() {
  ROUTE_FEATURE_STATE_ARGS=()
  if should_capture_route_evidence; then
    ROUTE_FEATURE_STATE_ARGS=(
      --feature-state "devices=native-shell,screenshot=screenshot.png,accessibility=ax.log"
    )
  fi
}

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
  mac_press_unique_exact_description_role "$navigation" "AXButton" >/dev/null || return 1
  mac_wait_safe_role_label "$heading" "AXHeading" "$state_dir/heading.tsv" 30 || return 1
  mac_capture_state "$state_dir"
}

capture_required_routes() {
  mac_recover_onboarding "$out/onboarding.tsv" 30 || return 1
  capture_route_state history "Library" "Library" || return 1
  if should_capture_route_evidence; then
    capture_route_state devices "Devices" "Devices" || return 1
    capture_route_state settings "Settings" "Settings" || return 1
  fi
}

seed_native_preferences() { # [preferences.json]
  # INV-35: unseeded allowScreenshots=false re-protects the web AX tree after
  # COPYPASTE_EVIDENCE_AX=1. Leave onboardingComplete false.
  local path="${1:-$HOME/Library/Application Support/com.copypaste.app/preferences.json}"
  mkdir -p "$(dirname "$path")"
  (cd "$REPO_ROOT" && node --input-type=module - "$path" <<'JS'
import { readFileSync, writeFileSync } from "node:fs";
import {
  DEFAULT_PREFS,
  PREFERENCES_VERSION,
  STORAGE_KEY,
} from "./crates/copypaste-ui/src/lib/preferenceContract.ts";

const path = process.argv[2];
let store = {};
try {
  const parsed = JSON.parse(readFileSync(path, "utf8"));
  if (parsed !== null && typeof parsed === "object" && !Array.isArray(parsed)) {
    store = parsed;
  }
} catch {}
const state = {
  ...DEFAULT_PREFS,
  allowScreenshots: true,
  onboardingComplete: false,
};
store[STORAGE_KEY] = JSON.stringify({ state, version: PREFERENCES_VERSION });
writeFileSync(path, `${JSON.stringify(store, null, 2)}\n`);
JS
  )
}

mac_launch_evidence_app() {
  open -n -a "$app" --env "COPYPASTE_EVIDENCE_AX=1"
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
  if COPYPASTE_SMOKE_PROFILE=invalid "$REPO_ROOT/scripts/release/macos-native-evidence.sh" --self-test >/dev/null 2>&1; then
    echo "self-test failed: invalid smoke profile was accepted" >&2
    exit 1
  fi
  library_receipt_self_test() {
    python3 - "$REPO_ROOT/scripts/release/write-native-evidence.py" "$fixture_dir/receipt" <<'PY'
import hashlib
import json
import pathlib
import subprocess
import sys

from PIL import Image

writer = pathlib.Path(sys.argv[1])
root = pathlib.Path(sys.argv[2])
history = root / "ui-history"
history.mkdir(parents=True)
qualified = root / "CopyPaste"
qualified.write_bytes(b"fixture app executable\n")
image = Image.new("RGB", (100, 100), "black")
for x in range(50, 100):
    for y in range(100):
        image.putpixel((x, y), (24, 120, 220))
(root / "screenshot.png").parent.mkdir(parents=True, exist_ok=True)
image.save(root / "screenshot.png")
image.save(history / "screenshot.png")
(root / "ax.log").write_text("AXMenuBar\tCopyPaste\n", encoding="utf-8")
(history / "ax.txt").write_text("AXHeading\tLibrary\n", encoding="utf-8")
(history / "heading.tsv").write_text("AXHeading\tLibrary\n", encoding="utf-8")
(root / "latency.json").write_text('{"latency_ms":1}\n', encoding="utf-8")
identity = subprocess.check_output(
    [sys.executable, str(writer), "--capture-qualified-artifact", str(qualified)],
    text=True,
)
receipt = root / "native-evidence.json"
command = [
    sys.executable, str(writer), "--output", str(receipt), "--platform", "macos",
    "--environment", "hosted-runner", "--os-version", "fixture", "--architecture", "arm64",
    "--commit", "a" * 40, "--run-id", "self-test", "--elapsed-ms", "1",
    "--qualified-artifact", str(qualified), "--qualified-artifact-identity", identity,
    "--artifact", "screenshot=screenshot.png", "--artifact", "accessibility=ax.log",
    "--artifact", "screenshot=ui-history/screenshot.png",
    "--artifact", "accessibility=ui-history/ax.txt",
    "--artifact", "accessibility=ui-history/heading.tsv", "--artifact", "measurement=latency.json",
]
result = subprocess.run(command, capture_output=True, text=True)
if result.returncode:
    raise SystemExit(f"Library receipt fixture failed: {result.stderr.strip()}")
records = {
    (record["kind"], record["path"]): record
    for record in json.loads(receipt.read_text(encoding="utf-8"))["artifacts"]
}
expected = {
    ("screenshot", "ui-history/screenshot.png"),
    ("accessibility", "ui-history/ax.txt"),
    ("accessibility", "ui-history/heading.tsv"),
}
if not expected <= records.keys():
    raise SystemExit("Library artifacts were not registered in the receipt")

def matches_receipt():
    for key in expected:
        record = records[key]
        path = root / record["path"]
        if not path.is_file() or path.stat().st_size != record["bytes"]:
            return False
        if hashlib.sha256(path.read_bytes()).hexdigest() != record["sha256"]:
            return False
    return True

def writer_rejects_current_library_proof(name):
    rejected = list(command)
    candidate = root / name
    rejected[rejected.index("--output") + 1] = str(candidate)
    result = subprocess.run(rejected, capture_output=True, text=True)
    if result.returncode == 0 or candidate.exists():
        raise SystemExit(f"{name} accepted invalid Library proof")

if not matches_receipt():
    raise SystemExit("original Library artifacts did not match their receipt")
(history / "screenshot.png").write_bytes(b"altered")
if matches_receipt():
    raise SystemExit("altered Library screenshot matched the original receipt")
writer_rejects_current_library_proof("altered-library.json")
image.save(history / "screenshot.png")
(history / "ax.txt").unlink()
if matches_receipt():
    raise SystemExit("missing Library accessibility proof matched the original receipt")
writer_rejects_current_library_proof("missing-library.json")
PY
  }
  if library_receipt_self_test; then
    ok "Library UI artifacts are receipt-bound and detect alteration or removal"
  else
    bad "Library UI artifacts are receipt-bound and detect alteration or removal"
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
    local original_ax mode=delayed library_queries=0 explore_queries=0 presses=0 paced_seconds=0 recovery_trace
    original_ax="$(declare -f mac_ax)"
    mac_recovery_fixture_clock() {
      printf '%s\n' "$paced_seconds"
    }
    mac_recovery_fixture_pace() {
      paced_seconds=$((paced_seconds + $1))
      recovery_trace="${recovery_trace:+$recovery_trace }pace-tick"
    }
    mac_ax() {
      case "$1" in
        find-unique-exact-description-role)
          if [[ "$2" == "Library" && "$3" == "AXButton" ]]; then
            library_queries=$((library_queries + 1))
            if [[ "$mode" == already-past || ( "$mode" == delayed && "$library_queries" -gt 1 ) ]]; then
              recovery_trace="${recovery_trace:+$recovery_trace }library-success"
              printf 'AXButton\tLibrary\n'
              return 0
            fi
            recovery_trace="${recovery_trace:+$recovery_trace }library-miss"
          fi
          echo "no accessible element named $2" >&2
          return 1
          ;;
        find-safe-role)
          if [[ "$2" == "Explore first" && "$3" == "AXButton" && "$mode" == delayed ]]; then
            explore_queries=$((explore_queries + 1))
            recovery_trace="${recovery_trace:+$recovery_trace }explore-find"
            printf 'AXButton\tExplore first\n'
            return 0
          fi
          echo "no accessible element named $2" >&2
          return 1
          ;;
        press-exact)
          [[ "$2" == "Explore first" && "$mode" == delayed ]] || return 1
          presses=$((presses + 1))
          recovery_trace="${recovery_trace:+$recovery_trace }explore-press"
          return 0
          ;;
        *) return 97 ;;
      esac
    }
    mac_recover_onboarding "$out/recovery-delayed.tsv" 2 mac_recovery_fixture_pace mac_recovery_fixture_clock \
      && [[ "$recovery_trace" == "library-miss explore-find explore-press pace-tick library-success" && "$presses" == 1 && "$library_queries" == 2 && "$explore_queries" == 1 && "$paced_seconds" == 1 ]] \
      && ok "delayed onboarding presses Explore first once before Library appears" \
      || bad "delayed onboarding presses Explore first once before Library appears"
    mode=already-past
    paced_seconds=0
    library_queries=0
    explore_queries=0
    presses=0
    recovery_trace=""
    mac_recover_onboarding "$out/recovery-past.tsv" 2 mac_recovery_fixture_pace mac_recovery_fixture_clock \
      && [[ "$recovery_trace" == "library-success" && "$presses" == 0 && "$library_queries" == 1 && "$explore_queries" == 0 && "$paced_seconds" == 0 ]] \
      && ok "already-past onboarding accepts a description Library control" \
      || bad "already-past onboarding accepts a description Library control"
    mode=absent
    paced_seconds=0
    recovery_trace=""
    if mac_recover_onboarding "$out/recovery-absent.tsv" 1 mac_recovery_fixture_pace mac_recovery_fixture_clock; then
      bad "absent onboarding controls fail by deadline"
    else
      local recovery_status=$?
      if [[ "$recovery_status" == 1 && "$paced_seconds" == 1 && "$recovery_trace" == "library-miss pace-tick" ]]; then
        ok "absent onboarding controls fail by deadline"
      else
        bad "absent onboarding controls fail by deadline"
      fi
    fi
    mode=provider-error
    mac_ax() { echo "System Events provider denied request" >&2; return 1; }
    paced_seconds=0
    if mac_recover_onboarding "$out/recovery-error.tsv" 1 mac_recovery_fixture_pace mac_recovery_fixture_clock; then
      bad "provider errors do not masquerade as absent controls"
    else
      local recovery_status=$?
      if [[ "$recovery_status" == 2 && "$paced_seconds" == 0 ]]; then
        ok "provider errors do not masquerade as absent controls"
      else
        bad "provider errors do not masquerade as absent controls"
      fi
    fi
    unset -f mac_ax mac_recovery_fixture_clock mac_recovery_fixture_pace
    eval "$original_ax"
  }
  mac_recovery_self_test
  mac_launcher_self_test() {
    local observed expected
    app="$fixture_dir/CopyPaste.app"
    open() { observed="$(printf '<%s>' "$@")"; }
    mac_launch_evidence_app
    expected="<-n><-a><$app><--env><COPYPASTE_EVIDENCE_AX=1>"
    [[ "$observed" == "$expected" ]] \
      && ok "installed-app launcher includes the exact AX evidence flag" \
      || bad "installed-app launcher includes the exact AX evidence flag"
    unset -f open
  }
  mac_launcher_self_test
  native_preference_seed_self_test() {
    local path="$fixture_dir/preferences.json" script="$REPO_ROOT/scripts/release/macos-native-evidence.sh"
    cat > "$path" <<'JSON'
{
  "unrelated": "kept",
  "copypaste.prefs": "{\"state\":{\"allowScreenshots\":\"no\",\"onboardingComplete\":true},\"version\":1}"
}
JSON
    if seed_native_preferences "$path" && (cd "$REPO_ROOT" && node --input-type=module - "$path" <<'JS'
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  DEFAULT_PREFS,
  PREFERENCES_VERSION,
  STORAGE_KEY,
} from "./crates/copypaste-ui/src/lib/preferenceContract.ts";

const store = JSON.parse(readFileSync(process.argv[2], "utf8"));
const blob = JSON.parse(store[STORAGE_KEY]);
assert.equal(store.unrelated, "kept");
assert.deepEqual(Object.keys(blob).sort(), ["state", "version"]);
assert.equal(blob.version, PREFERENCES_VERSION);
assert.equal(blob.state.allowScreenshots, true);
assert.equal(blob.state.onboardingComplete, false);
assert.notEqual(blob.state.onboardingComplete, true);
assert.deepEqual(blob.state, {
  ...DEFAULT_PREFS,
  allowScreenshots: true,
  onboardingComplete: false,
});
JS
    )
    then
      ok "native seed writes screenshots on and onboarding incomplete"
    else
      bad "native seed writes screenshots on and onboarding incomplete"
    fi
    if grep -E '^[[:space:]]*seed_onboarding_complete(\b|$)' "$script" >/dev/null; then
      bad "production native path never calls seed_onboarding_complete"
    else
      ok "production native path never calls seed_onboarding_complete"
    fi
  }
  native_preference_seed_self_test
  mac_prepare_order_self_test() {
    local original_ax recorded=() paced_seconds=0
    original_ax="$(declare -f mac_ax)"
    mac_prepare_order_clock() { printf '%s\n' "$paced_seconds"; }
    mac_prepare_order_pace() { paced_seconds=$((paced_seconds + $1)); }
    mac_ax() {
      recorded+=("$1${2:+ $2}${3:+ $3}")
      case "$1" in
        enable|menu-press) return 0 ;;
        find-unique-exact-description-role|find-safe-role)
          echo "no accessible element named $2" >&2
          return 1
          ;;
        *) return 97 ;;
      esac
    }
    mac_prepare_webview_ax
    if mac_recover_onboarding "$out/prepare-absent.tsv" 1 mac_prepare_order_pace mac_prepare_order_clock; then
      bad "prepare is not treated as Library/Explore success"
    else
      ok "prepare is not treated as Library/Explore success"
    fi
    if [[ "${recorded[0]:-}" == "enable" && "${recorded[1]:-}" == "menu-press Show CopyPaste" && "${recorded[2]:-}" == "find-unique-exact-description-role Library AXButton" ]]; then
      ok "prepare enables AX and shows CopyPaste before the first Library probe"
    else
      bad "prepare enables AX and shows CopyPaste before the first Library probe"
    fi
    unset -f mac_ax mac_prepare_order_clock mac_prepare_order_pace
    eval "$original_ax"
  }
  mac_prepare_order_self_test
  mac_description_role_self_test() {
    local original_ax rows presses=0 dispatcher="$fixture_dir/mac-ax-body.txt"
    original_ax="$(declare -f mac_ax)"
    declare -f mac_ax > "$dispatcher"
    if grep -Fq 'find-unique-exact-description-role' "$dispatcher" \
      && grep -Fq 'press-unique-exact-description-role' "$dispatcher" \
      && grep -Fq 'if descriptionText is targetLabel and roleText is inputValue then' "$dispatcher" \
      && ! grep -Fq 'descriptionText contains targetLabel and roleText' "$dispatcher"; then
      ok "unique description modes use AppleScript equality"
    else
      bad "unique description modes use AppleScript equality"
    fi
    mac_ax() {
      case "$1" in
        find-unique-exact-description-role)
          printf '%s' "$rows" | mac_unique_exact_description_role_label "$2" "$3"
          ;;
        press-unique-exact-description-role)
          printf '%s' "$rows" | mac_unique_exact_description_role_label "$2" "$3" >/dev/null || return 1
          presses=$((presses + 1))
          printf 'ok\n'
          ;;
        *) return 97 ;;
      esac
    }
    rows=$'AXButton\tmissing value\tLibrary\n'
    if mac_find_unique_exact_description_role "Library" "AXButton" >/dev/null \
      && mac_press_unique_exact_description_role "Library" "AXButton" >/dev/null \
      && [[ "$presses" == 1 ]]; then
      ok "unique description Library button matches and presses"
    else
      bad "unique description Library button matches and presses"
    fi
    presses=0
    rows=""
    if mac_find_unique_exact_description_role "Library" "AXButton" >/dev/null \
      || mac_press_unique_exact_description_role "Library" "AXButton" >/dev/null \
      || (( presses != 0 )); then
      bad "unique description matcher rejects zero hits"
    else
      ok "unique description matcher rejects zero hits"
    fi
    rows=$'AXButton\tmissing value\tLibrary\nAXButton\tmissing value\tLibrary\n'
    if mac_find_unique_exact_description_role "Library" "AXButton" >/dev/null \
      || mac_press_unique_exact_description_role "Library" "AXButton" >/dev/null \
      || (( presses != 0 )); then
      bad "unique description matcher rejects duplicate hits"
    else
      ok "unique description matcher rejects duplicate hits"
    fi
    rows=$'AXHeading\tmissing value\tLibrary\n'
    if mac_find_unique_exact_description_role "Library" "AXButton" >/dev/null \
      || mac_press_unique_exact_description_role "Library" "AXButton" >/dev/null \
      || (( presses != 0 )); then
      bad "unique description matcher rejects wrong-role matches"
    else
      ok "unique description matcher rejects wrong-role matches"
    fi
    rows=$'AXButton\tLibrary\t\n'
    if mac_find_unique_exact_description_role "Library" "AXButton" >/dev/null \
      || mac_press_unique_exact_description_role "Library" "AXButton" >/dev/null \
      || (( presses != 0 )); then
      bad "unique description matcher rejects name-only matches"
    else
      ok "unique description matcher rejects name-only matches"
    fi
    rows=$'AXButton\tmissing value\tMy Library\nAXButton\tmissing value\tLibrary \n'
    if mac_find_unique_exact_description_role "Library" "AXButton" >/dev/null \
      || mac_press_unique_exact_description_role "Library" "AXButton" >/dev/null \
      || (( presses != 0 )); then
      bad "unique description matcher rejects substring matches"
    else
      ok "unique description matcher rejects substring matches"
    fi
    unset -f mac_ax
    eval "$original_ax"
  }
  mac_description_role_self_test
  profile_self_test() {
    local saved_profile="$SMOKE_PROFILE"
    SMOKE_PROFILE=full
    set_route_feature_state_args
    if ! should_capture_route_evidence || [[ "${#ROUTE_FEATURE_STATE_ARGS[@]}" != 2 ]]; then
      bad "full profile captures the route inventory"
    fi
    SMOKE_PROFILE=critical
    set_route_feature_state_args
    if should_capture_route_evidence || [[ "${#ROUTE_FEATURE_STATE_ARGS[@]}" != 0 ]]; then
      bad "critical profile omits the route inventory"
    fi
    SMOKE_PROFILE="$saved_profile"
  }
  profile_self_test
  critical_array_expansion_self_test() {
    local expanded
    if expanded="$(/bin/bash -c 'set -u; ROUTE_FEATURE_STATE_ARGS=(); printf "%s\\n" before "${ROUTE_FEATURE_STATE_ARGS[@]+${ROUTE_FEATURE_STATE_ARGS[@]}}" after')" \
      && [[ "$expanded" == $'before\nafter' ]]; then
      ok "critical profile expands empty receipt arguments under Bash 3.2"
    else
      bad "critical profile expands empty receipt arguments under Bash 3.2"
    fi
  }
  critical_array_expansion_self_test
  native_production_order_self_test() {
    local script="$REPO_ROOT/scripts/release/macos-native-evidence.sh"
    if python3 - "$script" <<'PY'
from pathlib import Path
import sys
text = Path(sys.argv[1]).read_text()
marker = 'echo "macOS native accessibility self-test passed"\n  exit 0\nfi\n'
prod = text.split(marker, 1)[1]
names = [
    "seed_native_preferences",
    "mac_launch_evidence_app",
    "mac_ax ready",
    "mac_prepare_webview_ax",
    "mac_ax surface",
    "capture_required_routes",
]
idxs = [prod.index(name) for name in names]
sys.exit(0 if idxs == sorted(idxs) else 1)
PY
    then
      ok "production seeds, prepares, then reaches required routes in order"
    else
      bad "production seeds, prepares, then reaches required routes in order"
    fi
  }
  native_production_order_self_test
  route_presses=()
  onboarding_recoveries=0
  route_failure=""
  mac_press_unique_exact_description_role() {
    route_presses+=("$1 $2")
    [[ "$2" == "AXButton" && "$1" != "$route_failure" ]]
  }
  mac_wait_safe_role_label() {
    [[ "$2" == "AXHeading" ]] || return 1
    printf 'AXHeading\t%s\n' "$1" > "$3"
  }
  mac_capture_state() { mkdir -p "$1"; printf 'AXHeading\n' > "$1/ax.txt"; printf 'png' > "$1/screenshot.png"; }
  mac_recover_onboarding() {
    [[ "$1" == "$out/onboarding.tsv" ]] || return 1
    onboarding_recoveries=$((onboarding_recoveries + 1))
    printf 'AXButton\tExplore first\n' > "$1"
  }
  route_profile_self_test() {
    local saved_profile="$SMOKE_PROFILE"
    SMOKE_PROFILE=critical
    route_presses=()
    onboarding_recoveries=0
    route_failure=""
    if capture_required_routes \
      && [[ "${route_presses[*]}" == "Library AXButton" ]] \
      && [[ "$onboarding_recoveries" == 1 ]] \
      && [[ -s "$out/ui-history/heading.tsv" && -s "$out/ui-history/ax.txt" && -s "$out/ui-history/screenshot.png" ]]; then
      ok "critical profile reaches onboarding and Library with native artifacts"
    else
      bad "critical profile reaches onboarding and Library with native artifacts"
    fi
    SMOKE_PROFILE=full
    route_presses=()
    onboarding_recoveries=0
    if capture_required_routes \
      && [[ "${route_presses[*]}" == "Library AXButton Devices AXButton Settings AXButton" ]] \
      && [[ "$onboarding_recoveries" == 1 ]]; then
      ok "full profile retains the complete route inventory"
    else
      bad "full profile retains the complete route inventory"
    fi
    SMOKE_PROFILE=critical
    route_failure="Library"
    if capture_required_routes; then
      bad "critical Library route failures propagate"
    else
      ok "critical Library route failures propagate"
    fi
    route_failure=""
    SMOKE_PROFILE="$saved_profile"
  }
  route_profile_self_test
  unset -f osascript mac_press_unique_exact_description_role mac_wait_safe_role_label mac_capture_state mac_recover_onboarding mac_capture_state_self_test
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
seed_native_preferences

start_ms="$(python3 -c 'import time; print(time.time_ns() // 1000000)')"
mac_launch_evidence_app
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
mac_prepare_webview_ax
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

set_route_feature_state_args
# The critical profile reaches the first real WebView route. The full profile
# also records Devices and Settings for the manual/nightly evidence inventory.
capture_required_routes || {
  echo "Required macOS UI route did not expose its heading and artifacts" >&2
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
  "${ROUTE_FEATURE_STATE_ARGS[@]+${ROUTE_FEATURE_STATE_ARGS[@]}}" \
  --artifact screenshot=screenshot.png \
  --artifact accessibility=ax.log \
  --artifact screenshot=ui-history/screenshot.png \
  --artifact accessibility=ui-history/ax.txt \
  --artifact accessibility=ui-history/heading.tsv \
  --artifact measurement=latency.json
