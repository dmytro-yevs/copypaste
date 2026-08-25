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
source_bridge="$android_kotlin/ShizukuClipboard.kt"
if ! grep -q 'getPrimaryClipSource' "$source_bridge"; then
    printf 'FAIL: Android source attribution no longer asks getPrimaryClipSource\n' >&2
    exit 1
fi
if rg -n --glob '*.kt' --glob '!ShizukuClipboard.kt' \
    'ShizukuBinderWrapper|IClipboard\\$Stub' "$android_kotlin" >/dev/null; then
    printf 'FAIL: the Shizuku clipboard binder escaped its source-attribution boundary\n' >&2
    exit 1
fi
if rg --pcre2 -n \
    '"(?:getPrimaryClip(?!Source")|setPrimaryClip|clearPrimaryClip|hasPrimaryClip|hasClipboardText|addPrimaryClipChangedListener|removePrimaryClipChangedListener)"|OnPrimaryClipChangedListener' \
    "$android_kotlin" >/dev/null; then
    printf 'FAIL: Shizuku clipboard content transport reappeared in shipping Kotlin\n' >&2
    exit 1
fi
printf 'PASS: Android capture-ladder static contracts\n'
