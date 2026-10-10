#!/usr/bin/env python3
import argparse
from pathlib import Path
from copy import deepcopy
import hashlib
import json
import os
import re
import runpy
import subprocess
import sys
import tempfile
import unittest
from zipfile import ZipFile


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
        binary = ".method public static final native ingestBinary(J[BLjava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;[B)Z"
        verify(callback, text + "\n" + binary)
        with self.assertRaisesRegex(ValueError, "ingestBinary"):
            verify(callback, text)
        legacy = ".method public static final native ingestBinary(J[BLjava/lang/String;Ljava/lang/String;Ljava/lang/String;)Z"
        with self.assertRaisesRegex(ValueError, "ingestBinary"):
            verify(callback, text + "\n" + legacy)

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

    def test_linux_recovery_binds_skipped_builds_to_the_exact_origin(self):
        verify = runpy.run_path(str(ROOT / "scripts/release/verify-qualified-run.py"))["verify"]
        evidence = self.qualification()
        evidence[1]["jobs"].extend([
            {"name": "Linux full parity evidence gate", "status": "completed", "conclusion": "success"},
            *({"name": f"Linux build ({arch})", "status": "completed", "conclusion": "skipped"}
              for arch in ("x86_64", "aarch64")),
        ])
        evidence[1]["total_count"] = len(evidence[1]["jobs"])
        origin_run = {**deepcopy(evidence[0]), "id": 456}
        origin_jobs = {"total_count": 3, "jobs": [
            {"name": name, "status": "completed", "conclusion": "success"}
            for name in ("preflight", "Linux build (x86_64)", "Linux build (aarch64)")
        ]}
        origin_artifacts = {"artifacts": [
            {"name": f"production-linux-{arch}", "expired": False,
             "workflow_run": {"id": 456, "head_sha": "commit"}}
            for arch in ("x86_64", "aarch64")
        ]}
        origin = (origin_run, origin_jobs, origin_artifacts, {"platform": "linux", "run_id": "456"})
        verify(*evidence, "owner/repo", "commit", require_linux=True, linux_origin=origin)
        with self.assertRaises(ValueError):
            verify(*evidence, "owner/repo", "commit", require_linux=True)
        for index, key, value in (
            (0, "head_sha", "stale"),
            (0, "path", ".github/workflows/untrusted.yml"),
            (0, "head_repository", {"full_name": "fork/repo"}),
            (3, "run_id", "999"),
        ):
            broken = deepcopy(origin)
            broken[index][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                verify(*evidence, "owner/repo", "commit", require_linux=True, linux_origin=broken)
        for index in (5, 6, 7):
            broken = deepcopy(evidence)
            broken[1]["jobs"][index]["conclusion"] = "failure"
            with self.subTest(job=index), self.assertRaises(ValueError):
                verify(*broken, "owner/repo", "commit", require_linux=True, linux_origin=origin)

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

    def test_linux_recovery_requires_every_native_desktop_session(self):
        verify = runpy.run_path(str(ROOT / "scripts/release/verify-qualified-run.py"))["verify"]
        evidence = self.qualification()
        linux_jobs = [
            {"name": f"Linux build ({architecture})", "status": "completed", "conclusion": "success"}
            for architecture in ("x86_64", "aarch64")
        ]
        linux_jobs.append({
            "name": "Linux full parity evidence gate",
            "status": "completed",
            "conclusion": "success",
        })
        evidence[1]["jobs"].extend(linux_jobs)
        evidence[1]["total_count"] += len(linux_jobs)
        verify(*evidence, "owner/repo", "commit", require_linux=True)
        evidence[1]["jobs"].pop()
        evidence[1]["total_count"] -= 1
        with self.assertRaisesRegex(ValueError, "Linux qualification jobs"):
            verify(*evidence, "owner/repo", "commit", require_linux=True)

    def test_linux_artifact_source_requires_both_exact_native_builds(self):
        verify = runpy.run_path(str(ROOT / "scripts/release/verify-linux-artifact-source-run.py"))["verify"]
        run = {"id": 123, "head_sha": "commit", "head_repository": {"full_name": "owner/repo"},
               "path": ".github/workflows/release.yml", "event": "workflow_dispatch", "status": "completed"}
        jobs = {"total_count": 3, "jobs": [
            {"name": name, "status": "completed", "conclusion": "success"}
            for name in ("preflight", "Linux build (x86_64)", "Linux build (aarch64)")
        ]}
        artifacts = {"artifacts": [
            {"name": name, "expired": False, "workflow_run": {"id": 123, "head_sha": "commit"}}
            for name in ("production-linux-x86_64", "production-linux-aarch64")
        ]}
        verify(run, jobs, artifacts, "owner/repo", "commit")
        jobs["jobs"].pop()
        jobs["total_count"] -= 1
        with self.assertRaisesRegex(ValueError, "native build jobs"):
            verify(run, jobs, artifacts, "owner/repo", "commit")

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

    def test_linux_release_contract_requires_native_artifacts_and_sessions(self):
        workflow = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        build = (ROOT / "scripts/release/build-linux-flutter.sh").read_text(encoding="utf-8")
        packages = (ROOT / "scripts/release/build-linux-packages.sh").read_text(encoding="utf-8")
        smoke = (ROOT / "scripts/release/smoke-linux-production.sh").read_text(encoding="utf-8")
        contract = json.loads((ROOT / "packaging/linux/release-contract.json").read_text(encoding="utf-8"))
        self.assertEqual(contract["maximum_glibc"], "2.39")
        self.assertEqual(set(contract["architectures"]), {"x86_64", "aarch64"})
        self.assertEqual(contract["formats"], ["AppImage", "deb", "rpm"])
        self.assertIn("flutter build linux --release", build)
        self.assertIn("copypaste-daemon", build)
        self.assertIn("copypaste-cli", build)
        self.assertIn("dpkg-deb --root-owner-group --build", packages)
        self.assertIn("rm -rf \"$STAGE/DEBIAN\"", packages)
        self.assertIn("rpmbuild -bb", packages)
        self.assertIn("APPIMAGETOOL", packages)
        self.assertIn("package-metadata.json", packages)
        self.assertIn("copypaste-cli", packages)
        self.assertNotIn("/etc/xdg/autostart", packages)
        desktop = (ROOT / "packaging/linux/com.copypaste.CopyPaste.desktop").read_text(encoding="utf-8")
        self.assertIn("x-scheme-handler/copypaste", desktop)
        self.assertIn("X-GNOME-Autostart-enabled=true", desktop)
        self.assertIn("gnome-keyring-daemon", smoke)
        self.assertIn("XDG_SESSION_TYPE", smoke)
        self.assertIn("XVFB_RUN", smoke)
        self.assertIn("GLIBC_", smoke)
        self.assertIn('ARTIFACT="$(cd', smoke)
        self.assertIn('rpm2cpio "$ARTIFACT"', smoke)
        self.assertIn("Linux full parity evidence gate", workflow)
        qualification = (ROOT / "packaging/linux/native-qualification.md").read_text(encoding="utf-8")
        self.assertIn("GNOME", qualification)
        self.assertIn("KDE", qualification)
        self.assertIn("Wayland", qualification)
        self.assertIn("AppImage deb rpm", workflow)
        native_workflow = (ROOT / ".github/workflows/linux-native-qualification.yml").read_text(encoding="utf-8")
        self.assertIn("linux-native-qualification", native_workflow)
        self.assertIn("ubuntu-24.04-arm", native_workflow)
        self.assertIn("previous_artifact_run_id", native_workflow)
        self.assertNotIn("self-hosted", native_workflow)
        producer = (ROOT / "scripts/release/produce-linux-native-qualification.py").read_text(encoding="utf-8")
        self.assertIn("COPYPASTE_QUALIFICATION_COMMAND", producer)
        self.assertIn("artifact_run_id", producer)
        fixture = (ROOT / "scripts/release/linux-native-fixture-driver.py").read_text(encoding="utf-8")
        self.assertIn("set_private_mode", fixture)
        self.assertIn("encrypted_restart_persistence", fixture)
        self.assertIn("WaylandIntegration", fixture)

    def test_linux_arm64_flutter_bootstrap_uses_the_pinned_source_revision(self):
        action = (ROOT / ".github/actions/setup-flutter/action.yml").read_text(encoding="utf-8")
        self.assertIn("runner.arch == 'ARM64'", action)
        self.assertIn("FLUTTER_SOURCE_COMMIT", action)
        self.assertIn("git -C \"$root\" fetch --depth=1 origin \"$FLUTTER_SOURCE_COMMIT\"", action)
        self.assertIn("copypaste-flutter-${FLUTTER_VERSION}-linux-arm64", action)
        self.assertIn('[[ "$FLUTTER_VERSION" == 3.47.6 ]]', action)

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
                    f"CopyPaste-v{version}-android-arm64.apk",
                    f"CopyPaste-v{version}-android-armv7.apk",
                    f"CopyPaste-v{version}-windows-x86_64-setup.exe",
                    *(f"CopyPaste-v{version}-linux-{architecture}.{extension}"
                      for architecture in ("x86_64", "aarch64")
                      for extension in ("AppImage", "deb", "rpm")),
                ):
                    (artifacts / name).touch()
                notes = render(version, "dmytro-yevs/copypaste", artifacts)
                links = re.findall(r"https://github.com/[^\s)]+", notes)
                self.assertEqual(len(links), 11)
                self.assertTrue(all(f"/download/v{version}/" in link for link in links))
                self.assertNotIn("{{", notes)
                rows = [line for line in notes.splitlines() if line.startswith("|")]
                self.assertEqual(len(rows), 6)
                self.assertEqual(rows[0], "| Architecture | macOS | Android | Windows | Linux |")
                self.assertEqual(rows[2].count("[Download]("), 2)
                self.assertEqual(rows[2].count("[AppImage](") + rows[2].count("[deb](") + rows[2].count("[rpm]("), 3)
                self.assertEqual(rows[3].count("[Download]("), 1)
                self.assertEqual(rows[4].count("[Download]("), 1)
                self.assertEqual(rows[4].count("[AppImage](") + rows[4].count("[deb](") + rows[4].count("[rpm]("), 3)
                self.assertEqual(rows[5].count("[Download]("), 1)
                self.assertTrue(all(link.endswith((".dmg", ".apk", ".exe", ".AppImage", ".deb", ".rpm")) for link in links))
                for suffix in ("", "-arm64", "-armv7"):
                    artifact = artifacts / f"CopyPaste-v{version}-android{suffix}.apk"
                    artifact.unlink()
                    with self.assertRaisesRegex(ValueError, "artifact is missing"):
                        render(version, "dmytro-yevs/copypaste", artifacts)
                    artifact.touch()


