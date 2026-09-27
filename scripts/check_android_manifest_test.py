#!/usr/bin/env python3
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest

from scripts.check_android_manifest import (
    ManifestError,
    REQUIRED_PERMISSIONS,
    manifest_errors,
    validate_manifest_file,
)


REPO = Path(__file__).resolve().parent.parent
CURRENT_MANIFEST = (
    REPO
    / "crates/copypaste-ui/src-tauri/gen/android/app/src/main/AndroidManifest.xml"
)
CAPTURE_GUARD = REPO / "scripts/check-android-manifest.sh"
ANDROID_KOTLIN = Path(
    "crates/copypaste-ui/src-tauri/gen/android/app/src/main/java/com/copypaste/app"
)


def fixture(
    *,
    permission=True,
    launcher=True,
    shizuku_query=True,
    capture_service=True,
):
    permissions = []
    for name in sorted(REQUIRED_PERMISSIONS):
        node = f'<uses-permission android:name="{name}" />'
        permissions.append(
            node
            if permission or name != "android.permission.READ_LOGS"
            else f"<!-- {node} -->"
        )
    launcher_node = """
      <intent>
        <action android:name="android.intent.action.MAIN" />
        <category android:name="android.intent.category.LAUNCHER" />
      </intent>"""
    if not launcher:
        launcher_node = f"<!-- {launcher_node} -->"
    shizuku_node = '<package android:name="moe.shizuku.privileged.api" />'
    if not shizuku_query:
        shizuku_node = f"<!-- {shizuku_node} -->"
    capture_node = """
      <service android:name=".CaptureService" android:exported="false"
          android:foregroundServiceType="specialUse">
        <property android:name="android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE"
            android:value="capture purpose" />
      </service>"""
    if not capture_service:
        capture_node = f"<!-- {capture_node} -->"
    return f"""<?xml version="1.0"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
  {' '.join(permissions)}
  <!-- <uses-permission android:name="android.permission.QUERY_ALL_PACKAGES" /> -->
  <queries>
    {launcher_node}
    <intent>
      <action android:name="android.intent.action.MAIN" />
      <category android:name="android.intent.category.LEANBACK_LAUNCHER" />
    </intent>
    {shizuku_node}
  </queries>
  <application>
    <service android:name=".CaptureTileService" android:exported="true"
        android:permission="android.permission.BIND_QUICK_SETTINGS_TILE">
      <intent-filter>
        <action android:name="android.service.quicksettings.action.QS_TILE" />
      </intent-filter>
    </service>
    {capture_node}
    <provider android:name="rikka.shizuku.ShizukuProvider"
        android:enabled="true" android:exported="true"
        android:permission="android.permission.INTERACT_ACROSS_USERS_FULL" />
  </application>
</manifest>
"""


class AndroidManifestCheckTest(unittest.TestCase):
    def test_current_manifest_passes(self):
        self.assertEqual(manifest_errors(CURRENT_MANIFEST.read_text()), [])

    def test_missing_manifest_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(ManifestError, "missing or unreadable"):
                validate_manifest_file(root / "missing.xml", root / "adr")

    def test_commented_permission_does_not_count(self):
        self.assertTrue(
            any(
                "READ_LOGS" in error
                for error in manifest_errors(fixture(permission=False))
            )
        )

    def test_commented_launcher_query_does_not_count(self):
        self.assertTrue(
            any("LAUNCHER" in error for error in manifest_errors(fixture(launcher=False)))
        )

    def test_commented_shizuku_query_does_not_count(self):
        self.assertTrue(
            any(
                "moe.shizuku" in error
                for error in manifest_errors(fixture(shizuku_query=False))
            )
        )

    def test_commented_service_does_not_count(self):
        self.assertTrue(
            any(
                "CaptureService" in error
                for error in manifest_errors(fixture(capture_service=False))
            )
        )

    def test_commented_forbidden_permission_is_ignored(self):
        self.assertEqual(manifest_errors(fixture()), [])

    def test_active_forbidden_permission_requires_an_exemption(self):
        source = fixture().replace(
            '<!-- <uses-permission android:name="android.permission.QUERY_ALL_PACKAGES" /> -->',
            '<uses-permission android:name="android.permission.QUERY_ALL_PACKAGES" />',
        )
        self.assertTrue(
            any("requires an ADR" in error for error in manifest_errors(source))
        )
        self.assertEqual(manifest_errors(source, query_all_packages_exempt=True), [])

    def test_malformed_xml_fails_closed(self):
        self.assertTrue(
            any("not valid XML" in error for error in manifest_errors("<manifest>"))
        )


