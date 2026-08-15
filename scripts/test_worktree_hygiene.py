#!/usr/bin/env python3
"""Tests for the worktree hygiene mechanism. Runs on Windows, macOS and Linux.

Every protection is driven from a constructed fixture, because the live tree
has no dirty worktree left to exercise and a test that waits for one is a test
that never runs.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from worktree_hygiene import apply, discover, plan, render  # noqa: E402
from worktree_hygiene.safety import (  # noqa: E402
    CACHEDIR_SIGNATURE,
    is_build_active,
    is_cache_dir,
    is_contained,
    unrecoverable_files,
)


def git(*args: str, cwd: Path) -> str:
    done = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=True)
    return done.stdout


def make_repo(root: Path) -> Path:
    repo = root / "repo"
    repo.mkdir()
    git("init", "-q", "-b", "main", cwd=repo)
    git("config", "user.email", "t@example.com", cwd=repo)
    git("config", "user.name", "t", cwd=repo)
    (repo / "README.md").write_text("hello\n", encoding="utf-8")
    # Mirrors the real repo: a build cache must not make a worktree read dirty,
    # or the dirty-work protection would swallow every cache and prove nothing.
    (repo / ".gitignore").write_text("/target/\nnode_modules/\n", encoding="utf-8")
    git("add", "README.md", ".gitignore", cwd=repo)
    git("commit", "-qm", "init", cwd=repo)
    return repo


def make_cache(path: Path, *, payload: int = 2048) -> Path:
    path.mkdir(parents=True, exist_ok=True)
    (path / "CACHEDIR.TAG").write_bytes(CACHEDIR_SIGNATURE + b"\n")
    (path / "debug").mkdir(exist_ok=True)
    (path / "debug" / "blob.bin").write_bytes(b"\0" * payload)
    return path


class Base(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name).resolve()
        self.repo = make_repo(self.root)
        self.trees = self.root / "trees"
        self.trees.mkdir()

    def tearDown(self) -> None:
        # A worktree holds handles open on Windows; drop them before cleanup.
        subprocess.run(["git", "worktree", "prune"], cwd=self.repo, capture_output=True, check=False)
        self._tmp.cleanup()

    def add_worktree(self, name: str) -> Path:
        path = self.trees / name
        git("worktree", "add", "-q", "-b", name, str(path), cwd=self.repo)
        return path

    def run_plan(self):
        roots = [self.trees]
        return plan(self.repo, discover(self.repo, roots), roots)


class TestCacheTag(Base):
    def test_valid_tag_is_a_cache(self) -> None:
        self.assertTrue(is_cache_dir(make_cache(self.root / "t")))

    def test_missing_tag_is_not_a_cache(self) -> None:
        (self.root / "u").mkdir()
        self.assertFalse(is_cache_dir(self.root / "u"))

    def test_wrong_signature_is_not_a_cache(self) -> None:
        path = self.root / "v"
        path.mkdir()
        (path / "CACHEDIR.TAG").write_bytes(b"Signature: not-the-cachedir-signature-at-all")
        self.assertFalse(is_cache_dir(path))


class TestContainment(Base):
    def test_child_is_contained(self) -> None:
        child = self.trees / "a"
        child.mkdir()
        self.assertTrue(is_contained(child, [self.trees]))

    def test_root_is_not_contained_in_itself(self) -> None:
        self.assertFalse(is_contained(self.trees, [self.trees]))

    def test_sibling_is_not_contained(self) -> None:
        other = self.root / "elsewhere"
        other.mkdir()
        self.assertFalse(is_contained(other, [self.trees]))


class TestProtections(Base):
    def test_primary_checkout_is_never_removed(self) -> None:
        make_cache(self.repo / "target")
        actions = self.run_plan()
        primary = [a for a in actions if a.path == self.repo]
        self.assertEqual(len(primary), 1)
        self.assertFalse(primary[0].remove)
        self.assertIn("primary checkout", primary[0].reason)

    def test_dirty_worktree_is_preserved(self) -> None:
        tree = self.add_worktree("dirty")
        make_cache(tree / "target")
        (tree / "uncommitted.txt").write_text("work in progress\n", encoding="utf-8")
        actions = [a for a in self.run_plan() if tree in a.path.parents or a.path == tree]
        self.assertTrue(all(not a.remove for a in actions))
        self.assertTrue(any("uncommitted" in a.reason for a in actions))

    def test_clean_worktree_cache_is_removed(self) -> None:
        tree = self.add_worktree("clean")
        cache = make_cache(tree / "target")
        actions = [a for a in self.run_plan() if a.path == cache]
        self.assertEqual(len(actions), 1)
        self.assertTrue(actions[0].remove)

    def test_target_without_tag_is_preserved(self) -> None:
        tree = self.add_worktree("untagged")
        (tree / "target").mkdir()
        (tree / "target" / "thing.bin").write_bytes(b"\0" * 16)
        actions = [a for a in self.run_plan() if a.path == tree / "target"]
        self.assertEqual(len(actions), 1)
        self.assertFalse(actions[0].remove)
        self.assertIn("CACHEDIR.TAG", actions[0].reason)

    def test_orphan_with_unrecoverable_file_is_preserved(self) -> None:
        orphan = self.trees / "leftover"
        orphan.mkdir()
        (orphan / "screenshot.png").write_bytes(b"only-copy-of-this")
        actions = [a for a in self.run_plan() if a.path == orphan]
        self.assertEqual(len(actions), 1)
        self.assertFalse(actions[0].remove)
        self.assertIn("screenshot.png", actions[0].reason)

    def test_launcher_infrastructure_is_not_a_candidate(self) -> None:
        infra = self.trees / ".orca-worktree-trash"
        infra.mkdir()
        (infra / "note.txt").write_text("launcher bookkeeping\n", encoding="utf-8")
        self.assertFalse(any(a.path == infra for a in self.run_plan()))

    def test_orphan_of_committed_content_is_removable(self) -> None:
        orphan = self.trees / "spent"
        orphan.mkdir()
        (orphan / "README.md").write_text("hello\n", encoding="utf-8")
        actions = [a for a in self.run_plan() if a.path == orphan]
        self.assertEqual(len(actions), 1)
        self.assertTrue(actions[0].remove)

    def test_generated_directories_do_not_block_an_orphan(self) -> None:
        orphan = self.trees / "generated"
        (orphan / "node_modules" / "pkg").mkdir(parents=True)
        (orphan / "node_modules" / "pkg" / "index.js").write_text("x", encoding="utf-8")
        (orphan / "README.md").write_text("hello\n", encoding="utf-8")
        self.assertEqual(unrecoverable_files(orphan, self.repo), [])


class TestActiveBuild(Base):
    """A held cargo lock must stop a removal on every platform we ship."""

    @staticmethod
    def _lock(path: Path):
        handle = path.open("r+b")
        if sys.platform == "win32":
            import msvcrt

            msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl

            fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        return handle

    def test_unlocked_cache_is_not_active(self) -> None:
        cache = make_cache(self.root / "idle")
        (cache / "debug" / ".cargo-lock").write_bytes(b"\0")
        self.assertFalse(is_build_active(cache))

    def test_held_lock_makes_the_cache_active(self) -> None:
        cache = make_cache(self.root / "busy")
        lock = cache / "debug" / ".cargo-lock"
        lock.write_bytes(b"\0")
        handle = self._lock(lock)
        try:
            self.assertTrue(is_build_active(cache))
        finally:
            handle.close()

    def test_a_running_build_preserves_the_cache(self) -> None:
        tree = self.add_worktree("building")
        cache = make_cache(tree / "target")
        lock = cache / "debug" / ".cargo-lock"
        lock.write_bytes(b"\0")
        handle = self._lock(lock)
        try:
            actions = [a for a in self.run_plan() if a.path == cache]
            self.assertEqual(len(actions), 1)
            self.assertFalse(actions[0].remove)
            self.assertIn("cargo lock", actions[0].reason)
        finally:
            handle.close()


class TestApply(Base):
    def test_dry_run_removes_nothing(self) -> None:
        tree = self.add_worktree("dry")
        cache = make_cache(tree / "target")
        outcomes = apply(self.run_plan(), dry_run=True)
        self.assertTrue(cache.exists())
        self.assertTrue(any(o.action.remove and not o.removed for o in outcomes))

    def test_apply_removes_and_reports_bytes(self) -> None:
        tree = self.add_worktree("wet")
        cache = make_cache(tree / "target", payload=4096)
        outcomes = apply(self.run_plan(), dry_run=False)
        self.assertFalse(cache.exists())
        removed = [o for o in outcomes if o.removed]
        self.assertTrue(removed)
        self.assertGreaterEqual(sum(o.size for o in removed), 4096)

    def test_apply_is_idempotent(self) -> None:
        tree = self.add_worktree("twice")
        make_cache(tree / "target")
        apply(self.run_plan(), dry_run=False)
        second = apply(self.run_plan(), dry_run=False)
        self.assertFalse(any(o.error for o in second))

    def test_render_names_every_preserved_reason(self) -> None:
        tree = self.add_worktree("explain")
        make_cache(tree / "target")
        (tree / "uncommitted.txt").write_text("wip\n", encoding="utf-8")
        text = render(apply(self.run_plan(), dry_run=True), dry_run=True)
        self.assertIn("preserved", text)
        self.assertIn("uncommitted work", text)
        self.assertIn("reclaimable", text)

    def test_failure_is_reported_not_raised(self) -> None:
        tree = self.add_worktree("locked")
        cache = make_cache(tree / "target")
        actions = [a for a in self.run_plan() if a.path == cache]
        cache.rename(cache.parent / "moved-away")
        outcomes = apply(actions, dry_run=False)
        self.assertTrue(all(o.error is None for o in outcomes))
        self.assertFalse(any(o.removed for o in outcomes))


if __name__ == "__main__":
    unittest.main(verbosity=2)
