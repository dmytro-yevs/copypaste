"""Verify receipt merging follows the released architecture contract."""

import copy
import json
from pathlib import Path
import runpy
import tempfile
import unittest


MERGE = runpy.run_path(str(Path(__file__).resolve().parents[1] / "merge-linux-artifact-receipts.py"))


class MergeLinuxReceiptsTest(unittest.TestCase):
    def receipt(self):
        return {"schema": 2, "platform": "linux", "version": "1.0.24", "commit": "a" * 40,
                "run_id": "7", "artifacts": [{"name": "CopyPaste-v1.0.24-linux-x86_64.deb", "bytes": 1, "sha256": "b" * 64}]}

    def merge(self, payloads):
        with tempfile.TemporaryDirectory() as directory:
            paths = []
            for index, payload in enumerate(payloads):
                path = Path(directory) / f"{index}.json"
                path.write_text(json.dumps(payload))
                paths.append(path)
            return MERGE["merge"](paths)

    def test_single_released_architecture_preserves_exact_receipt_identity_and_bytes(self):
        receipt = self.receipt()
        self.assertEqual(self.merge([receipt]), receipt)

    def test_missing_and_extra_architecture_receipts_are_rejected(self):
        for payloads in ([], [self.receipt(), self.receipt()]):
            with self.subTest(count=len(payloads)), self.assertRaisesRegex(ValueError, "released architecture"):
                self.merge(payloads)

    def test_wrong_platform_or_schema_is_rejected(self):
        for override in ({"platform": "macos"}, {"schema": 1}):
            with self.subTest(override=override), self.assertRaisesRegex(ValueError, "identity"):
                self.merge([{**self.receipt(), **override}])

    def test_duplicate_or_nonobject_artifacts_cannot_be_hidden_by_merge(self):
        for invalid in ([self.receipt()["artifacts"][0]] * 2, ["unverified artifact"]):
            receipt = copy.deepcopy(self.receipt())
            receipt["artifacts"] = invalid
            with self.subTest(invalid=invalid), self.assertRaisesRegex(ValueError, "invalid or ambiguous"):
                self.merge([receipt])


if __name__ == "__main__":
    unittest.main()
