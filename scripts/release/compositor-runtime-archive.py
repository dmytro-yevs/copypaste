#!/usr/bin/env python3
"""Transport an immutable compositor runtime without losing Unix file metadata."""

from __future__ import annotations

import argparse
import os
import posixpath
import shutil
import stat
import tarfile
from pathlib import Path


MAX_MEMBERS = 4096
MAX_BYTES = 2 * 1024 * 1024 * 1024


def safe_name(value: str) -> str:
    if not value or value.startswith("/") or "\x00" in value:
        raise ValueError("runtime archive member path is unsafe")
    normalized = posixpath.normpath(value)
    if normalized in (".", "..") or normalized.startswith("../"):
        raise ValueError("runtime archive member path escapes its root")
    return normalized


def safe_link(value: str, member: str) -> str:
    if not value or value.startswith("/") or "\x00" in value:
        raise ValueError("runtime archive symlink is unsafe")
    resolved = posixpath.normpath(posixpath.join(posixpath.dirname(member), value))
    if resolved == ".." or resolved.startswith("../"):
        raise ValueError("runtime archive symlink escapes its root")
    return value


def pack(root: Path, output: Path) -> None:
    resolved = root.resolve(strict=True)
    if root.is_symlink() or not resolved.is_dir() or output.exists() or output.is_symlink():
        raise ValueError("runtime archive input or output is unsafe")
    members = sorted(resolved.rglob("*"))
    if not members or len(members) > MAX_MEMBERS:
        raise ValueError("runtime archive has an invalid member count")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(output, "w") as archive:
        for path in members:
            relative = path.relative_to(resolved).as_posix()
            if path.is_dir() or path.is_file() or path.is_symlink():
                archive.add(path, arcname=relative, recursive=False)
            else:
                raise ValueError("runtime archive input has an unsupported member")


def extract(archive_path: Path, output: Path) -> None:
    if not archive_path.is_file() or archive_path.is_symlink() or output.exists() or output.is_symlink():
        raise ValueError("runtime archive input or output is unsafe")
    with tarfile.open(archive_path, "r") as archive:
        members = archive.getmembers()
        if not members or len(members) > MAX_MEMBERS or sum(member.size for member in members) > MAX_BYTES:
            raise ValueError("runtime archive exceeds bounded transport limits")
        seen = set()
        manifest = {}
        for member in members:
            name = safe_name(member.name)
            if name in seen or not (member.isdir() or member.isfile() or member.issym()):
                raise ValueError("runtime archive member is unsupported or duplicated")
            seen.add(name)
            manifest[name] = member
            if member.issym():
                safe_link(member.linkname, name)
        for name in manifest:
            parent = posixpath.dirname(name)
            while parent and parent != ".":
                if manifest.get(parent) is not None and manifest[parent].issym():
                    raise ValueError("runtime archive member has a symlink ancestor")
                parent = posixpath.dirname(parent)
        output.mkdir(parents=True)
        for member in members:
            target = output / safe_name(member.name)
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
                target.chmod(stat.S_IMODE(member.mode))
            elif member.issym():
                target.parent.mkdir(parents=True, exist_ok=True)
                target.symlink_to(member.linkname)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                source = archive.extractfile(member)
                if source is None:
                    raise ValueError("runtime archive regular member is unreadable")
                with source, target.open("xb") as destination:
                    shutil.copyfileobj(source, destination)
                target.chmod(stat.S_IMODE(member.mode))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for command in (commands.add_parser("pack"), commands.add_parser("extract")):
        command.add_argument("--root" if command.prog.endswith(" pack") else "--archive", required=True, type=Path)
        command.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if args.command == "pack":
        pack(args.root, args.output)
    else:
        extract(args.archive, args.output)


if __name__ == "__main__":
    main()
