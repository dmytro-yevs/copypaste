#!/usr/bin/env python3
"""Self-test for check-docs.py protocol-version guard.

Proves that a stale protocol-version reference is caught — a negative test
that would fail if the guard were removed or weakened.
"""
import importlib.util
import pathlib
import sys
import unittest

SCRIPT = pathlib.Path(__file__).resolve().parent / "check-docs.py"
spec = importlib.util.spec_from_file_location("check_docs", SCRIPT)
check_docs = importlib.util.module_from_spec(spec)
sys.modules["check_docs"] = check_docs
spec.loader.exec_module(check_docs)


class ProtocolVersionGuardTest(unittest.TestCase):
    def setUp(self):
        self.pv = check_docs.ipc_protocol_version()
        self.assertGreaterEqual(self.pv, 1, f"parsed protocol version {self.pv} is nonsensical")

    def test_matching_docs_pass(self):
        inject = {
            "docs/rewrite/target-architecture.md":
                f"- `PROTOCOL_VERSION` is `{self.pv}`, and changing it is a decision",
            "docs/rewrite/port-manifest/06-ui-behaviour.md":
                f"| Protocol version | `CURRENT_PROTOCOL_VERSION = {self.pv}` |",
        }
        errs = check_docs.check_protocol_docs(self.pv, inject=inject)
        self.assertEqual(errs, [], f"expected no errors, got {errs}")

    def test_stale_version_fails(self):
        stale = self.pv - 1 if self.pv > 1 else self.pv + 1
        inject = {
            "docs/rewrite/target-architecture.md":
                f"- `PROTOCOL_VERSION` is `{stale}`, and changing it is a decision",
            "docs/rewrite/port-manifest/06-ui-behaviour.md":
                f"| Protocol version | `CURRENT_PROTOCOL_VERSION = {stale}` |",
        }
        errs = check_docs.check_protocol_docs(self.pv, inject=inject)
        self.assertEqual(len(errs), 2, f"expected 2 errors for stale docs, got {errs}")
        for err in errs:
            self.assertIn("does not match", err, f"unexpected error format: {err}")

    def test_missing_reference_fails(self):
        inject = {
            "docs/rewrite/target-architecture.md": "no version reference here",
            "docs/rewrite/port-manifest/06-ui-behaviour.md": "nothing here either",
        }
        errs = check_docs.check_protocol_docs(self.pv, inject=inject)
        self.assertEqual(len(errs), 2, f"expected 2 errors for missing refs, got {errs}")
        for err in errs:
            self.assertIn("not found", err, f"unexpected error format: {err}")


if __name__ == "__main__":
    unittest.main()
