#!/usr/bin/env python3
"""Stage an immutable, sidecar compositor runtime and its opt-in session.

This tool is deliberately a packaging boundary.  It never installs a runtime
on the host and it only copies bytes which are named and hashed in a compiler
receipt.  The generated session launcher has no user supplied command line.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import posixpath
import re
import shutil
import stat
import subprocess
import sys
from pathlib import Path
from typing import Any


ID = re.compile(r"^[a-z0-9][a-z0-9.-]{1,63}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
RELATIVE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._/+@-]{0,255}$")
MODE = re.compile(r"^0[0-7]{3}$")
DISTRO = re.compile(r"^[a-z0-9][a-z0-9._-]{0,63}$")
ENV_KEYS = {"QT_PLUGIN_PATH", "QML2_IMPORT_PATH", "GIO_EXTRA_MODULES"}


class ContractError(ValueError):
    pass


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def safe_relative(value: Any, label: str) -> str:
    if not isinstance(value, str) or not RELATIVE.fullmatch(value):
        raise ContractError(f"{label} is not a safe relative path")
    normalized = posixpath.normpath(value)
    if normalized != value or normalized.startswith("../") or value.startswith("/"):
        raise ContractError(f"{label} escapes its runtime root")
    return value


def safe_link_target(value: Any, relative: str) -> str:
    if not isinstance(value, str) or not value or value.startswith("/") or "\x00" in value:
        raise ContractError("payload symlink target is invalid")
    if not re.fullmatch(r"[A-Za-z0-9._/+@-]{1,255}", value):
        raise ContractError("payload symlink target has unsupported characters")
    resolved = posixpath.normpath(posixpath.join(posixpath.dirname(relative), value))
    if resolved == ".." or resolved.startswith("../"):
        raise ContractError("payload symlink target escapes its runtime root")
    return value


def checked_absolute(value: str, label: str) -> Path:
    path = Path(value)
    if not path.is_absolute() or ".." in path.parts:
        raise ContractError(f"{label} must be an absolute normalized path")
    return path


def inside(root: Path, candidate: Path) -> bool:
    try:
        candidate.relative_to(root)
        return True
    except ValueError:
        return False


def read_receipt(path: Path) -> dict[str, Any]:
    try:
        receipt = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"cannot read runtime receipt: {error}") from error
    if not isinstance(receipt, dict) or receipt.get("schema") != 1:
        raise ContractError("runtime receipt schema must be 1")
    return receipt


def validate_receipt(receipt: dict[str, Any]) -> None:
    runtime_id = receipt.get("runtime_id")
    if not isinstance(runtime_id, str) or not ID.fullmatch(runtime_id):
        raise ContractError("runtime_id is invalid")
    if receipt.get("desktop") not in {"GNOME", "KDE"}:
        raise ContractError("desktop must be GNOME or KDE")
    if receipt.get("architecture") not in {"x86_64", "aarch64"}:
        raise ContractError("architecture is invalid")
    distribution = receipt.get("distribution")
    if not isinstance(distribution, dict) or not all(
        isinstance(distribution.get(key), str) and DISTRO.fullmatch(distribution[key])
        for key in ("id", "version")
    ):
        raise ContractError("distribution must have safe id and version")
    if not isinstance(receipt.get("glibc_floor"), str) or not re.fullmatch(r"[0-9]+\.[0-9]+", receipt["glibc_floor"]):
        raise ContractError("glibc_floor is invalid")
    source = receipt.get("source")
    if not isinstance(source, dict) or not isinstance(source.get("revision"), str) or not re.fullmatch(r"[A-Za-z0-9._/+:-]{1,128}", source["revision"]) or not SHA256.fullmatch(source.get("patch_sha256", "")):
        raise ContractError("source revision and bridge patch digest are required")
    license_info = receipt.get("upstream_license")
    if not isinstance(license_info, dict) or license_info.get("spdx") != "GPL-2.0-or-later" or not isinstance(license_info.get("name"), str) or not SHA256.fullmatch(license_info.get("sha256", "")):
        raise ContractError("upstream GPL license receipt is required")
    payload = receipt.get("payload")
    if not isinstance(payload, list) or not payload:
        raise ContractError("receipt payload is empty")
    paths: set[str] = set()
    for row in payload:
        if not isinstance(row, dict):
            raise ContractError("payload entry is invalid")
        relative = safe_relative(row.get("path"), "payload path")
        if relative in paths:
            raise ContractError("payload paths must be unique")
        paths.add(relative)
        if row.get("type") not in {"file", "symlink"} or not SHA256.fullmatch(row.get("sha256", "")):
            raise ContractError("payload entry must declare type and sha256")
        if row["type"] == "file":
            if not isinstance(row.get("mode"), str) or not MODE.fullmatch(row["mode"]):
                raise ContractError("payload file mode is invalid")
        else:
            target = safe_link_target(row.get("target"), relative)
            if sha256_bytes(target.encode("utf-8")) != row["sha256"]:
                raise ContractError("payload symlink target digest differs")
    if license_info["name"] not in paths or payload_by_path(receipt)[license_info["name"]].get("sha256") != license_info["sha256"]:
        raise ContractError("upstream license bytes are not bound to runtime payload")
    launch = receipt.get("launch")
    if not isinstance(launch, dict) or launch.get("kind") not in {"private", "system-session"}:
        raise ContractError("launch kind is invalid")
    if launch["kind"] == "private":
        entrypoint = safe_relative(launch.get("entrypoint"), "private entrypoint")
        if entrypoint not in paths:
            raise ContractError("private entrypoint is not in payload")
    else:
        command = launch.get("command")
        host_binaries = launch.get("host_binaries")
        if receipt["desktop"] != "GNOME" or command != ["/usr/bin/gnome-session", "--session=gnome"]:
            raise ContractError("system session launch must use the matching GNOME session manager")
        if not isinstance(host_binaries, list) or not host_binaries:
            raise ContractError("system session launch requires host binary hashes")
        guarded_paths = set()
        for binary in host_binaries:
            if not isinstance(binary, dict) or not isinstance(binary.get("path"), str) or binary["path"] not in {"/usr/bin/gnome-session", "/usr/bin/gnome-shell"} or not SHA256.fullmatch(binary.get("sha256", "")):
                raise ContractError("host binary guard is invalid")
            guarded_paths.add(binary["path"])
        if guarded_paths != {"/usr/bin/gnome-session", "/usr/bin/gnome-shell"}:
            raise ContractError("system session launch must guard GNOME Shell and session manager")
    runtime_env = receipt.get("runtime_env", {})
    if not isinstance(runtime_env, dict) or set(runtime_env) - ENV_KEYS:
        raise ContractError("runtime environment contains an unsupported key")
    for key, value in runtime_env.items():
        safe_relative(value, f"runtime environment {key}")
    dependencies = receipt.get("package_dependencies", [])
    if not isinstance(dependencies, list) or not dependencies or not all(isinstance(item, dict) and set(item) == {"name", "version"} and isinstance(item["name"], str) and re.fullmatch(r"[A-Za-z0-9.+_-]{1,80}", item["name"]) and isinstance(item["version"], str) and re.fullmatch(r"[A-Za-z0-9.+:~_-]{1,120}", item["version"]) for item in dependencies):
        raise ContractError("package dependencies are invalid")
    if len({(item["name"], item["version"]) for item in dependencies}) != len(dependencies):
        raise ContractError("package dependencies must be unique")


def payload_by_path(receipt: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {row["path"]: row for row in receipt["payload"]}


def validate_payload(runtime_root: Path, receipt: dict[str, Any]) -> None:
    root = runtime_root.resolve(strict=True)
    if not root.is_dir() or runtime_root.is_symlink():
        raise ContractError("runtime input root is unsafe")
    declared = payload_by_path(receipt)
    actual: set[str] = set()
    for path in sorted(runtime_root.rglob("*")):
        relative = path.relative_to(runtime_root).as_posix()
        if path.is_dir():
            continue
        if relative not in declared:
            raise ContractError(f"runtime input has unbound file: {relative}")
        actual.add(relative)
        row = declared[relative]
        if path.is_symlink():
            if row["type"] != "symlink":
                raise ContractError(f"runtime input type differs: {relative}")
            target = os.readlink(path)
            if target != row["target"] or sha256_bytes(target.encode("utf-8")) != row["sha256"]:
                raise ContractError(f"runtime symlink differs: {relative}")
            resolved = (path.parent / target).resolve(strict=True)
            if not inside(root, resolved):
                raise ContractError(f"runtime symlink escapes: {relative}")
        elif path.is_file():
            if row["type"] != "file" or sha256_file(path) != row["sha256"]:
                raise ContractError(f"runtime file differs: {relative}")
            actual_mode = f"{stat.S_IMODE(path.stat().st_mode):04o}"
            if actual_mode != row["mode"]:
                raise ContractError(f"runtime file mode differs: {relative}")
        else:
            raise ContractError(f"runtime input is not a regular file or symlink: {relative}")
    if actual != set(declared):
        missing = sorted(set(declared) - actual)
        raise ContractError(f"runtime input is missing bound file: {missing[0]}")
    validate_elf_metadata(runtime_root, receipt)


def validate_elf_metadata(runtime_root: Path, receipt: dict[str, Any]) -> None:
    elf_paths = []
    for row in receipt["payload"]:
        if row["type"] != "file":
            continue
        path = runtime_root / row["path"]
        with path.open("rb") as handle:
            if handle.read(4) == b"\x7fELF":
                elf_paths.append(path)
    if not elf_paths:
        return
    readelf = shutil.which("readelf")
    if readelf is None:
        raise ContractError("readelf is required to stage ELF compositor runtime files")
    expected_machine = {"x86_64": "Advanced Micro Devices X86-64", "aarch64": "AArch64"}[receipt["architecture"]]
    declared_floor = tuple(int(part) for part in receipt["glibc_floor"].split("."))
    for path in elf_paths:
        header = subprocess.run([readelf, "--file-header", str(path)], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
        if header.returncode or f"Machine:                           {expected_machine}" not in header.stdout:
            raise ContractError(f"runtime ELF architecture differs: {path.relative_to(runtime_root)}")
        versions = subprocess.run([readelf, "--version-info", str(path)], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
        if versions.returncode:
            raise ContractError(f"cannot inspect runtime ELF glibc floor: {path.relative_to(runtime_root)}")
        required = [(int(major), int(minor)) for major, minor in re.findall(r"GLIBC_(\d+)\.(\d+)", versions.stdout)]
        if required and max(required) > declared_floor:
            raise ContractError(f"runtime ELF exceeds declared glibc floor: {path.relative_to(runtime_root)}")


def shell_quote(value: str) -> str:
    return "'" + value.replace("'", "'\\''") + "'"


def launcher_text(receipt: dict[str, Any], runtime_prefix: str) -> str:
    runtime_id = receipt["runtime_id"]
    distro = receipt["distribution"]
    root = f"{runtime_prefix}/{runtime_id}"
    lines = [
        "#!/bin/sh", "set -eu", "",
        'if [ "$#" -ne 0 ]; then',
        '  echo "CopyPaste compositor session does not accept arguments" >&2', "  exit 64", "fi", "",
        f"expected_id={shell_quote(distro['id'])}",
        f"expected_version={shell_quote(distro['version'])}",
        'os_release=/etc/os-release',
        'actual_id=$(sed -n -E \'s/^ID=\\"?([^\\"]*)\\"?$/\\1/p\' "$os_release" | head -n 1)',
        'actual_version=$(sed -n -E \'s/^VERSION_ID=\\"?([^\\"]*)\\"?$/\\1/p\' "$os_release" | head -n 1)',
        'if [ "$actual_id" != "$expected_id" ] || [ "$actual_version" != "$expected_version" ]; then',
        '  echo "CopyPaste compositor runtime targets a different distribution release" >&2', "  exit 65", "fi", "",
        "PATH=/usr/bin:/bin", "export PATH",
        "unset LD_PRELOAD LD_AUDIT LD_DEBUG LD_LIBRARY_PATH QT_PLUGIN_PATH QML2_IMPORT_PATH GIO_EXTRA_MODULES",
        f"runtime_root={shell_quote(root)}",
        'if [ ! -d "$runtime_root" ]; then', '  echo "CopyPaste compositor runtime is missing" >&2', "  exit 66", "fi",
        'library_path="$runtime_root/lib:$runtime_root/lib64"', 'export LD_LIBRARY_PATH="$library_path"',
        'export XDG_DATA_DIRS="$runtime_root/share:/usr/local/share:/usr/share"',
        'export XDG_CONFIG_DIRS="$runtime_root/etc/xdg:/etc/xdg"',
    ]
    for key, value in sorted(receipt.get("runtime_env", {}).items()):
        lines.append(f"export {key}={shell_quote(root + '/' + value)}")
    launch = receipt["launch"]
    if launch["kind"] == "private":
        lines.extend([
            f"entrypoint={shell_quote(root + '/' + launch['entrypoint'])}",
            'if [ ! -x "$entrypoint" ]; then', '  echo "CopyPaste compositor entrypoint is unavailable" >&2', "  exit 67", "fi",
            'exec "$entrypoint"', "",
        ])
    else:
        for binary in launch["host_binaries"]:
            path, expected = binary["path"], binary["sha256"]
            lines.extend([
                f"host_binary={shell_quote(path)}", f"expected_digest={shell_quote(expected)}",
                'if [ ! -f "$host_binary" ]; then echo "Required system session binary is missing" >&2; exit 68; fi',
                'actual_digest=$(sha256sum -- "$host_binary"); actual_digest=${actual_digest%% *}',
                'if [ "$actual_digest" != "$expected_digest" ]; then echo "Required system session binary differs" >&2; exit 69; fi',
            ])
        lines.append("exec " + " ".join(shell_quote(value) for value in launch["command"]))
        lines.append("")
    return "\n".join(lines)


def stage(receipt_path: Path, runtime_dir: Path, stage_root: Path, runtime_prefix: str, session_dir: str, launcher_dir: str, receipt_dir: str = "/usr/share/copypaste/compositor-runtime", expected_architecture: str | None = None) -> tuple[Path, Path]:
    receipt = read_receipt(receipt_path)
    validate_receipt(receipt)
    if expected_architecture is not None and receipt["architecture"] != expected_architecture:
        raise ContractError("runtime receipt architecture differs from package architecture")
    validate_payload(runtime_dir, receipt)
    stage_root = stage_root.resolve()
    prefix = checked_absolute(runtime_prefix, "runtime prefix")
    session = checked_absolute(session_dir, "session directory")
    launcher = checked_absolute(launcher_dir, "launcher directory")
    receipt_directory = checked_absolute(receipt_dir, "receipt directory")
    for destination in (prefix, session, launcher, receipt_directory):
        if not inside(stage_root, stage_root / destination.relative_to("/")):
            raise ContractError("staging destination escapes stage root")
    relative_prefix = prefix.relative_to("/") / receipt["runtime_id"]
    destination = stage_root / relative_prefix
    if destination.exists() or destination.is_symlink():
        raise ContractError(f"runtime destination already exists: {destination}")
    for row in receipt["payload"]:
        target = destination / row["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        source = runtime_dir / row["path"]
        if row["type"] == "symlink":
            target.symlink_to(row["target"])
        else:
            shutil.copyfile(source, target, follow_symlinks=False)
            target.chmod(int(row["mode"], 8))
    receipt_destination = stage_root / receipt_directory.relative_to("/") / f"{receipt['runtime_id']}.receipt.json"
    receipt_destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(receipt_path, receipt_destination)
    launcher_path = stage_root / launcher.relative_to("/") / f"copypaste-compositor-session-{receipt['runtime_id']}"
    launcher_path.parent.mkdir(parents=True, exist_ok=True)
    launcher_path.write_text(launcher_text(receipt, str(prefix)), encoding="utf-8")
    launcher_path.chmod(0o755)
    session_path = stage_root / session.relative_to("/") / f"copypaste-{receipt['runtime_id']}.desktop"
    session_path.parent.mkdir(parents=True, exist_ok=True)
    session_name = "GNOME" if receipt["desktop"] == "GNOME" else "Plasma"
    session_launcher = launcher / f"copypaste-compositor-session-{receipt['runtime_id']}"
    session_path.write_text(
        "[Desktop Entry]\n"
        f"Name={session_name} (CopyPaste Clipboard)\n"
        "Comment=User-selected CopyPaste compositor session\n"
        f"Exec={session_launcher}\n"
        "Type=Application\n"
        "DesktopNames=CopyPaste\n",
        encoding="utf-8",
    )
    return launcher_path, session_path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--runtime-dir", required=True, type=Path)
    parser.add_argument("--stage-root", required=True, type=Path)
    parser.add_argument("--runtime-prefix", default="/usr/lib/copypaste/compositor-runtime")
    parser.add_argument("--session-dir", default="/usr/share/wayland-sessions")
    parser.add_argument("--launcher-dir", default="/usr/lib/copypaste/compositor-runtime/bin")
    parser.add_argument("--receipt-dir", default="/usr/share/copypaste/compositor-runtime")
    parser.add_argument("--expected-architecture", choices=("x86_64", "aarch64"))
    args = parser.parse_args()
    try:
        launcher, session = stage(args.receipt, args.runtime_dir, args.stage_root, args.runtime_prefix, args.session_dir, args.launcher_dir, args.receipt_dir, args.expected_architecture)
    except ContractError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"staged immutable compositor runtime: {launcher} and {session}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
