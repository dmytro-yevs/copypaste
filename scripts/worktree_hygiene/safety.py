"""Decide whether a path may be removed. Every uncertain answer is "no".

The rule that earns its place: `complete-android-e2e` sat on disk as a full
source tree with no `.git`, holding `Screenshot_1786337929.png` whose content
is in no git object. A cleaner that removed leftover directories wholesale
would have destroyed the only copy.

Content is removed only where it is proved recoverable, with one stated
exception: directories that regenerate from a manifest they sit next to
([`ALWAYS_GENERATED`]) are skipped unread. `build/` and `target/` are *not* in
that set — Gradle writes instrumentation evidence under `build/reports/`, and
skipping the name let a scan return clean and take the whole leftover with it.
They are skipped only when they carry the cache tag that proves what they are.
"""

from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass
from pathlib import Path

from cargo_target import CACHEDIR_SIGNATURE, is_build_active, is_cache_dir  # noqa: F401

# Rebuilt from a manifest that sits beside them, so nothing here is a last copy.
ALWAYS_GENERATED = frozenset({"node_modules", "__pycache__"})


@dataclass(frozen=True)
class Verdict:
    removable: bool
    reason: str


def is_contained(path: Path, roots: list[Path]) -> bool:
    """True only if `path` resolves strictly inside one declared root.

    Resolved on both sides so a symlink cannot walk the cleaner out of its
    roots, and a root is never containable in itself.
    """
    resolved = path.resolve()
    for root in roots:
        root = root.resolve()
        if resolved != root and root in resolved.parents:
            return True
    return False


def is_dirty(worktree: Path) -> bool | None:
    """True/False for a git worktree, None when git cannot answer at all."""
    done = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=worktree,
        capture_output=True,
        text=True,
        check=False,
    )
    if done.returncode != 0:
        return None
    return bool(done.stdout.strip())


def unrecoverable_files(path: Path, repo: Path, limit: int = 5) -> list[Path]:
    """Files under `path` whose exact content is in no object of `repo`.

    A directory is skipped only when it says what it is: a manifest-backed name
    in [`ALWAYS_GENERATED`], or a cache carrying `CACHEDIR.TAG`. A bare `build/`
    or `target/` is read, because neither name proves anything on its own.
    """
    suspects: list[Path] = []
    for dirpath, dirnames, filenames in os.walk(path):
        here = Path(dirpath)
        dirnames[:] = [
            d
            for d in dirnames
            if d != ".git" and d not in ALWAYS_GENERATED and not is_cache_dir(here / d)
        ]
        for name in filenames:
            candidate = Path(dirpath) / name
            if not _in_object_store(candidate, repo):
                suspects.append(candidate)
                if len(suspects) >= limit:
                    return suspects
    return suspects


def _in_object_store(file: Path, repo: Path) -> bool:
    hashed = subprocess.run(
        ["git", "hash-object", "--", str(file)],
        cwd=repo,
        capture_output=True,
        text=True,
        check=False,
    )
    if hashed.returncode != 0:
        return False
    blob = hashed.stdout.strip()
    exists = subprocess.run(
        ["git", "cat-file", "-e", blob],
        cwd=repo,
        capture_output=True,
        check=False,
    )
    return exists.returncode == 0
