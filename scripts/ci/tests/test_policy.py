from copy import deepcopy
from pathlib import Path
import sys
import json
import tempfile
import unittest
import yaml

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from changes import select, verify_results
from policy import check, dependencies, queue_errors, validate


class PolicyTest(unittest.TestCase):
    def setUp(self):
        self.document = {"on": {"pull_request": None}, "permissions": {"contents": "read"},
                         "jobs": {"test": {"runs-on": "ubuntu-24.04", "timeout-minutes": 20,
                                           "steps": [{"uses": "actions/checkout@" + "a" * 40}]}}}

    def check(self, document):
        return validate(document, Path("ci.yml"), Path.cwd())

    def test_accepts_pinned_minimal_workflow(self):
        self.assertEqual(self.check(self.document), [])

    def test_rejects_mutations(self):
        variants = []
        for field, value in [("timeout-minutes", None), ("continue-on-error", "true"), ("needs", "missing")]:
            data = deepcopy(self.document)
            data["jobs"]["test"][field] = value
            variants.append(data)
        data = deepcopy(self.document)
        data["jobs"]["test"]["steps"][0]["uses"] = "actions/checkout@main"
        variants.append(data)
        data = deepcopy(self.document)
        data["permissions"]["contents"] = "write"
        variants.append(data)
        data = deepcopy(self.document)
        data["on"] = {"pull_request_target": None}
        variants.append(data)
        for data in variants:
            with self.subTest(data=data):
                self.assertTrue(self.check(data))

    def test_artifact_upload_requires_bounded_literal_retention(self):
        for value in (None, 0, 91, True, "3", "${{ inputs.days }}"):
            document = deepcopy(self.document)
            document["jobs"]["test"]["steps"] = [{
                "uses": "actions/upload-artifact@" + "a" * 40,
                "with": {"retention-days": value},
            }]
            with self.subTest(value=value):
                self.assertTrue(any("artifact retention" in error for error in self.check(document)))
        document["jobs"]["test"]["steps"][0]["with"]["retention-days"] = 3
        self.assertEqual(self.check(document), [])

    def test_artifact_retention_contract_rejects_longer_diagnostics_and_shorter_baselines(self):
        from unittest.mock import patch
        import policy
        root = Path(__file__).resolve().parents[3]
        read = policy.read_workflow
        for workflow, job, days in (("ci-metrics.yml", "report", 90), ("release.yml", "linux", 3)):
            def modified(path):
                document = read(path)
                if path.name == workflow:
                    for step in document["jobs"][job]["steps"]:
                        if step.get("uses", "").startswith("actions/upload-artifact@"):
                            step["with"]["retention-days"] = days
                return document
            with self.subTest(workflow=workflow, job=job), patch.object(policy, "read_workflow", modified):
                self.assertTrue(any("artifact retention differs" in error for error in check(root)))

    def test_rejects_cycles(self):
        with self.assertRaisesRegex(ValueError, "circular"):
            dependencies({"a": {"needs": "b"}, "b": {"needs": "a"}})

    def test_active_release_producers_cannot_be_cancelled_by_a_new_run(self):
        document = deepcopy(self.document)
        document["concurrency"] = {"group": "production-release-${{ github.sha }}", "cancel-in-progress": False}
        self.assertEqual(validate(document, Path("release.yml"), Path.cwd()), [])
        document["concurrency"]["cancel-in-progress"] = True
        self.assertTrue(any("must not cancel" in error for error in validate(document, Path("release.yml"), Path.cwd())))

    def test_queue_mode_has_full_structural_validation(self):
        for configuration in ({"queue": "max"}, {"queue": "max", "cancel-in-progress": False},
                              {"queue": "single", "cancel-in-progress": True}):
            self.assertEqual(queue_errors(configuration), [])
        for value in ("invalid", 100, True, [], {}, "${{ inputs.queue }}"):
            self.assertTrue(queue_errors({"queue": value}))
        for value in (True, "true", "${{ inputs.cancel }}"):
            self.assertTrue(queue_errors({"queue": "max", "cancel-in-progress": value}))

    def test_reusable_permission_ceiling_is_checked_before_github_startup(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workflows = root / ".github/workflows"
            workflows.mkdir(parents=True)
            (root / "config").mkdir()
            (root / "config/ci-policy.json").write_text(json.dumps({"schema": 1, "required_jobs": {}}))
            caller = {"name": "CI", "on": {"pull_request": None, "merge_group": None}, "permissions": {"contents": "read"},
                      "jobs": {"call": {"uses": "./.github/workflows/reusable.yaml", "permissions": {"contents": "write", "actions": "write"}}}}
            child = {"name": "Reusable", "on": {"workflow_call": None}, "permissions": {"contents": "read"},
                     "jobs": {"publish": {"runs-on": "ubuntu-24.04", "timeout-minutes": 5,
                                          "permissions": {"contents": "write", "actions": "write"}, "steps": [{"run": "echo fixture"}]}}}
            static = deepcopy(self.document)
            static["name"] = "Policy"
            static["on"]["merge_group"] = None
            for name, data in [("ci.yml", caller), ("ci-configuration.yml", static), ("reusable.yaml", child)]:
                (workflows / name).write_text(yaml.safe_dump(data))
            self.assertEqual(check(root), [])
            del caller["jobs"]["call"]["permissions"]["actions"]
            (workflows / "ci.yml").write_text(yaml.safe_dump(caller))
            self.assertTrue(any("actions: write" in error for error in check(root)))

    def test_change_selection_fails_closed(self):
        for path in ["Cargo.lock", "crates/core/lib.rs", "apps/copypaste_flutter/lib/generated.dart",
                     "scripts/build.sh", "unregistered-platform/file", "../README.md",
                     "config/runtime-resource-budgets.json", "packaging/linux/release-contract.json"]:
            self.assertTrue(select([path])["native"], path)
        self.assertFalse(select(["README.md", "LICENSE-MIT"])["native"])
        self.assertTrue(select(["README.md", "new-file"])["native"])

    def test_aggregate_rejects_failed_or_skipped_required_jobs(self):
        needs = {"changes": {"result": "success"}, "rust": {"result": "success"}}
        verify_results(needs, True, False)
        for status in ("skipped", "failure", "cancelled"):
            needs["rust"]["result"] = status
            with self.assertRaises(ValueError):
                verify_results(needs, True, False)
        needs["rust"]["result"] = "skipped"
        verify_results(needs, False, False)


if __name__ == "__main__":
    unittest.main()
