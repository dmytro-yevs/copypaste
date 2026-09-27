"""Enumerate the worktrees and leftover directories under declared roots.

Do not reduce this to `git worktree prune`. Prune removes registrations, and a
swept directory keeps none, so it cannot see a leftover at all: after the
2026-08-15 sweep `git worktree prune --dry-run` reported nothing while nine
leftover directories sat on disk, one of them holding 0.923 GiB. Orphans are
found by walking the roots and differencing against `git worktree list`.
"""

from __future__ import annotations

import subprocess
from dataclasses import dataclass
from pathlib import Path

# A codex worktree nests the checkout one level under its slug.
_NESTED = "CopyPaste"


@dataclass(frozen=True)
class Candidate:
    path: Path
    registered: bool
    branch: str | None

    @property
    def has_git(self) -> bool:
        return (self.path / ".git").exists()


def _run(args: list[str], cwd: Path) -> str:
    done = subprocess.run(
        args, cwd=cwd, capture_output=True, text=True, check=False
    )
    if done.returncode != 0:
        raise RuntimeError(f"{' '.join(args)} failed: {done.stderr.strip()}")
    return done.stdout


def registered_worktrees(repo: Path) -> dict[Path, str | None]:
    """Map each registered worktree to its branch, primary checkout included."""
    out = _run(["git", "worktree", "list", "--porcelain"], repo)
    found: dict[Path, str | None] = {}
    current: Path | None = None
    for line in out.splitlines():
        if line.startswith("worktree "):
            current = Path(line[len("worktree ") :]).resolve()
            found[current] = None
        elif line.startswith("branch ") and current is not None:
            found[current] = line[len("branch ") :].removeprefix("refs/heads/")
    return found


def discover(repo: Path, roots: list[Path]) -> list[Candidate]:
    """Every checkout-shaped directory under `roots`, plus the primary checkout.

    A root itself is never a candidate; only its children are. That keeps a
    mistyped root from nominating a shared user directory for removal.
    """
    known = registered_worktrees(repo)
    seen: set[Path] = set()
    out: list[Candidate] = []

    for path, branch in known.items():
        if path not in seen:
            seen.add(path)
            out.append(Candidate(path=path, registered=True, branch=branch))

    for root in roots:
        root = root.resolve()
        if not root.is_dir():
            continue
        for child in sorted(root.iterdir()):
            if not child.is_dir():
                continue
            # `.orca-worktree-trash` is the launcher's own bookkeeping, not a
            # worktree. A dotted directory beside worktrees is infrastructure.
            if child.name.startswith("."):
                continue
            nested = child / _NESTED
            path = (nested if nested.is_dir() else child).resolve()
            if path in seen:
                continue
            seen.add(path)
            out.append(Candidate(path=path, registered=False, branch=None))
    return out
