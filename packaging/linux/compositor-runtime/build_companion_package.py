#!/usr/bin/env python3
"""Build one signed-repository-ready DEB or RPM compositor companion package."""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from stage_runtime import ContractError, read_receipt, stage, validate_receipt
from verify_runtime_package import verify


VERSION = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")


def run(argv: list[str], **kwargs: object) -> None:
    completed = subprocess.run(argv, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False, **kwargs)
    if completed.returncode:
        raise ContractError(f"packaging command failed: {' '.join(argv)}\n{completed.stdout}")


def package_name(runtime_id: str) -> str:
    return f"copypaste-compositor-runtime-{runtime_id}"


def deb_dependencies(receipt: dict) -> str:
    return ", ".join(f"{row['name']} (= {row['version']})" for row in receipt["package_dependencies"])


def rpm_requires(receipt: dict) -> str:
    rows = [f"Requires: glibc >= {receipt['glibc_floor']}"]
    rows.extend(f"Requires: {row['name']} = {row['version']}" for row in receipt["package_dependencies"])
    return "\n".join(rows)


def build_deb(stage_root: Path, output: Path, release_version: str, receipt: dict) -> None:
    architecture = {"x86_64": "amd64", "aarch64": "arm64"}[receipt["architecture"]]
    control = stage_root / "DEBIAN/control"
    control.parent.mkdir(parents=True, exist_ok=True)
    control.write_text(
        f"Package: {package_name(receipt['runtime_id'])}\n"
        f"Version: {release_version}\n"
        "Section: utils\nPriority: optional\n"
        f"Architecture: {architecture}\n"
        "Maintainer: CopyPaste <support@copypaste.app>\n"
        f"Depends: {deb_dependencies(receipt)}\n"
        "Description: Opt-in CopyPaste compositor clipboard runtime\n"
        " Private compositor runtime exposed only as a separate login session.\n",
        encoding="utf-8",
    )
    run(["dpkg-deb", "--root-owner-group", "--build", str(stage_root), str(output)])


def build_rpm(stage_root: Path, output: Path, release_version: str, receipt: dict) -> None:
    architecture = {"x86_64": "x86_64", "aarch64": "aarch64"}[receipt["architecture"]]
    with tempfile.TemporaryDirectory() as temporary:
        topdir = Path(temporary)
        for name in ("BUILD", "BUILDROOT", "RPMS", "SOURCES", "SPECS", "SRPMS"):
            (topdir / name).mkdir()
        spec = topdir / "SPECS/runtime.spec"
        runtime_id = receipt["runtime_id"]
        spec.write_text(
            f"Name: {package_name(runtime_id)}\nVersion: {release_version}\nRelease: 1%{{?dist}}\n"
            "Summary: Opt-in CopyPaste compositor clipboard runtime\nLicense: MIT OR Apache-2.0\n"
            f"BuildArch: {architecture}\n{rpm_requires(receipt)}\n\n"
            "%description\nPrivate compositor runtime exposed only as a separate login session.\n\n"
            "%install\ncp -a %{_source_stage}/. %{buildroot}/\n\n%files\n"
            f"/usr/lib/copypaste/compositor-runtime/{runtime_id}\n"
            f"/usr/share/copypaste/compositor-runtime/{runtime_id}.receipt.json\n"
            f"/usr/share/wayland-sessions/copypaste-{runtime_id}.desktop\n",
            encoding="utf-8",
        )
        run(["rpmbuild", "-bb", str(spec), "--define", f"_topdir {topdir}", "--define", f"_source_stage {stage_root}", "--define", "_build_id_links none"])
        outputs = list((topdir / "RPMS").rglob("*.rpm"))
        if len(outputs) != 1:
            raise ContractError("RPM build produced an unexpected output set")
        shutil.move(str(outputs[0]), output)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--runtime-dir", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--format", choices=("deb", "rpm"), required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--stage-only", action="store_true")
    args = parser.parse_args()
    if not VERSION.fullmatch(args.version):
        print("ERROR: version must be a stable semantic version", file=sys.stderr)
        return 1
    try:
        receipt = read_receipt(args.receipt)
        validate_receipt(receipt)
        if args.output.exists() or args.output.is_symlink():
            raise ContractError("companion package output already exists")
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory() as temporary:
            stage_root = Path(temporary)
            stage(args.receipt, args.runtime_dir, stage_root, "/usr/lib/copypaste/compositor-runtime", "/usr/share/wayland-sessions", "/usr/lib/copypaste/compositor-runtime/bin")
            verify(stage_root, receipt["runtime_id"])
            if args.stage_only:
                print(f"staged companion package root for {receipt['runtime_id']}")
                return 0
            if os.uname().sysname != "Linux":
                raise ContractError("DEB and RPM companion packaging requires Linux")
            if args.format == "deb":
                build_deb(stage_root, args.output, args.version, receipt)
            else:
                build_rpm(stage_root, args.output, args.version, receipt)
    except (ContractError, OSError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"built {args.format} sidecar compositor companion: {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