class AndroidReleaseArtifactsTest(unittest.TestCase):
    def test_apk_variants_require_exact_abis_and_complete_runtime_libraries(self):
        module = runpy.run_path(str(ROOT / "scripts/release/verify-android-abis.py"))
        verify = module["verify"]
        with tempfile.TemporaryDirectory() as directory:
            apk = Path(directory) / "app.apk"
            for architecture, abis in module["ARCHITECTURES"].items():
                with self.subTest(architecture=architecture):
                    names = [
                        f"lib/{abi}/{library}"
                        for abi in abis for library in module["REQUIRED_LIBRARIES"]
                    ]
                    with ZipFile(apk, "w") as archive:
                        for name in names:
                            archive.writestr(name, b"native library")
                    verify(apk, architecture)
                    with ZipFile(apk, "w") as archive:
                        for name in names[1:]:
                            archive.writestr(name, b"native library")
                    with self.assertRaisesRegex(ValueError, "missing"):
                        verify(apk, architecture)
            with ZipFile(apk, "w") as archive:
                archive.writestr("lib/x86_64/libflutter.so", b"wrong ABI")
            for architecture in ("arm64", "armv7"):
                with self.assertRaisesRegex(ValueError, "architectures differ"):
                    verify(apk, architecture)

    def write_receipts(self, root, *, legacy=False):
        for platform, names in (
            ("macos", ["app.dmg"]),
            ("windows", ["app.exe"]),
            ("android", [f"CopyPaste-v1.2.3-android{suffix}.apk"
                         for suffix in (("",) if legacy else ("", "-arm64", "-armv7"))]),
        ):
            directory = root / platform
            directory.mkdir()
            command = [sys.executable, str(ROOT / "scripts/release/write-artifact-receipt.py"),
                       "--platform", platform, "--version", "1.2.3", "--commit", "a" * 40,
                       "--run-id", "123", "--output", str(directory / "production-receipt.json")]
            for name in names:
                artifact = directory / name
                artifact.write_bytes(b"qualified")
                command.extend(["--artifact", str(artifact)])
            subprocess.run(command, check=True, capture_output=True)

    def test_every_android_variant_is_bound_to_qualification(self):
        verify = runpy.run_path(str(ROOT / "scripts/release/verify-artifact-receipts.py"))["verify"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_receipts(root)
            verify(root, "1.2.3", "a" * 40, "123")
            for suffix in ("", "-arm64", "-armv7"):
                artifact = root / "android" / f"CopyPaste-v1.2.3-android{suffix}.apk"
                artifact.write_bytes(b"corrupted")
                with self.assertRaisesRegex(ValueError, "digest changed"):
                    verify(root, "1.2.3", "a" * 40, "123")
                artifact.unlink()
                with self.assertRaisesRegex(ValueError, "artifact is missing"):
                    verify(root, "1.2.3", "a" * 40, "123")
                artifact.write_bytes(b"qualified")

    def test_legacy_receipts_remain_valid_for_release_recovery(self):
        verify = runpy.run_path(str(ROOT / "scripts/release/verify-artifact-receipts.py"))["verify"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_receipts(root, legacy=True)
            verify(root, "1.2.3", "a" * 40, "123")

    def test_android_receipts_reject_omitted_and_unqualified_variants(self):
        verify = runpy.run_path(str(ROOT / "scripts/release/verify-artifact-receipts.py"))["verify"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_receipts(root)
            receipt_path = root / "android" / "production-receipt.json"
            receipt = json.loads(receipt_path.read_text())
            receipt["artifacts"].pop()
            receipt_path.write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError, "does not cover every APK"):
                verify(root, "1.2.3", "a" * 40, "123")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_receipts(root, legacy=True)
            (root / "android" / "extra.apk").write_bytes(b"unqualified")
            with self.assertRaisesRegex(ValueError, "does not cover every APK"):
                verify(root, "1.2.3", "a" * 40, "123")

    def test_linux_receipts_bind_every_signed_native_package(self):
        verify = runpy.run_path(str(ROOT / "scripts/release/verify-artifact-receipts.py"))["verify"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_receipts(root)
            linux = root / "linux"
            linux.mkdir()
            command = [sys.executable, str(ROOT / "scripts/release/write-artifact-receipt.py"),
                       "--platform", "linux", "--version", "1.2.3", "--commit", "a" * 40,
                       "--run-id", "123", "--output", str(linux / "production-receipt.json")]
            for architecture in ("x86_64", "aarch64"):
                for extension in ("AppImage", "deb", "rpm"):
                    package = linux / f"CopyPaste-v1.2.3-linux-{architecture}.{extension}"
                    package.write_bytes(b"qualified package")
                    signature = linux / f"{package.name}.sig"
                    signature.write_bytes(b"signature")
                    receipt = linux / f"{package.name}.sha256"
                    receipt.write_bytes(b"digest receipt")
                    for artifact in (package, signature, receipt):
                        command.extend(["--artifact", str(artifact)])
                baseline = linux / f"CopyPaste-v1.2.3-linux-{architecture}.runtime-baseline.json"
                baseline.write_text('{"glibc_minimum":"2.39"}')
                command.extend(["--artifact", str(baseline)])
            subprocess.run(command, check=True, capture_output=True)
            verify(root, "1.2.3", "a" * 40, "123", require_linux=True)
            missing_signature = linux / "CopyPaste-v1.2.3-linux-x86_64.deb.sig"
            missing_signature.unlink()
            with self.assertRaisesRegex(ValueError, "qualified artifact is missing"):
                verify(root, "1.2.3", "a" * 40, "123", require_linux=True)

    def test_linux_architecture_receipts_merge_only_when_their_identity_matches(self):
        merge = runpy.run_path(str(ROOT / "scripts/release/merge-linux-artifact-receipts.py"))["merge"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            receipts = []
            for architecture in ("x86_64", "aarch64"):
                path = root / f"{architecture}.json"
                path.write_text(json.dumps({
                    "schema": 2,
                    "platform": "linux",
                    "version": "1.2.3",
                    "commit": "a" * 40,
                    "run_id": "123",
                    "artifacts": [{"name": f"CopyPaste-v1.2.3-linux-{architecture}.deb"}],
                }))
                receipts.append(path)
            self.assertEqual(len(merge(receipts)["artifacts"]), 2)
            invalid = json.loads(receipts[1].read_text())
            invalid["run_id"] = "124"
            receipts[1].write_text(json.dumps(invalid))
            with self.assertRaisesRegex(ValueError, "identity differs"):
                merge(receipts)

    def test_linux_native_evidence_binds_all_formats_and_desktop_sessions(self):
        verifier = runpy.run_path(str(ROOT / "scripts/release/verify-linux-native-qualification.py"))
        verify = verifier["verify"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifacts = root / "artifacts"
            artifacts.mkdir()
            for architecture in ("x86_64", "aarch64"):
                for extension in ("AppImage", "deb", "rpm"):
                    (artifacts / f"CopyPaste-v1.2.3-linux-{architecture}.{extension}").write_bytes(b"package")
            package_rows = lambda architecture: [{
                "name": f"CopyPaste-v1.2.3-linux-{architecture}.{extension}",
                "sha256": hashlib.sha256(b"package").hexdigest(), "size_bytes": len(b"package"),
            } for extension in ("AppImage", "deb", "rpm")]
            for architecture in ("x86_64", "aarch64"):
                for desktop in ("GNOME", "KDE"):
                    for session in ("x11", "wayland"):
                        assertions = verifier["assertion_set"]("prior_release", session)
                        trace_name = f"linux-native-{architecture}-{desktop.lower()}-{session}.trace.json"
                        trace = {
                            "schema": 1, "version": "1.2.3", "commit": "a" * 40,
                            "source_run_id": "123", "artifact_run_id": "456",
                            "architecture": architecture, "desktop": desktop, "session": session,
                            "upgrade_mode": "prior_release",
                            "installed_formats": ["AppImage", "deb" if desktop == "GNOME" else "rpm"],
                            "environment": {"distribution": "ubuntu" if desktop == "GNOME" else "fedora", "distribution_version": "24.04"},
                            "commands": [{"argv": ["native-probe"], "returncode": 0, "assertions": sorted(assertions)}],
                        }
                        (root / trace_name).write_text(json.dumps(trace), encoding="utf-8")
                        trace_digest = hashlib.sha256((root / trace_name).read_bytes()).hexdigest()
                        (root / f"linux-native-{architecture}-{desktop.lower()}-{session}.json").write_text(json.dumps({
                            "schema": 1, "version": "1.2.3", "commit": "a" * 40,
                            "source_run_id": "123", "artifact_run_id": "456", "architecture": architecture,
                            "desktop": desktop, "session": session,
                            "upgrade_mode": "prior_release",
                            "installed_formats": ["AppImage", "deb" if desktop == "GNOME" else "rpm"],
                            "environment": {"distribution": "ubuntu" if desktop == "GNOME" else "fedora", "distribution_version": "24.04"},
                            "assertions": {name: True for name in assertions},
                            "packages": package_rows(architecture),
                            "trace": {"name": trace_name, "sha256": trace_digest, "size_bytes": (root / trace_name).stat().st_size},
                            "evidence": [{"name": trace_name, "sha256": trace_digest, "size_bytes": (root / trace_name).stat().st_size}],
                        }))
            verify(root, artifacts, "1.2.3", "a" * 40, "123", "456")
            x11 = root / "linux-native-x86_64-gnome-x11.json"
            receipt = json.loads(x11.read_text())
            receipt["assertions"].pop("native_x11_keyboard_input")
            receipt["assertions"]["portal_keyboard_grant"] = True
            x11.write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError, "assertions are incomplete"):
                verify(root, artifacts, "1.2.3", "a" * 40, "123", "456")
            receipt["assertions"].pop("portal_keyboard_grant")
            receipt["assertions"]["native_x11_keyboard_input"] = True
            x11.write_text(json.dumps(receipt))
            broken = root / "linux-native-x86_64-gnome-x11.json"
            receipt = json.loads(broken.read_text())
            receipt["assertions"]["package_upgrade"] = "true"
            broken.write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError, "assertions are incomplete"):
                verify(root, artifacts, "1.2.3", "a" * 40, "123", "456")

    def test_linux_native_producer_owns_trace_and_artifact_digests(self):
        producer = runpy.run_path(str(ROOT / "scripts/release/produce-linux-native-qualification.py"))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ("current", "previous"):
                (root / name).mkdir()
                for extension in ("AppImage", "deb", "rpm"):
                    version = "1.2.3" if name == "current" else "1.2.2"
                    (root / name / f"CopyPaste-v{version}-linux-x86_64.{extension}").write_bytes(name.encode())
            driver = root / "driver.py"
            driver_assertions = producer["scenario_assertions"]("prior_release", "wayland")
            driver.write_text("#!/usr/bin/env python3\nimport json\nprint('COPYPASTE_QUALIFICATION_COMMAND ' + json.dumps({'argv': ['probe'], 'returncode': 0, 'assertions': " + repr(sorted(driver_assertions)) + "}))\n", encoding="utf-8")
            driver.chmod(0o755)
            os.environ["COPYPASTE_INSTALLED_FORMATS"] = "AppImage,deb"
            receipt_path = producer["produce"](argparse.Namespace(
                artifacts=root / "current", previous_artifacts=root / "previous",
                version="1.2.3", previous_version="1.2.2", architecture="x86_64",
                desktop="GNOME", session="wayland", commit="a" * 40,
                source_run_id="123", artifact_run_id="456", driver=driver, output=root / "evidence",
            ))
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
            self.assertEqual(receipt["artifact_run_id"], "456")
            self.assertEqual(len(receipt["packages"]), 3)
            self.assertTrue(all(item["size_bytes"] == len(b"current") for item in receipt["packages"]))
            self.assertEqual(receipt["trace"]["name"], "linux-native-x86_64-gnome-wayland.trace.json")
            placeholder = "COPYPASTE_QUALIFICATION_COMMAND " + json.dumps({
                "argv": ["sh", "-ceu", "desktop entry URI handler and icon verified"],
                "returncode": 0,
                "assertions": [next(iter(driver_assertions))],
            })
            with self.assertRaisesRegex(ValueError, "placeholder command"):
                producer["command_rows"](placeholder, driver_assertions)


if __name__ == "__main__":
    unittest.main()
