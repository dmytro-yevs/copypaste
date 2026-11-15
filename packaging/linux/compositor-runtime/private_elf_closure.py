#!/usr/bin/env python3
"""Copy a private, RPM-owned ELF dependency closure into a compositor runtime.

The compositor is built against a pinned Fedora source tree, but must not make
the host Plasma stack solve its private ABI requirements.  This helper follows
DT_NEEDED recursively, copies every non-glibc library into the runtime's
``usr/lib`` prefix, and leaves an immutable manifest beside the payload.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import io
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import tarfile
from collections import deque
from pathlib import Path
from typing import Iterable


NEEDED = re.compile(r"\(NEEDED\).*\[(?P<soname>[^]]+)\]")
SONAME = re.compile(r"\(SONAME\).*\[(?P<soname>[^]]+)\]")
SEARCH_PATH = re.compile(r"\((?:RPATH|RUNPATH)\).*\[(?P<path>[^]]+)\]")
SAFE_NAME = re.compile(r"^[A-Za-z0-9.+_-]{1,80}$")
SAFE_EVR = re.compile(r"^[A-Za-z0-9.+:~^_-]{1,120}$")
SAFE_SOURCE_RPM = re.compile(r"^[A-Za-z0-9.+:~^_-]{1,160}\.src\.rpm$")
SAFE_LICENSE = re.compile(r"^[\x20-\x7e]{1,1024}$")
SAFE_SONAME = re.compile(r"^[A-Za-z0-9._+-]{1,255}$")
NOTICE_NAME = re.compile(r"^(?:LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT|README)(?:[._-].*)?$", re.IGNORECASE)
README_NAME = re.compile(r"^README(?:[._-].*)?$", re.IGNORECASE)
COPYRIGHT = re.compile(rb"copyright", re.IGNORECASE)
MIT_BANNER = re.compile(rb"permission\s+is\s+hereby\s+granted", re.IGNORECASE)
FULL_GPL_BANNER = re.compile(rb"gnu\s+(?:lesser\s+)?general\s+public\s+license.*end\s+of\s+terms\s+and\s+conditions", re.IGNORECASE | re.DOTALL)
LGPL_REFERENCE = re.compile(rb"this\s+library\s+is\s+free\s+software.*gnu\s+lesser\s+general\s+public\s+license", re.IGNORECASE | re.DOTALL)
LEADING_COMMENT = re.compile(rb"\A\s*/\*.*?\*/", re.DOTALL)
MAX_SOURCE_ARCHIVE_BYTES = 64 * 1024 * 1024
MAX_LICENSE_BYTES = 4 * 1024 * 1024
MAX_LICENSE_MEMBERS = 128
MAX_SOURCE_NOTICE_BYTES = 128 * 1024
STANDARD_LICENSES = (
    ("GNU-LGPL-3.0.txt", "https://www.gnu.org/licenses/lgpl-3.0.txt", "e3a994d82e644b03a792a930f574002658412f62407f5fee083f2555c5f23118"),
    ("GNU-GPL-3.0.txt", "https://www.gnu.org/licenses/gpl-3.0.txt", "3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986"),
)
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


def expand_origin(path: Path, value: str) -> str:
    return value.replace("${ORIGIN}", str(path.parent)).replace("$ORIGIN", str(path.parent))


def dynamic_search_directories(path: Path) -> list[Path]:
    directories = []
    for raw in SEARCH_PATH.findall(run(["readelf", "-d", str(path)])):
        for item in raw.split(":"):
            if "$" in item:
                item = expand_origin(path, item)
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
    return rpm_installed_license_files(owner)


def rpm_installed_license_files(owner: tuple[str, str, str, str]) -> list[tuple[tuple[str, str, str, str], Path]]:
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
        return []
    return records


def safe_source_path(value: str) -> str | None:
    member = value.removeprefix("./")
    path = Path(member)
    if not member or path.is_absolute() or ".." in path.parts:
        return None
    return member


def safe_source_member(value: str) -> str | None:
    member = safe_source_path(value)
    if member is None:
        return None
    path = Path(member)
    in_spdx_license_directory = any(part.lower() == "licenses" for part in path.parts)
    if NOTICE_NAME.fullmatch(path.name) is None and not in_spdx_license_directory:
        return None
    return member


def has_legal_notice(value: bytes) -> bool:
    return COPYRIGHT.search(value) is not None and (MIT_BANNER.search(value) is not None or FULL_GPL_BANNER.search(value) is not None)


def source_member_has_notice_text(member: str, value: bytes) -> bool:
    """Require a legal-notice marker before treating a README as license text."""
    return README_NAME.fullmatch(Path(member).name) is None or has_legal_notice(value)


def source_header_notice(value: bytes, *, allow_lgpl_reference: bool = False) -> bytes | None:
    """Return one leading source comment only when it is a complete legal banner."""
    match = LEADING_COMMENT.match(value[:16 * 1024])
    if match is None or COPYRIGHT.search(match.group()) is None:
        return None
    if not has_legal_notice(match.group()) and not (allow_lgpl_reference and LGPL_REFERENCE.search(match.group()) is not None):
        return None
    return match.group()


def bounded_source_notices(candidates: list[tuple[str, bytes]]) -> list[tuple[str, bytes]]:
    unique: list[tuple[str, bytes]] = []
    seen = set()
    total = 0
    for member, value in candidates:
        digest = hashlib.sha256(value).digest()
        if digest in seen:
            continue
        seen.add(digest)
        total += len(value)
        if len(unique) >= MAX_LICENSE_MEMBERS or total > MAX_SOURCE_NOTICE_BYTES:
            raise ClosureError("exact source RPM has too many legal notice bytes")
        unique.append((member, value))
    return unique


def standard_license_records(owner: tuple[str, str, str, str]) -> list[tuple[str, bytes, str, str]]:
    if owner[3] != "LGPL-3.0-or-later":
        return []
    records = []
    for name, url, expected in STANDARD_LICENSES:
        asset = Path(__file__).with_name("licenses") / name
        encoded = b"".join(asset.read_bytes().split())
        value = base64.b64decode(encoded, validate=True)
        if len(value) > MAX_LICENSE_BYTES or hashlib.sha256(value).hexdigest() != expected:
            raise ClosureError("canonical GNU license text verification failed")
        records.append((name, value, url, expected))
    return records


def cpio_member_bytes(archive: Path, member: str) -> bytes:
    payload = bounded_rpm2cpio(archive)
    result = subprocess.run(["cpio", "--quiet", "-i", "--to-stdout", member], input=payload, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if result.returncode or len(result.stdout) > MAX_SOURCE_ARCHIVE_BYTES:
        raise ClosureError("exact source RPM member is unavailable or oversized")
    return result.stdout


def bounded_rpm2cpio(archive: Path) -> bytes:
    if archive.stat().st_size > MAX_SOURCE_ARCHIVE_BYTES:
        raise ClosureError("exact source RPM archive is oversized")
    process = subprocess.Popen(["rpm2cpio", str(archive)], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None
    output = bytearray()
    while chunk := process.stdout.read(1024 * 1024):
        output.extend(chunk)
        if len(output) > MAX_SOURCE_ARCHIVE_BYTES:
            process.kill()
            process.wait()
            raise ClosureError("exact source RPM payload is oversized")
    if process.wait() != 0:
        raise ClosureError("cannot read exact source RPM payload")
    return bytes(output)


def source_rpm_license_files(owner: tuple[str, str, str, str], destination: Path) -> list[tuple[tuple[str, str, str, str], str, bytes, str, str, str, str, str]]:
    name, evr, source_rpm, _license = owner
    source_dir = destination / source_rpm
    if not source_dir.exists():
        source_dir.mkdir(parents=True)
        request = source_rpm.removesuffix(".src.rpm")
        command = ["dnf", "-q", "download", "--source", "--destdir", str(source_dir), request]
        completed = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
        if completed.returncode:
            raise ClosureError(f"exact source RPM download failed for {name}: request={request} exit={completed.returncode} output={completed.stdout[:512].replace(chr(10), ' ')!r}")
    archives = [path for path in source_dir.iterdir() if path.is_file() and path.name == source_rpm]
    if len(archives) != 1:
        raise ClosureError(f"exact source RPM is unavailable for {name}")
    archive = archives[0]
    signature = subprocess.run(["rpm", "--checksig", "--verbose", str(archive)], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
    if signature.returncode or re.search(r"Signature.*: OK", signature.stdout, re.IGNORECASE) is None:
        raise ClosureError(f"exact source RPM signature is invalid for {name}")
    identity = run(["rpm", "-qp", "--qf", "%{NAME}\t%{EPOCHNUM}\t%{VERSION}\t%{RELEASE}\t%{ARCH}\t%{SOURCEPACKAGE}\n", str(archive)]).splitlines()
    if len(identity) != 1:
        raise ClosureError(f"exact source RPM provenance differs for {name}")
    source_name, epoch, version, release, architecture, source_package = identity[0].split("\t")
    source_evr = f"{epoch}:{version}-{release}" if epoch not in {"", "0", "(none)"} else f"{version}-{release}"
    if source_package != "1" or source_evr != evr or archive.name != f"{source_name}-{version}-{release}.src.rpm":
        raise ClosureError(f"exact source RPM provenance differs for {name}: expected={source_rpm}/{evr} header={source_name}/{source_evr}/{architecture}/{source_package}")
    listing = bounded_rpm2cpio(archive)
    members_process = subprocess.run(["cpio", "-it"], input=listing, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    verbose_process = subprocess.run(["cpio", "-itv"], input=listing, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if members_process.returncode or verbose_process.returncode:
        raise ClosureError(f"cannot inspect exact source RPM for {name}")
    members = [line.removeprefix("./") for line in members_process.stdout.decode(errors="replace").splitlines()]
    candidates: list[tuple[str, bytes]] = []
    header_candidates: list[tuple[str, bytes]] = []
    hints = [member for member in members if re.search(r"(?:LICENSE|LICENCE|COPYING|NOTICE)", member, re.IGNORECASE)][:16]
    for member in members:
        safe = safe_source_member(member)
        if safe is not None:
            verbose = next((line for line in verbose_process.stdout.decode(errors="replace").splitlines() if line.startswith("-") and line.rstrip().endswith(member)), "")
            fields = verbose.split()
            if len(fields) >= 2 and fields[1] == "1":
                value = cpio_member_bytes(archive, member)
                if len(value) <= MAX_LICENSE_BYTES and source_member_has_notice_text(safe, value):
                    candidates.append((safe, value))
        if not member.lower().endswith((".tar", ".tar.gz", ".tar.xz", ".tar.bz2")):
            continue
        archive_bytes = cpio_member_bytes(archive, member)
        try:
            with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode="r:*") as upstream:
                for entry in upstream.getmembers():
                    safe = safe_source_member(entry.name)
                    source_path = safe_source_path(entry.name)
                    if source_path is None or not entry.isfile() or entry.size > MAX_LICENSE_BYTES:
                        continue
                    handle = upstream.extractfile(entry)
                    if handle is None:
                        continue
                    value = handle.read(MAX_LICENSE_BYTES + 1)
                    if safe is not None and source_member_has_notice_text(safe, value):
                        candidates.append((safe, value))
                    elif not candidates and (notice := source_header_notice(value, allow_lgpl_reference=_license == "LGPL-3.0-or-later")) is not None:
                        header_candidates.append((source_path, notice))
        except (tarfile.TarError, OSError):
            continue
    if not candidates:
        candidates.extend(header_candidates)
    candidates = bounded_source_notices([(member, value) for member, value in candidates if len(value) <= MAX_LICENSE_BYTES])
    if not candidates:
        raise ClosureError(f"exact source RPM has no safe license or notice bytes for {name}: candidates={','.join(hints) or 'none'}")
    archive_hash = sha256(archive)
    return [(owner, Path(member).name, value, source_rpm, archive_hash, source_name, source_evr, member) for member, value in candidates]


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
    with tempfile.TemporaryDirectory(prefix="copypaste-compositor-source-rpms-") as temporary:
        source_cache = Path(temporary)
        for package in sorted(packages.values(), key=lambda item: item["name"]):
            owner = (package["name"], package["evr"], package["source_rpm"], package["license"])
            installed = rpm_installed_license_files(owner)
            source_records: list[tuple[tuple[str, str, str, str], str, bytes, str, str, str, str, str]] = []
            if not installed:
                source_records = source_rpm_license_files(owner, source_cache)
            records = [("installed-rpm", record_owner, source.name, source.read_bytes(), None, None, None, None, None, None, None) for record_owner, source in installed] if installed else [("source-rpm", *record, None, None) for record in source_records]
            if not installed:
                records.extend(("standard-license", owner, source_name, source_bytes, None, None, None, None, None, standard_url, standard_hash) for source_name, source_bytes, standard_url, standard_hash in standard_license_records(owner))
            for index, (origin, license_owner, source_name, source_bytes, archive_name, archive_hash, archive_supplier, archive_evr, source_member, standard_url, standard_hash) in enumerate(records):
                target = license_destination / f"{package['name']}-{index}-{source_name}"
                if target.exists() or target.is_symlink():
                    raise ClosureError("private RPM license destination collides")
                target.write_bytes(source_bytes)
                record = {
                    "package": package["name"], "license_package": license_owner[0], "license_evr": license_owner[1],
                    "license_source_rpm": license_owner[2], "license": package["license"],
                    "path": target.relative_to(runtime).as_posix(), "sha256": sha256(target), "license_origin": origin,
                }
                if origin == "source-rpm":
                    record.update({"license_archive": archive_name, "license_archive_sha256": archive_hash, "license_archive_supplier": archive_supplier, "license_archive_evr": archive_evr, "license_source_member": source_member})
                if origin == "standard-license":
                    record.update({"standard_license_url": standard_url, "standard_license_sha256": standard_hash})
                licenses.append(record)
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
