#!/usr/bin/env python3
"""Bundle the Keybinder desktop library required by the portable AppImage.

Keybinder is deliberately kept separate from the camera runtime bundler: it is
a desktop integration dependency, and only its ABI-stable shared object is
portable. GTK, X11, and GLib stay host-provided by the desktop environment.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import stat
import subprocess
import sys
from pathlib import Path


LIBRARY_SONAME = "libkeybinder-3.0.so.0"
PACKAGE = "libkeybinder-3.0-0"
MAX_GLIBC = "2.39"
ARCHITECTURES = {
    "x86_64": "Advanced Micro Devices X86-64",
    "aarch64": "AArch64",
}
SONAME = re.compile(r"\(SONAME\).*\[(?P<soname>[^]]+)\]")
NEEDED = re.compile(r"\(NEEDED\).*\[(?P<soname>[^]]+)\]")
MACHINE = re.compile(r"Machine:\s*(?P<machine>.+)")


class BundleError(RuntimeError):
    pass


def run(argv: list[str]) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(argv, check=True, text=True, capture_output=True)
    except (OSError, subprocess.CalledProcessError) as error:
        raise BundleError(f"required command failed: {Path(argv[0]).name}") from error


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def regular(path: Path) -> Path:
    try:
        details = path.lstat()
    except OSError as error:
        raise BundleError("Keybinder library is unavailable") from error
    if stat.S_ISLNK(details.st_mode) or not stat.S_ISREG(details.st_mode) or details.st_size <= 0:
        raise BundleError("Keybinder library must be a non-empty regular file")
    return path


def trusted_library(path: Path) -> Path:
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise BundleError("Keybinder library is unavailable") from error
    if not resolved.is_file() or not any(str(resolved).startswith(root) for root in ("/lib/", "/usr/lib/")):
        raise BundleError("Keybinder library escapes trusted system library roots")
    return regular(resolved)


def library_from_cache() -> Path:
    candidates = []
    for line in run(["ldconfig", "-p"]).stdout.splitlines():
        if " => " not in line:
            continue
        soname, path = line.strip().split(" => ", 1)
        if soname.split(" ", 1)[0] == LIBRARY_SONAME:
            candidates.append(trusted_library(Path(path)))
    resolved = sorted(set(candidates))
    if len(resolved) != 1:
        raise BundleError("installed Keybinder library is ambiguous or unavailable")
    return resolved[0]


def readelf_dynamic(path: Path) -> str:
    return run(["readelf", "-d", str(path)]).stdout


def verify_soname(path: Path) -> list[str]:
    dynamic = readelf_dynamic(path)
    sonames = SONAME.findall(dynamic)
    if sonames != [LIBRARY_SONAME]:
        raise BundleError("Keybinder library has an unexpected SONAME")
    needed = sorted(set(NEEDED.findall(dynamic)))
    if not needed:
        raise BundleError("Keybinder library is missing its host ABI dependencies")
    return needed


def verify_architecture(path: Path, architecture: str) -> None:
    expected = ARCHITECTURES.get(architecture)
    if expected is None:
        raise BundleError("unsupported Keybinder architecture")
    match = MACHINE.search(run(["readelf", "-h", str(path)]).stdout)
    if match is None or match.group("machine").strip() != expected:
        raise BundleError("Keybinder library architecture does not match the AppImage")


def package_owner(path: Path) -> str:
    output = run(["dpkg-query", "--search", str(path)]).stdout
    owners = {
        line.split(": ", 1)[0].split(":", 1)[0]
        for line in output.splitlines()
        if ": " in line
    }
    if owners != {PACKAGE}:
        raise BundleError("Keybinder library is not owned by the expected Debian package")
    version = run(["dpkg-query", "--showformat=${Version}", "--show", PACKAGE]).stdout.strip()
    if not version:
        raise BundleError("Keybinder package version is unavailable")
    return version


def package_license() -> Path:
    candidates = [Path(item) for item in run(["dpkg-query", "--listfiles", PACKAGE]).stdout.splitlines()]
    source = next((item for item in candidates if item.name == "copyright" and "/usr/share/doc/" in str(item)), None)
    if source is None:
        raise BundleError("Keybinder license bytes are unavailable")
    return regular(source)


def copy_library(source: Path, destination: Path) -> Path:
    destination.mkdir(parents=True, exist_ok=True)
    target = destination / source.name
    link = destination / LIBRARY_SONAME
    if target.exists() or target.is_symlink() or link.exists() or link.is_symlink():
        raise BundleError("Keybinder AppImage destination is not empty")
    shutil.copy2(source, target, follow_symlinks=False)
    target.chmod(0o755)
    if target.name != LIBRARY_SONAME:
        link.symlink_to(target.name)
    return target


def validate_glibc(library: Path, *, architecture: str, output: Path) -> None:
    root = Path(__file__).resolve().parents[2]
    run([
        sys.executable, str(root / "scripts/release/verify-linux-runtime-baseline.py"),
        "--bundle", str(library.parent), "--architecture", architecture,
        "--maximum", MAX_GLIBC, "--output", str(output),
    ])


def bundle(*, appdir: Path, architecture: str, manifest: Path) -> None:
    appdir = appdir.resolve(strict=True)
    manifest = manifest.resolve(strict=False)
    try:
        manifest.relative_to(appdir)
    except ValueError as error:
        raise BundleError("Keybinder manifest must stay inside the AppImage") from error
    source = library_from_cache()
    verify_architecture(source, architecture)
    needed = verify_soname(source)
    version = package_owner(source)
    destination = appdir / "usr/lib/copypaste/lib"
    copied = copy_library(source, destination)
    license_source = package_license()
    licenses = appdir / "usr/share/licenses/copypaste-keybinder"
    if licenses.exists() or licenses.is_symlink():
        raise BundleError("Keybinder license destination is not empty")
    licenses.mkdir(parents=True)
    license_target = licenses / f"{PACKAGE}.copyright"
    shutil.copyfile(license_source, license_target, follow_symlinks=False)
    baseline = appdir / "usr/share/copypaste/keybinder-runtime-glibc.json"
    baseline.parent.mkdir(parents=True, exist_ok=True)
    validate_glibc(copied, architecture=architecture, output=baseline)
    manifest.parent.mkdir(parents=True, exist_ok=True)
    manifest.write_text(json.dumps({
        "schema": 1,
        "architecture": architecture,
        "library": {
            "soname": LIBRARY_SONAME,
            "path": str(copied.relative_to(appdir)),
            "sha256": sha256(copied),
            "source_package": PACKAGE,
            "source_version": version,
            "host_abi_sonames": needed,
        },
        "license": {
            "path": str(license_target.relative_to(appdir)),
            "sha256": sha256(license_target),
        },
        "glibc_baseline": str(baseline.relative_to(appdir)),
    }, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--appdir", required=True, type=Path)
    parser.add_argument("--architecture", required=True, choices=tuple(ARCHITECTURES))
    parser.add_argument("--manifest", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        bundle(appdir=args.appdir, architecture=args.architecture, manifest=args.manifest)
    except BundleError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
