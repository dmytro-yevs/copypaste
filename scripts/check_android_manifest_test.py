#!/usr/bin/env python3
from pathlib import Path
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


if __name__ == "__main__":
    unittest.main()
