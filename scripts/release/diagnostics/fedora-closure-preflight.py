#!/usr/bin/env python3
"""Mirror closure resolution and report the first RPM-provenance failure."""

from __future__ import annotations

import argparse
import importlib.util
import json
import subprocess
import tempfile
from collections import deque
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]


def closure_module():
    spec = importlib.util.spec_from_file_location(
        "copypaste_private_elf_closure",
        ROOT / "packaging/linux/compositor-runtime/private_elf_closure.py",
    )
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def raw_rpm_fields(path: Path) -> dict[str, object]:
    completed = subprocess.run(
        ["rpm", "-qf", "--qf", "%{NAME}\t%{EVR}\t%{SOURCERPM}\t%{LICENSE}\n", str(path)],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    return {"exit": completed.returncode, "fields": completed.stdout.splitlines()}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime", required=True, type=Path)
    args = parser.parse_args()
    runtime = args.runtime.resolve(strict=True)
    closure = closure_module()
    initial = closure.exported_elfs(runtime)
    private = {value for path in initial if (value := closure.provided_soname(path)) is not None}
    cache = closure.library_cache()
    queue = deque(initial)
    seen = set()
    host_libraries = 0
    failures = []
    with tempfile.TemporaryDirectory(prefix="copypaste-compositor-preflight-") as temporary:
      source_cache = Path(temporary)
      while queue:
        source = queue.popleft()
        if source in seen:
            continue
        seen.add(source)
        for dependency in sorted(closure.needed(source)):
            if dependency in closure.GLIBC_SONAMES or dependency in private:
                continue
            resolved = closure.resolve_dependency(source, dependency, cache)
            if resolved is None:
                failures.append({"failure": "unresolved", "introducer": str(source), "soname": dependency})
                continue
            if resolved not in seen:
                try:
                    owner = closure.rpm_owner(resolved)
                    if not closure.rpm_installed_license_files(owner):
                        closure.source_rpm_license_files(owner, source_cache)
                except closure.ClosureError as error:
                    failures.append({
                        "failure": str(error),
                        "library": str(resolved),
                        "rpm": raw_rpm_fields(resolved),
                    })
                host_libraries += 1
                queue.append(resolved)
    print(json.dumps({"failures": failures, "host_libraries": host_libraries, "status": "failed" if failures else "ok"}, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
