"""Record which unit hashes a configuration actually uses, from cargo itself.

`cargo build --message-format=json` reports every unit it considered, fresh
ones included, so a no-op build enumerates the live set for the price of a
fingerprint check. On this workspace that is exact: 696 `compiler-artifact`
plus 87 `build-script-executed` messages against 783 generations on disk, with
the only two unmatched being genuinely stale output from the previous build.

This replaces access time, which cargo-sweep uses and which does not survive
here — 14,898 of 15,106 files under `target/debug/deps` shared one atime minute
because something read the tree's contents.

Marks accumulate. Sweeping a checkout that builds several configurations means
marking each one; a configuration never marked is not protected.
"""

from __future__ import annotations

import json
import re
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

MARK_FILE = ".copypaste-target-budget.json"
HASH = re.compile(r"-([0-9a-f]{16})(?:[./\\]|$)")


@dataclass(frozen=True)
class Marks:
    hashes: frozenset[str]
    recorded_at: float
    configurations: tuple[str, ...]

    @property
    def known(self) -> bool:
        return bool(self.configurations)


def load(target: Path) -> Marks:
    try:
        raw = json.loads((target / MARK_FILE).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return Marks(frozenset(), 0.0, ())
    return Marks(
        hashes=frozenset(raw.get("hashes", [])),
        recorded_at=float(raw.get("recorded_at", 0.0)),
        configurations=tuple(raw.get("configurations", [])),
    )


def record(target: Path, cargo_args: list[str], repo: Path) -> tuple[Marks, int, str]:
    """Run cargo for one configuration and union its live hashes into the mark."""
    argv = ["cargo", *cargo_args, "--message-format=json"]
    done = subprocess.run(argv, cwd=repo, capture_output=True, text=True, check=False)
    if done.returncode != 0:
        complaint = done.stderr.strip().splitlines()
        return load(target), done.returncode, complaint[-1] if complaint else ""

    live = _hashes(done.stdout)
    previous = load(target)
    configuration = " ".join(cargo_args)
    merged = Marks(
        hashes=previous.hashes | live,
        recorded_at=time.time(),
        configurations=tuple(dict.fromkeys((*previous.configurations, configuration))),
    )
    _store(target, merged)
    return merged, 0, ""


def forget(target: Path) -> None:
    (target / MARK_FILE).unlink(missing_ok=True)


def _hashes(stdout: str) -> frozenset[str]:
    found: set[str] = set()
    for line in stdout.splitlines():
        try:
            message = json.loads(line)
        except ValueError:
            continue
        reason = message.get("reason")
        if reason == "compiler-artifact":
            for name in message.get("filenames") or []:
                found.update(HASH.findall(name))
            if message.get("executable"):
                found.update(HASH.findall(message["executable"]))
        elif reason == "build-script-executed":
            if message.get("out_dir"):
                found.update(HASH.findall(message["out_dir"]))
    return frozenset(found)


def _store(target: Path, marks: Marks) -> None:
    (target / MARK_FILE).write_text(
        json.dumps(
            {
                "hashes": sorted(marks.hashes),
                "recorded_at": marks.recorded_at,
                "configurations": list(marks.configurations),
            },
            indent=1,
        ),
        encoding="utf-8",
    )
