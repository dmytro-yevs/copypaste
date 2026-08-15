"""Group a cargo target directory into per-unit artifact generations.

Cargo stamps every compilation unit with a 16-hex metadata hash and writes one
`.fingerprint/<pkg>-<hash>/` directory for it; `deps/` and `build/` reuse that
hash. A hash `.fingerprint/` does not name is left alone rather than guessed at.

Do not group by package name. In one fresh build `copypaste-ipc` has four live
hashes — the lib, its test binary, and the `wire_roundtrip` and
`method_contract` integration tests — and `getrandom` has eleven across the host
and target graphs. Every one is current. Keeping "the newest N per package"
therefore deletes current output and forces the rebuild this tool exists to
avoid; the unit hash is the only identity that separates them.

Profile directories are found by looking for a `.fingerprint/` child, not by
name: `target/<triple>/debug/`, `target/evidence/` and `target/macos-typecheck/`
are profile directories too.
"""

from __future__ import annotations

import os
import re
from dataclasses import dataclass, field
from pathlib import Path

HASH = re.compile(r"^(?P<stem>.+)-(?P<hash>[0-9a-f]{16})(?P<ext>\..*)?$")

# `incremental/` is keyed by a base36 session id, not a unit hash, so cargo's
# JSON never names it and it is tracked separately. The id is not fixed-width —
# observed ids are 13 characters — so this splits on the last dash rather than
# asserting a length, which silently classified every session as unattributed.
SESSION = re.compile(r"^(?P<stem>.+)-(?P<session>[0-9a-z]+)$")

AREAS = ("deps", "build", ".fingerprint")


@dataclass
class Generation:
    stem: str
    key: str
    paths: list[Path] = field(default_factory=list)
    size: int = 0
    mtime: float = 0.0


@dataclass
class Profile:
    root: Path
    generations: dict[str, Generation] = field(default_factory=dict)
    incremental: dict[str, Generation] = field(default_factory=dict)
    unattributed: int = 0


def profile_dirs(target: Path) -> list[Path]:
    found = []
    for dirpath, dirnames, _ in os.walk(target):
        if ".fingerprint" in dirnames:
            found.append(Path(dirpath))
            dirnames[:] = [d for d in dirnames if d not in AREAS and d != "incremental"]
    return sorted(found)


def _measure(path: Path) -> tuple[int, float]:
    """Total bytes and newest mtime under `path`. Metadata only.

    Reading file *content* would update atime across the tree, which is how an
    access-time-based selector loses the signal it selects on.
    """
    if path.is_file():
        st = path.lstat()
        return st.st_size, st.st_mtime
    size = 0
    newest = 0.0
    for dirpath, _, filenames in os.walk(path):
        for name in filenames:
            try:
                st = (Path(dirpath) / name).lstat()
            except OSError:
                continue
            size += st.st_size
            newest = max(newest, st.st_mtime)
    return size, newest


def _record(bucket: dict[str, Generation], stem: str, key: str, path: Path) -> None:
    gen = bucket.setdefault(key, Generation(stem=stem, key=key))
    size, mtime = _measure(path)
    gen.paths.append(path)
    gen.size += size
    gen.mtime = max(gen.mtime, mtime)


def survey(target: Path) -> list[Profile]:
    return [_survey_profile(root) for root in profile_dirs(target)]


def _survey_profile(root: Path) -> Profile:
    profile = Profile(root=root)

    known: dict[str, str] = {}
    fingerprints = root / ".fingerprint"
    if fingerprints.is_dir():
        for entry in fingerprints.iterdir():
            match = HASH.match(entry.name)
            if match and not match.group("ext"):
                known[match.group("hash")] = match.group("stem")

    for area in AREAS:
        directory = root / area
        if not directory.is_dir():
            continue
        for entry in directory.iterdir():
            match = HASH.match(entry.name)
            stem = known.get(match.group("hash")) if match else None
            if stem is None:
                profile.unattributed += 1
                continue
            _record(profile.generations, stem, match.group("hash"), entry)

    incremental = root / "incremental"
    if incremental.is_dir():
        for entry in incremental.iterdir():
            match = SESSION.match(entry.name)
            if not match:
                profile.unattributed += 1
                continue
            _record(profile.incremental, match.group("stem"), match.group("session"), entry)

    return profile


def total_size(target: Path) -> int:
    return _measure(target)[0]
