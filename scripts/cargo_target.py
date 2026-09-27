"""What cargo's own markers say about a target directory.

Shared by `worktree_hygiene` and `target_budget`, which both have to answer the
same two questions before removing anything: is this really a cargo target, and
is a build using it. Two copies of a fail-closed check drift into one strict and
one permissive, and the permissive one is the one that deletes.
"""

from __future__ import annotations

import sys
from pathlib import Path

# https://bford.info/cachedir/ — a tag file begins with exactly these 43 bytes.
# Cargo writes one into `target/`; Gradle writes none into `build/`, which is
# what makes this a discriminator rather than a name check.
CACHEDIR_SIGNATURE = b"Signature: 8a477f597d28d172789f06886806bc55"

# Cargo takes `<profile>/.cargo-lock`, and under `--target <triple>` puts the
# profile a level deeper. Those are the only two shapes it uses, so this is the
# whole search rather than a sample of it — but it must not be narrowed to
# `debug` and `release`, which misses every cross-target build and every custom
# profile such as `[profile.evidence]`.
_LOCK_GLOBS = ("*/.cargo-lock", "*/*/.cargo-lock")


def is_cache_dir(path: Path) -> bool:
    try:
        with (path / "CACHEDIR.TAG").open("rb") as handle:
            return handle.read(len(CACHEDIR_SIGNATURE)) == CACHEDIR_SIGNATURE
    except OSError:
        return False


def build_locks(target: Path) -> list[Path]:
    return sorted(lock for pattern in _LOCK_GLOBS for lock in target.glob(pattern))


def is_build_active(target: Path) -> bool:
    """True if cargo holds a build lock, or we cannot prove that it does not."""
    try:
        locks = build_locks(target)
    except OSError:
        return True
    return any(_is_held(lock) for lock in locks)


def held_locks(target: Path) -> list[Path]:
    """Every lock under `target` we cannot prove is free, for reporting which."""
    try:
        return [lock for lock in build_locks(target) if _is_held(lock)]
    except OSError:
        return [target]


def _is_held(lock: Path) -> bool:
    try:
        handle = lock.open("r+b")
    except FileNotFoundError:
        # The only benign miss. `Path.exists()` cannot be used to pre-filter: it
        # reports a permission error as absence, which is a fail-open step
        # inside a function whose whole job is to fail closed.
        return False
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
