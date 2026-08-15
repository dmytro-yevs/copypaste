"""Refuse to touch a target directory a build is using. Uncertainty means busy.

Cargo takes `<profile>/.cargo-lock` for the duration of a build. The lock is
found by walking for it, never by naming profiles: a probe that checks only
`debug` and `release` reports "idle" during a `--target x86_64-linux-android`
or `--profile evidence` build and would delete that build's inputs underneath
it.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

CACHEDIR_SIGNATURE = b"Signature: 8a477f597d28d172789f06886806bc55"


def is_cargo_target(target: Path) -> bool:
    """True only when cargo's own CACHEDIR.TAG proves this is a target dir.

    https://bford.info/cachedir/ — the tag opens with exactly these 43 bytes.
    Gradle's `build/` carries no tag, which is what makes this a discriminator
    rather than a name check.
    """
    try:
        with (target / "CACHEDIR.TAG").open("rb") as handle:
            return handle.read(len(CACHEDIR_SIGNATURE)) == CACHEDIR_SIGNATURE
    except OSError:
        return False


def build_locks(target: Path) -> list[Path]:
    found = []
    for dirpath, _, filenames in os.walk(target):
        if ".cargo-lock" in filenames:
            found.append(Path(dirpath) / ".cargo-lock")
    return sorted(found)


def held_locks(target: Path) -> list[Path]:
    """Every `.cargo-lock` under `target` we cannot prove is free."""
    return [lock for lock in build_locks(target) if _is_held(lock)]


def _is_held(lock: Path) -> bool:
    try:
        handle = lock.open("r+b")
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
