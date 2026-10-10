#!/usr/bin/env python3
"""Contract tests for opt-in compositor runtime staging."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path


MODULE = Path(__file__).parents[1] / "stage_runtime.py"
sys.path.insert(0, str(MODULE.parent))
SPEC = importlib.util.spec_from_file_location("stage_runtime", MODULE)
assert SPEC and SPEC.loader
stage_runtime = importlib.util.module_from_spec(SPEC)
sys.modules["stage_runtime"] = stage_runtime
SPEC.loader.exec_module(stage_runtime)
verify_spec = importlib.util.spec_from_file_location("verify_runtime_package", Path(__file__).parents[1] / "verify_runtime_package.py")
assert verify_spec and verify_spec.loader
verify_runtime_package = importlib.util.module_from_spec(verify_spec)
verify_spec.loader.exec_module(verify_runtime_package)
builder_spec = importlib.util.spec_from_file_location("build_companion_package", Path(__file__).parents[1] / "build_companion_package.py")
assert builder_spec and builder_spec.loader
build_companion_package = importlib.util.module_from_spec(builder_spec)
builder_spec.loader.exec_module(build_companion_package)
closure_spec = importlib.util.spec_from_file_location("private_elf_closure", Path(__file__).parents[1] / "private_elf_closure.py")
assert closure_spec and closure_spec.loader
private_elf_closure = importlib.util.module_from_spec(closure_spec)
closure_spec.loader.exec_module(private_elf_closure)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def os_release() -> tuple[str, str]:
    if not Path("/etc/os-release").is_file():
        return "fedora", "40"
    values = {}
    for line in Path("/etc/os-release").read_text(encoding="utf-8").splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            values[key] = value.strip('"')
    return values["ID"], values["VERSION_ID"]


class RuntimeStageTests(unittest.TestCase):
    def make_receipt(self, runtime: Path, launch: dict, desktop: str = "KDE") -> dict:
        license_file = runtime / "COPYING"
        if not license_file.exists():
            license_file.write_text("GPL-2.0-or-later", encoding="utf-8")
        if desktop == "KDE":
            private_library = runtime / "usr/lib/libKDecoration2.so.6.0.0"
            private_library.parent.mkdir(parents=True, exist_ok=True)
            private_library.write_text("private decoration ABI", encoding="utf-8")
            closure_license = runtime / "usr/share/doc/copypaste-compositor-runtime-private-licenses/kdecoration2-LICENSE"
            closure_license.parent.mkdir(parents=True, exist_ok=True)
            closure_license.write_text("LGPL-2.1-or-later", encoding="utf-8")
            closure_notice = runtime / "usr/share/doc/copypaste-compositor-runtime-private-licenses/kdecoration2-NOTICE"
            closure_notice.write_text("KDecoration notice", encoding="utf-8")
            closure_manifest = runtime / "usr/share/copypaste/compositor-runtime-private-closure.json"
            closure_manifest.parent.mkdir(parents=True, exist_ok=True)
            closure_manifest.write_text(json.dumps({
                "schema": 1,
                "libraries": [{"path": "usr/lib/libKDecoration2.so.6.0.0", "soname": "libKDecoration2.so.6", "sha256": digest(private_library.read_bytes()), "package": "kdecoration2", "evr": "5.115.0-1"}],
                "packages": [{"name": "kdecoration2", "evr": "5.115.0-1", "source_rpm": "kdecoration2-5.115.0-1.src.rpm", "license": "LGPL-2.1-or-later"}],
                "licenses": [
                    {"package": "kdecoration2", "license_package": "kdecoration2", "license_evr": "5.115.0-1", "license_source_rpm": "kdecoration2-5.115.0-1.src.rpm", "license": "LGPL-2.1-or-later", "path": "usr/share/doc/copypaste-compositor-runtime-private-licenses/kdecoration2-LICENSE", "sha256": digest(closure_license.read_bytes())},
                    {"package": "kdecoration2", "license_package": "kdecoration2", "license_evr": "5.115.0-1", "license_source_rpm": "kdecoration2-5.115.0-1.src.rpm", "license": "LGPL-2.1-or-later", "path": "usr/share/doc/copypaste-compositor-runtime-private-licenses/kdecoration2-NOTICE", "sha256": digest(closure_notice.read_bytes())},
                ],
            }), encoding="utf-8")
        rows = []
        for path in sorted(runtime.rglob("*")):
            if path.is_dir():
                continue
            relative = path.relative_to(runtime).as_posix()
            if path.is_symlink():
                target = os.readlink(path)
                rows.append({"path": relative, "type": "symlink", "target": target, "sha256": digest(target.encode())})
            else:
                rows.append({"path": relative, "type": "file", "mode": f"{path.stat().st_mode & 0o7777:04o}", "sha256": digest(path.read_bytes())})
        distribution_id, distribution_version = os_release()
        qualification = launch["entrypoint"] if launch["kind"] == "private" else "bin/headless"
        licenses = [{"spdx": "GPL-2.0-or-later", "name": "COPYING", "sha256": digest(license_file.read_bytes())}]
        if desktop == "KDE":
            closure_license = runtime / "usr/share/doc/copypaste-compositor-runtime-private-licenses/kdecoration2-LICENSE"
            licenses.append({"spdx": "LGPL-2.1-or-later", "name": closure_license.relative_to(runtime).as_posix(), "sha256": digest(closure_license.read_bytes())})
            closure_notice = runtime / "usr/share/doc/copypaste-compositor-runtime-private-licenses/kdecoration2-NOTICE"
            licenses.append({"spdx": "LGPL-2.1-or-later", "name": closure_notice.relative_to(runtime).as_posix(), "sha256": digest(closure_notice.read_bytes())})
        return {
            "schema": 1, "runtime_id": "kwin-6.3-test", "desktop": desktop,
            "architecture": "x86_64", "distribution": {"id": distribution_id, "version": distribution_version},
            "glibc_floor": "2.39", "source": {"revision": "v6.3.0", "patch_sha256": "a" * 64},
            "payload": rows, "launch": launch, "qualification": {"kind": "headless", "entrypoint": qualification}, "runtime_env": {}, "host_requirements": [],
            "upstream_licenses": licenses,
        }

    @unittest.skipUnless(Path("/etc/os-release").is_file(), "launcher executes only in a Linux session")
    def test_private_launcher_is_argument_free_and_scrubs_loader_injection(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runtime = root / "input"
            (runtime / "bin").mkdir(parents=True)
            executable = runtime / "bin/start"
            executable.write_text("#!/bin/sh\nprintf '%s|%s|%s|%s\\n' \"$#\" \"${LD_PRELOAD-unset}\" \"$LD_LIBRARY_PATH\" \"$CAPTURE\" > \"$CAPTURE\"\n", encoding="utf-8")
            executable.chmod(0o755)
            receipt = self.make_receipt(runtime, {"kind": "private", "entrypoint": "bin/start"})
            receipt_path = root / "receipt.json"
            receipt_path.write_text(json.dumps(receipt), encoding="utf-8")
            runtime_prefix = root / "private"
            launcher_dir = root / "launchers"
            session_dir = root / "sessions"
            launcher, _ = stage_runtime.stage(receipt_path, runtime, Path("/"), str(runtime_prefix), str(session_dir), str(launcher_dir), str(root / "receipts"))
            capture = root / "capture"
            completed = subprocess.run([str(launcher)], env={**os.environ, "LD_PRELOAD": "/tmp/injected.so", "CAPTURE": str(capture)}, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(capture.read_text(encoding="utf-8"), f"0|unset|{runtime_prefix}/kwin-6.3-test/lib:{runtime_prefix}/kwin-6.3-test/lib64|{capture}\n")
            rejected = subprocess.run([str(launcher), "unexpected"], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            self.assertEqual(rejected.returncode, 64)

    def test_receipt_rejects_an_escape_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runtime = root / "input"
            runtime.mkdir()
            (runtime / "libescape.so").symlink_to("../../etc/passwd")
            receipt = self.make_receipt(runtime, {"kind": "private", "entrypoint": "libescape.so"})
            receipt_path = root / "receipt.json"
            receipt_path.write_text(json.dumps(receipt), encoding="utf-8")
            with self.assertRaises(stage_runtime.ContractError):
                stage_runtime.stage(receipt_path, runtime, root / "stage", str(root / "private"), str(root / "sessions"), str(root / "launchers"))

    def test_receipt_materializes_a_verified_internal_soname_link(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runtime = root / "input"
            (runtime / "bin").mkdir(parents=True)
            (runtime / "lib").mkdir()
            (runtime / "lib64").mkdir()
            executable = runtime / "bin/start"
            executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            executable.chmod(0o755)
            (runtime / "lib/libbridge.so.1").write_bytes(b"immutable bridge")
            (runtime / "lib64/libbridge.so").symlink_to("../lib/libbridge.so.1")
            receipt = self.make_receipt(runtime, {"kind": "private", "entrypoint": "bin/start"})
            receipt_path = root / "receipt.json"
            receipt_path.write_text(json.dumps(receipt), encoding="utf-8")
            stage_runtime.stage(receipt_path, runtime, root, "/usr/lib/copypaste/compositor-runtime", "/usr/share/wayland-sessions", "/usr/lib/copypaste/compositor-runtime/bin")
            link = root / "usr/lib/copypaste/compositor-runtime/kwin-6.3-test/lib64/libbridge.so"
            self.assertTrue(link.is_symlink())
            self.assertEqual(os.readlink(link), "../lib/libbridge.so.1")

    def test_gnome_system_session_requires_the_pinned_matching_pair(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runtime = root / "input"
            (runtime / "lib").mkdir(parents=True)
            (runtime / "lib/libmutter.so").write_bytes(b"private mutter")
            (runtime / "bin").mkdir()
            headless = runtime / "bin/headless"
            headless.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            headless.chmod(0o755)
            receipt = self.make_receipt(runtime, {
                "kind": "system-session", "command": ["/usr/bin/gnome-session", "--session=gnome"],
                "host_binaries": [
                    {"path": "/usr/bin/gnome-session", "sha256": "b" * 64},
                    {"path": "/usr/bin/gnome-shell", "sha256": "c" * 64},
                ],
            }, desktop="GNOME")
            stage_runtime.validate_receipt(receipt)
            receipt["launch"]["command"] = ["/usr/bin/gnome-shell"]
            with self.assertRaises(stage_runtime.ContractError):
                stage_runtime.validate_receipt(receipt)

    def test_receipt_accepts_empty_host_requirements_but_rejects_exact_pin(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runtime = root / "input"
            (runtime / "bin").mkdir(parents=True)
            executable = runtime / "bin/start"
            executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            executable.chmod(0o755)
            receipt = self.make_receipt(runtime, {"kind": "private", "entrypoint": "bin/start"})
            receipt["host_requirements"] = [{"name": "plasma-workspace", "operator": "=", "version": "6.0"}]
            with self.assertRaises(stage_runtime.ContractError):
                stage_runtime.validate_receipt(receipt)

    def test_kde_closure_manifest_requires_every_bundled_rpm_license(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runtime = root / "input"
            (runtime / "bin").mkdir(parents=True)
            executable = runtime / "bin/start"
            executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            executable.chmod(0o755)
            receipt = self.make_receipt(runtime, {"kind": "private", "entrypoint": "bin/start"})
            receipt_path = root / "receipt.json"
            receipt_path.write_text(json.dumps(receipt), encoding="utf-8")
            stage_runtime.validate_payload(runtime, receipt)
            manifest = runtime / "usr/share/copypaste/compositor-runtime-private-closure.json"
            data = json.loads(manifest.read_text(encoding="utf-8"))
            data["licenses"] = []
            manifest.write_text(json.dumps(data), encoding="utf-8")
            with self.assertRaises(stage_runtime.ContractError):
                stage_runtime.validate_payload(runtime, receipt)

    def test_rpm_safety_rejects_host_decoration_constraints_but_accepts_private_soname(self) -> None:
        build_companion_package.rpm_desktop_safety(
            requires="libKDecoration2.so.6()(64bit)\n", provides="libKDecoration2.so.6()(64bit)\n",
            obsoletes="", conflicts="",
        )
        for key in ("requires", "obsoletes", "conflicts"):
            values = {"requires": "kdecoration2 >= 5.0\n", "obsoletes": "kdecoration3\n", "conflicts": "kdecoration2\n"}
            with self.assertRaises(stage_runtime.ContractError):
                build_companion_package.rpm_desktop_safety(
                    requires=values[key] if key == "requires" else "", provides="",
                    obsoletes=values[key] if key == "obsoletes" else "", conflicts=values[key] if key == "conflicts" else "",
                )

    def test_private_elf_closure_follows_transitive_needed_libraries_and_records_rpm_licenses(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runtime = root / "runtime"
            entrypoint = runtime / "usr/bin/kwin_wayland"
            entrypoint.parent.mkdir(parents=True)
            entrypoint.write_bytes(b"\x7fELF KWin")
            decoration = root / "libKDecoration2.so.6.0.0"
            framework = root / "libKF6CoreAddons.so.6.0.0"
            decoration.write_bytes(b"\x7fELF decoration")
            framework.write_bytes(b"\x7fELF framework")
            license_source = root / "LICENSE"
            license_source.write_text("LGPL-2.1-or-later", encoding="utf-8")
            notice_source = root / "NOTICE"
            notice_source.write_text("Qt exception notice", encoding="utf-8")

            def needed(path: Path) -> set[str]:
                return {
                    entrypoint: {"libKDecoration2.so.6"},
                    decoration: {"libKF6CoreAddons.so.6"},
                    framework: set(),
                }[path]

            def soname(path: Path) -> str | None:
                return {
                    entrypoint: None,
                    decoration: "libKDecoration2.so.6",
                    framework: "libKF6CoreAddons.so.6",
                }[path]

            with mock.patch.object(private_elf_closure, "library_cache", return_value={"libKDecoration2.so.6": decoration, "libKF6CoreAddons.so.6": framework}), \
                 mock.patch.object(private_elf_closure, "needed", side_effect=needed), \
                 mock.patch.object(private_elf_closure, "provided_soname", side_effect=soname), \
                 mock.patch.object(private_elf_closure, "dynamic_search_directories", return_value=[]), \
                 mock.patch.object(private_elf_closure, "trusted_library", side_effect=lambda path: path), \
                 mock.patch.object(private_elf_closure, "rpm_owner", side_effect=lambda path: ("kdecoration2" if path == decoration else "kf6-kcoreaddons", "6.0.0-1", "kde-6.0.0-1.src.rpm", "LGPL-2.1-or-later")), \
                 mock.patch.object(private_elf_closure, "rpm_license_files", side_effect=lambda owner: [(("kdecoration2-doc", "6.0.0-1", "kde-6.0.0-1.src.rpm", "LGPL-2.1-or-later"), license_source), (("kdecoration2-doc", "6.0.0-1", "kde-6.0.0-1.src.rpm", "LGPL-2.1-or-later"), notice_source)] if owner[0] == "kdecoration2" else [(("kf6-kcoreaddons", "6.0.0-1", "kde-6.0.0-1.src.rpm", "LGPL-2.1-or-later"), license_source)]):
                manifest = private_elf_closure.copy_closure(runtime, [entrypoint])

            self.assertEqual([item["soname"] for item in manifest["libraries"]], ["libKDecoration2.so.6", "libKF6CoreAddons.so.6"])
            self.assertEqual({item["package"] for item in manifest["licenses"]}, {"kdecoration2", "kf6-kcoreaddons"})
            self.assertEqual(sum(item["package"] == "kdecoration2" for item in manifest["licenses"]), 2)
            self.assertEqual({item["license_package"] for item in manifest["licenses"] if item["package"] == "kdecoration2"}, {"kdecoration2-doc"})
            self.assertEqual(os.readlink(runtime / "usr/lib/libKDecoration2.so.6"), "libKDecoration2.so.6.0.0")

    def test_rpm_owner_accepts_a_bounded_compound_license_expression(self) -> None:
        expression = "BSD-2-Clause AND BSD-3-Clause AND CC0-1.0 AND GPL-2.0-only AND GPL-2.0-or-later AND GPL-3.0-only AND GPL-3.0-or-later AND LGPL-2.0-only AND LGPL-2.0-or-later AND LGPL-2.1-only AND LGPL-2.1-or-later AND LGPL-3.0-only AND (GPL-2.0-only OR GPL-3.0-only) AND (LGPL-2.1-only OR LGPL-3.0-only) AND MIT"
        with mock.patch.object(private_elf_closure, "run", return_value=f"qtbase-gui\t6.7.0-1.fc40\tqtbase-6.7.0-1.fc40.src.rpm\t{expression}\n"):
            self.assertEqual(
                private_elf_closure.rpm_owner(Path("/usr/lib64/libQt6Core.so.6")),
                ("qtbase-gui", "6.7.0-1.fc40", "qtbase-6.7.0-1.fc40.src.rpm", expression),
            )

    def test_rpm_owner_reports_only_the_invalid_license_length(self) -> None:
        expression = "L" * 1025
        with mock.patch.object(private_elf_closure, "run", return_value=f"libproxy\t0.5.3-5.fc40\tlibproxy-0.5.3-5.fc40.src.rpm\t{expression}\n"):
            with self.assertRaisesRegex(private_elf_closure.ClosureError, r"library=libproxy\.so\.0 field=license length=1025"):
                private_elf_closure.rpm_owner(Path("/usr/lib64/libproxy.so.0"))

    def test_rpm_owner_accepts_a_fedora_snapshot_evr_and_source_rpm(self) -> None:
        with mock.patch.object(private_elf_closure, "run", return_value="libimobiledevice\t1.3.0^20230705git6fc41f5-4.fc40\tlibimobiledevice-1.3.0^20230705git6fc41f5-4.fc40.src.rpm\tLGPL-2.0-or-later\n"):
            self.assertEqual(
                private_elf_closure.rpm_owner(Path("/usr/lib64/libimobiledevice-1.0.so.6")),
                ("libimobiledevice", "1.3.0^20230705git6fc41f5-4.fc40", "libimobiledevice-1.3.0^20230705git6fc41f5-4.fc40.src.rpm", "LGPL-2.0-or-later"),
            )

    def test_rpm_siblings_require_an_exact_source_rpm_and_evr(self) -> None:
        owner = ("kwin-libs", "6.0.3.1-2.fc40", "kwin-6.0.3.1-2.fc40.src.rpm", "GPL-2.0-only")
        inventory = "\n".join((
            "kwin-libs\t6.0.3.1-2.fc40\tkwin-6.0.3.1-2.fc40.src.rpm\tGPL-2.0-only",
            "kwin-doc\t6.0.3.1-2.fc40\tkwin-6.0.3.1-2.fc40.src.rpm\tGPL-2.0-only",
            "kwin-old-doc\t6.0.2-1.fc40\tkwin-6.0.2-1.fc40.src.rpm\tGPL-2.0-only",
            "unrelated\t6.0.3.1-2.fc40\tunrelated-6.0.3.1-2.fc40.src.rpm\tMIT",
        )) + "\n"
        with mock.patch.object(private_elf_closure, "run", return_value=inventory):
            self.assertEqual(
                private_elf_closure.rpm_siblings(owner),
                [("kwin-doc", "6.0.3.1-2.fc40", "kwin-6.0.3.1-2.fc40.src.rpm", "GPL-2.0-only"), owner],
            )

    def test_declared_runpath_resolves_before_the_global_library_cache(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            dependency = directory / "libpxbackend-1.0.so"
            dependency.write_bytes(b"private backend")
            with mock.patch.object(private_elf_closure, "dynamic_search_directories", return_value=[directory]), \
                 mock.patch.object(private_elf_closure, "trusted_library", side_effect=lambda path: path):
                self.assertEqual(
                    private_elf_closure.resolve_dependency(Path("/usr/lib64/libproxy.so.0"), "libpxbackend-1.0.so", {}),
                    dependency,
                )

    def test_declared_search_path_rejects_escaped_directories(self) -> None:
        readelf = " 0x000000000000001d (RUNPATH)            Library runpath: [/usr/lib64/libproxy:/usr/lib64/libproxy/../../etc]\n"
        def trusted(path: Path) -> Path | None:
            return Path("/trusted/libproxy") if str(path) == "/usr/lib64/libproxy" else None
        with mock.patch.object(private_elf_closure, "run", return_value=readelf), \
             mock.patch.object(private_elf_closure, "trusted_library_directory", side_effect=trusted):
            self.assertEqual(
                private_elf_closure.dynamic_search_directories(Path("/usr/lib64/libproxy.so.0")),
                [Path("/trusted/libproxy")],
            )

    def test_staged_package_has_no_vendor_replacement_path(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runtime = root / "input"
            (runtime / "bin").mkdir(parents=True)
            executable = runtime / "bin/start"
            executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            executable.chmod(0o755)
            receipt = self.make_receipt(runtime, {"kind": "private", "entrypoint": "bin/start"})
            receipt_path = root / "receipt.json"
            receipt_path.write_text(json.dumps(receipt), encoding="utf-8")
            stage_runtime.stage(receipt_path, runtime, root, "/usr/lib/copypaste/compositor-runtime", "/usr/share/wayland-sessions", "/usr/lib/copypaste/compositor-runtime/bin")
            verify_runtime_package.verify(root, "kwin-6.3-test")
            (root / "usr/bin").mkdir(parents=True)
            (root / "usr/bin/kwin_wayland").write_text("bad", encoding="utf-8")
            with self.assertRaises(stage_runtime.ContractError):
                verify_runtime_package.verify(root, "kwin-6.3-test")


if __name__ == "__main__":
    unittest.main()
