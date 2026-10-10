import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).parents[1]))
SPEC = importlib.util.spec_from_file_location("release_receipts", Path(__file__).parents[1] / "verify-release-packages.py")
receipts = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(receipts)


class ReleaseReceiptsTest(unittest.TestCase):
    def fixtures(self, root, module_id, supported_platforms=None):
        module = {"id": module_id, "version": "0.1.0"}
        if supported_platforms is not None:
            module["supported_platforms"] = supported_platforms
        paths = []
        for platform, architecture in receipts.receipt_targets(module):
            package = root / f"CopyPasteModule-{module_id}-v0.1.0-{platform}-{architecture}.cpmodule"
            package.write_bytes(b"exact-production-package")
            receipt = package.with_name(package.name + ".receipt.json")
            receipt.write_text(json.dumps({
                "commit": "a" * 40, "run_id": "123", "target": {"platform": platform, "architecture": architecture},
                "module_id": module_id, "module_version": "0.1.0",
                "package_sha256": hashlib.sha256(package.read_bytes()).hexdigest(), "package_size_bytes": package.stat().st_size,
                "cases_passed": 3, "signature_verified": True, "removal_completed_after_restart": True,
                "restart_required": module_id == "copypaste.ocr",
                **({
                    "glibc_floor": "2.39.0",
                    "effective_uid": 1001,
                    "network_namespace_isolated": True,
                } if platform == "linux" else {}),
            }))
            paths.append(receipt)
        return module, paths

    def test_legacy_receipts_remain_valid_for_each_provider(self):
        for module_id in ["copypaste.supabase", "copypaste.semantic-search", "copypaste.ocr"]:
            with self.subTest(module=module_id), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                module, _ = self.fixtures(root, module_id)
                receipts.verify_receipts(root, module, "a" * 40, "123")

    def test_linux_receipts_require_both_native_architectures(self):
        platforms = ["macos", "windows", "linux", "android"]
        for architecture in ["x86_64", "aarch64"]:
            with self.subTest(architecture=architecture), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                module, paths = self.fixtures(root, "copypaste.ocr", platforms)
                receipts.verify_receipts(root, module, "a" * 40, "123")
                next(path for path in paths if f"linux-{architecture}.cpmodule.receipt.json" in path.name).unlink()
                with self.assertRaises(FileNotFoundError):
                    receipts.verify_receipts(root, module, "a" * 40, "123")

    def test_linux_receipts_require_the_derived_glibc_floor(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            module, paths = self.fixtures(root, "copypaste.semantic-search", ["linux"])
            linux = next(path for path in paths if "linux-x86_64.cpmodule.receipt.json" in path.name)
            receipt = json.loads(linux.read_text())
            del receipt["glibc_floor"]
            linux.write_text(json.dumps(receipt))
            with self.assertRaises(ValueError):
                receipts.verify_receipts(root, module, "a" * 40, "123")

    def test_linux_receipts_reject_root_or_unisolated_qualification(self):
        for field, value in [("effective_uid", 0), ("network_namespace_isolated", False)]:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                module, paths = self.fixtures(root, "copypaste.ocr", ["linux"])
                receipt_path = next(path for path in paths if "linux-x86_64.cpmodule.receipt.json" in path.name)
                receipt = json.loads(receipt_path.read_text())
                receipt[field] = value
                receipt_path.write_text(json.dumps(receipt))
                with self.assertRaises(ValueError):
                    receipts.verify_receipts(root, module, "a" * 40, "123")

    def test_rejects_wrong_provenance_incomplete_execution_and_missing_platform(self):
        mutations = {"commit": "b" * 40, "run_id": "124", "module_id": "copypaste.ocr", "module_version": "0.2.0",
                     "package_sha256": "0" * 64, "package_size_bytes": 1, "cases_passed": 2,
                     "signature_verified": False, "removal_completed_after_restart": False,
                     "restart_required": True,
                     "target": {"platform": "windows", "architecture": "aarch64"}}
        for field, value in mutations.items():
            with self.subTest(field=field), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                module, paths = self.fixtures(root, "copypaste.semantic-search")
                data = json.loads(paths[-1].read_text())
                data[field] = value
                paths[-1].write_text(json.dumps(data))
                with self.assertRaises(ValueError):
                    receipts.verify_receipts(root, module, "a" * 40, "123")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            module, paths = self.fixtures(root, "copypaste.supabase")
            paths[-1].unlink()
            with self.assertRaises(FileNotFoundError):
                receipts.verify_receipts(root, module, "a" * 40, "123")
