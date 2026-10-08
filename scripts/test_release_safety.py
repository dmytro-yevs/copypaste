#!/usr/bin/env python3
from pathlib import Path
from copy import deepcopy
import hashlib
import re
import runpy
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class FlutterCacheKeyTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.key = runpy.run_path(str(ROOT / "scripts/ci/flutter-cache-key.py"))["fingerprint"]
        self.files = {
            "Cargo.toml": '[workspace.package]\nversion = "1.0.1"\n[workspace.metadata]\nandroid-release-version-code = 300000001\n[profile.release]\nlto = "thin"\n',
            "Cargo.lock": 'version = 4\n[[package]]\nname = "local"\nversion = "1.0.1"\ndependencies = ["helper 1.0.1", "external"]\n[[package]]\nname = "helper"\nversion = "1.0.1"\n[[package]]\nname = "external"\nversion = "2.0.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "abc"\n',
            "crates/local/Cargo.toml": '[package]\nname = "local"\nversion = "1.0.1"\n[features]\ncapture = []\n',
            ".flutter-version": "3.47.6\n",
            "apps/copypaste_flutter/pubspec.lock": "packages: {}\n",
            "apps/copypaste_flutter/hook/build.dart": "void main() {}\n",
            "crates/copypaste-flutter-bridge/rust-toolchain.toml": '[toolchain]\nchannel = "1.96.0"\n',
        }
        self.write_files()

    def write_files(self):
        for name, content in self.files.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")

    def test_release_bump_reuses_dependency_cache(self):
        original = self.key(self.root)
        for name in ("Cargo.toml", "Cargo.lock", "crates/local/Cargo.toml"):
            self.files[name] = self.files[name].replace("1.0.1", "1.0.2")
        self.files["Cargo.toml"] = self.files["Cargo.toml"].replace("300000001", "300000002")
        self.write_files()
        self.assertEqual(original, self.key(self.root))

    def test_locked_external_dependency_change_invalidates_cache(self):
        original = self.key(self.root)
        self.files["Cargo.lock"] = self.files["Cargo.lock"].replace("2.0.0", "2.0.1")
        self.write_files()
        self.assertNotEqual(original, self.key(self.root))

    def test_compiler_and_dependency_inputs_invalidate_cache(self):
        changes = {
            "Cargo.toml": ('lto = "thin"', 'lto = false'),
            "crates/local/Cargo.toml": ('capture = []', 'capture = ["helper/capture"]'),
            ".flutter-version": ("3.47.6", "3.47.7"),
            "apps/copypaste_flutter/pubspec.lock": ("packages: {}", "packages: {cryptography: {version: 2.9.0}}"),
            "apps/copypaste_flutter/hook/build.dart": ("void main() {}", "void main() { configureCargo(); }"),
            "crates/copypaste-flutter-bridge/rust-toolchain.toml": ("1.96.0", "1.97.0"),
        }
        for name, (old, new) in changes.items():
            with self.subTest(name=name):
                original = self.key(self.root)
                before = self.files[name]
                self.files[name] = before.replace(old, new)
                self.write_files()
                self.assertNotEqual(original, self.key(self.root))
                self.files[name] = before
                self.write_files()

    def test_cargo_flags_invalidate_cache(self):
        original = self.key(self.root)
        path = self.root / ".cargo/config.toml"
        path.parent.mkdir()
        path.write_text('[build]\nrustflags = ["-C", "target-cpu=native"]\n', encoding="utf-8")
        self.assertNotEqual(original, self.key(self.root))

    def test_checkout_line_endings_do_not_create_another_cache(self):
        original = self.key(self.root)
        for name in self.files:
            path = self.root / name
            path.write_bytes(path.read_bytes().replace(b"\n", b"\r\n"))
        self.assertEqual(original, self.key(self.root))


