#!/usr/bin/env python3
"""Emit a receipt for an installed private compositor runtime tree."""
from __future__ import annotations

import argparse, hashlib, json, os, platform, posixpath, re, shutil, subprocess, sys
from pathlib import Path

SAFE = re.compile(r"^[a-z0-9][a-z0-9.-]{1,63}$")
SHA = re.compile(r"^[0-9a-f]{64}$")
NAME = re.compile(r"^[A-Za-z0-9.+_-]{1,80}$")
VERSION = re.compile(r"^[A-Za-z0-9.+:~^_-]{1,120}$")
LICENSE = re.compile(r"^[\x20-\x7e]{1,1024}$")
SOURCE_RPM = re.compile(r"^[A-Za-z0-9.+:~^_-]{1,160}\.src\.rpm$")
MANIFEST = Path("usr/share/copypaste/compositor-runtime-private-closure.json")

def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()

def os_release() -> dict[str, str]:
    values = {}
    for line in Path("/etc/os-release").read_text(encoding="utf-8").splitlines():
        if "=" in line:
            k, v = line.split("=", 1); values[k] = v.strip('"')
    if not re.fullmatch(r"[a-z0-9][a-z0-9._-]{0,63}", values.get("ID", "")) or not re.fullmatch(r"[a-z0-9][a-z0-9._-]{0,63}", values.get("VERSION_ID", "")):
        raise ValueError("unsafe or missing distribution identity")
    return {"id": values["ID"], "version": values["VERSION_ID"]}

def architecture() -> str:
    return {"x86_64": "x86_64", "aarch64": "aarch64", "arm64": "aarch64"}.get(platform.machine(), "")

def payload(root: Path) -> list[dict]:
    rows = []
    for path in sorted(root.rglob("*")):
        if path.is_dir(): continue
        relative = path.relative_to(root).as_posix()
        if path.is_symlink():
            target = os.readlink(path)
            resolved = posixpath.normpath(posixpath.join(posixpath.dirname(relative), target))
            if target.startswith("/") or resolved == ".." or resolved.startswith("../"):
                raise ValueError(f"runtime symlink escapes: {relative}")
            rows.append({"path": relative, "type": "symlink", "target": target, "sha256": hashlib.sha256(target.encode()).hexdigest()})
        elif path.is_file():
            rows.append({"path": relative, "type": "file", "mode": f"{path.stat().st_mode & 0o7777:04o}", "sha256": digest(path)})
        else: raise ValueError(f"runtime member is unsafe: {relative}")
    if not rows: raise ValueError("installed runtime is empty")
    return rows

def host_requirement(value: str) -> dict[str, str]:
    match = re.fullmatch(r"(?P<name>[A-Za-z0-9.+_-]{1,80})(?:(?P<operator>>=|<=|>|<)(?P<version>[A-Za-z0-9.+:~_-]{1,120}))?", value)
    if match is None:
        raise ValueError("host requirements may only use a package name or a compatible range")
    requirement = {"name": match["name"]}
    if match["operator"]:
        requirement.update({"operator": match["operator"], "version": match["version"]})
    return requirement


def closure_licenses(root: Path) -> list[dict[str, str]]:
    manifest_path = root / MANIFEST
    if not manifest_path.is_file() or manifest_path.is_symlink():
        return []
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ValueError("private ELF closure manifest is invalid") from error
    licenses = manifest.get("licenses") if isinstance(manifest, dict) else None
    if not isinstance(licenses, list) or not licenses:
        raise ValueError("private ELF closure manifest has no RPM license records")
    result = []
    for item in licenses:
        base = {"package", "license_package", "license_evr", "license_source_rpm", "license", "path", "sha256", "license_origin"}
        source = base | {"license_archive", "license_archive_sha256", "license_archive_supplier", "license_archive_evr"}
        if not isinstance(item, dict) or set(item) not in (base, source):
            raise ValueError("private ELF closure license record is invalid")
        path = item["path"]
        if (not isinstance(path, str) or path.startswith("/") or ".." in Path(path).parts
                or not NAME.fullmatch(item["package"]) or not NAME.fullmatch(item["license_package"]) or not VERSION.fullmatch(item["license_evr"]) or not SOURCE_RPM.fullmatch(item["license_source_rpm"]) or not LICENSE.fullmatch(item["license"]) or item["license"] != item["license"].strip()
                or not SHA.fullmatch(item["sha256"])):
            raise ValueError("private ELF closure license metadata is unsafe")
        if item["license_origin"] not in {"installed-rpm", "source-rpm"} or (item["license_origin"] == "source-rpm" and (item.get("license_archive") != item["license_source_rpm"] or not NAME.fullmatch(item.get("license_archive_supplier", "")) or not VERSION.fullmatch(item.get("license_archive_evr", "")) or not SHA.fullmatch(item.get("license_archive_sha256", "")))):
            raise ValueError("private ELF closure license origin is invalid")
        source = root / path
        if not source.is_file() or source.is_symlink() or digest(source) != item["sha256"]:
            raise ValueError("private ELF closure license bytes differ")
        result.append({"spdx": item["license"], "name": path, "sha256": item["sha256"]})
    return result

