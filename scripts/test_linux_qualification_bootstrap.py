from pathlib import Path
import runpy
import unittest


ROOT = Path(__file__).resolve().parents[1]


class LinuxQualificationBootstrapTest(unittest.TestCase):
    def evidence(self, parent):
        prefix = "Run exact staged Linux native qualification / " if parent else ""
        names = [prefix + name for name in runpy.run_path(str(ROOT / "scripts/release/verify-linux-qualification-evidence-source.py"))["BASE"]]
        jobs = [{"name": name, "status": "completed", "conclusion": "success"} for name in names]
        if parent:
            jobs.extend({"name": name, "status": "completed", "conclusion": "skipped"} for name in runpy.run_path(str(ROOT / "scripts/release/verify-linux-qualification-evidence-source.py"))["PARENT_SKIPPED"])
            jobs.append({"name": "Linux build ()", "status": "completed", "conclusion": "skipped"})
        run = {"head_sha": "a" * 40, "head_repository": {"full_name": "owner/repo"}, "status": "completed", "conclusion": "success", "path": ".github/workflows/release.yml" if parent else ".github/workflows/linux-native-qualification.yml", "event": "workflow_dispatch"}
        return run, {"total_count": len(jobs), "jobs": jobs}

    def test_evidence_source_accepts_exact_parent_and_rejects_unprefixed_or_rebuilt_parent(self):
        verify = runpy.run_path(str(ROOT / "scripts/release/verify-linux-qualification-evidence-source.py"))["verify"]
        run, jobs = self.evidence(True)
        verify(run, jobs, "owner/repo", "a" * 40)
        jobs["jobs"][0]["name"] = "Verify exact artifact sources"
        with self.assertRaisesRegex(ValueError, "native qualification"):
            verify(run, jobs, "owner/repo", "a" * 40)
        run, jobs = self.evidence(True)
        next(job for job in jobs["jobs"] if job["name"] == "publish")["conclusion"] = "success"
        with self.assertRaisesRegex(ValueError, "skip rebuild"):
            verify(run, jobs, "owner/repo", "a" * 40)
        run, jobs = self.evidence(True)
        jobs["jobs"].pop()
        jobs["jobs"].extend({"name": name, "status": "completed", "conclusion": "skipped"} for name in ("Linux build (x86_64)", "Linux build (aarch64)"))
        jobs["total_count"] = len(jobs["jobs"])
        verify(run, jobs, "owner/repo", "a" * 40)
        jobs["jobs"][-1]["conclusion"] = "success"
        with self.assertRaisesRegex(ValueError, "every Linux build"):
            verify(run, jobs, "owner/repo", "a" * 40)
        run, jobs = self.evidence(False)
        jobs["jobs"].pop()
        jobs["total_count"] -= 1
        with self.assertRaisesRegex(ValueError, "native qualification"):
            verify(run, jobs, "owner/repo", "a" * 40)
        run, jobs = self.evidence(False)
        jobs["jobs"][0]["conclusion"] = "failure"
        with self.assertRaisesRegex(ValueError, "native qualification"):
            verify(run, jobs, "owner/repo", "a" * 40)
    def test_release_bootstrap_calls_reusable_qualification_with_exact_runs(self):
        workflow = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        self.assertIn("linux_qualification_only:", workflow)
        self.assertIn("linux-native-bootstrap:", workflow)
        for name in ("artifact_run_id: ${{ inputs.linux_artifact_run_id }}", "previous_artifact_run_id: ${{ inputs.previous_linux_artifact_run_id }}", "supabase_module_run_id", "semantic_search_module_run_id", "ocr_module_run_id", "compositor_runtime_run_id"):
            self.assertIn(name, workflow)
        self.assertIn("needs.preflight.outputs.linux_qualification_only != 'true'", workflow)
        self.assertIn("GITHUB_EVENT_NAME\" == workflow_dispatch", workflow)
        self.assertIn("linux_evidence_run_id", workflow)
        helper = (ROOT / "scripts/release/verify-linux-qualification-evidence-source.py").read_text(encoding="utf-8")
        self.assertIn("Run exact staged Linux native qualification /", helper)
        gate = workflow.split("  linux-full-parity-evidence:\n", 1)[1].split("  qualify:\n", 1)[0]
        self.assertLess(gate.index("actions/checkout"), gate.index("verify-linux-qualification-evidence-source.py"))
        self.assertIn("ref: ${{ needs.preflight.outputs.release_commit }}", gate)

    def test_native_workflow_is_callable_and_downloads_public_runtime_for_final_verification(self):
        workflow = (ROOT / ".github/workflows/linux-native-qualification.yml").read_text(encoding="utf-8")
        self.assertIn("workflow_call:", workflow)
        evidence = workflow.split("  evidence:\n", 1)[1]
        self.assertIn("name: production-compositor-runtime", evidence)
        self.assertIn("path: compositor-runtime", evidence)
        self.assertIn("run-id: ${{ inputs.artifact_run_id }}", evidence)
        self.assertEqual(evidence.count("path: previous-artifacts/x86_64"), 1)
        self.assertEqual(evidence.count("path: previous-artifacts/aarch64"), 1)
        self.assertIn("run-id: ${{ inputs.previous_artifact_run_id }}", evidence)


if __name__ == "__main__":
    unittest.main()
