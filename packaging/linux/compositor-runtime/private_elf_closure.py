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
SEARCH_PATH = re.compile(r"\((?:RPATH|RUNPATH)\).*\[(?P<path>[^]]+)\]")
SAFE_NAME = re.compile(r"^[A-Za-z0-9.+_-]{1,80}$")
SAFE_EVR = re.compile(r"^[A-Za-z0-9.+:~_-]{1,120}$")
SAFE_SOURCE_RPM = re.compile(r"^[A-Za-z0-9.+:~_-]{1,160}\.src\.rpm$")
SAFE_LICENSE = re.compile(r"^[\x20-\x7e]{1,1024}$")
SAFE_SONAME = re.compile(r"^[A-Za-z0-9._+-]{1,255}$")
NOTICE_NAME = re.compile(r"^(?:LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)(?:[._-].*)?$", re.IGNORECASE)
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


def trusted_library_directory(path: Path) -> Path | None:
    try:
        resolved = path.resolve(strict=True)
        details = resolved.stat()
    except OSError:
        return None
    roots = ("/lib", "/lib64", "/usr/lib", "/usr/lib64")
    if not stat.S_ISDIR(details.st_mode) or not any(str(resolved) == root or str(resolved).startswith(root + "/") for root in roots):
        return None
    return resolved


def dynamic_search_directories(path: Path) -> list[Path]:
    directories = []
    for raw in SEARCH_PATH.findall(run(["readelf", "-d", str(path)])):
        for item in raw.split(":"):
            if "$" in item:
                item = item.replace("$ORIGIN", str(path.parent))
            if "$" in item or not item.startswith("/"):
                continue
            directory = trusted_library_directory(Path(item))
            if directory is not None:
                directories.append(directory)
    return list(dict.fromkeys(directories))


def resolve_dependency(source: Path, soname: str, cache: dict[str, Path]) -> Path | None:
    for directory in dynamic_search_directories(source):
        candidate = directory / soname
        if candidate.exists() or candidate.is_symlink():
            return trusted_library(candidate)
    candidate = cache.get(soname)
    return trusted_library(candidate) if candidate is not None else None


def trusted_library(path: Path) -> Path:
    try:
        resolved = path.resolve(strict=True)
        details = resolved.stat()
    except OSError as error:
        raise ClosureError("dynamic runtime dependency is unavailable") from error
    if not stat.S_ISREG(details.st_mode) or not any(str(resolved).startswith(root) for root in ("/lib/", "/lib64/", "/usr/lib/", "/usr/lib64/")):
        raise ClosureError("dynamic runtime dependency escapes trusted library roots")
    return resolved


def provenance_error(library: str, field: str, value: str | None = None) -> ClosureError:
    safe_library = library if SAFE_SONAME.fullmatch(library) else "unknown"
    suffix = f" length={len(value)}" if value is not None else ""
    return ClosureError(f"bundled ELF RPM provenance is invalid: library={safe_library} field={field}{suffix}")


def rpm_provenance(arguments: list[str], library: str) -> tuple[str, str, str, str]:
    package = run(["rpm", *arguments, "--qf", "%{NAME}\t%{EVR}\t%{SOURCERPM}\t%{LICENSE}\n"])
    rows = package.splitlines()
    if len(rows) != 1:
        raise provenance_error(library, "record")
    values = rows[0].split("\t")
    if len(values) != 4:
        raise provenance_error(library, "record")
    if not SAFE_NAME.fullmatch(values[0]):
        raise provenance_error(library, "name")
    if not SAFE_EVR.fullmatch(values[1]):
        raise provenance_error(library, "evr")
    if not SAFE_SOURCE_RPM.fullmatch(values[2]):
        raise provenance_error(library, "source_rpm")
    if not SAFE_LICENSE.fullmatch(values[3]) or values[3] != values[3].strip():
        raise provenance_error(library, "license", values[3])
    return values[0], values[1], values[2], values[3]


def rpm_owner(path: Path) -> tuple[str, str, str, str]:
    return rpm_provenance(["-qf", str(path)], path.name)


