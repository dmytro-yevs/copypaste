from copy import deepcopy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from changes import select, verify_results
from policy import dependencies, validate


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

    def test_rejects_cycles(self):
        with self.assertRaisesRegex(ValueError, "circular"):
            dependencies({"a": {"needs": "b"}, "b": {"needs": "a"}})

    def test_change_selection_fails_closed(self):
        for path in ["Cargo.lock", "crates/core/lib.rs", "apps/copypaste_flutter/lib/generated.dart",
                     "scripts/build.sh", "unregistered-platform/file", "../README.md",
                     "docs/release-qualification.md", "docs/engineering/ci-standards.md"]:
            self.assertTrue(select([path])["native"], path)
        self.assertFalse(select(["README.md", "docs/cloud-privacy.md"])["native"])
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