class AndroidCaptureGuardTest(unittest.TestCase):
    @staticmethod
    def run_guard(overrides=None, directories=None):
        files = {
            "MainActivity.kt": "window.setFlags(FLAG_SECURE, FLAG_SECURE)\n",
            "ScreenProtectionPlugin.kt": "window.clearFlags(FLAG_SECURE)\n",
            "CaptureService.kt": "return START_NOT_STICKY\n",
            "ShizukuClipboard.kt": "internal fun sourcePackage(): String? = null\n",
        }
        files.update(overrides or {})

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            kotlin = root / ANDROID_KOTLIN
            kotlin.mkdir(parents=True)
            for name, source in files.items():
                target = kotlin / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(source)
            for name in directories or ():
                (kotlin / name).mkdir(parents=True)

            bin_dir = root / "bin"
            bin_dir.mkdir()
            for command in ("grep", "sh"):
                executable = shutil.which(command)
                if executable is None:
                    raise RuntimeError(f"missing required fixture command: {command}")
                (bin_dir / command).symlink_to(executable)
            python = bin_dir / "python3"
            python.write_text(
                "#!/usr/bin/env sh\n"
                "if [ \"$1\" = \"-m\" ] && [ \"$2\" = \"unittest\" ]; then exit 0; fi\n"
                "if [ \"$1\" = \"scripts/check_android_manifest.py\" ]; then exit 0; fi\n"
                f"exec {shlex.quote(sys.executable)} \"$@\"\n"
            )
            python.chmod(0o755)
            self_path = str(bin_dir)
            if shutil.which("rg", path=self_path) is not None:
                raise AssertionError("fixture tool path unexpectedly includes rg")
            environment = os.environ | {"PATH": self_path}
            return subprocess.run(
                ["sh", str(CAPTURE_GUARD)],
                cwd=root,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )

    def test_onboarding_only_null_source_passes_capture_guard(self):
        result = self.run_guard()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("PASS: Android capture-ladder static contracts", result.stdout)

    def test_capture_guard_does_not_require_rg(self):
        result = self.run_guard()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("PASS: Android capture-ladder static contracts", result.stdout)

    def test_capture_guard_fails_closed_when_kotlin_source_cannot_be_read(self):
        result = self.run_guard(directories=("Unreadable.kt",))

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("FAIL: Android Kotlin source inspection failed", result.stderr)

    def test_capture_guard_rejects_protected_regressions(self):
        regressions = {
            "missing startup screen protection": (
                {"MainActivity.kt": "class MainActivity\n"},
                "MainActivity no longer sets FLAG_SECURE",
            ),
            "media projection": (
                {"MediaProjection.kt": "val capture = MediaProjection\n"},
                "MediaProjection appeared in shipping Kotlin",
            ),
            "sticky restart": (
                {
                    "CaptureService.kt": (
                        "return START_NOT_STICKY\nreturn START_STICKY\n"
                    )
                },
                "CaptureService uses START_STICKY",
            ),
            "escaped Shizuku binder": (
                {"ClipboardRead.kt": "val binder = ShizukuBinderWrapper()\n"},
                "Shizuku clipboard binder escaped its source-attribution boundary",
            ),
            "nested Shizuku binder": (
                {"nested/ClipboardRead.kt": "val binder = ShizukuBinderWrapper()\n"},
                "Shizuku clipboard binder escaped its source-attribution boundary",
            ),
            "literal Shizuku binder stub": (
                {"ClipboardRead.kt": "val binder = IClipboard$Stub\n"},
                "Shizuku clipboard binder escaped its source-attribution boundary",
            ),
            "escaped Shizuku binder stub": (
                {"ClipboardRead.kt": 'val binder = "IClipboard\\$Stub"\n'},
                "Shizuku clipboard binder escaped its source-attribution boundary",
            ),
            "Shizuku clipboard transport": (
                {"ClipboardRead.kt": 'val read = "getPrimaryClip"\n'},
                "Shizuku clipboard content transport reappeared",
            ),
            "Java Shizuku clipboard transport": (
                {"ClipboardBridge.java": 'String read = "getPrimaryClip";\n'},
                "Shizuku clipboard content transport reappeared",
            ),
            "nested Shizuku clipboard transport": (
                {"nested/ClipboardRead.kt": 'val read = "getPrimaryClip"\n'},
                "Shizuku clipboard content transport reappeared",
            ),
        }

        for name, (overrides, message) in regressions.items():
            with self.subTest(regression=name):
                result = self.run_guard(overrides)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(message, result.stderr)


if __name__ == "__main__":
    unittest.main()
