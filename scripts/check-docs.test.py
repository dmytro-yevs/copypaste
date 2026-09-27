#!/usr/bin/env python3
"""Self-tests for the check-docs.py doc-drift guards.

Each case proves a stale or overclaiming statement is caught — negative tests
that would fail if a guard were removed or weakened.
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


HONEST_LIMITS = """### Product limits

Native clipboard capture takes one representation, plain text: a change that
offers only an image, a file reference or rich text is acknowledged and skipped.
Non-text content is still first class everywhere else.

## Build and run
"""

STALE_LIMITS = """### Product limits

CopyPaste is text-only: it does not capture images, files, or rich text.

## Build and run
"""

WORKS_CLOUD_ROW = (
    "| Cloud sync | Supabase auth, PostgREST, Realtime; rows sealed client-side. "
    "Wired to the daemon and the CLI — but see below |\n"
)
GATE_SENTENCE = (
    "`release.yml`'s `supabase-gate` job runs `supabase/tests/real-supabase.sh`, which "
    "brings up a **disposable local Supabase stack**, applies `supabase/`'s migrations "
    "and asserts schema and RLS behaviour. "
)
DEMO_SENTENCE = (
    "`scripts/demo-cloud.sh` drives two daemons against a **local stub** "
    "(`scripts/cloud-stub.py`), and no workflow runs it. "
)
UNVERIFIED_CLOUD_ROW = (
    "| Cloud sync against Supabase | "
    + GATE_SENTENCE
    + DEMO_SENTENCE
    + "Neither layer leaves the runner: no hosted or production Supabase project is part "
    "of any check. |\n"
)
HONEST_CLOUD = WORKS_CLOUD_ROW + UNVERIFIED_CLOUD_ROW


class ProductLimitsGuardTest(unittest.TestCase):
    def setUp(self):
        self.kinds = check_docs.non_plain_text_types()
        self.assertIn("image/png", self.kinds, f"parsed KNOWN looks wrong: {self.kinds}")
        self.assertIn("file", self.kinds, f"parsed KNOWN looks wrong: {self.kinds}")
        self.assertNotIn("text", self.kinds, "plain text is not a non-text kind")

    def test_scoped_capture_wording_passes(self):
        errs = check_docs.check_product_limits(self.kinds, inject={"README.md": HONEST_LIMITS})
        self.assertEqual(errs, [], f"expected no errors, got {errs}")

    def test_stale_text_only_claim_fails(self):
        errs = check_docs.check_product_limits(self.kinds, inject={"README.md": STALE_LIMITS})
        self.assertTrue(errs, "a text-only product claim must fail while KNOWN has non-text kinds")
        self.assertTrue(
            all("image/png" in err for err in errs),
            f"the error must name the shipped non-text kinds: {errs}",
        )

    def test_missing_section_fails(self):
        errs = check_docs.check_product_limits(self.kinds, inject={"README.md": "no limits here"})
        self.assertEqual(len(errs), 1, f"expected one error, got {errs}")
        self.assertIn("not found", errs[0])


class CloudClaimGuardTest(unittest.TestCase):
    def test_both_layers_qualified_copy_passes(self):
        errs = check_docs.check_cloud_claims(inject={"README.md": HONEST_CLOUD})
        self.assertEqual(errs, [], f"expected no errors, got {errs}")

    def test_dropped_stub_qualification_fails(self):
        weakened = HONEST_CLOUD.replace(DEMO_SENTENCE, "`scripts/demo-cloud.sh` drives it. ")
        errs = check_docs.check_cloud_claims(inject={"README.md": weakened})
        self.assertTrue(errs, "dropping the demo's stub qualification must fail")
        self.assertTrue(
            any("local stub" in err for err in errs),
            f"the error must name the missing qualification: {errs}",
        )

    def test_dropped_real_stack_gate_fails(self):
        weakened = HONEST_CLOUD.replace(GATE_SENTENCE, "A release gate covers the schema. ")
        errs = check_docs.check_cloud_claims(inject={"README.md": weakened})
        self.assertTrue(errs, "dropping the real-stack gate must fail")
        self.assertTrue(
            any("real-Supabase gate script" in err for err in errs),
            f"the error must name the missing gate script: {errs}",
        )

    def test_hosted_project_claim_fails(self):
        overclaim = HONEST_CLOUD + (
            "Cloud sync is exercised against a hosted Supabase project on every release.\n"
        )
        errs = check_docs.check_cloud_claims(inject={"README.md": overclaim})
        self.assertTrue(errs, "claiming a hosted Supabase project must fail")
        self.assertTrue(
            any("hosted Supabase project" in err for err in errs),
            f"expected the overclaim error, got {errs}",
        )

    def test_production_deployment_claim_fails(self):
        overclaim = HONEST_CLOUD + (
            "The gate applies the schema to our production Supabase deployment.\n"
        )
        errs = check_docs.check_cloud_claims(inject={"README.md": overclaim})
        self.assertTrue(
            any("hosted Supabase project" in err for err in errs),
            f"expected the overclaim error, got {errs}",
        )

    def test_works_row_must_keep_its_caveat_pointer(self):
        stripped = HONEST_CLOUD.replace(" — but see below", "")
        errs = check_docs.check_cloud_claims(inject={"README.md": stripped})
        self.assertTrue(
            any("Works row" in err for err in errs),
            f"expected the Works-row pointer error, got {errs}",
        )


if __name__ == "__main__":
    unittest.main()
