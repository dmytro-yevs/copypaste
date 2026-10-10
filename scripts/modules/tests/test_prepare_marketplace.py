import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

ROOT = Path(__file__).parents[1]
CATALOG_SPEC = importlib.util.spec_from_file_location("catalog", ROOT / "catalog.py")
catalog = importlib.util.module_from_spec(CATALOG_SPEC)
CATALOG_SPEC.loader.exec_module(catalog)
SPEC = importlib.util.spec_from_file_location("prepare_marketplace", ROOT / "prepare-marketplace.py")
prepare = importlib.util.module_from_spec(SPEC)
with patch.dict(sys.modules, {"catalog": catalog}):
    SPEC.loader.exec_module(prepare)


class PublicationPreparationTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="marketplace-preparation-test-")
        self.addCleanup(directory.cleanup)
        previous_directory = os.getcwd()
        self.addCleanup(os.chdir, previous_directory)
        os.chdir(directory.name)
        environment = patch.dict(os.environ, {
            "GITHUB_REPOSITORY": "dmytro-yevs/copypaste",
            "MODULE_RELEASE_TAG": "module-copypaste.ocr-v0.1.0",
        })
        environment.start()
        self.addCleanup(environment.stop)

    def release(self, draft=False, prerelease=False):
        return subprocess.CompletedProcess([], 0, json.dumps({"draft": draft, "prerelease": prerelease}), "")

    def test_only_not_found_is_treated_as_absent_metadata(self):
        for status in [404, 403, 500]:
            error = HTTPError("https://github.com/catalog", status, "failure", {}, None)
            with patch.object(prepare, "urlopen", side_effect=error):
                if status == 404:
                    self.assertIsNone(prepare.download_optional("https://github.com/catalog", 100))
                else:
                    with self.assertRaises(HTTPError):
                        prepare.download_optional("https://github.com/catalog", 100)

    def test_missing_catalog_never_erases_an_existing_release(self):
        with patch.object(prepare.subprocess, "run", side_effect=[self.release(), subprocess.CompletedProcess([], 0), self.release()]), \
                patch.object(prepare, "download_optional", return_value=None), \
                patch.object(prepare, "build_catalog") as build:
            with self.assertRaisesRegex(ValueError, "exists but its catalog is missing"):
                prepare.main()
            build.assert_not_called()
        self.assertFalse(Path("dist/create-marketplace-release").exists())

    def test_network_or_auth_failure_never_creates_a_fresh_catalog(self):
        for status in ["403", "500"]:
            Path("dist/packages").mkdir(parents=True, exist_ok=True)
            Path("dist/packages").rmdir()
            with patch.object(prepare.subprocess, "run", side_effect=[self.release(), subprocess.CompletedProcess([], 0), subprocess.CompletedProcess([], 1, f"HTTP/2.0 {status}\n", "")]), \
                    patch.object(prepare, "download_optional", return_value=None):
                with self.assertRaisesRegex(ValueError, "Could not confirm"):
                    prepare.main()
            self.assertFalse(Path("dist/create-marketplace-release").exists())

    def test_missing_signature_blocks_catalog_update(self):
        with patch.object(prepare.subprocess, "run", side_effect=[self.release(), subprocess.CompletedProcess([], 0)]), \
                patch.object(prepare, "download_optional", side_effect=[b'{"schema_version":1,"modules":[]}', None]), \
                patch.object(prepare, "build_catalog") as build:
            with self.assertRaisesRegex(ValueError, "missing its signature"):
                prepare.main()
            build.assert_not_called()

    def test_v2_bootstraps_the_verified_legacy_catalog_without_rewriting_it(self):
        legacy = b'{"schema_version":1,"modules":[]}'
        signature = b"c2lnbmF0dXJl"
        prepared = {"schema_version": 1, "modules": [{"id": "copypaste.ocr"}]}
        with patch.object(prepare.subprocess, "run", side_effect=[
                self.release(), subprocess.CompletedProcess([], 0)]), \
                patch.object(prepare, "download_optional", side_effect=[
                    legacy, signature, None]), \
                patch.object(prepare, "verify_signature") as verify, \
                patch.object(prepare, "build_catalog", return_value=prepared) as build:
            prepare.main()
        verify.assert_called_once_with(
            legacy, b"signature", catalog.LEGACY_CATALOG_NAME)
        self.assertEqual(build.call_args.args[2], json.loads(legacy))
        self.assertEqual(
            json.loads(Path("dist/modules-v2.json").read_text()), prepared)
        self.assertFalse(Path("dist/modules.json").exists())

    def test_draft_and_prerelease_packages_cannot_become_public_catalog_entries(self):
        for draft, prerelease in [(True, False), (False, True)]:
            with patch.object(prepare.subprocess, "run", return_value=self.release(draft, prerelease)) as run:
                with self.assertRaisesRegex(ValueError, "published stable"):
                    prepare.main()
                self.assertEqual(run.call_count, 1)


if __name__ == "__main__":
    unittest.main()