def rpm_siblings(owner: tuple[str, str, str, str]) -> list[tuple[str, str, str, str]]:
    name, evr, source_rpm, _license = owner
    output = run(["rpm", "-qa", "--qf", "%{NAME}\t%{EVR}\t%{SOURCERPM}\t%{LICENSE}\n"])
    siblings = {owner}
    for line in output.splitlines():
        values = line.split("\t")
        if len(values) != 4:
            raise ClosureError("installed RPM provenance inventory is malformed")
        candidate = tuple(values)
        if candidate[1:3] != (evr, source_rpm):
            continue
        if not SAFE_NAME.fullmatch(candidate[0]) or not SAFE_LICENSE.fullmatch(candidate[3]) or candidate[3] != candidate[3].strip():
            raise ClosureError("installed RPM sibling provenance is unsafe")
        siblings.add(candidate)
    return sorted(siblings)


def rpm_license_files(owner: tuple[str, str, str, str]) -> list[tuple[tuple[str, str, str, str], Path]]:
    records = []
    for candidate in rpm_siblings(owner):
        paths = [Path(item) for item in run(["rpm", "-ql", candidate[0]]).splitlines()]
        for path in paths:
            in_license_root = str(path).startswith("/usr/share/licenses/")
            in_doc_root = str(path).startswith("/usr/share/doc/") and NOTICE_NAME.fullmatch(path.name) is not None
            if (in_license_root or in_doc_root) and path.is_file():
                records.append((candidate, trusted_license(path)))
    records = sorted(set(records), key=lambda item: (item[0][0], str(item[1])))
    if not records:
        raise ClosureError(f"bundled RPM {owner[0]} has no readable license or notice bytes in exact source siblings")
    return records


def trusted_license(path: Path) -> Path:
    try:
        resolved = path.resolve(strict=True)
        details = resolved.stat()
    except OSError as error:
        raise ClosureError("bundled RPM license is unavailable") from error
    if not stat.S_ISREG(details.st_mode) or not (str(resolved).startswith("/usr/share/licenses/") or str(resolved).startswith("/usr/share/doc/")):
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
                resolved = resolve_dependency(source, dependency, cache)
                if resolved is None:
                    raise ClosureError(f"could not resolve dynamic dependency {dependency}")
                queue.append((dependency, resolved))
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
        package, evr, source_rpm, license_expression = rpm_owner(source)
        row = {
            "path": bundled.relative_to(runtime).as_posix(),
            "soname": library_soname,
            "sha256": sha256(bundled),
            "package": package,
            "evr": evr,
        }
        copied[source] = row
        libraries.append(row)
        packages.setdefault(package, {"name": package, "evr": evr, "source_rpm": source_rpm, "license": license_expression})
        for dependency in sorted(needed(source)):
            if dependency not in GLIBC_SONAMES:
                if dependency in private_sonames:
                    continue
                resolved = resolve_dependency(source, dependency, cache)
                if resolved is None:
                    raise ClosureError(f"could not resolve dynamic dependency {dependency}")
                queue.append((dependency, resolved))
    if not libraries:
        raise ClosureError("private compositor runtime has no non-glibc ELF closure")
    licenses = []
    license_destination = runtime / "usr/share/doc/copypaste-compositor-runtime-private-licenses"
    license_destination.mkdir(parents=True, exist_ok=True)
    for package in sorted(packages.values(), key=lambda item: item["name"]):
        owner = (package["name"], package["evr"], package["source_rpm"], package["license"])
        for index, (license_owner, source) in enumerate(rpm_license_files(owner)):
            target = license_destination / f"{package['name']}-{index}-{source.name}"
            if target.exists() or target.is_symlink():
                raise ClosureError("private RPM license destination collides")
            shutil.copyfile(source, target, follow_symlinks=False)
            licenses.append({
                "package": package["name"], "license_package": license_owner[0], "license_evr": license_owner[1],
                "license_source_rpm": license_owner[2], "license": package["license"],
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
