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


def host_dependencies(receipt: dict, *, rpm: bool) -> str:
    result = []
    for requirement in receipt["host_requirements"]:
        if "operator" not in requirement:
            result.append(requirement["name"])
        elif rpm:
            result.append(f"{requirement['name']} {requirement['operator']} {requirement['version']}")
        else:
            result.append(f"{requirement['name']} ({requirement['operator']} {requirement['version']})")
    return ", ".join(result)


def rpm_requires(receipt: dict) -> str:
    rows = [f"Requires: glibc >= {receipt['glibc_floor']}"]
    rows.extend(f"Requires: {requirement}" for requirement in host_dependencies(receipt, rpm=True).split(", ") if requirement)
    return "\n".join(rows)


def rpm_spec(release_version: str, receipt: dict) -> str:
    """Render an RPM spec that treats the staged runtime as immutable payload."""
    runtime_id = receipt["runtime_id"]
    private_root = "/usr/lib/copypaste/compositor-runtime"
    launcher = f"{private_root}/bin/copypaste-compositor-session-{runtime_id}"
    architecture = {"x86_64": "x86_64", "aarch64": "aarch64"}[receipt["architecture"]]
    return (
        f"Name: {package_name(runtime_id)}\nVersion: {release_version}\nRelease: 1%{{?dist}}\n"
        "Summary: Opt-in CopyPaste compositor clipboard runtime\nLicense: GPL-2.0-or-later\n"
        f"BuildArch: {architecture}\n"
        # The staged runtime is receipt-bound byte-for-byte.  Its ELF files are
        # private implementation details, so RPM must neither rewrite them nor
        # publish their SONAMEs as host package-manager capabilities.
        f"%global __requires_exclude_from ^{private_root}/.*$\n"
        f"%global __provides_exclude_from ^{private_root}/.*$\n"
        "%global __brp_strip %{nil}\n"
        "%global __brp_strip_comment_note %{nil}\n"
        "%global __brp_strip_lto %{nil}\n"
        "%global __brp_strip_static_archive %{nil}\n"
        f"{rpm_requires(receipt)}\n\n"
        "%description\nPrivate compositor runtime exposed only as a separate login session.\n\n"
        "%install\ncp -a %{_source_stage}/. %{buildroot}/\n\n%files\n"
        f"{private_root}/{runtime_id}\n"
        f"{launcher}\n"
        f"/usr/share/copypaste/compositor-runtime/{runtime_id}.receipt.json\n"
        f"/usr/share/wayland-sessions/copypaste-{runtime_id}.desktop\n"
    )


def rpm_desktop_safety(*, requires: str, provides: str, obsoletes: str, conflicts: str, allowed_host_requires: set[str] | None = None) -> None:
    """Reject private ELF metadata and host compositor replacement constraints."""
    allowed_host_requires = allowed_host_requires or set()
    for label, values in (("Provides", provides), ("Requires", requires)):
        for item in (line.strip() for line in values.splitlines() if line.strip()):
            if ".so" in item and ("(64bit)" in item or "(32bit)" in item):
                raise ContractError(f"RPM {label} leaked a private ELF capability: {item}")
    desktop_package = re.compile(r"(^|\s)(?:kdecorations?|kdecorations?[0-9._+-]*|plasma-workspace)(?:\s|[<>=()]|$)", re.IGNORECASE)
    for label, values in (("Requires", requires), ("Obsoletes", obsoletes), ("Conflicts", conflicts)):
        for item in (line.strip() for line in values.splitlines() if line.strip()):
            if label == "Requires" and item in allowed_host_requires:
                continue
            if desktop_package.search(item):
                raise ContractError(f"RPM {label} would constrain a host decoration or Plasma package: {item}")


def build_deb(stage_root: Path, output: Path, release_version: str, receipt: dict) -> None:
    architecture = {"x86_64": "amd64", "aarch64": "arm64"}[receipt["architecture"]]
    dependencies = host_dependencies(receipt, rpm=False)
    control = stage_root / "DEBIAN/control"
    control.parent.mkdir(parents=True, exist_ok=True)
    header = (
        f"Package: {package_name(receipt['runtime_id'])}\n"
        f"Version: {release_version}\n"
        "Section: utils\nPriority: optional\n"
        f"Architecture: {architecture}\n"
        "Maintainer: CopyPaste <support@copypaste.app>\n"
    )
    control.write_text(
        header + (f"Depends: {dependencies}\n" if dependencies else "")
        + "Description: Opt-in CopyPaste compositor clipboard runtime\n"
        " Private compositor runtime exposed only as a separate login session.\n",
        encoding="utf-8",
    )
    run(["dpkg-deb", "--root-owner-group", "--build", str(stage_root), str(output)])


def build_rpm(stage_root: Path, output: Path, release_version: str, receipt: dict) -> None:
    with tempfile.TemporaryDirectory() as temporary:
        topdir = Path(temporary)
        for name in ("BUILD", "BUILDROOT", "RPMS", "SOURCES", "SPECS", "SRPMS"):
            (topdir / name).mkdir()
        spec = topdir / "SPECS/runtime.spec"
        spec.write_text(rpm_spec(release_version, receipt), encoding="utf-8")
        run(["rpmbuild", "-bb", str(spec), "--define", f"_topdir {topdir}", "--define", f"_source_stage {stage_root}", "--define", "_build_id_links none"])
        outputs = list((topdir / "RPMS").rglob("*.rpm"))
        if len(outputs) != 1:
            raise ContractError("RPM build produced an unexpected output set")
        shutil.move(str(outputs[0]), output)


def verify_packaged_payload(package_format: str, package: Path, runtime_id: str) -> None:
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        if package_format == "deb":
            run(["dpkg-deb", "-x", str(package), str(root)])
        else:
            unpack = subprocess.run(["rpm2cpio", str(package)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
            if unpack.returncode:
                raise ContractError("cannot extract built RPM for payload verification")
            restore = subprocess.run(["cpio", "-idm", "--quiet"], input=unpack.stdout, cwd=root, check=False)
            if restore.returncode:
                raise ContractError("cannot restore built RPM for payload verification")
        verify(root, runtime_id)


def verify_rpm_desktop_safety(package: Path, receipt: dict) -> None:
    def query(flag: str) -> str:
        completed = subprocess.run(["rpm", "-qp", flag, str(package)], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
        if completed.returncode:
            raise ContractError(f"cannot inspect built RPM {flag}")
        return completed.stdout
    rpm_desktop_safety(
        requires=query("--requires"), provides=query("--provides"), obsoletes=query("--obsoletes"), conflicts=query("--conflicts"),
        allowed_host_requires=set(host_dependencies(receipt, rpm=True).split(", ")) - {""},
    )


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
                verify_rpm_desktop_safety(args.output, receipt)
            verify_packaged_payload(args.format, args.output, receipt["runtime_id"])
    except (ContractError, OSError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"built {args.format} sidecar compositor companion: {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
