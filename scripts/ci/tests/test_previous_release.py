from copy import deepcopy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "release"))
from previous_release import previous, verify_published_receipt


class PreviousReleaseTest(unittest.TestCase):
    def release(self, tag, **extra):
        return {"tag_name": tag, "draft": False, "prerelease": False,
                "assets": [{"name": f"CopyPaste-{tag}-linux-x86_64.deb"}], **extra}

    def test_selects_latest_older_stable_linux_release(self):
        candidates = [self.release("v1.0.9"), self.release("v1.0.10"), self.release("v1.0.11", draft=True),
                      self.release("v1.0.12", prerelease=True), self.release("v1.0.13", assets=[]), self.release("v2.0.0")]
        self.assertEqual(previous(candidates, "1.0.14")["tag_name"], "v1.0.10")
        self.assertIsNone(previous(candidates, "1.0.0"))
        with self.assertRaises(ValueError):
            previous(candidates, "main")

    def test_published_digests_bind_previous_qualification_to_public_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            records, assets = [], []
            for extension in ("deb", "AppImage"):
                name = f"CopyPaste-v1.0.10-linux-x86_64.{extension}"
                content = extension.encode()
                (root / name).write_bytes(content)
                digest = hashlib.sha256(content).hexdigest()
                records.append({"name": name, "bytes": len(content), "sha256": digest})
                assets.append({"name": name, "size": len(content), "digest": "sha256:" + digest})
            release = self.release("v1.0.10", assets=assets)
            receipt = {"platform": "linux", "version": "1.0.10", "commit": "a" * 40, "artifacts": records}
            verify_published_receipt(receipt, release, root, "a" * 40)
            for key, value in [("commit", "b" * 40), ("platform", "windows"), ("version", "1.0.9")]:
                with self.assertRaises(ValueError):
                    verify_published_receipt({**receipt, key: value}, release, root, "a" * 40)
            corrupted = deepcopy(release)
            corrupted["assets"][0]["digest"] = "sha256:" + "0" * 64
            with self.assertRaises(ValueError):
                verify_published_receipt(receipt, corrupted, root, "a" * 40)
            (root / records[0]["name"]).write_bytes(b"different bytes")
            with self.assertRaises(ValueError):
                verify_published_receipt(receipt, release, root, "a" * 40)


if __name__ == "__main__":
    unittest.main()
