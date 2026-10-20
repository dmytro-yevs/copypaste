from pathlib import Path
import runpy
import tempfile
import unittest


CHECK = runpy.run_path(str(Path(__file__).resolve().parents[2] / "check-docs.py"))["check"]


class DocumentationTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.tracked = set()
        self.write("README.md", "# Project\n")
        self.write("SECURITY.md", "# Security\n")

    def write(self, name, content, tracked=True):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)
        if tracked:
            self.tracked.add(path)
        return path

    def test_clean_checkout_needs_only_public_entry_documents(self):
        self.assertEqual(CHECK(self.root, self.tracked), [])

    def test_required_documents_must_exist_and_be_tracked(self):
        for name in ("README.md", "SECURITY.md"):
            path = self.root / name
            with self.subTest(name=name):
                self.tracked.remove(path)
                self.assertIn(f"missing tracked document: {name}", CHECK(self.root, self.tracked))
                self.tracked.add(path)
                path.unlink()
                self.assertIn(f"missing tracked document: {name}", CHECK(self.root, self.tracked))
                self.write(name, "# Restored\n")

    def test_local_ignored_file_cannot_hide_a_broken_repository_link(self):
        self.write("README.md", "[private instructions](local-notes/setup.md)\n")
        self.write("local-notes/setup.md", "Private notes\n", tracked=False)
        self.assertTrue(any("not tracked" in error for error in CHECK(self.root, self.tracked)))

    def test_tracked_links_external_urls_and_fragments_remain_valid(self):
        self.write("README.md", "[security](SECURITY.md#policy) [external](https://example.com)\n")
        self.assertEqual(CHECK(self.root, self.tracked), [])

    def test_missing_links_and_removed_implementation_references_fail(self):
        self.write("README.md", "[missing](absent.md)\ncrates/copypaste-ui\n")
        errors = CHECK(self.root, self.tracked)
        self.assertTrue(any("missing local link" in error for error in errors))
        self.assertTrue(any("removed implementation reference" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
