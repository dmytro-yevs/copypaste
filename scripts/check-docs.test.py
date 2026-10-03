#!/usr/bin/env python3
import importlib.util
import pathlib
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).resolve().parent / "check-docs.py"
spec = importlib.util.spec_from_file_location("check_docs", SCRIPT)
check_docs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check_docs)


class LocalLinkTest(unittest.TestCase):
    def test_existing_local_link_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "target.md").write_text("ok", encoding="utf-8")
            source = root / "source.md"
            source.write_text("[target](target.md)", encoding="utf-8")
            self.assertEqual(check_docs.local_link_errors(source, root), [])

    def test_missing_local_link_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            source = root / "source.md"
            source.write_text("[target](missing.md)", encoding="utf-8")
            self.assertIn("missing local link missing.md", check_docs.local_link_errors(source, root)[0])


if __name__ == "__main__":
    unittest.main()
