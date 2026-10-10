#!/usr/bin/env python3
"""Record bounded Fedora evidence for one unresolved ELF SONAME."""

from __future__ import annotations

import argparse
import importlib.util
import json
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
TAG = re.compile(r"\((?:NEEDED|RPATH|RUNPATH)\).*\[([^]]+)\]")


def closure_module():
    spec = importlib.util.spec_from_file_location(
        "copypaste_private_elf_closure",
        ROOT / "packaging/linux/compositor-runtime/private_elf_closure.py",
    )
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def output(arguments: list[str]) -> dict[str, object]:
    completed = subprocess.run(arguments, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
    return {"exit": completed.returncode, "output": completed.stdout.splitlines()}


def repository_locations(provider: dict[str, object], soname: str) -> list[dict[str, object]]:
    locations = []
    for row in provider["output"]:
        fields = row.split("\t")
        if len(fields) != 3:
            continue
        name, evr, source_rpm = fields
        listed = output(["dnf", "repoquery", "--list", f"{name}-{evr}"])
        locations.append({
            "name": name,
            "evr": evr,
            "source_rpm": source_rpm,
            "paths": [path for path in listed["output"] if Path(path).name == soname],
            "list_exit": listed["exit"],
        })
    return locations


def dynamic_tags(path: Path) -> list[str]:
    completed = subprocess.run(["readelf", "-d", str(path)], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
    if completed.returncode:
        raise ValueError(f"cannot read ELF dynamic tags: {path}")
    return [line.strip() for line in completed.stdout.splitlines() if TAG.search(line)]


def resolved_rpath_directories(path: Path, tags: list[str], runtime: Path) -> list[str]:
    directories = []
    for tag in tags:
        if "RPATH" not in tag and "RUNPATH" not in tag:
            continue
        value = TAG.search(tag)
        assert value is not None
        for item in value.group(1).split(":"):
            candidate = Path(item.replace("$ORIGIN", str(path.parent)))
            if candidate.is_absolute() and candidate.is_dir():
                try:
                    candidate.relative_to(runtime)
                    directories.append(str(candidate))
                except ValueError:
                    pass
    return sorted(set(directories))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime", required=True, type=Path)
    parser.add_argument("--soname", required=True)
    args = parser.parse_args()
    runtime = args.runtime.resolve(strict=True)
    closure = closure_module()
    initial = closure.exported_elfs(runtime)
    private_sonames = {value for path in initial if (value := closure.provided_soname(path)) is not None}
    cache = closure.library_cache()
    queue = [(path, "seed") for path in initial]
    seen = set()
    introducers = []
    while queue:
        path, origin = queue.pop()
        if path in seen:
            continue
        seen.add(path)
        tags = dynamic_tags(path)
        for dependency in closure.needed(path):
            if dependency == args.soname:
                introducers.append({
                    "path": path.relative_to(runtime).as_posix() if path.is_relative_to(runtime) else str(path),
                    "origin": origin,
                    "dynamic_tags": tags,
                    "private_rpath_directories": resolved_rpath_directories(path, tags, runtime),
                })
            if dependency in closure.GLIBC_SONAMES or dependency in private_sonames:
                continue
            candidate = cache.get(dependency)
            if candidate is not None:
                queue.append((closure.trusted_library(candidate), "host"))
    provides = f"{args.soname}()(64bit)"
    repository_provider = output(["dnf", "-q", "repoquery", "--qf", "%{name}\t%{evr}\t%{sourcerpm}", "--whatprovides", provides])
    print(json.dumps({
        "soname": args.soname,
        "ldconfig_present": args.soname in closure.library_cache(),
        "introducers": introducers,
        "installed_provider": output(["rpm", "-q", "--whatprovides", provides]),
        "repository_provider": repository_provider,
        "repository_locations": repository_locations(repository_provider, args.soname),
        "repository_file_provider": output(["dnf", "-q", "provides", f"*/{args.soname}"]),
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
