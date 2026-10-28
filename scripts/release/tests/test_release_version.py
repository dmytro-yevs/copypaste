"""One release identity drives local builds, workflow inputs and the publication tag."""

import json
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "version.py"
VERSION = runpy.run_path(str(SCRIPT))


class ReleaseVersionTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="release-version-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.release = {"version": "1.2.3", "build_number": 17, "android_version_code": 300001203}
        files = {
            "config/release.json": json.dumps(self.release) + "\n",
            "Cargo.toml": '[workspace]\nresolver = "2"\nmembers = ["crates/app", "crates/sdk"]\n'
                          '[workspace.package]\nversion = "1.2.3"\n'
                          '[workspace.metadata.copypaste]\nandroid-release-version-code = 300001203\n',
            "crates/app/Cargo.toml": '[package]\nname = "app"\nversion.workspace = true\nedition = "2021"\n',
            "crates/app/src/lib.rs": "",
            "crates/sdk/Cargo.toml": '[package]\nname = "sdk"\nversion = "1.0.0"\nedition = "2021"\n',
            "crates/sdk/src/lib.rs": "",
            "apps/copypaste_flutter/pubspec.yaml": 'name: app\nversion: 1.2.3+17\ndependencies:\n  library: 2.0.0\n',
            "modules/fixture/module.json": '{"version": "0.4.0", "app_versions": ">=1.2.3, <2.0.0"}\n',
        }
        for name, content in files.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
        subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.root, check=True, capture_output=True)

    def command(self, *args):
        return subprocess.run([sys.executable, str(SCRIPT), "--root", str(self.root), *args],
                              capture_output=True, text=True)

    def snapshot(self):
        names = ("config/release.json", "Cargo.toml", "Cargo.lock", VERSION["PUBSPEC"], "modules/fixture/module.json")
        return {name: (self.root / name).read_bytes() for name in names}

    def test_one_bump_updates_build_inputs_and_creates_only_the_exact_committed_tag(self):
        VERSION["bump"](self.root, "1.2.4")
        expected = {"version": "1.2.4", "build_number": 18, "android_version_code": 300001204}
        self.assertEqual(VERSION["load"](self.root), expected)
        VERSION["check"](self.root, expected)
        pubspec = (self.root / VERSION["PUBSPEC"]).read_text()
        self.assertIn("version: 1.2.4+18", pubspec)
        self.assertIn("  library: 2.0.0", pubspec)
        self.assertIn('name = "sdk"\nversion = "1.0.0"', (self.root / "Cargo.lock").read_text())
        self.assertEqual(json.loads((self.root / "modules/fixture/module.json").read_text()),
                         {"version": "0.4.0", "app_versions": ">=1.2.4, <2.0.0"})
        values = self.command("values")
        self.assertEqual(values.returncode, 0, values.stderr)
        self.assertEqual(values.stdout.strip(), "1.2.4 18 300001204 v1.2.4")
        for args in (("init", "-q"), ("config", "user.name", "Release Test"),
                     ("config", "user.email", "release@example.com"), ("add", "."),
                     ("commit", "-qm", "Release fixture")):
            subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True)
        VERSION["create_tag"](self.root, expected)
        head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=self.root)
        tag = subprocess.check_output(["git", "rev-parse", "v1.2.4^{commit}"], cwd=self.root)
        self.assertEqual(tag, head)
        (self.root / "uncommitted-change").touch()
        with self.assertRaisesRegex(ValueError, "commit the release changes"):
            VERSION["create_tag"](self.root, expected)

    def test_stale_generated_input_and_wrong_tag_are_rejected_before_building(self):
        path = self.root / VERSION["PUBSPEC"]
        path.write_text(path.read_text().replace("1.2.3+17", "9.9.9+99"))
        result = self.command("values")
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "")
        VERSION["sync"](self.root, self.release)
        result = self.command("check", "--tag", "v9.9.9")
        self.assertEqual(result.returncode, 1)
        self.assertIn("canonical release identity", result.stderr)
        lock = self.root / "Cargo.lock"
        lock.write_text(lock.read_text().replace('version = "1.2.3"', 'version = "1.2.2"'))
        with self.assertRaisesRegex(ValueError, "stale workspace version for app"):
            VERSION["check"](self.root, self.release)

    def test_failed_lock_resolution_restores_all_release_files(self):
        before = self.snapshot()
        with patch.object(VERSION["subprocess"], "run", side_effect=subprocess.CalledProcessError(1, "cargo")):
            with self.assertRaises(subprocess.CalledProcessError):
                VERSION["bump"](self.root, "1.2.4")
        self.assertEqual(self.snapshot(), before)

    def test_version_downgrades_and_invalid_counters_do_not_modify_sources(self):
        before = self.snapshot()
        with self.assertRaisesRegex(ValueError, "must increase"):
            VERSION["bump"](self.root, "1.2.2")
        with self.assertRaisesRegex(ValueError, "android_version_code"):
            VERSION["sync"](self.root, {**self.release, "android_version_code": True})
        with self.assertRaisesRegex(ValueError, "module compatibility decision"):
            VERSION["bump"](self.root, "2.0.0")
        self.assertEqual(self.snapshot(), before)


if __name__ == "__main__":
    unittest.main()
