#!/usr/bin/env python3
"""Report and reclaim per-worktree build artifacts. Dry-run unless --apply.

    scripts/worktree-hygiene.py                 # report, remove nothing
    scripts/worktree-hygiene.py --apply         # remove what the report listed

Roots default to the Orca and codex worktree parents beside this checkout; a
root that does not exist is skipped rather than guessed at.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from worktree_hygiene import apply, discover, plan, render  # noqa: E402

DEFAULT_ROOTS = (
    Path.home() / "orca" / "workspaces" / "copypaste",
    Path.home() / ".codex" / "worktrees",
    Path.home() / "orca" / "worktrees",
)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apply", action="store_true", help="remove; default is a dry run")
    parser.add_argument("--root", action="append", type=Path, default=None,
                        help="a worktree parent directory; repeatable")
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args(argv)

    repo = args.repo.resolve()
    roots = [r.resolve() for r in (args.root or DEFAULT_ROOTS) if r.exists()]
    if not roots:
        print("no declared root exists; nothing to inspect", file=sys.stderr)
        return 0

    print(f"repo:  {repo}")
    for root in roots:
        print(f"root:  {root}")
    print()

    actions = plan(repo, discover(repo, roots), roots)
    outcomes = apply(actions, dry_run=not args.apply)
    print(render(outcomes, dry_run=not args.apply))

    if any(o.error for o in outcomes):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
