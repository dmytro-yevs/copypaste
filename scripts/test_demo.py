#!/usr/bin/env python3
"""Structural regressions for the isolated fake-capture demo."""

from pathlib import Path
import unittest


SCRIPT = Path(__file__).with_name("demo.sh").read_text()


class DemoScriptTest(unittest.TestCase):
    def test_capture_proof_precedes_direct_ingest(self) -> None:
        capture, direct = SCRIPT.split('step "Direct ingest: add items (not a capture proof)"', 1)
        self.assertIn('write_fake_text "$CAPTURED_VALUE"', capture)
        self.assertIn("wait_for_capture_event", capture)
        self.assertIn("wait_for_search", capture)
        self.assertNotIn('"$CLI" add ', capture)
        self.assertIn('"$CLI" add ', direct)

    def test_demo_selects_the_fake_and_avoids_network_discovery(self) -> None:
        self.assertIn("copypaste-daemon/dev-fake-clipboard", SCRIPT)
        self.assertIn("status_field clipboard_backend", SCRIPT)
        self.assertIn(' --port 0 &', SCRIPT)
        self.assertIn("-u COPYPASTE_CLOUD_URL -u COPYPASTE_CLOUD_ANON_KEY", SCRIPT)
        self.assertNotIn('"$CLI" discover', SCRIPT)

    def test_private_copy_requires_a_content_free_acknowledgement(self) -> None:
        private = SCRIPT.split('step "Capture: private mode acknowledges without replaying"', 1)[1]
        self.assertIn("set_private_mode true", private)
        self.assertIn("wait_for_ack_after", private)
        self.assertLess(private.index("wait_for_ack_after"), private.index("set_private_mode false"))

    def test_size_probe_sets_the_canonical_minimum_before_the_boundary(self) -> None:
        size_probe = SCRIPT.split('step "Capture: exact limit succeeds', 1)[1]
        self.assertLess(
            size_probe.index("--max-text-size-bytes 65536"),
            size_probe.index("head -c 65536"),
        )


if __name__ == "__main__":
    unittest.main()
