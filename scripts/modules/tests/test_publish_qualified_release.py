import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SPEC = importlib.util.spec_from_file_location(
    "publish_qualified_release", Path(__file__).parents[1] / "publish-qualified-release.py"
)
publisher = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publisher)


class QualifiedModulePublisherTest(unittest.TestCase):
    def test_accepts_only_first_party_versioned_module_tag(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "module.json"
            manifest.write_text(json.dumps({"id": "copypaste.supabase", "version": "0.1.21"}))
            self.assertEqual(publisher.module_tag(manifest), "module-copypaste.supabase-v0.1.21")
            for module_id, version in (("copypaste.foreign", "0.1.21"),
                                       ("copypaste.supabase", "0.1.21-evil")):
                manifest.write_text(json.dumps({"id": module_id, "version": version}))
                with self.assertRaises(ValueError):
                    publisher.module_tag(manifest)

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
