from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]


class LinuxQualificationBootstrapTest(unittest.TestCase):
    def test_release_bootstrap_calls_reusable_qualification_with_exact_runs(self):
        workflow = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        self.assertIn("linux_qualification_only:", workflow)
        self.assertIn("linux-native-bootstrap:", workflow)
        for name in ("artifact_run_id: ${{ inputs.linux_artifact_run_id }}", "previous_artifact_run_id: ${{ inputs.previous_linux_artifact_run_id }}", "supabase_module_run_id", "semantic_search_module_run_id", "ocr_module_run_id", "compositor_runtime_run_id"):
            self.assertIn(name, workflow)
        self.assertIn("needs.preflight.outputs.linux_qualification_only != 'true'", workflow)

    def test_native_workflow_is_callable_and_downloads_public_runtime_for_final_verification(self):
        workflow = (ROOT / ".github/workflows/linux-native-qualification.yml").read_text(encoding="utf-8")
        self.assertIn("workflow_call:", workflow)
        evidence = workflow.split("  evidence:\n", 1)[1]
        self.assertIn("name: production-compositor-runtime", evidence)
        self.assertIn("path: compositor-runtime", evidence)
        self.assertIn("run-id: ${{ inputs.artifact_run_id }}", evidence)


if __name__ == "__main__":
    unittest.main()
