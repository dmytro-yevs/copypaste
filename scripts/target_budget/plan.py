"""Choose which generations to evict, and state what the sweep costs.

A generation is evicted only when cargo did not name it for any marked
configuration *and* it predates the newest mark. The second condition is the
fail-closed half: output produced after we last asked cargo is work in progress
we have no knowledge of, and unknown means keep.

The cost is therefore exact rather than estimated. Zero rebuild for every
configuration that has been marked; a full rebuild of one that never was.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path

from .marks import Marks
from .survey import Generation, Profile

UNUSED = "not used by any marked configuration"
STALE_SCRATCH = "incremental scratch older than the mark"
SCRATCH = "incremental scratch, over budget"


@dataclass(frozen=True)
class Eviction:
    profile: Path
    stem: str
    key: str
    paths: tuple[Path, ...]
    size: int
    reason: str


@dataclass
class Plan:
    budget: int | None = None
    evictions: list[Eviction] = field(default_factory=list)
    kept_bytes: int = 0
    newer_than_mark: int = 0
    shortfall: int = 0

    @property
    def freed(self) -> int:
        return sum(e.size for e in self.evictions)


def build(
    profiles: list[Profile],
    marks: Marks,
    budget: int | None = None,
    keep_incremental: bool = False,
) -> Plan:
    if not marks.known:
        raise ValueError("no configuration has been marked; run --mark before sweeping")

    plan = Plan(budget=budget)
    scratch: list[tuple[Profile, Generation]] = []

    for profile in profiles:
        for gen in profile.generations.values():
            if gen.key in marks.hashes:
                plan.kept_bytes += gen.size
            elif gen.mtime > marks.recorded_at:
                plan.newer_than_mark += 1
                plan.kept_bytes += gen.size
            else:
                plan.evictions.append(_evict(profile, gen, UNUSED))

        # `incremental/` is keyed by session id, so no mark can name it and the
        # unmarked-generation rule cannot reach it. It is also the largest area
        # measured — 15.82 GiB of 46.13 GiB — so leaving it alone means the
        # sweep does not bound the tree. A session the marked build did not
        # write to is therefore dropped; rustc scratch cannot make a build
        # wrong. Six alternating waves showed no separable build-time cost
        # (dropped 10.6/17.1/18.8 s against kept 18.5/10.4/10.6 s) for a
        # consistent 0.97 GiB, but that is one workload — `--keep-incremental`
        # exists for anyone who measures otherwise.
        for gen in profile.incremental.values():
            if keep_incremental or gen.mtime > marks.recorded_at:
                plan.kept_bytes += gen.size
                scratch.append((profile, gen))
            else:
                plan.evictions.append(_evict(profile, gen, STALE_SCRATCH))

    if budget is None or plan.kept_bytes <= budget:
        return plan

    # Still over, so the scratch that --keep-incremental spared goes too.
    # Nothing beyond this is evicted automatically: the next cheapest thing is a
    # live artifact, and removing one buys bytes by forcing the rebuild this
    # tool exists to avoid.
    for profile, gen in sorted(scratch, key=lambda pair: pair[1].mtime):
        if plan.kept_bytes <= budget:
            break
        plan.evictions.append(_evict(profile, gen, SCRATCH))
        plan.kept_bytes -= gen.size

    plan.shortfall = max(0, plan.kept_bytes - budget)
    return plan


def _evict(profile: Profile, gen: Generation, reason: str) -> Eviction:
    return Eviction(
        profile=profile.root,
        stem=gen.stem,
        key=gen.key,
        paths=tuple(gen.paths),
        size=gen.size,
        reason=reason,
    )
