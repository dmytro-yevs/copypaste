#!/usr/bin/env python3
"""Audit whether a Fedora RPM's declared license bytes were excluded locally."""

from __future__ import annotations

import argparse
import hashlib
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


def sha256(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def archive_bytes(archive: Path, path: str) -> bytes:
    rpm = subprocess.run(["rpm2cpio", str(archive)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    cpio = subprocess.run(["cpio", "--quiet", "-i", "--to-stdout", "." + path], input=rpm.stdout, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if rpm.returncode or cpio.returncode:
        raise ValueError(f"cannot extract {path} from exact RPM")
    return cpio.stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", required=True)
    parser.add_argument("--restore", action="store_true")
    args = parser.parse_args()
    identity = run(["rpm", "-q", "--qf", "%{NAME}\t%{EVR}\t%{SOURCERPM}\n", args.package])
    if identity["exit"] != 0 or len(identity["output"]) != 1:
        print(json.dumps({"identity": identity}, sort_keys=True))
        return 1
    name, evr, source_rpm = identity["output"][0].split("\t")
    architecture = run(["rpm", "-q", "--qf", "%{ARCH}", name])
    if architecture["exit"] != 0 or len(architecture["output"]) != 1:
        raise ValueError("installed RPM architecture is unavailable")
    nevra = f"{name}-{evr}.{architecture['output'][0]}"
    installed = file_records(["rpm", "-ql", "--qf", "[%{FILENAMES}\t%{FILEFLAGS:fflags}\n]", name])
    with tempfile.TemporaryDirectory(prefix="copypaste-rpm-license-audit-") as temporary:
        downloaded = run(["dnf", "-q", "download", "--destdir", temporary, f"{name}-{evr}"])
        packages = sorted(Path(temporary).glob("*.rpm"))
        archive = file_records(["rpm", "-qpl", "--qf", "[%{FILENAMES}\t%{FILEFLAGS:fflags}\n]", str(packages[0])]) if len(packages) == 1 else []
        archive_paths = run(["rpm", "-qpl", str(packages[0])]) if len(packages) == 1 else {"exit": 1, "output": []}
        license_paths = [path for path in archive_paths["output"] if path.startswith("/usr/share/licenses/") or path.startswith("/usr/share/doc/")]
        restoration: dict[str, object] = {"attempted": False}
        if args.restore and len(packages) == 1 and license_paths:
            archive_path = packages[0]
            restoration = {
                "attempted": True,
                "archive_sha256": sha256(archive_path),
                "reinstall": run(["dnf", "-y", "reinstall", "--setopt=tsflags=", nevra]),
            }
            restored_identity = run(["rpm", "-q", "--qf", "%{NAME}\t%{EVR}\t%{SOURCERPM}\n", name])
            byte_records = []
            for path in license_paths:
                filesystem = Path(path)
                if filesystem.is_file():
                    expected = hashlib.sha256(archive_bytes(archive_path, path)).hexdigest()
                    byte_records.append({"path": path, "archive_sha256": expected, "filesystem_sha256": sha256(filesystem), "matches": expected == sha256(filesystem)})
                else:
                    byte_records.append({"path": path, "missing": True})
            restoration.update({"installed_identity": restored_identity, "license_bytes": byte_records})
    config = run(["dnf", "-q", "config-manager", "--dump"])
    print(json.dumps({
        "name": name,
        "evr": evr,
        "nevra": nevra,
        "source_rpm": source_rpm,
        "rpm_excludedocs": run(["rpm", "--eval", "%{_excludedocs}"]),
        "dnf_tsflags": [line for line in config["output"] if "tsflags" in line or "excludedocs" in line],
        "installed_records": installed,
        "download": downloaded,
        "archive_records": archive,
        "archive_license_paths": license_paths,
        "restoration": restoration,
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