class ReleaseSafetyTest(unittest.TestCase):
    def test_optimized_capture_contract_rejects_the_stripped_release_callback(self):
        verify = runpy.run_path(str(ROOT / "scripts/release/verify-capture-jni.py"))["verify"]
        callback = ".class public interface abstract Lcom/copypaste/app/CaptureCallback;\n"
        with self.assertRaisesRegex(ValueError, "CaptureCallback.run"):
            verify(callback, "")

    def test_optimized_capture_contract_requires_both_ingest_methods(self):
        verify = runpy.run_path(str(ROOT / "scripts/release/verify-capture-jni.py"))["verify"]
        callback = ".method public abstract run(J)V"
        text = ".method public static final native ingestText(JLjava/lang/String;)Z"
        binary = ".method public static final native ingestBinary(J[BLjava/lang/String;Ljava/lang/String;Ljava/lang/String;)Z"
        verify(callback, text + "\n" + binary)
        with self.assertRaisesRegex(ValueError, "ingestBinary"):
            verify(callback, text)

    def qualification(self):
        return (
            {"id": 123, "head_sha": "commit", "head_repository": {"full_name": "owner/repo"},
             "path": ".github/workflows/release.yml", "event": "workflow_dispatch",
             "status": "completed", "conclusion": "success"},
            {"total_count": 5, "jobs": [
                {"name": name, "status": "completed", "conclusion": "success"}
                for name in ("preflight", "macos", "android", "windows", "qualify")
            ]},
            {"artifacts": [{"name": "production-qualified", "expired": False,
                            "workflow_run": {"id": 123, "head_sha": "commit"}}]},
        )

    def test_recovery_refuses_skipped_or_failed_native_gates(self):
        verify = runpy.run_path(str(ROOT / "scripts/release/verify-qualified-run.py"))["verify"]
        original = self.qualification()
        verify(*original, "owner/repo", "commit")
        for index in range(5):
            for conclusion in ("skipped", "failure", "cancelled"):
                evidence = deepcopy(original)
                evidence[1]["jobs"][index]["conclusion"] = conclusion
                with self.subTest(index=index, conclusion=conclusion), self.assertRaises(ValueError):
                    verify(*evidence, "owner/repo", "commit")

    def test_recovery_refuses_other_commits_forks_and_expired_artifacts(self):
        verify = runpy.run_path(str(ROOT / "scripts/release/verify-qualified-run.py"))["verify"]
        for target, key, value in (
            ("run", "head_sha", "different"),
            ("run", "head_repository", {"full_name": "fork/repo"}),
            ("run", "conclusion", "cancelled"),
            ("artifact", "expired", True),
            ("artifact", "workflow_run", {"id": 999, "head_sha": "commit"}),
        ):
            evidence = self.qualification()
            item = evidence[0] if target == "run" else evidence[2]["artifacts"][0]
            item[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                verify(*evidence, "owner/repo", "commit")

    def test_publication_preserves_existing_bytes_and_uploads_only_missing_files(self):
        missing = runpy.run_path(str(ROOT / "scripts/release/publish-release.py"))["missing_assets"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = root / "app.apk"
            package.write_bytes(b"qualified")
            signature = root / "app.apk.sig"
            signature.write_bytes(b"signature")
            release = {"draft": False, "prerelease": False, "assets": [{
                "name": package.name, "size": package.stat().st_size,
                "digest": "sha256:" + hashlib.sha256(package.read_bytes()).hexdigest(),
            }]}
            self.assertEqual(missing(root, release), [signature])
            release["assets"][0]["digest"] = "sha256:changed"
            with self.assertRaisesRegex(ValueError, "differs from qualification"):
                missing(root, release)

    def test_publication_rejects_unqualified_assets_and_drafts(self):
        missing = runpy.run_path(str(ROOT / "scripts/release/publish-release.py"))["missing_assets"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "app.apk").write_bytes(b"qualified")
            with self.assertRaisesRegex(ValueError, "unqualified"):
                missing(root, {"draft": False, "prerelease": False, "assets": [{"name": "other.apk"}]})
            with self.assertRaisesRegex(ValueError, "stable"):
                missing(root, {"draft": True, "prerelease": False, "assets": []})

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
                ):
                    (artifacts / name).touch()
                notes = render(version, "dmytro-yevs/copypaste", artifacts)
                links = re.findall(r"https://github.com/[^\s)]+", notes)
                self.assertEqual(len(links), 3)
                self.assertTrue(all(f"/download/v{version}/" in link for link in links))
                self.assertNotIn("{{", notes)
                rows = [line for line in notes.splitlines() if line.startswith("|")]
                self.assertEqual(len(rows), 3)
                self.assertEqual(rows[0], "| macOS / arm64 | Android / Universal | Windows / x86_64 |")
                self.assertEqual(rows[2].count("[Download]("), 3)
                self.assertTrue(all(link.endswith((".dmg", ".apk", ".exe")) for link in links))
                (artifacts / f"CopyPaste-v{version}-android.apk").unlink()
                with self.assertRaisesRegex(ValueError, "artifact is missing"):
                    render(version, "dmytro-yevs/copypaste", artifacts)


if __name__ == "__main__":
    unittest.main()
