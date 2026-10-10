#!/usr/bin/env python3
import argparse
import base64
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
from unittest import mock
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

    def test_compositor_runtime_recovery_requires_the_public_asset_gate(self):
        verify = runpy.run_path(str(ROOT / "scripts/release/verify-qualified-run.py"))["verify"]
        evidence = self.qualification()
        evidence[1]["jobs"].extend([
            {"name": f"Linux build ({architecture})", "status": "completed", "conclusion": "success"}
            for architecture in ("x86_64", "aarch64")
        ])
        evidence[1]["jobs"].append({
            "name": "Linux full parity evidence gate", "status": "completed", "conclusion": "success"
        })
        evidence[1]["total_count"] = len(evidence[1]["jobs"])
        with self.assertRaisesRegex(ValueError, "compositor runtime"):
            verify(*evidence, "owner/repo", "commit", require_linux=True, require_compositor_runtime=True)
        evidence[1]["jobs"].append({
            "name": "Sign and bind opt-in compositor runtime companions",
            "status": "completed", "conclusion": "success",
        })
        evidence[1]["total_count"] += 1
        verify(*evidence, "owner/repo", "commit", require_linux=True, require_compositor_runtime=True)

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

    def test_compositor_runtime_source_requires_every_exact_producer_artifact(self):
        verify = runpy.run_path(
            str(ROOT / "scripts/release/verify-compositor-runtime-source-run.py")
        )["verify"]
        run = {
            "id": 987,
            "head_sha": "commit",
            "head_repository": {"full_name": "owner/repo"},
            "path": ".github/workflows/compositor-runtime.yml",
            "event": "workflow_dispatch",
            "status": "completed",
            "conclusion": "success",
        }
        jobs = {"total_count": 4, "jobs": [
            {"name": name, "status": "completed", "conclusion": "success"}
            for name in (
                "GNOME 46 Ubuntu 24.04 (x86_64)",
                "GNOME 46 Ubuntu 24.04 (aarch64)",
                "KDE 6.0 Fedora 40 (x86_64)",
                "KDE 6.0 Fedora 40 (aarch64)",
            )
        ]}
        artifacts = {"artifacts": [
            {
                "name": name,
                "expired": False,
                "workflow_run": {"id": 987, "head_sha": "commit"},
            }
            for name in (
                "copypaste-compositor-runtime-gnome-46-ubuntu24.04-x86_64",
                "copypaste-compositor-runtime-gnome-46-ubuntu24.04-aarch64",
                "copypaste-compositor-runtime-kwin-6.0-fedora40-x86_64",
                "copypaste-compositor-runtime-kwin-6.0-fedora40-aarch64",
            )
        ]}
        verify(run, jobs, artifacts, "owner/repo", "commit")
        ci_run = {**run, "path": ".github/workflows/ci.yml", "event": "pull_request", "pull_requests": [{"number": 7}]}
        ci_jobs = {"total_count": 5, "jobs": [
            {"name": "Compositor runtime metadata", "status": "completed", "conclusion": "success"},
            *[{"name": f"Compositor runtime producer / {row['name']}", "status": "completed", "conclusion": "success"} for row in jobs["jobs"]],
        ]}
        ci_artifacts = {"artifacts": [*artifacts["artifacts"], *[
            {"name": name, "expired": False, "workflow_run": {"id": 987, "head_sha": "commit"}}
            for name in ("linux-pr-release-candidate-x86_64", "linux-pr-release-candidate-aarch64")
        ]]}
        verify(ci_run, ci_jobs, ci_artifacts, "owner/repo", "commit")
        ci_artifacts["artifacts"][-1]["expired"] = True
        with self.assertRaisesRegex(ValueError, "expired"):
            verify(ci_run, ci_jobs, ci_artifacts, "owner/repo", "commit")
        ci_artifacts["artifacts"][-1]["expired"] = False
        ci_artifacts["artifacts"].append({"name": "unknown-artifact", "expired": False, "workflow_run": {"id": 987, "head_sha": "commit"}})
        with self.assertRaisesRegex(ValueError, "inventory"):
            verify(ci_run, ci_jobs, ci_artifacts, "owner/repo", "commit")
        ci_artifacts["artifacts"].pop()
        ci_run["head_sha"] = "merge-commit"
        with self.assertRaisesRegex(ValueError, "trusted dispatch"):
            verify(ci_run, ci_jobs, artifacts, "owner/repo", "commit")
        ci_run["head_sha"] = "commit"
        ci_run["head_repository"] = {"full_name": "fork/repo"}
        with self.assertRaisesRegex(ValueError, "trusted dispatch"):
            verify(ci_run, ci_jobs, artifacts, "owner/repo", "commit")
        artifacts["artifacts"].pop()
        with self.assertRaisesRegex(ValueError, "artifact inventory"):
            verify(run, jobs, artifacts, "owner/repo", "commit")

    def test_compositor_runtime_uses_kwin_artifact_names_everywhere(self):
        expected = {
            "copypaste-compositor-runtime-kwin-6.0-fedora40-x86_64",
            "copypaste-compositor-runtime-kwin-6.0-fedora40-aarch64",
        }
        contract = json.loads((ROOT / "packaging/linux/compositor-runtime/release-contract.json").read_text(encoding="utf-8"))
        self.assertTrue(expected <= {row["artifact"] for row in contract["baselines"]})
        stage = runpy.run_path(str(ROOT / "scripts/release/stage-compositor-runtime-release.py"))
        self.assertTrue(expected <= {
            stage["coordinate_name"](short, family, distribution, architecture)
            for short, _desktop, family, distribution, _id, _version, _format in stage["COORDINATES"]
            for architecture in stage["ARCHITECTURES"]
        })
        for path in (
            ROOT / ".github/workflows/compositor-runtime.yml",
            ROOT / ".github/workflows/linux-native-qualification.yml",
            ROOT / ".github/workflows/release.yml",
        ):
            value = path.read_text(encoding="utf-8")
            for name in expected:
                self.assertIn(name, value)
            self.assertNotIn("copypaste-compositor-runtime-kde-", value)
        verifier = runpy.run_path(str(ROOT / "scripts/release/verify-compositor-runtime-source-run.py"))
        self.assertTrue(expected <= verifier["EXPECTED"])

    def test_compositor_runtime_public_receipt_binds_signed_package_bytes(self):
        module = runpy.run_path(
            str(ROOT / "scripts/release/stage-compositor-runtime-release.py")
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            source.mkdir()
            release_version = "1.2.3"
            commit = "a" * 40
            patch = "b" * 64
            for short, desktop, family, distribution, distribution_id, distribution_version, package_format in module["COORDINATES"]:
                for architecture in module["ARCHITECTURES"]:
                    artifact = source / module["coordinate_name"](short, family, distribution, architecture)
                    artifact.mkdir()
                    runtime_id = f"{short}-{family}-{architecture}"
                    package = artifact / f"copypaste-compositor-runtime-{runtime_id}-v{release_version}-linux-{architecture}.{package_format}"
                    package.write_bytes(f"{desktop}-{architecture}".encode())
                    runtime = artifact / "runtime-receipt.json"
                    runtime.write_text(json.dumps({
                        "schema": 1, "runtime_id": runtime_id,
                        "desktop": desktop, "architecture": architecture,
                        "distribution": {"id": distribution_id, "version": distribution_version},
                        "glibc_floor": "2.39",
                        "source": {"revision": "source", "patch_sha256": patch},
                    }))
                    license_path = artifact / "COPYING"
                    license_path.write_text("license")
                    producer = {
                        "schema": 1, "commit": commit, "producer_run_id": 456,
                        "source_run_id": 123, "version": release_version,
                        "architecture": architecture, "desktop": desktop, "family": family,
                        "distribution": {"id": distribution_id, "version": distribution_version},
                        "format": package_format, "runtime_id": runtime_id,
                        "glibc_floor": "2.39", "source": {"revision": "source", "patch_sha256": patch},
                        "runtime_receipt": module["metadata"](runtime),
                        "package": module["metadata"](package),
                        "upstream_licenses": [module["metadata"](license_path)],
                    }
                    (artifact / "compositor-runtime-producer-receipt.json").write_text(json.dumps(producer))
            public = root / "public"
            module["stage"](source, public, release_version, commit, "456")
            packages = sorted(path for path in public.iterdir() if path.suffix in (".deb", ".rpm"))
            self.assertEqual(len(packages), 4)
            self.assertTrue(all(f"-v{release_version}-linux-" in path.name for path in packages))
            for package in packages:
                package.with_name(package.name + ".sig").write_bytes(b"signature")
                package.with_name(package.name + ".sha256").write_text(
                    f"{module['digest'](package)}  {package.name}\n"
                )
            receipt = module["verify_public"](
                public,
                release_version,
                commit,
                "789",
                signature_validator=lambda _package, _signature: None,
            )
            self.assertEqual(receipt["producer_run_id"], 456)
            self.assertEqual(len(receipt["companions"]), 4)
            (public / "unlisted").write_bytes(b"must not publish")
            with self.assertRaisesRegex(ValueError, "unlisted file"):
                module["verify_public"](
                    public,
                    release_version,
                    commit,
                    "789",
                    signature_validator=lambda _package, _signature: None,
                )
            (public / "unlisted").unlink()
            packages[0].write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "changed after staging"):
                module["verify_public"](
                    public,
                    release_version,
                    commit,
                    "789",
                    signature_validator=lambda _package, _signature: None,
                )

    def test_compositor_runtime_signature_requires_a_valid_pinned_envelope(self):
        module = runpy.run_path(
            str(ROOT / "scripts/release/stage-compositor-runtime-release.py")
        )
        fixture_key = 'RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3'
        fixture_signature = '\n'.join((
            'untrusted comment: signature from minisign secret key',
            'RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=',
            'trusted comment: timestamp:1556193335\tfile:test',
            'y/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==',
            '',
        ))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = root / 'test'
            package.write_bytes(b'test')
            signature = root / 'test.sig'
            signature.write_text(base64.b64encode(fixture_signature.encode()).decode())
            module['verify_package_signature'](
                package,
                signature,
                public_key=fixture_key,
            )
            signature.write_text('not-a-signature')
            with self.assertRaisesRegex(ValueError, 'pinned updater key'):
                module['verify_package_signature'](
                    package,
                    signature,
                    public_key=fixture_key,
                )

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

    def test_linux_runtime_host_requirements_do_not_restore_legacy_pins(self):
        packages = (ROOT / "scripts/release/build-linux-packages.sh").read_text(encoding="utf-8")
        workflow = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        smoke = (ROOT / "scripts/release/smoke-linux-production.sh").read_text(encoding="utf-8")
        desktop = (ROOT / "packaging/linux/com.copypaste.CopyPaste.desktop").read_text(encoding="utf-8")
        self.assertIn('receipt.get("host_requirements", [])', packages)
        self.assertIn('receipt.get("package_dependencies") == []', packages)
        self.assertNotIn('item["name"], item["version"]', packages)
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
        self.assertIn("linux-desktop-acceptance.py", fixture)
        self.assertIn("linux-wayland-quick-paste.py", fixture)
        self.assertIn("portal_keyboard_grant", fixture)
        self.assertNotIn("Wayland companion authentication and portal keyboard grant require", fixture)

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

    def test_compositor_runtime_receipt_is_required_only_for_the_new_linux_contract(self):
        verify = runpy.run_path(str(ROOT / "scripts/release/verify-artifact-receipts.py"))["verify"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_receipts(root)
            with self.assertRaisesRegex(ValueError, "compositor runtime"):
                verify(root, "1.2.3", "a" * 40, "123", require_compositor_runtime=True)

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
        def module_ipc():
            rows = []
            for module_id in ("copypaste.ocr", "copypaste.semantic-search", "copypaste.supabase"):
                for operation, enabled in (("install", None), ("set_enabled", True), ("invoke", None), ("set_enabled", False), ("remove", None)):
                    row = {
                        "endpoint_category": "gui_owned_unix_socket", "method": "modules",
                        "operation": operation, "module_id": module_id,
                        "request_sha256": "a" * 64, "response_sha256": "b" * 64,
                        "success": True, "assertions": ["modules"],
                    }
                    if enabled is not None:
                        row["enabled"] = enabled
                    rows.append(row)
            rows.append({
                "endpoint_category": "gui_owned_unix_socket", "method": "modules",
                "operation": "set_preferences", "module_id": "copypaste.semantic-search",
                "request_sha256": "c" * 64, "response_sha256": "d" * 64,
                "success": True, "assertions": ["modules"],
            })
            return rows
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifacts = root / "artifacts"
            previous_artifacts = root / "previous-artifacts"
            artifacts.mkdir()
            previous_artifacts.mkdir()
            for architecture in ("x86_64", "aarch64"):
                for extension in ("AppImage", "deb", "rpm"):
                    (artifacts / f"CopyPaste-v1.2.3-linux-{architecture}.{extension}").write_bytes(b"package")
                    (previous_artifacts / f"CopyPaste-v1.2.2-linux-{architecture}.{extension}").write_bytes(b"prior-package")
            package_rows = lambda architecture: [{
                "name": f"CopyPaste-v1.2.3-linux-{architecture}.{extension}",
                "sha256": hashlib.sha256(b"package").hexdigest(), "size_bytes": len(b"package"),
            } for extension in ("AppImage", "deb", "rpm")]
            previous_package_rows = lambda architecture: [{
                "name": f"CopyPaste-v1.2.2-linux-{architecture}.{extension}",
                "sha256": hashlib.sha256(b"prior-package").hexdigest(), "size_bytes": len(b"prior-package"),
            } for extension in ("AppImage", "deb", "rpm")]
            for architecture in ("x86_64", "aarch64"):
                for desktop in ("GNOME", "KDE"):
                    for session in ("x11", "wayland"):
                        assertions = verifier["assertion_set"]("prior_release", session)
                        trace_name = f"linux-native-{architecture}-{desktop.lower()}-{session}.trace.json"
                        native_format = "deb" if desktop == "GNOME" else "rpm"
                        installed = [row for row in package_rows(architecture) if row["name"].endswith((".AppImage", f".{native_format}"))]
                        appimage = next(row for row in installed if row["name"].endswith(".AppImage"))
                        native = next(row for row in installed if row["name"].endswith(f".{native_format}"))
                        appimage = {"format": "AppImage", "path": f"/driver/{appimage['name']}", **appimage}
                        native = {"format": native_format, "path": f"/driver/{native['name']}", **native}
                        appimage_argv = ["env", "APPIMAGE_EXTRACT_AND_RUN=1", appimage["path"], "--appimage-extract"]
                        native_argv = (["sudo", "apt-get", "install", "--yes", native["path"]]
                                       if native_format == "deb" else ["sudo", "dnf", "--assumeyes", "install", native["path"]])
                        prior_installed = [row for row in previous_package_rows(architecture) if row["name"].endswith((".AppImage", f".{native_format}"))]
                        prior_appimage = next(row for row in prior_installed if row["name"].endswith(".AppImage"))
                        prior_native = next(row for row in prior_installed if row["name"].endswith(f".{native_format}"))
                        prior_appimage = {"format": "AppImage", "path": f"/driver/{prior_appimage['name']}", **prior_appimage}
                        prior_native = {"format": native_format, "path": f"/driver/{prior_native['name']}", **prior_native}
                        prior_appimage_argv = ["env", "APPIMAGE_EXTRACT_AND_RUN=1", prior_appimage["path"], "--appimage-extract"]
                        prior_native_argv = (["sudo", "apt-get", "install", "--yes", prior_native["path"]]
                                             if native_format == "deb" else ["sudo", "dnf", "--assumeyes", "install", prior_native["path"]])
                        upgrade = {
                            "prior": {"version": "1.2.2", "packages": [prior_appimage, prior_native], "appimage_extract_argv": prior_appimage_argv, "native_install_argv": prior_native_argv},
                            "current": {"version": "1.2.3", "packages": [appimage, native], "appimage_extract_argv": appimage_argv, "native_install_argv": native_argv},
                            "seeds": [
                                {"format": "AppImage", "source_executable": {"path": "/driver/appimage-prior/squashfs-root/usr/lib/copypaste/copypaste", "sha256": "d" * 64, "size_bytes": 1}, "canary": {"id": "prior-appimage", "content_sha256": verifier["UPGRADE_CANARY_SHA256"], "executable_sha256": "d" * 64}},
                                {"format": native_format, "source_executable": {"path": "/usr/lib/copypaste/copypaste", "sha256": "f" * 64, "size_bytes": 1}, "canary": {"id": "prior-native", "content_sha256": verifier["UPGRADE_CANARY_SHA256"], "executable_sha256": "f" * 64}},
                            ],
                        }
                        compositor_runtime = {"schema": 1, "producer_run_id": "1", "commit": "a" * 40, "runtime_id": "fixture", "desktop": desktop, "architecture": architecture, "distribution": "ubuntu" if desktop == "GNOME" else "fedora", "format": native_format, "package": {"name": "runtime-package", "sha256": "a" * 64, "size_bytes": 1}, "runtime_receipt": {"name": "runtime-receipt", "sha256": "b" * 64, "size_bytes": 1}}
                        compositor_session = None
                        session_evidence = []
                        if session == "wayland":
                            binding_path = root / f"linux-compositor-{architecture}-{desktop.lower()}-{session}.binding.json"
                            binding_path.write_text(json.dumps(compositor_runtime), encoding="utf-8")
                            record_path = root / f"linux-compositor-{architecture}-{desktop.lower()}-{session}.session.json"
                            record_path.write_text(json.dumps({
                                "schema": 1,
                                "binding_sha256": hashlib.sha256(binding_path.read_bytes()).hexdigest(),
                                "session": "wayland",
                                "runtime_id": "fixture",
                                "desktop": desktop,
                                "pid": 42,
                                "executable": {"path": "usr/bin/private-compositor", "sha256": "c" * 64},
                                "mapped_private_libraries": [{"path": "lib/libmutter-private.so" if desktop == "GNOME" else "lib/libkwin-private.so", "sha256": "d" * 64}],
                            }), encoding="utf-8")
                            def attachment(path):
                                return {"name": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "size_bytes": path.stat().st_size}
                            compositor_session = {"binding": attachment(binding_path), "record": attachment(record_path)}
                            session_evidence = [compositor_session["binding"], compositor_session["record"]]
                        trace = {
                            "schema": 1, "version": "1.2.3", "commit": "a" * 40,
                            "source_run_id": "123", "artifact_run_id": "456",
                            "architecture": architecture, "desktop": desktop, "session": session,
                            "upgrade_mode": "prior_release",
                            "installed_formats": ["AppImage", native_format],
                            "environment": {"distribution": "ubuntu" if desktop == "GNOME" else "fedora", "distribution_version": "24.04"},
                            "commands": [
                                {"argv": prior_appimage_argv, "returncode": 0, "assertions": ["package_upgrade"]},
                                {"argv": prior_native_argv, "returncode": 0, "assertions": ["package_upgrade"]},
                                {"argv": appimage_argv, "returncode": 0, "assertions": ["package_install"]},
                                {"argv": native_argv, "returncode": 0, "assertions": ["package_install"]},
                                {"argv": ["native-probe"], "returncode": 0, "assertions": sorted(assertions - {"modules", "package_install", "package_upgrade"})},
                            ],
                            "ipc": module_ipc(),
                            "installation": {
                                "formats": ["AppImage", native_format], "packages": [appimage, native],
                                "appimage_extract_argv": appimage_argv, "native_install_argv": native_argv,
                            },
                            "upgrade": upgrade,
                            "runtimes": [
                                {"format": "AppImage", "gui_owned_daemon": True, "executables": {name: {"path": f"/runtime/squashfs-root/usr/lib/copypaste/{name}", "sha256": "e" * 64, "size_bytes": 1} for name in ("copypaste", "copypaste-daemon", "copypaste-cli")}},
                                {"format": native_format, "gui_owned_daemon": True, "executables": {name: {"path": f"/usr/lib/copypaste/{name}", "sha256": "e" * 64, "size_bytes": 1} for name in ("copypaste", "copypaste-daemon", "copypaste-cli")}},
                            ],
                            "compositor_runtime": compositor_runtime,
                            "compositor_session": compositor_session,
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
                            "compositor_runtime": compositor_runtime,
                            "compositor_session": compositor_session,
                            "packages": package_rows(architecture),
                            "previous_packages": previous_package_rows(architecture),
                            "upgrade": upgrade,
                            "trace": {"name": trace_name, "sha256": trace_digest, "size_bytes": (root / trace_name).stat().st_size},
                            "evidence": [{"name": trace_name, "sha256": trace_digest, "size_bytes": (root / trace_name).stat().st_size}, *session_evidence],
                        }))
            verify(root, artifacts, "1.2.3", "a" * 40, "123", "456", previous_artifacts=previous_artifacts, previous_version="1.2.2")
            trace_path = root / "linux-native-x86_64-gnome-x11.trace.json"
            x11 = root / "linux-native-x86_64-gnome-x11.json"
            original_trace = json.loads(trace_path.read_text())
            original_receipt = json.loads(x11.read_text())
            def rejects_trace(mutator, message):
                trace = deepcopy(original_trace)
                mutator(trace)
                trace_path.write_text(json.dumps(trace))
                receipt = deepcopy(original_receipt)
                attachment = {"name": trace_path.name, "sha256": hashlib.sha256(trace_path.read_bytes()).hexdigest(), "size_bytes": trace_path.stat().st_size}
                receipt["trace"] = attachment
                receipt["evidence"] = [attachment]
                with self.assertRaisesRegex(ValueError, message):
                    verifier["verify_trace"](root, receipt, "x86_64", "GNOME", "x11", "1.2.3", "a" * 40, "123", "456", verifier["assertion_set"]("prior_release", "x11"), "1.2.2", verifier["expected_packages"](previous_artifacts, "1.2.2", "x86_64"))
                trace_path.write_text(json.dumps(original_trace))
            rejects_trace(lambda trace: trace["installation"].update({"formats": ["AppImage", "deb", "deb"]}), "installation formats")
            rejects_trace(lambda trace: trace["installation"]["appimage_extract_argv"].append("--extra"), "installation commands")
            rejects_trace(lambda trace: trace["installation"]["native_install_argv"].__setitem__(-1, "/driver/substituted.deb"), "package manager command")
            rejects_trace(lambda trace: trace["runtimes"][1]["executables"]["copypaste"].__setitem__("sha256", "f" * 64), "executables differ")
            def rejects_upgrade(mutator, message):
                trace = deepcopy(original_trace)
                receipt = deepcopy(original_receipt)
                mutator(trace["upgrade"])
                receipt["upgrade"] = deepcopy(trace["upgrade"])
                trace_path.write_text(json.dumps(trace))
                attachment = {"name": trace_path.name, "sha256": hashlib.sha256(trace_path.read_bytes()).hexdigest(), "size_bytes": trace_path.stat().st_size}
                receipt["trace"] = attachment
                receipt["evidence"] = [attachment]
                x11.write_text(json.dumps(receipt))
                with self.assertRaisesRegex(ValueError, message):
                    verify(root, artifacts, "1.2.3", "a" * 40, "123", "456", previous_artifacts=previous_artifacts, previous_version="1.2.2")
                trace_path.write_text(json.dumps(original_trace))
                x11.write_text(json.dumps(original_receipt))
            rejects_upgrade(lambda upgrade: upgrade["prior"].__setitem__("version", "1.2.1"), "prior upgrade transition")
            rejects_upgrade(lambda upgrade: upgrade["prior"]["packages"][0].__setitem__("sha256", "f" * 64), "prior upgrade digest")
            trace = json.loads(trace_path.read_text())
            trace["commands"][0]["argv"] = ["unix-ipc", "modules"]
            trace_path.write_text(json.dumps(trace))
            receipt = json.loads(x11.read_text())
            trace_attachment = {
                "name": trace_path.name, "sha256": hashlib.sha256(trace_path.read_bytes()).hexdigest(),
                "size_bytes": trace_path.stat().st_size,
            }
            receipt["trace"] = trace_attachment
            receipt["evidence"] = [trace_attachment]
            x11.write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError, "typed IPC"):
                verify(root, artifacts, "1.2.3", "a" * 40, "123", "456", previous_artifacts=previous_artifacts, previous_version="1.2.2")
            trace["commands"][0]["argv"] = ["native-probe"]
            trace_path.write_text(json.dumps(trace))
            receipt = json.loads(x11.read_text())
            receipt["assertions"].pop("native_x11_keyboard_input")
            receipt["assertions"]["portal_keyboard_grant"] = True
            x11.write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError, "assertions are incomplete"):
                verify(root, artifacts, "1.2.3", "a" * 40, "123", "456", previous_artifacts=previous_artifacts, previous_version="1.2.2")
            receipt["assertions"].pop("portal_keyboard_grant")
            receipt["assertions"]["native_x11_keyboard_input"] = True
            x11.write_text(json.dumps(receipt))
            broken = root / "linux-native-x86_64-gnome-x11.json"
            receipt = json.loads(broken.read_text())
            receipt["assertions"]["package_upgrade"] = "true"
            broken.write_text(json.dumps(receipt))
            with self.assertRaisesRegex(ValueError, "assertions are incomplete"):
                verify(root, artifacts, "1.2.3", "a" * 40, "123", "456", previous_artifacts=previous_artifacts, previous_version="1.2.2")

    def test_linux_native_producer_owns_trace_and_artifact_digests(self):
        producer = runpy.run_path(str(ROOT / "scripts/release/produce-linux-native-qualification.py"))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ("current", "previous"):
                (root / name).mkdir()
                for extension in ("AppImage", "deb", "rpm"):
                    version = "1.2.3" if name == "current" else "1.2.2"
                    (root / name / f"CopyPaste-v{version}-linux-x86_64.{extension}").write_bytes(name.encode())
            module_artifacts = root / "module-artifacts"
            module_fixtures = root / "module-fixtures"
            module_artifacts.mkdir()
            module_fixtures.mkdir()
            binding = root / "binding.json"
            binding.write_text(json.dumps({"schema": 1, "producer_run_id": "1", "commit": "a" * 40, "runtime_id": "fixture", "desktop": "GNOME", "architecture": "x86_64", "distribution": producer["runtime_environment"]()["distribution"], "format": "deb", "package": {"name": "runtime.deb", "sha256": "a" * 64, "size_bytes": 1}, "runtime_receipt": {"name": "runtime.json", "sha256": "b" * 64, "size_bytes": 1}}), encoding="utf-8")
            driver = root / "driver.py"
            driver_assertions = producer["scenario_assertions"]("prior_release", "wayland")
            module_ipc = []
            for module_id in ("copypaste.ocr", "copypaste.semantic-search", "copypaste.supabase"):
                for operation, enabled in (("install", None), ("set_enabled", True), ("invoke", None), ("set_enabled", False), ("remove", None)):
                    row = {"endpoint_category": "gui_owned_unix_socket", "method": "modules", "operation": operation, "module_id": module_id, "request_sha256": "a" * 64, "response_sha256": "b" * 64, "success": True, "assertions": ["modules"]}
                    if enabled is not None:
                        row["enabled"] = enabled
                    module_ipc.append(row)
            module_ipc.append({"endpoint_category": "gui_owned_unix_socket", "method": "modules", "operation": "set_preferences", "module_id": "copypaste.semantic-search", "request_sha256": "c" * 64, "response_sha256": "d" * 64, "success": True, "assertions": ["modules"]})
            driver.write_text(
                "#!/usr/bin/env python3\nimport hashlib,json,os,sys\n"
                "def value(flag): return sys.argv[sys.argv.index(flag)+1]\n"
                "artifacts=value('--artifacts'); previous=value('--previous-artifacts'); appimage=os.path.join(artifacts,'CopyPaste-v1.2.3-linux-x86_64.AppImage'); native=os.path.join(artifacts,'CopyPaste-v1.2.3-linux-x86_64.deb'); prior_appimage=os.path.join(previous,'CopyPaste-v1.2.2-linux-x86_64.AppImage'); prior_native=os.path.join(previous,'CopyPaste-v1.2.2-linux-x86_64.deb')\n"
                "def package(path,format):\n d=hashlib.sha256(open(path,'rb').read()).hexdigest(); return {'format':format,'name':os.path.basename(path),'path':os.path.realpath(path),'sha256':d,'size_bytes':os.path.getsize(path)}\n"
                "appimage_argv=['env','APPIMAGE_EXTRACT_AND_RUN=1',appimage,'--appimage-extract']; native_argv=['sudo','apt-get','install','--yes',native]; prior_appimage_argv=['env','APPIMAGE_EXTRACT_AND_RUN=1',prior_appimage,'--appimage-extract']; prior_native_argv=['sudo','apt-get','install','--yes',prior_native]\n"
                "print('COPYPASTE_QUALIFICATION_COMMAND '+json.dumps({'argv':prior_appimage_argv,'returncode':0,'assertions':['package_upgrade']}))\n"
                "print('COPYPASTE_QUALIFICATION_COMMAND '+json.dumps({'argv':prior_native_argv,'returncode':0,'assertions':['package_upgrade']}))\n"
                "print('COPYPASTE_QUALIFICATION_COMMAND '+json.dumps({'argv':appimage_argv,'returncode':0,'assertions':['package_install']}))\n"
                "print('COPYPASTE_QUALIFICATION_COMMAND '+json.dumps({'argv':native_argv,'returncode':0,'assertions':['package_install']}))\n"
                "print('COPYPASTE_QUALIFICATION_COMMAND '+json.dumps({'argv':['probe'],'returncode':0,'assertions':" + repr(sorted(driver_assertions - {"modules", "package_install"})) + "}))\n"
                "for row in " + repr(module_ipc) + ": print('COPYPASTE_QUALIFICATION_IPC '+json.dumps(row))\n"
                "print('COPYPASTE_QUALIFICATION_INSTALL '+json.dumps({'formats':['AppImage','deb'],'packages':[package(appimage,'AppImage'),package(native,'deb')],'appimage_extract_argv':appimage_argv,'native_install_argv':native_argv}))\n"
                "canary=hashlib.sha256(b'copypaste-upgrade-canary').hexdigest(); source_app={'path':'/runtime/appimage-prior/squashfs-root/usr/lib/copypaste/copypaste','sha256':'d'*64,'size_bytes':1}; source_native={'path':'/usr/lib/copypaste/copypaste','sha256':'f'*64,'size_bytes':1}\n"
                "print('COPYPASTE_QUALIFICATION_UPGRADE '+json.dumps({'prior':{'version':'1.2.2','packages':[package(prior_appimage,'AppImage'),package(prior_native,'deb')],'appimage_extract_argv':prior_appimage_argv,'native_install_argv':prior_native_argv},'current':{'version':'1.2.3','packages':[package(appimage,'AppImage'),package(native,'deb')],'appimage_extract_argv':appimage_argv,'native_install_argv':native_argv},'seeds':[{'format':'AppImage','source_executable':source_app,'canary':{'id':'prior-appimage','content_sha256':canary,'executable_sha256':source_app['sha256']}},{'format':'deb','source_executable':source_native,'canary':{'id':'prior-native','content_sha256':canary,'executable_sha256':source_native['sha256']}}]}))\n"
                "for format in ['AppImage','deb']: print('COPYPASTE_QUALIFICATION_RUNTIME '+json.dumps({'format':format,'gui_owned_daemon':True,'executables':{name:{'path':('/runtime/squashfs-root/usr/lib/copypaste/' if format=='AppImage' else '/usr/lib/copypaste/')+name,'sha256':'e'*64,'size_bytes':1} for name in ['copypaste','copypaste-daemon','copypaste-cli']}}))\n",
                encoding="utf-8",
            )
            driver.chmod(0o755)
            session_record = root / "compositor-session.json"
            session_record.write_text(json.dumps({
                "schema": 1,
                "binding_sha256": hashlib.sha256(binding.read_bytes()).hexdigest(),
                "session": "wayland",
                "runtime_id": "fixture",
                "desktop": "GNOME",
                "pid": 42,
                "executable": {"path": "usr/bin/private-compositor", "sha256": "c" * 64},
                "mapped_private_libraries": [{"path": "lib/libmutter-private.so", "sha256": "d" * 64}],
            }), encoding="utf-8")
            previous_session_record = os.environ.get("COPYPASTE_COMPOSITOR_SESSION_RECORD")
            os.environ["COPYPASTE_COMPOSITOR_SESSION_RECORD"] = str(session_record)
            try:
                receipt_path = producer["produce"](argparse.Namespace(
                    artifacts=root / "current", previous_artifacts=root / "previous",
                    version="1.2.3", previous_version="1.2.2", architecture="x86_64",
                    desktop="GNOME", session="wayland", commit="a" * 40,
                    source_run_id="123", artifact_run_id="456", driver=driver, output=root / "evidence",
                    module_artifacts=module_artifacts, module_fixtures=module_fixtures, compositor_runtime_binding=binding,
                ))
            finally:
                if previous_session_record is None:
                    os.environ.pop("COPYPASTE_COMPOSITOR_SESSION_RECORD", None)
                else:
                    os.environ["COPYPASTE_COMPOSITOR_SESSION_RECORD"] = previous_session_record
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
            fake_ipc = "COPYPASTE_QUALIFICATION_COMMAND " + json.dumps({
                "argv": ["unix-ipc", "modules", "invented"], "returncode": 0,
                "assertions": [next(iter(driver_assertions - {"modules"}))],
            })
            with self.assertRaisesRegex(ValueError, "typed IPC"):
                producer["command_rows"](fake_ipc, driver_assertions)

    def test_linux_native_producer_forwards_resolved_module_staging_paths(self):
        producer = runpy.run_path(str(ROOT / "scripts/release/produce-linux-native-qualification.py"))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            current = root / "current"
            current.mkdir()
            for extension in ("AppImage", "deb", "rpm"):
                (current / f"CopyPaste-v1.2.3-linux-x86_64.{extension}").write_bytes(b"package")
            module_artifacts = root / "module-artifacts"
            module_fixtures = root / "module-fixtures"
            module_artifacts.mkdir()
            module_fixtures.mkdir()
            binding = root / "binding.json"
            binding.write_text(json.dumps({"schema": 1, "producer_run_id": "1", "commit": "a" * 40, "runtime_id": "fixture", "desktop": "GNOME", "architecture": "x86_64", "distribution": producer["runtime_environment"]()["distribution"], "format": "deb", "package": {"name": "runtime.deb", "sha256": "a" * 64, "size_bytes": 1}, "runtime_receipt": {"name": "runtime.json", "sha256": "b" * 64, "size_bytes": 1}}), encoding="utf-8")
            driver = root / "driver"
            driver.write_text("fixture", encoding="utf-8")
            driver.chmod(0o755)
            arguments = argparse.Namespace(
                artifacts=current, previous_artifacts=None, version="1.2.3", previous_version=None,
                architecture="x86_64", desktop="GNOME", session="x11", commit="a" * 40,
                source_run_id="123", artifact_run_id="456", driver=driver, output=root / "evidence",
                module_artifacts=module_artifacts, module_fixtures=module_fixtures, compositor_runtime_binding=binding,
            )
            expected = producer["scenario_assertions"]("first_install_baseline", "x11")
            module_ipc = []
            for module_id in ("copypaste.ocr", "copypaste.semantic-search", "copypaste.supabase"):
                for operation, enabled in (("install", None), ("set_enabled", True), ("invoke", None), ("set_enabled", False), ("remove", None)):
                    row = {"endpoint_category": "gui_owned_unix_socket", "method": "modules", "operation": operation, "module_id": module_id, "request_sha256": "a" * 64, "response_sha256": "b" * 64, "success": True, "assertions": ["modules"]}
                    if enabled is not None:
                        row["enabled"] = enabled
                    module_ipc.append(row)
            module_ipc.append({"endpoint_category": "gui_owned_unix_socket", "method": "modules", "operation": "set_preferences", "module_id": "copypaste.semantic-search", "request_sha256": "c" * 64, "response_sha256": "d" * 64, "success": True, "assertions": ["modules"]})
            appimage = current / "CopyPaste-v1.2.3-linux-x86_64.AppImage"
            native = current / "CopyPaste-v1.2.3-linux-x86_64.deb"
            appimage_argv = ["env", "APPIMAGE_EXTRACT_AND_RUN=1", str(appimage.resolve()), "--appimage-extract"]
            native_argv = ["sudo", "apt-get", "install", "--yes", str(native.resolve())]
            output = "COPYPASTE_QUALIFICATION_COMMAND " + json.dumps({"argv": appimage_argv, "returncode": 0, "assertions": ["package_install"]})
            output += "\nCOPYPASTE_QUALIFICATION_COMMAND " + json.dumps({"argv": native_argv, "returncode": 0, "assertions": ["package_install"]})
            output += "\nCOPYPASTE_QUALIFICATION_COMMAND " + json.dumps({"argv": ["probe"], "returncode": 0, "assertions": sorted(expected - {"modules", "package_install"})})
            output += "\n" + "\n".join("COPYPASTE_QUALIFICATION_IPC " + json.dumps(row) for row in module_ipc)
            output += "\nCOPYPASTE_QUALIFICATION_INSTALL " + json.dumps({
                "formats": ["AppImage", "deb"], "packages": [
                    {"format": "AppImage", "name": appimage.name, "path": str(appimage.resolve()), "sha256": hashlib.sha256(appimage.read_bytes()).hexdigest(), "size_bytes": appimage.stat().st_size},
                    {"format": "deb", "name": native.name, "path": str(native.resolve()), "sha256": hashlib.sha256(native.read_bytes()).hexdigest(), "size_bytes": native.stat().st_size},
                ], "appimage_extract_argv": appimage_argv, "native_install_argv": native_argv,
            })
            output += "\n" + "\n".join("COPYPASTE_QUALIFICATION_RUNTIME " + json.dumps({
                "format": format_name, "gui_owned_daemon": True,
                "executables": {name: {"path": (f"/runtime/squashfs-root/usr/lib/copypaste/{name}" if format_name == "AppImage" else f"/usr/lib/copypaste/{name}"), "sha256": "e" * 64, "size_bytes": 1} for name in ("copypaste", "copypaste-daemon", "copypaste-cli")},
            }) for format_name in ("AppImage", "deb"))
            completed = subprocess.CompletedProcess(["driver"], 0, stdout=output)
            with mock.patch.dict(os.environ, {}, clear=True), \
                    mock.patch.object(producer["subprocess"], "run", return_value=completed) as run:
                producer["produce"](arguments)
            argv = run.call_args.args[0]
            self.assertEqual(argv[argv.index("--module-artifacts") + 1], str(module_artifacts.resolve()))
            self.assertEqual(argv[argv.index("--module-fixtures") + 1], str(module_fixtures.resolve()))
            packages = producer["artifact_inventory"](current, "1.2.3", "x86_64")
            with self.assertRaisesRegex(ValueError, "exactly one actual package installation"):
                producer["installation_report"]("", packages, current)
            with self.assertRaisesRegex(ValueError, "invalid formats"):
                producer["installation_report"]("COPYPASTE_QUALIFICATION_INSTALL " + json.dumps({
                    "formats": ["AppImage"], "packages": [], "appimage_extract_argv": [], "native_install_argv": [],
                }), packages, current)


if __name__ == "__main__":
    unittest.main()
