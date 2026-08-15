#!/usr/bin/env python3
"""Bound one cargo target directory against what cargo says it still uses.

    scripts/target-budget.py --mark -- build --workspace --tests --locked
    scripts/target-budget.py                     # report, remove nothing
    scripts/target-budget.py --apply             # remove the unmarked
    scripts/target-budget.py --budget 12GiB      # report against a ceiling

Mark every configuration you build; sweeping removes what no mark named. Dry
run unless `--apply`. Nothing invokes this on a schedule: no hook, no timer, no
CI step. An interactive checkout is swept because someone chose to.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from target_budget import marks as marking  # noqa: E402
from target_budget.locks import held_locks, is_cargo_target  # noqa: E402
from target_budget.plan import build  # noqa: E402
from target_budget.reclaim import apply, gib, render  # noqa: E402
from target_budget.survey import survey, total_size  # noqa: E402

SIZE = re.compile(r"^(?P<n>\d+(?:\.\d+)?)\s*(?P<unit>[KMGT]?i?B?)$", re.IGNORECASE)
SCALE = {"": 1, "B": 1, "KIB": 1024, "MIB": 1024**2, "GIB": 1024**3, "TIB": 1024**4}


def parse_size(text: str) -> int:
    match = SIZE.match(text.strip())
    if not match:
        raise argparse.ArgumentTypeError(f"not a size: {text!r} (try 12GiB)")
    unit = match.group("unit").upper()
    if unit and unit.endswith("B") and unit not in SCALE:
        unit = unit[:-1] + "IB"
    if unit not in SCALE:
        raise argparse.ArgumentTypeError(f"unknown unit in {text!r}")
    return int(float(match.group("n")) * SCALE[unit])


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--target", type=Path, default=None)
    parser.add_argument("--mark", action="store_true",
                        help="record the live unit set for the cargo args after --")
    parser.add_argument("--forget", action="store_true", help="discard every recorded mark")
    parser.add_argument("--budget", type=parse_size, default=None,
                        help="report against a ceiling, e.g. 12GiB")
    parser.add_argument("--keep-incremental", action="store_true",
                        help="retain rustc incremental scratch older than the mark")
    parser.add_argument("--apply", action="store_true", help="remove; default is a dry run")
    parser.add_argument("cargo_args", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)

    repo = args.repo.resolve()
    target = (args.target or repo / "target").resolve()
    if not target.is_dir():
        print(f"no such directory: {target}", file=sys.stderr)
        return 2
    if not is_cargo_target(target):
        print(f"not a cargo target directory (no CACHEDIR.TAG): {target}", file=sys.stderr)
        return 2

    if args.forget:
        marking.forget(target)
        print("marks discarded")
        return 0

    if args.mark:
        cargo_args = [a for a in args.cargo_args if a != "--"]
        if not cargo_args:
            print("--mark needs cargo arguments, e.g. --mark -- build --workspace --tests",
                  file=sys.stderr)
            return 2
        recorded, code, detail = marking.record(target, cargo_args, repo)
        if code != 0:
            print(f"cargo exited {code}; mark unchanged. {detail}", file=sys.stderr)
            return code
        print(f"marked {len(recorded.hashes)} live units across "
              f"{len(recorded.configurations)} configuration(s)")
        return 0

    held = held_locks(target)
    if held:
        for lock in held:
            print(f"build in progress, holding {lock}", file=sys.stderr)
        print("refusing to remove anything", file=sys.stderr)
        return 3

    recorded = marking.load(target)
    if not recorded.known:
        print("no configuration marked yet; run --mark before sweeping", file=sys.stderr)
        return 2

    before = total_size(target)
    print(f"target: {target}")
    print(f"before: {gib(before)}")
    for configuration in recorded.configurations:
        print(f"  marked: cargo {configuration}")

    profiles = survey(target)
    for profile in profiles:
        note = f", {profile.unattributed} unattributed (kept)" if profile.unattributed else ""
        print(f"  {profile.root.name}: {len(profile.generations)} generations, "
              f"{len(profile.incremental)} incremental sessions{note}")

    plan = build(profiles, recorded, budget=args.budget,
                 keep_incremental=args.keep_incremental)
    outcomes = apply(plan, target, dry_run=not args.apply)
    print(render(plan, outcomes, dry_run=not args.apply))

    if args.apply:
        after = total_size(target)
        print(f"after:  {gib(after)}  (freed {gib(before - after)})")

    return 1 if any(o.error for o in outcomes) else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
