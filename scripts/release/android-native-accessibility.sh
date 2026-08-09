#!/usr/bin/env bash
set -uo pipefail

PKG="${PKG:-com.copypaste.app}"
OUT="${SMOKE_OUT:-artifacts/android-smoke}"

check_tree() {
    python3 - "$1" "$PKG" <<'PY'
import sys
import xml.etree.ElementTree as ET

path, package = sys.argv[1:]
try:
    root = ET.parse(path).getroot()
except (OSError, ET.ParseError) as error:
    raise SystemExit(f"native accessibility observation unavailable: {error}")

owned = [node for node in root.iter("node") if node.get("package") == package]
if not owned:
    raise SystemExit(f"native accessibility tree contains no nodes owned by {package}")

def has_name(node):
    return any((node.get(attribute) or "").strip() for attribute in ("text", "content-desc"))

named = [node for node in owned if has_name(node)]
actionable = [node for node in owned if node.get("clickable") == "true" or node.get("focusable") == "true"]
unnamed = [node for node in actionable if not has_name(node)]
classes = {node.get("class", "") for node in owned}

if len(named) < 3:
    raise SystemExit(f"native accessibility tree exposes only {len(named)} named CopyPaste nodes")
if not actionable:
    raise SystemExit("native accessibility tree exposes no actionable CopyPaste nodes")
if unnamed:
    sample = ", ".join(node.get("class", "unknown") for node in unnamed[:5])
    raise SystemExit(f"{len(unnamed)} actionable CopyPaste nodes have no accessible name: {sample}")
if not any("WebView" in name for name in classes):
    raise SystemExit("native accessibility tree contains no Android WebView surface")

print(f"native accessibility surface: {len(owned)} nodes, {len(named)} named, {len(actionable)} actionable")
PY
}

self_test() {
    local dir good unnamed no_webview no_action too_few
    dir="$(mktemp -d)"
    trap 'rm -rf -- "$dir"' RETURN
    good="$dir/good.xml"
    unnamed="$dir/unnamed.xml"
    no_webview="$dir/no-webview.xml"
    no_action="$dir/no-action.xml"
    too_few="$dir/too-few.xml"
    printf '%s\n' '<hierarchy><node package="com.copypaste.app" class="android.webkit.WebView" text="CopyPaste"><node package="com.copypaste.app" class="android.widget.Button" text="History" clickable="true" focusable="true"/><node package="com.copypaste.app" class="android.widget.Button" content-desc="Settings" clickable="true"/><node package="com.copypaste.app" class="android.widget.EditText" text="Search" focusable="true"/></node></hierarchy>' > "$good"
    printf '%s\n' '<hierarchy><node package="com.copypaste.app" class="android.webkit.WebView" text="CopyPaste"><node package="com.copypaste.app" class="android.widget.Button" text="History" clickable="true"/><node package="com.copypaste.app" class="android.widget.Button" content-desc="Settings" clickable="true"/><node package="com.copypaste.app" class="android.widget.EditText" text="Search" focusable="true"/><node package="com.copypaste.app" class="android.widget.Button" clickable="true"/></node></hierarchy>' > "$unnamed"
    printf '%s\n' '<hierarchy><node package="com.copypaste.app" class="android.widget.FrameLayout" text="CopyPaste"><node package="com.copypaste.app" class="android.widget.Button" text="History" clickable="true"/><node package="com.copypaste.app" class="android.widget.Button" text="Settings" clickable="true"/></node></hierarchy>' > "$no_webview"
    printf '%s\n' '<hierarchy><node package="com.copypaste.app" class="android.webkit.WebView" text="CopyPaste"><node package="com.copypaste.app" class="android.widget.TextView" text="History"/><node package="com.copypaste.app" class="android.widget.TextView" text="Settings"/></node></hierarchy>' > "$no_action"
    printf '%s\n' '<hierarchy><node package="com.copypaste.app" class="android.webkit.WebView" text="CopyPaste"><node package="com.copypaste.app" class="android.widget.Button" text="History" clickable="true"/></node></hierarchy>' > "$too_few"
    check_tree "$good" >/dev/null || return 1
    reject_fixture "$unnamed" "actionable CopyPaste nodes have no accessible name" || return 1
    reject_fixture "$no_webview" "contains no Android WebView surface" || return 1
    reject_fixture "$no_action" "exposes no actionable CopyPaste nodes" || return 1
    reject_fixture "$too_few" "exposes only 2 named CopyPaste nodes" || return 1
    echo "android native accessibility self-test passed"
}

reject_fixture() {
    local output
    if output="$(check_tree "$1" 2>&1)"; then
        echo "self-test failed: invalid fixture passed: $1" >&2
        return 1
    fi
    if [[ "$output" != *"$2"* ]]; then
        echo "self-test failed: invalid fixture reached the wrong rejection: $output" >&2
        return 1
    fi
}

if [[ "${1:-}" == "--self-test" ]]; then
    self_test
    exit $?
fi

command -v python3 >/dev/null 2>&1 || { echo "native accessibility observation unavailable: python3 not found" >&2; exit 1; }

dump="${NATIVE_AX_TREE:-}"
if [[ -z "$dump" ]]; then
    command -v adb >/dev/null 2>&1 || { echo "native accessibility observation unavailable: adb not found" >&2; exit 1; }
    mkdir -p "$OUT"
    dump="$OUT/native-accessibility.xml"
    rm -f "$dump"
    adb shell uiautomator dump /sdcard/copypaste-native-accessibility.xml >/dev/null 2>&1 || {
        echo "native accessibility observation unavailable: uiautomator dump failed" >&2
        exit 1
    }
    adb pull /sdcard/copypaste-native-accessibility.xml "$dump" >/dev/null 2>&1 || {
        echo "native accessibility observation unavailable: could not retrieve uiautomator tree" >&2
        exit 1
    }
fi

[[ -s "$dump" ]] || { echo "native accessibility observation unavailable: empty uiautomator tree" >&2; exit 1; }
check_tree "$dump"
