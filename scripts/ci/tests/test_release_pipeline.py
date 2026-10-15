from copy import deepcopy
from pathlib import Path
import runpy
import sys
import hashlib
import json
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts/release"))


def load(name):
    return runpy.run_path(str(ROOT / "scripts/release" / name))


class ReleasePipelineTest(unittest.TestCase):
    def setUp(self):
        self.run = {"id": 123, "path": ".github/workflows/release.yml", "event": "workflow_dispatch",
                    "status": "in_progress", "conclusion": None, "head_sha": "a" * 40,
                    "head_repository": {"full_name": "owner/repo"}}

    def jobs(self, names):
        return {"total_count": len(names), "jobs": [{"name": name, "status": "completed", "conclusion": "success"} for name in names]}

    def artifact(self, name):
        return {"total_count": 1, "artifacts": [{"id": 1, "name": name, "expired": False,
                "workflow_run": {"id": 123, "head_sha": "a" * 40}}]}

    def test_current_artifact_requires_successful_builds_and_same_run(self):
        verify = load("verify-linux-artifact-source-run.py")["verify"]
        jobs = self.jobs(["preflight", "Linux build (x86_64)"])
        artifacts = self.artifact("production-linux-x86_64")
        verify(self.run, jobs, artifacts, "owner/repo", "a" * 40, current_run_id="123")
        for current in (None, "124"):
            with self.assertRaises(ValueError):
                verify(self.run, jobs, artifacts, "owner/repo", "a" * 40, current_run_id=current)
        for status in ("failure", "skipped", "cancelled"):
            bad = deepcopy(jobs)
            bad["jobs"][-1]["conclusion"] = status
            with self.assertRaises(ValueError):
                verify(self.run, bad, artifacts, "owner/repo", "a" * 40, current_run_id="123")

    def test_current_module_requires_its_own_producer_and_exact_provenance(self):
        verify = load("verify-linux-module-source-run.py")["verify"]
        jobs = self.jobs(["Build Linux supabase module / preflight", "Build Linux supabase module / desktop (linux, x86_64, ubuntu-24.04)"])
        artifacts = self.artifact("provider-supabase-linux-x86_64")
        args = [self.run, jobs, artifacts, "owner/repo", "a" * 40, ".github/workflows/provider-module.yml", "supabase"]
        verify(*args, current_run_id="123")
        for index, mutation in [(0, {**self.run, "head_sha": "b" * 40}), (1, self.jobs(["unrelated linux x86_64"])),
                                (2, self.artifact("provider-supabase-linux-aarch64"))]:
            bad = deepcopy(args)
            bad[index] = mutation
            with self.assertRaises(ValueError):
                verify(*bad, current_run_id="123")
        artifacts["artifacts"][0]["expired"] = True
        with self.assertRaises(ValueError):
            verify(*args, current_run_id="123")

    def test_current_evidence_requires_complete_native_matrix(self):
        module = load("verify-linux-qualification-evidence-source.py")
        jobs = self.jobs(["Qualify current Linux artifacts / " + name for name in module["BASE"]])
        module["verify"](self.run, jobs, "owner/repo", "a" * 40, current_run_id="123")
        jobs["jobs"][-1]["conclusion"] = "skipped"
        with self.assertRaises(ValueError):
            module["verify"](self.run, jobs, "owner/repo", "a" * 40, current_run_id="123")

    def test_current_compositor_requires_its_finished_producer(self):
        module = load("verify-linux-compositor-runtime.py")
        artifacts = self.artifact("copypaste-compositor-runtime-gnome-46-ubuntu24.04-x86_64")
        jobs = self.jobs(["Build opt-in compositor runtime companions / GNOME 46 Ubuntu 24.04 (x86_64)"])
        module["verify_source"](self.run, artifacts, "owner/repo", "a" * 40, current_run_id="123", jobs=jobs)
        with self.assertRaises(ValueError):
            module["verify_source"](self.run, artifacts, "owner/repo", "a" * 40, current_run_id="123", jobs=self.jobs([]))

    def test_preflight_missing_secrets_fails_without_exposing_values(self):
        require = load("preflight-sources.py")["require_secrets"]
        require({}, build=False, publish=False)
        for build, publish in [(True, False), (False, True)]:
            with self.assertRaisesRegex(ValueError, "Missing release"):
                require({}, build=build, publish=publish)

    def test_partial_module_matrix_can_never_publish(self):
        matrix = runpy.run_path(str(ROOT / "scripts/ci/module-matrix.py"))["matrix"]
        contract = {"schema": 1, "desktop": {"provider": [{"platform": "linux", "architecture": "x86_64"}, {"platform": "macos", "architecture": "aarch64"}]}}
        self.assertEqual(len(matrix(contract, "provider", True, False)), 1)
        self.assertEqual(len(matrix(contract, "provider", False, True)), 2)
        with self.assertRaises(ValueError):
            matrix(contract, "provider", True, True)

    def test_publication_stays_draft_until_every_asset_matches(self):
        publish = load("publish-release.py")["publish"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "package.deb").write_bytes(b"qualified bytes")
            assets = [{"name": "package.deb", "size": 15,
                       "digest": "sha256:" + hashlib.sha256(b"qualified bytes").hexdigest()}]
            draft = {"tag_name": "v1.2.3", "draft": True, "prerelease": False, "assets": assets}
            stable = {**draft, "draft": False}
            args = SimpleNamespace(repository="owner/repo", version="1.2.3", artifacts=root, notes=Path("notes.md"))
            with patch("subprocess.check_output", side_effect=["[[]]", json.dumps(draft), json.dumps(stable)]), patch("subprocess.run") as execute:
                publish(args)
                self.assertIn("--draft", execute.call_args_list[0].args[0])
                self.assertIn("--draft=false", execute.call_args_list[1].args[0])
            incomplete = {**draft, "assets": []}
            with patch("subprocess.check_output", side_effect=["[[]]", json.dumps(incomplete)]), patch("subprocess.run") as execute:
                with self.assertRaisesRegex(ValueError, "incomplete"):
                    publish(args)
                self.assertEqual(execute.call_count, 1)
            # A crashed upload resumes the draft; a repeated completed publication is a no-op.
            with patch("subprocess.check_output", side_effect=[json.dumps([[incomplete]]), json.dumps(draft), json.dumps(stable)]), patch("subprocess.run") as execute:
                publish(args)
                self.assertEqual(execute.call_args_list[0].args[0][1:3], ["release", "upload"])
            with patch("subprocess.check_output", side_effect=[json.dumps([[stable]]), json.dumps(stable)]), patch("subprocess.run") as execute:
                publish(args)
                execute.assert_not_called()

    def test_recovery_accepts_only_isolated_publisher_failure(self):
        verify = load("verify-qualified-run.py")["verify"]
        run = {**self.run, "status": "completed", "conclusion": "failure"}
        jobs = self.jobs(["preflight", "macos", "macos-regression", "android", "windows", "qualify", "publish"])
        jobs["jobs"][-1]["conclusion"] = "failure"
        artifacts = self.artifact("production-qualified")
        verify(run, jobs, artifacts, "owner/repo", "a" * 40)
        jobs["jobs"][2]["conclusion"] = "skipped"
        with self.assertRaises(ValueError):
            verify(run, jobs, artifacts, "owner/repo", "a" * 40)


if __name__ == "__main__":
    unittest.main()
