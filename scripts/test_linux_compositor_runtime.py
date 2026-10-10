#!/usr/bin/env python3
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from types import SimpleNamespace
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]


def load():
    path = ROOT / "scripts/release/verify-linux-compositor-runtime.py"
    spec = importlib.util.spec_from_file_location("verify_linux_compositor_runtime", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class CompositorRuntimeSourceTest(unittest.TestCase):
    def setUp(self):
        self.module = load()
        self.run = {
            "id": 420, "head_sha": "a" * 40, "head_repository": {"full_name": "owner/repo"},
            "path": ".github/workflows/compositor-runtime.yml", "event": "workflow_dispatch",
            "status": "completed", "conclusion": "success",
        }
        self.artifacts = {"total_count": 4, "artifacts": [
            {"name": self.module.artifact_name(desktop, architecture), "expired": False,
             "workflow_run": {"id": 420, "head_sha": "a" * 40}}
            for desktop, architecture in self.module.RUNTIMES
        ]}

    def test_accepts_every_exact_baseline_runtime_artifact(self):
        self.module.verify_source(self.run, self.artifacts, "owner/repo", "a" * 40)

    def test_rejects_partial_and_duplicate_runtime_inventory(self):
        partial = copy.deepcopy(self.artifacts)
        partial["total_count"] = 5
        with self.assertRaisesRegex(ValueError, "inventory is incomplete"):
            self.module.verify_source(self.run, partial, "owner/repo", "a" * 40)
        duplicate = copy.deepcopy(self.artifacts)
        duplicate["artifacts"].append(copy.deepcopy(duplicate["artifacts"][0]))
        duplicate["total_count"] += 1
        with self.assertRaisesRegex(ValueError, "exactly one artifact"):
            self.module.verify_source(self.run, duplicate, "owner/repo", "a" * 40)

    def test_requires_checked_out_baseline_revision_and_patch_bytes(self):
        revision, patch = self.module.BASELINES["GNOME"]
        source = {"revision": revision, "patch_sha256": self.module.sha256(ROOT / patch)}
        self.module.verify_baseline_source("GNOME", source)
        source["patch_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "baseline"):
            self.module.verify_baseline_source("GNOME", source)


class InstalledRuntimeTest(unittest.TestCase):
    def setUp(self):
        self.module = load()
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binding = {"schema": 1, "producer_run_id": "7", "commit": "a" * 40,
                        "runtime_id": "kwin-6.0-fedora40", "desktop": "KDE", "architecture": "x86_64",
                        "distribution": {"id": "fedora", "version": "40"}, "format": "rpm",
                        "package": {"name": "sidecar.rpm", "sha256": "a" * 64, "size_bytes": 1},
                        "runtime_receipt": {"name": "runtime-receipt.json", "sha256": "", "size_bytes": 2}}
        self.receipt = {"runtime_id": self.binding["runtime_id"], "desktop": "KDE", "architecture": "x86_64", "distribution": self.binding["distribution"]}
        receipt = self.root / "usr/share/copypaste/compositor-runtime/kwin-6.0-fedora40.receipt.json"
        receipt.parent.mkdir(parents=True)
        receipt.write_text("{}", encoding="utf-8")
        self.binding["runtime_receipt"]["sha256"] = self.module.sha256(receipt)
        runtime = self.root / "usr/lib/copypaste/compositor-runtime/kwin-6.0-fedora40"
        runtime.mkdir(parents=True)
        launcher = self.root / "usr/lib/copypaste/compositor-runtime/bin/copypaste-compositor-session-kwin-6.0-fedora40"
        launcher.parent.mkdir(parents=True)
        launcher.write_text("expected launcher\n", encoding="utf-8")
        launcher.chmod(0o755)
        descriptor = self.root / "usr/share/wayland-sessions/copypaste-kwin-6.0-fedora40.desktop"
        descriptor.parent.mkdir(parents=True)
        descriptor.write_text(self.module.session_descriptor_text(self.receipt), encoding="utf-8")
        self.stage = SimpleNamespace(read_receipt=lambda path: self.receipt, validate_receipt=lambda receipt: None,
                                     validate_payload=lambda runtime, receipt: None,
                                     launcher_text=lambda receipt, prefix: "expected launcher\n")

    def verify(self):
        with mock.patch.object(self.module, "stage_runtime_module", return_value=self.stage):
            self.module.verify_installed(self.binding, self.root)

    def test_rejects_stock_kwin_and_substituted_gnome_launchers(self):
        self.verify()
        launcher = self.root / "usr/lib/copypaste/compositor-runtime/bin/copypaste-compositor-session-kwin-6.0-fedora40"
        launcher.write_text("#!/bin/sh\nexec kwin_wayland --virtual\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "deterministic launcher"):
            self.verify()
        launcher.write_text("expected launcher\n", encoding="utf-8")
        descriptor = self.root / "usr/share/wayland-sessions/copypaste-kwin-6.0-fedora40.desktop"
        descriptor.write_text("[Desktop Entry]\nExec=/usr/bin/gnome-shell\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "deterministic descriptor"):
            self.verify()


class CompositorRuntimeWorkflowTest(unittest.TestCase):
    def test_uses_authenticated_runtime_artifacts_and_generated_launchers(self):
        workflow = (ROOT / ".github/workflows/linux-native-qualification.yml").read_text(encoding="utf-8")
        self.assertIn("compositor_runtime_run_id:", workflow)
        self.assertIn("verify-linux-compositor-runtime.py source", workflow)
        self.assertIn("verify-linux-compositor-runtime.py artifact", workflow)
        self.assertIn("--compositor-runtime-binding compositor-runtime/binding.json", workflow)
        self.assertIn("--compositor-runtime-binding /compositor-runtime/binding.json", workflow)
        for name in ("gnome-46-ubuntu24.04", "kwin-6.0-fedora40"):
            self.assertIn(name, workflow)
        for name in ("run-linux-native-desktop-session.sh", "run-fedora-plasma6-session.sh"):
            script = (ROOT / "scripts/release" / name).read_text(encoding="utf-8")
            self.assertIn("verify-linux-compositor-runtime.py", script)
            self.assertIn(" installed --binding", script)
            self.assertIn("copypaste-compositor-session-$runtime_id", script)
            self.assertNotIn("gnome-shell --headless", script)
            self.assertIn("COPYPASTE_COMPOSITOR_EXECUTION=stock-x11", script)
            self.assertIn("receipt-listed private compositor entrypoint", script)
        generic = (ROOT / "scripts/release/run-linux-native-desktop-session.sh").read_text(encoding="utf-8")
        self.assertIn("KDE qualification must use the Fedora", generic)


if __name__ == "__main__":
    unittest.main()
