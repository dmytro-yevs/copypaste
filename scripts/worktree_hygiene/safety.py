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
import sys
from dataclasses import dataclass
from pathlib import Path

# https://bford.info/cachedir/ — a tag file begins with exactly these 43 bytes.
CACHEDIR_SIGNATURE = b"Signature: 8a477f597d28d172789f06886806bc55"

# Rebuilt from a manifest that sits beside them, so nothing here is a last copy.
ALWAYS_GENERATED = frozenset({"node_modules", "__pycache__"})

# How deep a `.cargo-lock` can sit: `<profile>/` for a plain build,
# `<triple>/<profile>/` once `--target` is given.
_LOCK_GLOBS = ("*/.cargo-lock", "*/*/.cargo-lock")


@dataclass(frozen=True)
class Verdict:
    removable: bool
    reason: str


def is_cache_dir(path: Path) -> bool:
    tag = path / "CACHEDIR.TAG"
    try:
        with tag.open("rb") as handle:
            return handle.read(len(CACHEDIR_SIGNATURE)) == CACHEDIR_SIGNATURE
    except OSError:
        return False


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


def is_build_active(target: Path) -> bool:
    """True if cargo holds a build lock, or we cannot prove that it does not.

    Locks are discovered, not listed. Naming `debug` and `release` missed both
    `--target <triple>` builds and `[profile.evidence]`, and each miss deletes a
    cache under a running build.
    """
    try:
        locks = [lock for pattern in _LOCK_GLOBS for lock in target.glob(pattern)]
    except OSError:
        return True
    for lock in locks:
        try:
            handle = lock.open("r+b")
        except FileNotFoundError:
            # The only benign miss. `Path.exists()` cannot be used to pre-filter:
            # it reports a permission error as absence, which is a fail-open step
            # inside a function whose whole job is to fail closed.
            continue
        except OSError:
            return True
        try:
            if sys.platform == "win32":
                import msvcrt

                msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
                msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                import fcntl

                fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
        except OSError:
            return True
        finally:
            handle.close()
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
