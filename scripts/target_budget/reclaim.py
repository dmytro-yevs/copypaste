"""Carry out a plan, or describe it. Removal is `shutil.rmtree`/`Path.unlink`.

Containment is re-checked per path at removal time rather than trusted from the
survey: the survey and the removal are separated by a directory walk, and a
symlink swapped in between them is the whole reason to check late.
"""

from __future__ import annotations

import shutil
from dataclasses import dataclass
from pathlib import Path

from .plan import Eviction, Plan


@dataclass
class Outcome:
    eviction: Eviction
    removed: bool
    error: str | None = None


def removal_order(paths: tuple[Path, ...]) -> list[Path]:
    """Fingerprint first.

    It is what cargo consults to decide a unit is fresh, so an interruption
    after it is gone leaves a unit that rebuilds. The reverse order can leave a
    fingerprint vouching for artifacts that no longer exist.
    """
    return sorted(paths, key=lambda p: p.parent.name != ".fingerprint")


def _contained(path: Path, target: Path) -> bool:
    resolved = path.resolve()
    root = target.resolve()
    return resolved != root and root in resolved.parents


def apply(plan: Plan, target: Path, dry_run: bool = True) -> list[Outcome]:
    outcomes = []
    for eviction in plan.evictions:
        outcomes.extend(_apply_one(eviction, target, dry_run))
    return outcomes


def _apply_one(eviction: Eviction, target: Path, dry_run: bool) -> list[Outcome]:
    for path in eviction.paths:
        if not _contained(path, target):
            return [Outcome(eviction, removed=False, error=f"outside target: {path}")]
    if dry_run:
        return [Outcome(eviction, removed=False)]
    for path in removal_order(eviction.paths):
        try:
            if path.is_symlink() or path.is_file():
                path.unlink()
            elif path.is_dir():
                shutil.rmtree(path)
        except OSError as exc:
            return [Outcome(eviction, removed=False, error=str(exc))]
    return [Outcome(eviction, removed=True)]


def gib(value: int) -> str:
    return f"{value / 1024 ** 3:.2f} GiB"


def render(plan: Plan, outcomes: list[Outcome], dry_run: bool) -> str:
    errors = [o for o in outcomes if o.error]
    by_reason: dict[str, tuple[int, int]] = {}
    for outcome in outcomes:
        if outcome.error:
            continue
        count, size = by_reason.get(outcome.eviction.reason, (0, 0))
        by_reason[outcome.eviction.reason] = (count + 1, size + outcome.eviction.size)

    lines = ["would remove" if dry_run else "removed"]
    for reason, (count, size) in sorted(by_reason.items()):
        lines.append(f"  {count:>6} generations  {gib(size):>12}  {reason}")
    if not by_reason:
        lines.append("  nothing; every generation on disk is used by a marked configuration")

    lines.append(f"keeping {gib(plan.kept_bytes)}")
    if plan.newer_than_mark:
        lines.append(
            f"  {plan.newer_than_mark} generations kept only because they postdate the mark; "
            "re-run --mark to classify them"
        )
    if plan.budget is not None:
        lines.append(f"budget: {gib(plan.budget)}")
        if plan.shortfall:
            lines.append(
                f"OVER BUDGET by {gib(plan.shortfall)} after evicting every unmarked generation "
                "and all incremental scratch. What remains is live output for the marked "
                "configurations; removing it would force the rebuild this tool exists to avoid. "
                "Mark fewer configurations, or accept the figure."
            )
    for outcome in errors:
        lines.append(f"  ERROR {outcome.eviction.stem}-{outcome.eviction.key}: {outcome.error}")
    return "\n".join(lines)
