from copy import deepcopy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "release"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from previous_release import previous, verify_published_receipt
from workflow_yaml import read_workflow


class PreviousReleaseTest(unittest.TestCase):
    def test_upgrade_metadata_recovers_the_authenticated_source_after_refs_change(self):
        root = Path(__file__).resolve().parents[3]
        workflow = read_workflow(root / ".github/workflows/linux-native-qualification.yml")
        body = next(step["run"] for step in workflow["jobs"]["source"]["steps"] if step.get("id") == "source")
        metadata = body.split('--repository "$GITHUB_REPOSITORY" --commit "$previous_commit"\n', 1)[1].split('upgrade_mode=prior_release', 1)[0]
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            publisher, origin, consumer = (directory / name for name in ("publisher", "origin.git", "consumer"))

            def git(path, *arguments):
                return subprocess.check_output(["git", "-C", str(path), *arguments], text=True, stderr=subprocess.PIPE).strip()

            publisher.mkdir()
            git(publisher, "init", "-q", "-b", "current")
            git(publisher, "config", "user.name", "CI Test")
            git(publisher, "config", "user.email", "ci@example.com")
            pubspec = publisher / "apps/copypaste_flutter/pubspec.yaml"
            pubspec.parent.mkdir(parents=True)
            pubspec.write_text("version: 1.0.30+31\n")
            git(publisher, "add", ".")
            git(publisher, "commit", "-qm", "Previous release")
            previous_commit = git(publisher, "rev-parse", "HEAD")
            git(directory, "clone", "--bare", "--no-local", str(publisher), str(origin))
            git(publisher, "checkout", "--orphan", "rewritten")
            pubspec.write_text("version: 1.0.31+32\n")
            git(publisher, "add", ".")
            git(publisher, "commit", "-qm", "Current source")
            current_commit = git(publisher, "rev-parse", "HEAD")
            git(origin, "fetch", "--no-tags", str(publisher), current_commit)
            git(origin, "update-ref", "refs/heads/current", current_commit, previous_commit)
            git(origin, "config", "uploadpack.allowAnySHA1InWant", "true")
            git(directory, "clone", "--no-local", "--single-branch", str(origin), str(consumer))
            self.assertNotEqual(subprocess.run(["git", "-C", str(consumer), "cat-file", "-e", previous_commit], capture_output=True).returncode, 0)
            result = subprocess.run(
                ["bash", "-c", 'set -euo pipefail\n' + metadata + '\nprintf "%s" "$previous_version"'],
                cwd=consumer, env={**os.environ, "previous_commit": previous_commit},
                text=True, capture_output=True, check=True,
            )
            self.assertEqual(result.stdout, "1.0.30")
            self.assertEqual(git(consumer, "rev-parse", "HEAD"), current_commit)
            self.assertEqual(git(consumer, "tag", "--list"), "")

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
