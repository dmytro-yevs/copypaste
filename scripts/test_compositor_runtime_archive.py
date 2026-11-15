#!/usr/bin/env python3
"""Regression tests for compositor runtime artifact transport."""

import io
import runpy
import tarfile
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[1]
ARCHIVE = runpy.run_path(str(ROOT / "scripts/release/compositor-runtime-archive.py"))


class CompositorRuntimeArchiveTest(unittest.TestCase):
    def test_round_trip_preserves_hidden_files_modes_and_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            (source / "lib").mkdir(parents=True)
            executable = source / ".headless-launcher"
            executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            executable.chmod(0o755)
            library = source / "lib/private.so"
            library.write_bytes(b"private-runtime")
            (source / "lib/alias.so").symlink_to("private.so")
            transport = root / "runtime.tar"
            output = root / "output"
            ARCHIVE["pack"](source, transport)
            ARCHIVE["extract"](transport, output)
            self.assertEqual((output / ".headless-launcher").stat().st_mode & 0o777, 0o755)
            self.assertEqual((output / ".headless-launcher").read_text(encoding="utf-8"), executable.read_text(encoding="utf-8"))
            self.assertTrue((output / "lib/alias.so").is_symlink())
            self.assertEqual((output / "lib/alias.so").readlink(), Path("private.so"))

    def test_rejects_a_traversal_member(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            transport = root / "runtime.tar"
            with tarfile.open(transport, "w") as archive:
                member = tarfile.TarInfo("../outside")
                member.size = 1
                archive.addfile(member, io.BytesIO(b"x"))
            with self.assertRaisesRegex(ValueError, "escapes"):
                ARCHIVE["extract"](transport, root / "output")

    def test_rejects_a_symlink_parent_escape_before_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            transport = root / "runtime.tar"
            with tarfile.open(transport, "w") as archive:
                directory_link = tarfile.TarInfo("dir")
                directory_link.type = tarfile.SYMTYPE
                directory_link.linkname = "."
                archive.addfile(directory_link)
                escape_link = tarfile.TarInfo("dir/link")
                escape_link.type = tarfile.SYMTYPE
                escape_link.linkname = "../inside"
                archive.addfile(escape_link)
                member = tarfile.TarInfo("dir/link/file")
                member.size = 1
                archive.addfile(member, io.BytesIO(b"x"))
            output = root / "output"
            outside = root / "inside/file"
            with self.assertRaisesRegex(ValueError, "symlink ancestor"):
                ARCHIVE["extract"](transport, output)
            self.assertFalse(output.exists())
            self.assertFalse(outside.exists())


if __name__ == "__main__":
    unittest.main()
