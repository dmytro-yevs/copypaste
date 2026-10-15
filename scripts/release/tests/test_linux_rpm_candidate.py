"""Verify x86_64 RPM previews require exact installation evidence for publication."""

from copy import deepcopy
import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
BUILDER = ROOT / "scripts/release/build-linux-packages.sh"
INSTALL = ROOT / "scripts/release/test-linux-rpm.sh"
RELEASE = runpy.run_path(str(ROOT / "scripts/release/linux_release_contract.py"))
VERIFY = runpy.run_path(str(ROOT / "scripts/release/verify-artifact-receipts.py"))["verify_rpm_preview"]


class RpmPreviewTest(unittest.TestCase):
    def test_extra_packaging_options_fail_before_building(self):
        result = subprocess.run(
            ["bash", str(BUILDER), "1.2.3", "x86_64", "--release-scope", "unexpected"],
            capture_output=True, text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("too many packaging arguments", result.stderr)

    def test_install_rejects_wrong_architecture_and_symbolic_links(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            wrong = root / "CopyPaste-v1.2.3-linux-aarch64.rpm"
            wrong.write_bytes(b"package")
            linked = root / "CopyPaste-v1.2.3-linux-x86_64.rpm"
            linked.symlink_to(wrong)
            for artifact in (wrong, linked):
                with self.subTest(artifact=artifact):
                    result = subprocess.run(["bash", str(INSTALL), str(artifact), "1.2.3"], capture_output=True, text=True)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("an exact, nonempty x86_64 RPM package is required", result.stderr)

    def test_installation_evidence_binds_bytes_environment_provenance_and_assertions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = root / "CopyPaste-v1.2.3-linux-x86_64.rpm"
            package.write_bytes(b"tested RPM bytes")
            receipt = root / f"{package.name}.installation.json"
            payload = {
                "schema": 1, "qualification": "package_installation_only", "version": "1.2.3",
                "architecture": "x86_64", "commit": "a" * 40, "run_id": "123",
                "artifact": {"name": package.name, "bytes": package.stat().st_size,
                             "sha256": hashlib.sha256(package.read_bytes()).hexdigest()},
                "environment": RELEASE["CONTRACT"]["rpm_preview_environment"],
                "assertions": {name: True for name in RELEASE["RPM_PREVIEW_ASSERTIONS"]},
            }
            receipt.write_text(json.dumps(payload))
            self.assertEqual(VERIFY(root, "1.2.3", "a" * 40, "123"), receipt.name)
            changes = (
                ("commit", "b" * 40), ("run_id", "124"), ("version", "1.2.4"),
                ("architecture", "aarch64"), ("qualification", "desktop_parity"),
                ("artifact", {**payload["artifact"], "sha256": "f" * 64}),
                ("artifact", {**payload["artifact"], "bytes": 999}),
                ("environment", {**payload["environment"], "version": "40"}),
                ("assertions", {**payload["assertions"], "dnf_install": False}),
                ("assertions", {**payload["assertions"], "dnf_install": 1}),
                ("assertions", {"dnf_install": True}),
            )
            for field, value in changes:
                invalid = deepcopy(payload)
                invalid[field] = value
                receipt.write_text(json.dumps(invalid))
                with self.subTest(field=field, value=value), self.assertRaisesRegex(ValueError, "differs from exact artifact"):
                    VERIFY(root, "1.2.3", "a" * 40, "123")
            receipt.unlink()
            with self.assertRaisesRegex(ValueError, "receipt is missing"):
                VERIFY(root, "1.2.3", "a" * 40, "123")
            target = root / "unbound-receipt.json"
            target.write_text(json.dumps(payload))
            receipt.symlink_to(target)
            with self.assertRaisesRegex(ValueError, "receipt is missing"):
                VERIFY(root, "1.2.3", "a" * 40, "123")


if __name__ == "__main__":
    unittest.main()
