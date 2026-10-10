#!/usr/bin/env python3
import copy
import importlib.util
from pathlib import Path
import unittest


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


if __name__ == "__main__":
    unittest.main()
