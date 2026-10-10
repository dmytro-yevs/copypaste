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
        return {
            "schema": 1, "runtime_id": "kwin-6.3-test", "desktop": desktop,
            "architecture": "x86_64", "distribution": {"id": distribution_id, "version": distribution_version},
            "glibc_floor": "2.39", "source": {"revision": "v6.3.0", "patch_sha256": "a" * 64},
            "payload": rows, "launch": launch, "runtime_env": {}, "package_dependencies": [{"name": "kwin", "version": "6.3.0"}],
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

    def test_receipt_requires_matching_package_dependencies(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runtime = root / "input"
            (runtime / "bin").mkdir(parents=True)
            executable = runtime / "bin/start"
            executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            executable.chmod(0o755)
            receipt = self.make_receipt(runtime, {"kind": "private", "entrypoint": "bin/start"})
            receipt["package_dependencies"] = []
            with self.assertRaises(stage_runtime.ContractError):
                stage_runtime.validate_receipt(receipt)

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
