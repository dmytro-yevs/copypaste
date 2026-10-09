#!/usr/bin/env python3
"""Record the highest glibc symbol version required by a staged Linux bundle."""

import argparse
import json
from pathlib import Path
import re
import subprocess


def version_key(value: str) -> tuple[int, ...]:
    return tuple(int(part) for part in value.split("."))


def elf_files(root: Path) -> list[Path]:
    return [
        path for path in root.rglob("*")
        if path.is_file() and not path.is_symlink()
        and subprocess.run(["file", "--brief", str(path)], text=True, capture_output=True).stdout.startswith("ELF")
    ]


def highest_glibc(root: Path) -> tuple[str, list[str]]:
    required: set[str] = set()
    files = elf_files(root)
    if not files:
        raise ValueError("Linux bundle contains no ELF files")
    for path in files:
        output = subprocess.run(
            ["readelf", "--version-info", str(path)], check=True, text=True, capture_output=True
        ).stdout
        required.update(re.findall(r"GLIBC_([0-9]+(?:\.[0-9]+)+)", output))
    if not required:
        raise ValueError("Linux bundle has no GLIBC symbol requirements")
    return max(required, key=version_key), [str(path.relative_to(root)) for path in files]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle", required=True, type=Path)
    parser.add_argument("--architecture", required=True, choices=("x86_64", "aarch64"))
    parser.add_argument("--maximum", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    glibc, files = highest_glibc(args.bundle)
    if version_key(glibc) > version_key(args.maximum):
        raise ValueError(f"bundle requires glibc {glibc}, exceeding supported {args.maximum}")
    args.output.write_text(json.dumps({
        "schema": 1,
        "architecture": args.architecture,
        "glibc_minimum": glibc,
        "files": files,
    }, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
