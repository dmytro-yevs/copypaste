"""Size and remove planned paths, and report what was preserved and why."""

from __future__ import annotations

import os
import shutil
import stat
import sys
from dataclasses import dataclass, replace
from pathlib import Path

from .plan import Action
from .safety import is_build_active, is_cache_dir


@dataclass(frozen=True)
class Outcome:
    action: Action
    size: int
    removed: bool
    error: str | None


def directory_size(path: Path) -> int:
    total = 0
    for dirpath, _, filenames in os.walk(path, onerror=lambda _: None):
        for name in filenames:
            try:
                total += (Path(dirpath) / name).lstat().st_size
            except OSError:
                continue
    return total


def _force_writable(func, path, _exc):
    """Windows refuses to unlink a read-only file; clear the bit and retry."""
    try:
        os.chmod(path, stat.S_IWRITE)
        func(path)
    except OSError:
        pass


def _rmtree(path: Path) -> None:
    # `onerror` is removed in 3.14; `onexc` arrived in 3.12. Both hand the
    # callback (func, path, ...), so one callback serves either.
    if sys.version_info >= (3, 12):
        shutil.rmtree(path, onexc=_force_writable)
    else:
        shutil.rmtree(path, onerror=_force_writable)


def _stood_down(action: Action) -> Outcome:
    return Outcome(
        replace(action, remove=False, reason="a build took the cargo lock after the plan was made"),
        0,
        False,
        None,
    )


def apply(actions: list[Action], *, dry_run: bool) -> list[Outcome]:
    outcomes: list[Outcome] = []
    for action in actions:
        size = directory_size(action.path) if action.path.exists() else 0
        if not action.remove or dry_run:
            outcomes.append(Outcome(action, size, False, None))
            continue
        if not action.path.exists():
            # Idempotent: a second run finds the work already done.
            outcomes.append(Outcome(action, 0, False, None))
            continue
        # Re-tested here, not just in the plan: sizing walks the whole tree, and
        # on 11 GiB that is long enough for a build to start in.
        if is_cache_dir(action.path) and is_build_active(action.path):
            outcomes.append(_stood_down(action))
            continue
        try:
            _rmtree(action.path)
        except OSError as exc:
            outcomes.append(Outcome(action, size, False, str(exc)))
            continue
        if action.path.exists():
            outcomes.append(Outcome(action, size, False, "path survived removal"))
            continue
        outcomes.append(Outcome(action, size, True, None))
    return outcomes


def render(outcomes: list[Outcome], *, dry_run: bool) -> str:
    lines: list[str] = []
    reclaimable = 0
    reclaimed = 0
    failures = 0

    lines.append("== would remove" if dry_run else "== removed")
    for outcome in outcomes:
        if not outcome.action.remove:
            continue
        if outcome.error:
            failures += 1
            lines.append(f"  FAILED {_gib(outcome.size)}  {outcome.action.path}")
            lines.append(f"         {outcome.error}")
            continue
        reclaimable += outcome.size
        if outcome.removed:
            reclaimed += outcome.size
        lines.append(f"  {_gib(outcome.size)}  {outcome.action.path}")

    lines.append("")
    lines.append("== preserved")
    for outcome in outcomes:
        if outcome.action.remove:
            continue
        lines.append(f"  {_gib(outcome.size)}  {outcome.action.path}")
        lines.append(f"         {outcome.action.reason}")

    lines.append("")
    total = reclaimable if dry_run else reclaimed
    label = "reclaimable" if dry_run else "reclaimed"
    lines.append(f"{label}: {_gib(total)} ({total} bytes), {failures} failure(s)")
    return "\n".join(lines)


def _gib(size: int) -> str:
    return f"{size / (1024 ** 3):8.3f} GiB"