def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--runtime-dir", required=True, type=Path); p.add_argument("--output", required=True, type=Path)
    p.add_argument("--runtime-id", required=True); p.add_argument("--desktop", choices=("GNOME", "KDE"), required=True)
    p.add_argument("--source-revision", required=True); p.add_argument("--patch", required=True, type=Path)
    p.add_argument("--glibc-floor", required=True); p.add_argument("--host-requirement", action="append", default=[])
    # Keep older producer invocations working, but never turn these names into
    # exact host-desktop version pins.
    p.add_argument("--dependency", action="append", default=[])
    p.add_argument("--private-entrypoint"); p.add_argument("--gnome-major", type=int)
    p.add_argument("--qualification-entrypoint")
    p.add_argument("--shell-revision")
    p.add_argument("--license-file", required=True, type=Path); p.add_argument("--license-spdx", default="GPL-2.0-or-later")
    a = p.parse_args()
    try:
        if not SAFE.fullmatch(a.runtime_id) or not re.fullmatch(r"[A-Za-z0-9._/+:-]{1,128}", a.source_revision) or not re.fullmatch(r"[0-9]+\.[0-9]+", a.glibc_floor): raise ValueError("invalid immutable runtime identity")
        root = a.runtime_dir.resolve(strict=True)
        if a.runtime_dir.is_symlink() or not root.is_dir() or not a.patch.is_file() or not a.license_file.is_file(): raise ValueError("runtime source inputs are unsafe")
        deps = [host_requirement(value) for value in [*a.host_requirement, *a.dependency]]
        if len({json.dumps(value, sort_keys=True) for value in deps}) != len(deps):
            raise ValueError("host requirements must be unique")
        rows=payload(root)
        paths={r["path"] for r in rows}
        if a.desktop == "KDE":
            if a.gnome_major or not a.private_entrypoint or a.private_entrypoint not in paths: raise ValueError("KDE requires an installed private entrypoint")
            launch={"kind":"private", "entrypoint":a.private_entrypoint}
        else:
            if a.private_entrypoint:
                if a.private_entrypoint not in paths: raise ValueError("GNOME private Shell entrypoint is not installed")
                launch={"kind":"private","entrypoint":a.private_entrypoint}
            else:
                if a.gnome_major not in (46,47): raise ValueError("GNOME requires its expected Shell major")
                shell="/usr/bin/gnome-shell"; session="/usr/bin/gnome-session"
                version=subprocess.check_output([shell,"--version"],text=True).strip()
                if not re.search(rf"\b{a.gnome_major}(?:\.|\b)",version): raise ValueError(f"system GNOME Shell does not match Mutter {a.gnome_major}: {version}")
                launch={"kind":"system-session","command":[session,"--session=gnome"],"host_binaries":[{"path":session,"sha256":digest(Path(session))},{"path":shell,"sha256":digest(Path(shell))}]}
        if not a.qualification_entrypoint or a.qualification_entrypoint not in paths:
            raise ValueError("a receipt-listed zero-argument qualification entrypoint is required")
        license_target=root / "usr/share/doc" / f"copypaste-compositor-runtime-{a.runtime_id}" / a.license_file.name
        license_target.parent.mkdir(parents=True, exist_ok=True); shutil.copyfile(a.license_file, license_target)
        licenses = [{"spdx": a.license_spdx, "name": license_target.relative_to(root).as_posix(), "sha256": digest(license_target)}, *closure_licenses(root)]
        if len({item["name"] for item in licenses}) != len(licenses):
            raise ValueError("runtime license paths must be unique")
        source={"revision":a.source_revision,"patch_sha256":digest(a.patch)}
        if a.shell_revision:
            if not re.fullmatch(r"[0-9a-f]{40}", a.shell_revision): raise ValueError("private Shell revision must be a full SHA")
            source["shell_revision"]=a.shell_revision
        receipt={"schema":1,"runtime_id":a.runtime_id,"desktop":a.desktop,"architecture":architecture(),"distribution":os_release(),"glibc_floor":a.glibc_floor,"source":source,"payload":payload(root),"launch":launch,"qualification":{"kind":"headless","entrypoint":a.qualification_entrypoint},"runtime_env":{},"host_requirements":deps,"package_dependencies":[],"upstream_licenses":licenses}
        if receipt["architecture"] not in {"x86_64","aarch64"}: raise ValueError("unsupported build architecture")
        a.output.parent.mkdir(parents=True, exist_ok=True); a.output.write_text(json.dumps(receipt,indent=2)+"\n",encoding="utf-8")
    except (OSError, subprocess.CalledProcessError, ValueError) as e:
        print(f"ERROR: {e}",file=sys.stderr); return 1
    print(f"emitted immutable runtime receipt: {a.output}"); return 0
if __name__ == "__main__": raise SystemExit(main())
