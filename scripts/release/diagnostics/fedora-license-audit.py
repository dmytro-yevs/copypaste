#!/usr/bin/env python3
"""Audit whether a Fedora RPM's declared license bytes were excluded locally."""

from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path


def run(arguments: list[str]) -> dict[str, object]:
    completed = subprocess.run(arguments, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
    return {"exit": completed.returncode, "output": completed.stdout.splitlines()}


def file_records(arguments: list[str]) -> list[dict[str, object]]:
    result = run(arguments)
    if result["exit"] != 0:
        return []
    records = []
    for row in result["output"]:
        fields = row.split("\t", 1)
        if len(fields) != 2:
            continue
        path, flags = fields
        if not (path.startswith("/usr/share/licenses/") or path.startswith("/usr/share/doc/") or "d" in flags or "l" in flags):
            continue
        records.append({"path": path, "flags": flags, "exists": Path(path).is_file()})
    return records


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", required=True)
    args = parser.parse_args()
    identity = run(["rpm", "-q", "--qf", "%{NAME}\t%{EVR}\t%{SOURCERPM}\n", args.package])
    if identity["exit"] != 0 or len(identity["output"]) != 1:
        print(json.dumps({"identity": identity}, sort_keys=True))
        return 1
    name, evr, source_rpm = identity["output"][0].split("\t")
    installed = file_records(["rpm", "-ql", "--qf", "[%{FILENAMES}\t%{FILEFLAGS:fflags}\n]", name])
    with tempfile.TemporaryDirectory(prefix="copypaste-rpm-license-audit-") as temporary:
        downloaded = run(["dnf", "-q", "download", "--destdir", temporary, f"{name}-{evr}"])
        packages = sorted(Path(temporary).glob("*.rpm"))
        archive = file_records(["rpm", "-qpl", "--qf", "[%{FILENAMES}\t%{FILEFLAGS:fflags}\n]", str(packages[0])]) if len(packages) == 1 else []
        archive_paths = run(["rpm", "-qpl", str(packages[0])]) if len(packages) == 1 else {"exit": 1, "output": []}
    config = run(["dnf", "-q", "config-manager", "--dump"])
    print(json.dumps({
        "name": name,
        "evr": evr,
        "source_rpm": source_rpm,
        "rpm_excludedocs": run(["rpm", "--eval", "%{_excludedocs}"]),
        "dnf_tsflags": [line for line in config["output"] if "tsflags" in line or "excludedocs" in line],
        "installed_records": installed,
        "download": downloaded,
        "archive_records": archive,
        "archive_license_paths": [path for path in archive_paths["output"] if path.startswith("/usr/share/licenses/") or path.startswith("/usr/share/doc/")],
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
