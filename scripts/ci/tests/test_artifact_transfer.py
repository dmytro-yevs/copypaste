from copy import deepcopy
import hashlib
import io
from pathlib import Path
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "release"))
from github_inventory import latest_artifacts
import download_artifact as transfer


class ArtifactTransferTest(unittest.TestCase):
    def record(self, identity, created, payload):
        return {"id": identity, "name": "production-test", "created_at": created, "expired": False,
                "digest": "sha256:" + hashlib.sha256(payload).hexdigest(),
                "workflow_run": {"id": 123, "head_sha": "a" * 40}}

    def archive(self, name="payload.txt", payload=b"qualified"):
        output = io.BytesIO()
        with zipfile.ZipFile(output, "w") as archive:
            archive.writestr(name, payload)
        return output.getvalue()

    def test_retry_selection_uses_creation_time_not_numeric_id(self):
        data = {"total_count": 2, "artifacts": [self.record(20, "2026-01-01T00:00:00Z", b"old"),
                                               self.record(10, "2026-01-01T00:01:00Z", b"new")]}
        self.assertEqual(latest_artifacts(data, 2)["artifacts"][0]["id"], 10)
        with self.assertRaisesRegex(ValueError, "outside a workflow rerun"):
            latest_artifacts(data, 1)
        data["artifacts"][1]["created_at"] = data["artifacts"][0]["created_at"]
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            latest_artifacts(data, 2)
        with self.assertRaisesRegex(ValueError, "incomplete"):
            latest_artifacts({**data, "total_count": 3}, 2)

    def test_transfer_binds_latest_immutable_id_and_checksum(self):
        old, current = self.archive(payload=b"old"), self.archive(payload=b"current")
        records = [self.record(20, "2026-01-01T00:00:00Z", old), self.record(10, "2026-01-01T00:01:00Z", current)]
        run = {"id": 123, "status": "completed", "head_sha": "a" * 40, "run_attempt": 2,
               "head_repository": {"full_name": "owner/repo"}}
        def inventory(repository, run_id, kind, **kwargs):
            return run if kind == "run" else {"total_count": 2, "artifacts": records}
        def fetch(command, stdout, **kwargs):
            self.assertTrue(command[-1].endswith("/artifacts/10/zip"))
            stdout.write(current)
        with tempfile.TemporaryDirectory() as directory, patch.object(transfer, "inventory", side_effect=inventory), patch.object(transfer.subprocess, "run", side_effect=fetch) as request:
            root = Path(directory)
            references = transfer.download("owner/repo", 123, root, name="production-test", commit="a" * 40)
            self.assertEqual(references[0]["id"], 10)
            self.assertEqual((root / "payload.txt").read_bytes(), b"current")
            records[1]["expired"] = True
            request.reset_mock()
            with self.assertRaisesRegex(ValueError, "expiry"):
                transfer.download("owner/repo", 123, root, name="production-test")
            request.assert_not_called()
            records[1]["expired"] = False
            records[1]["digest"] = "sha256:" + "0" * 64
            with self.assertRaisesRegex(ValueError, "ZIP SHA-256"):
                transfer.download("owner/repo", 123, root, name="production-test")
            with self.assertRaisesRegex(ValueError, "source commit"):
                transfer.download("owner/repo", 123, root, name="production-test", commit="b" * 40)

    def test_archive_paths_and_conflicting_merges_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / "source.zip"
            for name in ("../outside", "/absolute", "C:/absolute", "..\\outside", "line\ncommand"):
                archive.write_bytes(self.archive(name=name))
                with self.assertRaisesRegex(ValueError, "unsafe"):
                    transfer.extract(archive, root / "output")
            with zipfile.ZipFile(archive, "w") as output:
                info = zipfile.ZipInfo("symlink")
                info.create_system = 3
                info.external_attr = (stat.S_IFLNK | 0o777) << 16
                output.writestr(info, "../outside")
            with self.assertRaisesRegex(ValueError, "symlink"):
                transfer.extract(archive, root / "output")
            source, destination = root / "source", root / "destination"
            source.mkdir(); destination.mkdir()
            (source / "same").write_bytes(b"same")
            (destination / "same").write_bytes(b"same")
            transfer.merge(source, destination)
            (source / "same").write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "conflicting"):
                transfer.merge(source, destination)


if __name__ == "__main__":
    unittest.main()
