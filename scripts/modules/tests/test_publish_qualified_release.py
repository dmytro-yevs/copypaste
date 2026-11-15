import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "publish_qualified_release", Path(__file__).parents[1] / "publish-qualified-release.py"
)
publisher = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publisher)


class QualifiedModulePublisherTest(unittest.TestCase):
    def test_waits_for_new_draft_to_appear_in_release_inventory(self):
        release = {"id": 123, "draft": True}
        with patch.object(publisher.publisher, "find_release", side_effect=[None, release]) as find, \
                patch.object(publisher.time, "sleep") as sleep:
            self.assertIs(publisher.wait_for_created_release("dmytro-yevs/copypaste", "tag"), release)
        self.assertEqual(find.call_count, 2)
        sleep.assert_called_once_with(1)

    def test_accepts_only_first_party_versioned_module_tag(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "module.json"
            manifest.write_text(json.dumps({"id": "copypaste.sms-codes", "version": "0.1.21"}))
            self.assertEqual(publisher.module_tag(manifest), "module-copypaste.sms-codes-v0.1.21")
            for module_id, version in (("copypaste.foreign", "0.1.21"),
                                       ("copypaste.supabase", "0.1.21-evil")):
                manifest.write_text(json.dumps({"id": module_id, "version": version}))
                with self.assertRaises(ValueError):
                    publisher.module_tag(manifest)

    def test_rejects_any_qualification_artifact_that_is_not_a_package_or_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            module = json.loads((publisher.ROOT / "modules/sms-codes/module.json").read_text())
            for architecture in ("arm", "aarch64", "x86_64"):
                package = root / f"CopyPasteModule-{module['id']}-v{module['version']}-android-{architecture}.cpmodule"
                package.write_bytes(b"package")
            package.with_name(package.name + ".receipt.json").write_text("{}")
            self.assertEqual(publisher.qualified_paths(root, module), sorted(root.iterdir()))
            (root / "qualification.apk").write_bytes(b"test-only")
            with self.assertRaisesRegex(ValueError, "unexpected artifact"):
                publisher.qualified_paths(root, module)

    def test_draft_recovery_rejects_a_different_tag_or_commit(self):
        tag = "module-copypaste.supabase-v0.1.21"
        commit = "a" * 40
        release = {"id": 123, "tag_name": tag, "target_commitish": commit,
                   "draft": True, "prerelease": False}
        publisher.validate_release(release, tag, commit)
        for change in ({"tag_name": "module-copypaste.supabase-v0.1.20"},
                       {"target_commitish": "b" * 40}, {"prerelease": True},
                       {"id": 0}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                publisher.validate_release(release | change, tag, commit)


if __name__ == "__main__":
    unittest.main()
