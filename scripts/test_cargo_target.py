#!/usr/bin/env python3
"""Tests for the cargo markers shared by worktree_hygiene and target_budget.

Both callers delete bytes on the strength of these two answers, so each case
here is a way a cleaner could conclude "idle" or "regenerable" while it is not.
"""

import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import cargo_target  # noqa: E402

HOLD_LOCK = """
import sys, time
handle = open(sys.argv[1], "r+b")
if sys.platform == "win32":
    import msvcrt
    msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
else:
    import fcntl
    fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
print("held", flush=True)
time.sleep(120)
"""


class CacheTagTest(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def test_a_tagged_directory_is_a_cache(self):
        target = self.tmp / "target"
        target.mkdir()
        (target / "CACHEDIR.TAG").write_bytes(cargo_target.CACHEDIR_SIGNATURE + b"\n")
        self.assertTrue(cargo_target.is_cache_dir(target))

    def test_gradles_build_directory_is_not(self):
        """The name proves nothing; Gradle writes no tag, which is the point."""
        build = self.tmp / "build"
        build.mkdir()
        self.assertFalse(cargo_target.is_cache_dir(build))

    def test_a_near_miss_signature_is_not_a_cache(self):
        target = self.tmp / "target"
        target.mkdir()
        (target / "CACHEDIR.TAG").write_bytes(b"Signature: 0000" + b"\n")
        self.assertFalse(cargo_target.is_cache_dir(target))


class LockTest(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.target = self.tmp / "target"
        (self.target / "debug").mkdir(parents=True)

    def test_a_lock_is_found_under_any_profile_not_just_debug_and_release(self):
        """A cross-target or custom-profile build must not read as idle.

        Probing a hardcoded `debug`/`release` pair reports "no build running"
        during `--target x86_64-linux-android` or `--profile evidence`, and the
        cleaner then removes that build's inputs while it runs.
        """
        for profile in ("debug", "evidence", "x86_64-linux-android/debug"):
            path = self.target / profile
            path.mkdir(parents=True, exist_ok=True)
            (path / ".cargo-lock").write_bytes(b"\0")

        found = cargo_target.build_locks(self.target)

        self.assertEqual({p.parent.name for p in found}, {"debug", "evidence"})
        self.assertIn(
            "x86_64-linux-android",
            {p.parent.parent.name for p in found},
            "a --target build's lock sits one level deeper and must still be seen",
        )

    def test_no_lock_file_means_no_build(self):
        self.assertEqual(cargo_target.build_locks(self.target), [])
        self.assertFalse(cargo_target.is_build_active(self.target))

    def test_an_unheld_lock_file_means_no_build(self):
        (self.target / "debug" / ".cargo-lock").write_bytes(b"\0")
        self.assertFalse(cargo_target.is_build_active(self.target))

    def test_a_lock_held_by_another_process_blocks(self):
        """A real build holds it from another process, so the test must too.

        `fcntl.flock` and `msvcrt.locking` are both per-process: taking the lock
        in this process leaves the probe free to take it again, and the test
        then passes while the guard does nothing.
        """
        lock = self.target / "debug" / ".cargo-lock"
        lock.write_bytes(b"\0")
        self.assertFalse(cargo_target.is_build_active(self.target))

        holder = subprocess.Popen(
            [sys.executable, "-c", HOLD_LOCK, str(lock)],
            stdout=subprocess.PIPE,
            text=True,
        )
        self.addCleanup(holder.kill)
        self.assertEqual(holder.stdout.readline().strip(), "held")

        self.assertTrue(cargo_target.is_build_active(self.target))
        self.assertEqual(cargo_target.held_locks(self.target), [lock])


if __name__ == "__main__":
    unittest.main(verbosity=2)
