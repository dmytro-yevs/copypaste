#!/usr/bin/env python3
import datetime
import pathlib
import tempfile
import unittest

import check_rustsec_policy as policy_check


def finding(advisory="RUSTSEC-2024-0429", package="glib", version="0.18.5"):
    return {
        "advisory": {
            "id": advisory,
            "aliases": ["GHSA-wrw7-89jp-8q8g"],
        },
        "package": {"name": package, "version": version},
    }


class RustsecPolicyTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temporary.name)
        decision = self.root / "docs/adr/decision.md"
        decision.parent.mkdir(parents=True)
        decision.write_text("accepted\n", encoding="utf-8")
        variant = self.root / "vendor/glib/src/variant_iter.rs"
        variant.parent.mkdir(parents=True)
        variant.write_text("let mut p:\n&mut p\n", encoding="utf-8")
        self.exception = {
            "advisory": "RUSTSEC-2024-0429",
            "aliases": ["GHSA-wrw7-89jp-8q8g"],
            "package": "glib",
            "version": "0.18.5",
            "kind": "unsound",
            "owner": "@owner",
            "expires": "2026-11-10",
            "affected_targets": ["x86_64-unknown-linux-gnu"],
            "unaffected_targets": ["x86_64-pc-windows-msvc"],
            "decision": "docs/adr/decision.md",
        }
        self.today = datetime.date(2026, 8, 10)

    def tearDown(self):
        self.temporary.cleanup()

    def evaluate(self, report, exception=None, today=None):
        return policy_check.evaluate(
            report,
            {"exceptions": [exception or self.exception]},
            today or self.today,
            self.root,
        )

    def test_accepts_exact_reviewed_advisory(self):
        report = {"warnings": {"unsound": [finding()]}}
        errors, accepted = self.evaluate(report)
        self.assertEqual(errors, [])
        self.assertEqual([item["advisory"] for item in accepted], ["RUSTSEC-2024-0429"])

    def test_rejects_seeded_disallowed_advisory(self):
        report = {
            "vulnerabilities": {
                "list": [finding("RUSTSEC-2099-0001", "seeded-advisory", "1.0.0")]
            },
            "warnings": {"unsound": [finding()]},
        }
        errors, _ = self.evaluate(report)
        self.assertTrue(any("RUSTSEC-2099-0001" in error for error in errors))

    def test_rejects_expired_exception(self):
        report = {"warnings": {"unsound": [finding()]}}
        errors, _ = self.evaluate(report, today=datetime.date(2026, 11, 11))
        self.assertTrue(any("expired" in error for error in errors))

    def test_rejects_stale_exception_after_remediation(self):
        errors, _ = self.evaluate({"warnings": {}})
        self.assertTrue(any("stale" in error for error in errors))

    def test_rejects_glib_exception_when_vendor_patch_is_missing(self):
        (self.root / "vendor/glib/src/variant_iter.rs").write_text("&p\n", encoding="utf-8")
        report = {"warnings": {"unsound": [finding()]}}
        errors, _ = self.evaluate(report)
        self.assertTrue(any("VariantStrIter" in error for error in errors))

    def test_glib_vendor_gate_requires_the_mutability_fix(self):
        self.assertEqual(policy_check.glib_vendor_errors(self.root), [])
        (self.root / "vendor/glib/src/variant_iter.rs").write_text("&p\n", encoding="utf-8")
        errors = policy_check.glib_vendor_errors(self.root)
        self.assertTrue(any("lost" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
