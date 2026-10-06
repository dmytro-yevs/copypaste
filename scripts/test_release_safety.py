#!/usr/bin/env python3
from pathlib import Path
import re
import runpy
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class ReleaseSafetyTest(unittest.TestCase):
    def test_macos_smoke_requires_disposable_keychain_before_launch(self):
        script = (ROOT / "scripts/release/smoke-macos-production.sh").read_text(
            encoding="utf-8"
        )
        guard = script.index('COPYPASTE_KEYCHAIN_TEST:-')
        search_list = script.index('security list-keychains -d user')
        launch = script.index('Contents/MacOS/CopyPaste" >/dev/null')
        self.assertLess(guard, search_list)
        self.assertLess(search_list, launch)
        self.assertNotIn("--force", script)

    def test_release_workflow_builds_only_release_artifacts(self):
        workflow = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        self.assertIn("flutter build macos --release", (ROOT / "scripts/release/build-macos-flutter.sh").read_text(encoding="utf-8"))
        self.assertIn("flutter build apk --release", workflow)
        self.assertIn("flutter build windows --release", (ROOT / "scripts/release/build-windows-flutter.ps1").read_text(encoding="utf-8"))
        self.assertNotIn("flutter build macos --debug", workflow)
        self.assertNotIn("flutter build apk --debug", workflow)
        self.assertNotIn("flutter build windows --debug", workflow)

    def test_stable_version_and_no_cloud_release_configuration(self):
        cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
        pubspec = (ROOT / "apps/copypaste_flutter/pubspec.yaml").read_text(encoding="utf-8")
        release = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        workspace = re.search(r"\[workspace\.package\]\n([^\[]+)", cargo).group(1)
        version = re.search(r'^version = "([^"]+)"$', workspace, re.MULTILINE).group(1)
        self.assertRegex(version, r"^[0-9]+\.[0-9]+\.[0-9]+$")
        match = re.search(r"^version: ([0-9.]+)\+([1-9][0-9]*)$", pubspec, re.MULTILINE)
        self.assertIsNotNone(match)
        self.assertEqual(version, match.group(1))
        code = int(re.search(r"^android-release-version-code = ([0-9]+)$", cargo, re.MULTILINE).group(1))
        self.assertGreater(code, 200000038)
        self.assertLessEqual(code, 2100000000)
        self.assertNotIn("COPYPASTE_CLOUD_URL", release)
        self.assertNotIn("SUPABASE_URL", release)

    def test_release_table_resolves_every_asset_for_future_versions(self):
        render = runpy.run_path(str(ROOT / "scripts/release/render-release-notes.py"))["render"]
        for version in ("1.0.2", "2.3.4"):
            with self.subTest(version=version), tempfile.TemporaryDirectory() as directory:
                artifacts = Path(directory)
                for name in (
                    f"CopyPaste-v{version}-macos-arm64.dmg",
                    f"CopyPaste-v{version}-android.apk",
                    f"CopyPaste-v{version}-windows-x86_64-setup.exe",
                    f"copypaste-cli-v{version}-macos-arm64.tar.gz",
                ):
                    (artifacts / name).touch()
                    (artifacts / (name + ".sha256")).touch()
                    if name.endswith((".apk", ".exe")):
                        (artifacts / (name + ".sig")).touch()
                notes = render(version, "dmytro-yevs/copypaste", artifacts)
                links = re.findall(r"https://github.com/[^\s)]+", notes)
                self.assertEqual(len(links), 10)
                self.assertTrue(all(f"/download/v{version}/" in link for link in links))
                self.assertNotIn("{{", notes)
                for platform in ("macOS", "Android", "Windows"):
                    self.assertIn(f"| {platform} |", notes)
                (artifacts / f"CopyPaste-v{version}-android.apk").unlink()
                with self.assertRaisesRegex(ValueError, "artifact is missing"):
                    render(version, "dmytro-yevs/copypaste", artifacts)


if __name__ == "__main__":
    unittest.main()
