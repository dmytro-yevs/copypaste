"""Decide whether a path may be removed. Every uncertain answer is "no".

The rule that earns its place: `complete-android-e2e` sat on disk as a full
source tree with no `.git`, holding `Screenshot_1786337929.png` whose content
is in no git object. A cleaner that removed leftover directories wholesale
would have destroyed the only copy. Nothing without a recoverable-content
proof or a cache tag is ever removed.
"""

from __future__ import annotations

import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

# https://bford.info/cachedir/ — a tag file begins with exactly these 43 bytes.
CACHEDIR_SIGNATURE = b"Signature: 8a477f597d28d172789f06886806bc55"

# Regenerable by their own toolchain, and too large to hash file by file.
GENERATED_DIR_NAMES = frozenset({"node_modules", "build", "target", "__pycache__"})


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
    """True if cargo holds a build lock, or we cannot prove that it does not."""
    locks = [target / profile / ".cargo-lock" for profile in ("debug", "release")]
    for lock in locks:
        if not lock.exists():
            continue
        try:
            handle = lock.open("r+b")
        except PermissionError:
            return True
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

    Directories named in GENERATED_DIR_NAMES are skipped: their toolchain
    rebuilds them, and hashing gigabytes of them would make this unusable.
    """
    suspects: list[Path] = []
    for dirpath, dirnames, filenames in os.walk(path):
        dirnames[:] = [d for d in dirnames if d not in GENERATED_DIR_NAMES and d != ".git"]
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
