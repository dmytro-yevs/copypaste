"""Turn discovered candidates into a plan of removals and recorded preserves.

Policy only; nothing here touches the filesystem beyond reading. Every branch
that cannot prove a path is disposable produces a Preserve with the reason,
because an unexplained skip and a silent deletion are the same defect from
opposite ends.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from .discovery import Candidate
from .safety import is_build_active, is_cache_dir, is_contained, is_dirty, unrecoverable_files

CACHE_SUBPATHS = (
    Path("target"),
    Path("crates/copypaste-ui/node_modules"),
    Path("e2e/node_modules"),
    Path("node_modules"),
)


@dataclass(frozen=True)
class Action:
    path: Path
    remove: bool
    reason: str


def plan(repo: Path, candidates: list[Candidate], roots: list[Path]) -> list[Action]:
    repo = repo.resolve()
    actions: list[Action] = []
    for candidate in candidates:
        actions.extend(_for_candidate(repo, candidate, roots))
    return actions


def _for_candidate(repo: Path, candidate: Candidate, roots: list[Path]) -> list[Action]:
    path = candidate.path
    if path == repo:
        return _for_primary(path)

    if candidate.registered:
        return _for_registered(candidate, roots)
    return _for_orphan(repo, candidate, roots)


_PRIMARY_REASON = (
    "primary checkout, never removed here; reclaim with scripts/clean-target.sh (DMY-189)"
)


def _for_primary(repo: Path) -> list[Action]:
    """Size the primary checkout's caches without ever offering to remove them.

    Reported per cache rather than as one checkout total: it holds most of the
    bytes on disk, and a report that omitted them would understate the problem
    by roughly four fifths.
    """
    caches = [repo / relative for relative in CACHE_SUBPATHS if (repo / relative).is_dir()]
    if not caches:
        return [Action(repo, False, _PRIMARY_REASON)]
    return [Action(cache, False, _PRIMARY_REASON) for cache in caches]


def _for_registered(candidate: Candidate, roots: list[Path]) -> list[Action]:
    path = candidate.path
    dirty = is_dirty(path)
    if dirty is None:
        return [Action(path, False, "git could not report status; ownership unproven")]
    if dirty:
        return [Action(path, False, "worktree has uncommitted work")]

    actions: list[Action] = []
    for relative in CACHE_SUBPATHS:
        cache = path / relative
        if not cache.is_dir():
            continue
        if not is_contained(cache, roots):
            actions.append(Action(cache, False, "resolves outside every declared root"))
            continue
        if relative == Path("target"):
            if not is_cache_dir(cache):
                actions.append(Action(cache, False, "no valid CACHEDIR.TAG"))
                continue
            if is_build_active(cache):
                actions.append(Action(cache, False, "a build holds the cargo lock"))
                continue
        actions.append(Action(cache, True, f"regenerable cache in a clean worktree ({candidate.branch})"))
    return actions


def _for_orphan(repo: Path, candidate: Candidate, roots: list[Path]) -> list[Action]:
    path = candidate.path
    if not is_contained(path, roots):
        return [Action(path, False, "resolves outside every declared root")]
    if candidate.has_git:
        return [Action(path, False, "carries .git but is not a registered worktree")]

    stranded = unrecoverable_files(path, repo)
    if stranded:
        names = ", ".join(p.name for p in stranded)
        return [Action(path, False, f"holds content in no git object: {names}")]
    return [Action(path, True, "leftover directory; every file is generated or in the object store")]
