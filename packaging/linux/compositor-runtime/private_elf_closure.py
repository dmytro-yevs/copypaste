#!/usr/bin/env python3
"""Copy a private, RPM-owned ELF dependency closure into a compositor runtime.

The compositor is built against a pinned Fedora source tree, but must not make
the host Plasma stack solve its private ABI requirements.  This helper follows
DT_NEEDED recursively, copies every non-glibc library into the runtime's
``usr/lib`` prefix, and leaves an immutable manifest beside the payload.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
from collections import deque
from pathlib import Path
from typing import Iterable


NEEDED = re.compile(r"\(NEEDED\).*\[(?P<soname>[^]]+)\]")
SONAME = re.compile(r"\(SONAME\).*\[(?P<soname>[^]]+)\]")
SAFE_NAME = re.compile(r"^[A-Za-z0-9.+_-]{1,80}$")
SAFE_EVR = re.compile(r"^[A-Za-z0-9.+:~_-]{1,120}$")
SAFE_LICENSE = re.compile(r"^[\x20-\x7e]{1,240}$")
SAFE_SONAME = re.compile(r"^[A-Za-z0-9._+-]{1,255}$")
GLIBC_SONAMES = {
    "libc.so.6", "libdl.so.2", "libm.so.6", "libpthread.so.0", "librt.so.1",
    "libutil.so.1", "libresolv.so.2", "libnsl.so.1", "ld-linux-x86-64.so.2",
    "ld-linux-aarch64.so.1",
}
MANIFEST = "usr/share/copypaste/compositor-runtime-private-closure.json"


class ClosureError(RuntimeError):
    pass


def run(argv: list[str]) -> str:
    try:
        return subprocess.run(argv, check=True, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout
    except (OSError, subprocess.CalledProcessError) as error:
        raise ClosureError(f"required command failed: {Path(argv[0]).name}") from error


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def needed(path: Path) -> set[str]:
    return set(NEEDED.findall(run(["readelf", "-d", str(path)])))


def provided_soname(path: Path) -> str | None:
    values = SONAME.findall(run(["readelf", "-d", str(path)]))
    if not values:
        return None
    if len(values) != 1 or not SAFE_SONAME.fullmatch(values[0]):
        raise ClosureError("private runtime library must expose one safe SONAME")
    return values[0]


def soname(path: Path) -> str:
    value = provided_soname(path)
    if value is None:
        raise ClosureError("private runtime library must expose one safe SONAME")
    return value


def exported_elfs(runtime: Path) -> list[Path]:
    result = []
    for path in sorted(runtime.rglob("*")):
        if not path.is_file() or path.is_symlink():
            continue
        with path.open("rb") as handle:
            if handle.read(4) == b"\x7fELF":
                result.append(path)
    if not result:
        raise ClosureError("private compositor runtime exports no ELF files")
    return result


def library_cache() -> dict[str, Path]:
    result: dict[str, Path] = {}
    for line in run(["ldconfig", "-p"]).splitlines():
        if " => " not in line:
            continue
        name, raw_path = line.strip().split(" => ", 1)
        library = name.split(" ", 1)[0]
        candidate = Path(raw_path)
        if SAFE_SONAME.fullmatch(library) and candidate.is_absolute() and candidate.exists():
            result.setdefault(library, candidate)
    return result


def trusted_library(path: Path) -> Path:
    try:
        resolved = path.resolve(strict=True)
        details = resolved.stat()
    except OSError as error:
        raise ClosureError("dynamic runtime dependency is unavailable") from error
    if not stat.S_ISREG(details.st_mode) or not any(str(resolved).startswith(root) for root in ("/lib/", "/lib64/", "/usr/lib/", "/usr/lib64/")):
        raise ClosureError("dynamic runtime dependency escapes trusted library roots")
    return resolved


def rpm_owner(path: Path) -> tuple[str, str, str]:
    package = run(["rpm", "-qf", "--qf", "%{NAME}\n%{EVR}\n%{LICENSE}\n", str(path)]).splitlines()
    if (len(package) != 3 or not SAFE_NAME.fullmatch(package[0])
            or not SAFE_EVR.fullmatch(package[1])
            or not SAFE_LICENSE.fullmatch(package[2])
            or package[2] != package[2].strip()):
        raise ClosureError("bundled ELF library has unsafe or unavailable RPM provenance")
    return package[0], package[1], package[2]


def rpm_license_files(package: str) -> list[Path]:
    candidates = [Path(item) for item in run(["rpm", "-ql", package]).splitlines()]
    candidates = sorted({trusted_license(item) for item in candidates if str(item).startswith("/usr/share/licenses/") and item.is_file()})
    if not candidates:
        raise ClosureError(f"bundled RPM {package} has no readable license bytes")
    return candidates


def trusted_license(path: Path) -> Path:
    try:
        resolved = path.resolve(strict=True)
        details = resolved.stat()
    except OSError as error:
        raise ClosureError("bundled RPM license is unavailable") from error
    if not stat.S_ISREG(details.st_mode) or not str(resolved).startswith("/usr/share/licenses/"):
        raise ClosureError("bundled RPM license escapes trusted license root")
    return resolved


def copy_file(source: Path, destination: Path) -> Path:
    target = destination / source.name
    if target.exists() or target.is_symlink():
        if not target.is_file() or target.is_symlink() or sha256(target) != sha256(source):
            raise ClosureError("two bundled ELF dependencies collide")
    else:
        shutil.copyfile(source, target, follow_symlinks=False)
        target.chmod(0o755)
    return target


def link(destination: Path, name: str, target: str) -> None:
    if name == target:
        return
    candidate = destination / name
    if candidate.exists() or candidate.is_symlink():
        if not candidate.is_symlink() or os.readlink(candidate) != target:
            raise ClosureError("private ELF SONAME link collides")
    else:
        candidate.symlink_to(target)


def copy_closure(runtime: Path, initial: Iterable[Path]) -> dict:
    cache = library_cache()
    initial = list(initial)
    private_sonames = {value for path in initial if (value := provided_soname(path)) is not None}
    destination = runtime / "usr/lib"
    destination.mkdir(parents=True, exist_ok=True)
    queue: deque[tuple[str, Path]] = deque()
    for source in initial:
        for dependency in sorted(needed(source)):
            if dependency not in GLIBC_SONAMES:
                if dependency in private_sonames:
                    continue
                if dependency not in cache:
                    raise ClosureError(f"could not resolve dynamic dependency {dependency}")
                queue.append((dependency, cache[dependency]))
    libraries: list[dict] = []
    packages: dict[str, dict] = {}
    copied: dict[Path, dict] = {}
    while queue:
        requested, candidate = queue.popleft()
        source = trusted_library(candidate)
        if source in copied:
            link(destination, requested, copied[source]["path"].rsplit("/", 1)[-1])
            continue
        bundled = copy_file(source, destination)
        library_soname = soname(source)
        link(destination, requested, bundled.name)
        link(destination, library_soname, bundled.name)
        package, evr, license_expression = rpm_owner(source)
        row = {
            "path": bundled.relative_to(runtime).as_posix(),
            "soname": library_soname,
            "sha256": sha256(bundled),
            "package": package,
            "evr": evr,
        }
        copied[source] = row
        libraries.append(row)
        packages.setdefault(package, {"name": package, "evr": evr, "license": license_expression})
        for dependency in sorted(needed(source)):
            if dependency not in GLIBC_SONAMES:
                if dependency in private_sonames:
                    continue
                if dependency not in cache:
                    raise ClosureError(f"could not resolve dynamic dependency {dependency}")
                queue.append((dependency, cache[dependency]))
    if not libraries:
        raise ClosureError("private compositor runtime has no non-glibc ELF closure")
    licenses = []
    license_destination = runtime / "usr/share/doc/copypaste-compositor-runtime-private-licenses"
    license_destination.mkdir(parents=True, exist_ok=True)
    for package in sorted(packages.values(), key=lambda item: item["name"]):
        for index, source in enumerate(rpm_license_files(package["name"])):
            target = license_destination / f"{package['name']}-{index}-{source.name}"
            if target.exists() or target.is_symlink():
                raise ClosureError("private RPM license destination collides")
            shutil.copyfile(source, target, follow_symlinks=False)
            licenses.append({
                "package": package["name"], "license": package["license"],
                "path": target.relative_to(runtime).as_posix(), "sha256": sha256(target),
            })
    manifest = {"schema": 1, "libraries": sorted(libraries, key=lambda item: item["path"]),
                "packages": sorted(packages.values(), key=lambda item: item["name"]), "licenses": licenses}
    manifest_path = runtime / MANIFEST
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime-dir", required=True, type=Path)
    parser.add_argument("--entrypoint", required=True)
    args = parser.parse_args()
    try:
        runtime = args.runtime_dir.resolve(strict=True)
        if args.runtime_dir.is_symlink() or not runtime.is_dir():
            raise ClosureError("runtime directory is unsafe")
        entrypoint = runtime / args.entrypoint
        if not entrypoint.is_file() or entrypoint.is_symlink() or not args.entrypoint.startswith("usr/"):
            raise ClosureError("private compositor entrypoint is unsafe")
        manifest = copy_closure(runtime, exported_elfs(runtime))
    except (ClosureError, OSError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"bundled {len(manifest['libraries'])} private ELF libraries from {len(manifest['packages'])} RPMs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
