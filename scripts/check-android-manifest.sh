#!/usr/bin/env sh
# Guard against reintroduction of broad Android package visibility.
#
# QUERY_ALL_PACKAGES grants visibility to every installed package. Google Play
# requires a declaration review for it, and the exclusion picker only needs
# ACTION_MAIN+CATEGORY_LAUNCHER intent visibility. This check fails the build
# if the permission reappears without an ADR exemption.
set -eu

manifest="crates/copypaste-ui/src-tauri/gen/android/app/src/main/AndroidManifest.xml"

python3 -m unittest scripts.check_android_manifest_test
python3 scripts/check_android_manifest.py "$manifest" docs/adr

# Capture-ladder contracts that cannot run on a stock emulator still have to
# fail the build when the integration is deleted.
if ! grep -q 'FLAG_SECURE' crates/copypaste-ui/src-tauri/gen/android/app/src/main/java/com/copypaste/app/MainActivity.kt; then
    printf 'FAIL: MainActivity no longer sets FLAG_SECURE before first paint\n' >&2
    exit 1
fi
if ! grep -q 'FLAG_SECURE' crates/copypaste-ui/src-tauri/gen/android/app/src/main/java/com/copypaste/app/ScreenProtectionPlugin.kt; then
    printf 'FAIL: ScreenProtectionPlugin no longer toggles FLAG_SECURE\n' >&2
    exit 1
fi
if grep -R --include='*.kt' -n 'MediaProjection' crates/copypaste-ui/src-tauri/gen/android/app/src/main >/dev/null; then
    printf 'FAIL: MediaProjection appeared in shipping Kotlin; FLAG_SECURE is the capture block\n' >&2
    exit 1
fi
if ! grep -q 'START_NOT_STICKY' crates/copypaste-ui/src-tauri/gen/android/app/src/main/java/com/copypaste/app/CaptureService.kt; then
    printf 'FAIL: CaptureService lost the sticky-restart fail-closed return\n' >&2
    exit 1
fi
if grep -E 'return[[:space:]]+START_STICKY([^_]|$)' crates/copypaste-ui/src-tauri/gen/android/app/src/main/java/com/copypaste/app/CaptureService.kt >/dev/null; then
    printf 'FAIL: CaptureService uses START_STICKY; OEM kills must fail closed\n' >&2
    exit 1
fi
android_kotlin="crates/copypaste-ui/src-tauri/gen/android/app/src/main/java/com/copypaste/app"
if ! python3 - "$android_kotlin" <<'PY'
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
try:
    if not root.is_dir():
        raise OSError
    for source in root.rglob("*.kt"):
        if source.name != "ShizukuClipboard.kt" and re.search(
            r"ShizukuBinderWrapper|IClipboard(?:\\)?\$Stub",
            source.read_text(encoding="utf-8"),
        ):
            print(
                "FAIL: the Shizuku clipboard binder escaped its source-attribution boundary",
                file=sys.stderr,
            )
            sys.exit(1)
except (OSError, UnicodeError):
    print("FAIL: Android Kotlin source inspection failed", file=sys.stderr)
    sys.exit(1)
PY
then
    exit 1
fi
if ! python3 - "$android_kotlin" <<'PY'
import re
import sys
from pathlib import Path

pattern = re.compile(
    r'"(?:getPrimaryClip(?!Source")|setPrimaryClip|clearPrimaryClip|hasPrimaryClip|'
    r'hasClipboardText|addPrimaryClipChangedListener|removePrimaryClipChangedListener)"'
    r"|OnPrimaryClipChangedListener"
)
root = Path(sys.argv[1])
try:
    if not root.is_dir():
        raise OSError
    for source in root.rglob("*"):
        if source.is_dir():
            continue
        if pattern.search(source.read_text(encoding="utf-8")):
            print(
                "FAIL: Shizuku clipboard content transport reappeared in shipping Kotlin",
                file=sys.stderr,
            )
            sys.exit(1)
except (OSError, UnicodeError):
    print("FAIL: Android Kotlin source inspection failed", file=sys.stderr)
    sys.exit(1)
PY
then
    exit 1
fi
printf 'PASS: Android capture-ladder static contracts\n'
