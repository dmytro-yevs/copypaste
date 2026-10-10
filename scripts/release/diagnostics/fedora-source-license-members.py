#!/usr/bin/env python3
"""Report notice-bearing members from exact Fedora source RPMs without extraction."""

from __future__ import annotations

import argparse
import importlib.util
import io
import json
import re
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from collections import deque
from pathlib import Path
from typing import Any


MAX_NESTED_ARCHIVES = 64
MAX_NESTED_DEPTH = 4
NOTICE_MEMBER = re.compile(
    r"(?:^|/)(?:LICENSES?(?:[._-].*)?|LICENCES?(?:[._-].*)?|COPYING(?:[._-].*)?|"
    r"NOTICE(?:[._-].*)?|COPYRIGHT(?:[._-].*)?|README(?:[._-].*)?)$",
    re.IGNORECASE,
)


def closure_module() -> Any:
    root = Path(__file__).resolve().parents[3]
    spec = importlib.util.spec_from_file_location(
        "private_elf_closure", root / "packaging/linux/compositor-runtime/private_elf_closure.py"
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load compositor closure helper")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def command(argv: list[str]) -> str:
    completed = subprocess.run(argv, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
    if completed.returncode:
        raise RuntimeError(f"command failed: {Path(argv[0]).name}: {completed.stdout[:512].replace(chr(10), ' ')}")
    return completed.stdout


def source_archive(module: Any, package: str, cache: Path) -> tuple[tuple[str, str, str, str], Path]:
    owner = module.rpm_provenance(["-q", package], package)
    name, evr, source_rpm, _license = owner
    destination = cache / source_rpm
    destination.mkdir()
    command(["dnf", "-q", "download", "--source", "--destdir", str(destination), source_rpm.removesuffix(".src.rpm")])
    archives = [candidate for candidate in destination.iterdir() if candidate.is_file() and candidate.name == source_rpm]
    if len(archives) != 1:
        raise RuntimeError(f"exact source RPM is unavailable for {name}")
    archive = archives[0]
    signature = command(["rpm", "--checksig", "--verbose", str(archive)])
    if re.search(r"Signature.*: OK", signature, re.IGNORECASE) is None:
        raise RuntimeError(f"exact source RPM signature is invalid for {name}")
    header = command(["rpm", "-qp", "--qf", "%{NAME}\\t%{EPOCHNUM}\\t%{VERSION}\\t%{RELEASE}\\t%{SOURCEPACKAGE}\\n", str(archive)]).splitlines()
    if len(header) != 1 or len(header[0].split("\t")) != 5:
        raise RuntimeError(f"exact source RPM header is malformed for {name}")
    source_name, epoch, version, release, source_package = header[0].split("\t")
    source_evr = f"{epoch}:{version}-{release}" if epoch not in {"", "0", "(none)"} else f"{version}-{release}"
    if source_package != "1" or source_evr != evr or archive.name != f"{source_name}-{version}-{release}.src.rpm":
        raise RuntimeError(f"exact source RPM provenance differs for {name}")
    return owner, archive


def archive_members(raw: bytes) -> tuple[str, list[tuple[str, bool, int, bytes]]]:
    try:
        with tarfile.open(fileobj=io.BytesIO(raw), mode="r:*") as archive:
            result = []
            for member in archive.getmembers():
                payload = b""
                if member.isfile() and member.size <= 64 * 1024 * 1024:
                    handle = archive.extractfile(member)
                    if handle is not None:
                        payload = handle.read(64 * 1024 * 1024 + 1)
                result.append((member.name, member.isfile(), member.size, payload))
            return "tar", result
    except (tarfile.TarError, OSError):
        pass
    try:
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            result = []
            for member in archive.infolist():
                regular = not member.is_dir() and member.file_size <= 64 * 1024 * 1024
                payload = archive.read(member) if regular else b""
                result.append((member.filename, regular, member.file_size, payload))
            return "zip", result
    except (zipfile.BadZipFile, OSError):
        raise ValueError("not an archive")


def inspect(module: Any, package: str, cache: Path) -> dict[str, object]:
    owner, source = source_archive(module, package, cache)
    payload = module.bounded_rpm2cpio(source)
    listed = subprocess.run(["cpio", "-it"], input=payload, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    verbose = subprocess.run(["cpio", "-itv"], input=payload, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if listed.returncode or verbose.returncode:
        raise RuntimeError(f"cannot list exact source RPM payload for {owner[0]}")
    cpio_members = listed.stdout.decode(errors="strict").splitlines()
    archive_reports = []
    notices = []
    queue: deque[tuple[str, bytes, int]] = deque()
    for member in cpio_members:
        content = module.cpio_member_bytes(source, member)
        if NOTICE_MEMBER.search(member):
            notices.append({"container": "cpio", "path": member, "bytes": len(content)})
        queue.append((member, content, 0))
    inspected = 0
    while queue and inspected < MAX_NESTED_ARCHIVES:
        container, content, depth = queue.popleft()
        try:
            kind, members = archive_members(content)
        except ValueError:
            continue
        inspected += 1
        report = {"container": container, "type": kind, "members": [name for name, _regular, _size, _payload in members]}
        archive_reports.append(report)
        for name, regular, size, nested in members:
            if regular and NOTICE_MEMBER.search(name):
                notices.append({"container": container, "path": name, "bytes": size})
            if regular and depth < MAX_NESTED_DEPTH and nested and len(nested) <= 64 * 1024 * 1024:
                queue.append((f"{container}!{name}", nested, depth + 1))
    return {
        "package": owner[0],
        "evr": owner[1],
        "source_rpm": owner[2],
        "source_sha256": module.sha256(source),
        "cpio_members": cpio_members,
        "cpio_verbose": verbose.stdout.decode(errors="strict").splitlines(),
        "nested_archives": archive_reports,
        "notice_members": notices,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", action="append", required=True)
    args = parser.parse_args()
    module = closure_module()
    with tempfile.TemporaryDirectory(prefix="fedora-source-license-members-") as temporary:
        rows = [inspect(module, package, Path(temporary)) for package in args.package]
    print(json.dumps(rows, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
