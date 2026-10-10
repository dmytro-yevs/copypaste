#!/usr/bin/env python3
"""Verify immutable compositor-runtime producer artifacts and installed sidecars."""

import argparse
import hashlib
import importlib.util
import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SHA256 = re.compile(r"^[0-9a-f]{64}$")
RUNTIMES = {
    ("GNOME", "x86_64"): ("gnome", "46", "ubuntu", "24.04", "deb"),
    ("GNOME", "aarch64"): ("gnome", "46", "ubuntu", "24.04", "deb"),
    ("KDE", "x86_64"): ("kwin", "6.0", "fedora", "40", "rpm"),
    ("KDE", "aarch64"): ("kwin", "6.0", "fedora", "40", "rpm"),
}
BASELINES = {
    "GNOME": ("fe8d2be3f90f89f286c89b164c94a4f86552bc97", "9fca03bb1544c85928041a935f4ce895333722f1", "packaging/linux/desktop-integrations/gnome-shell-extension/mutter/mutter-46-writer-identity.patch"),
    "KDE": ("1ddcb4e288c4f7dcecdc94efccd655b7e3666d30", "packaging/linux/desktop-integrations/kde-native-clipboard/patches/kwin-6.0.patch"),
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_json(path: Path, label: str) -> dict:
    if not path.is_file() or path.is_symlink():
        raise ValueError(f"{label} is missing or unsafe")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ValueError(f"{label} is not JSON") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label} is invalid")
    return value


def runtime_identity(desktop: str, architecture: str) -> tuple[str, str, str, str, str]:
    try:
        return RUNTIMES[(desktop, architecture)]
    except KeyError as error:
        raise ValueError("unsupported compositor runtime target") from error


def artifact_name(desktop: str, architecture: str) -> str:
    family, version, distro, distro_version, _ = runtime_identity(desktop, architecture)
    return f"copypaste-compositor-runtime-{family}-{version}-{distro}{distro_version}-{architecture}"


def verify_baseline_source(desktop: str, source: dict) -> None:
    baseline = BASELINES[desktop]
    revision, patch = baseline[0], baseline[-1]
    if source.get("revision") != revision or source.get("patch_sha256") != sha256(ROOT / patch):
        raise ValueError("producer source revision or patch bytes differ from the checked-out baseline")
    if desktop == "GNOME" and source.get("shell_revision") != baseline[1]:
        raise ValueError("producer private Shell revision differs from the checked-out baseline")


def verify_source(run: dict, artifacts: dict, repository: str, commit: str) -> None:
    direct = run.get("path") == ".github/workflows/compositor-runtime.yml" and run.get("event") == "workflow_dispatch"
    trusted_ci = (run.get("path") == ".github/workflows/ci.yml" and run.get("event") == "pull_request"
                  and isinstance(run.get("pull_requests"), list) and len(run["pull_requests"]) == 1)
    if (
        type(run.get("id")) is not int or run.get("head_sha") != commit
        or run.get("head_repository", {}).get("full_name") != repository
        or not (direct or trusted_ci) or run.get("status") != "completed"
        or run.get("conclusion") != "success"
    ):
        raise ValueError("compositor runtime source is not a successful exact-commit producer run")
    entries = artifacts.get("artifacts", [])
    if artifacts.get("total_count") != len(entries):
        raise ValueError("compositor runtime artifact inventory is incomplete")
    expected = {artifact_name(desktop, architecture) for desktop, architecture in RUNTIMES}
    for name in expected:
        matches = [entry for entry in entries if entry.get("name") == name]
        if len(matches) != 1:
            raise ValueError("compositor runtime source requires exactly one artifact per baseline")
        source = matches[0].get("workflow_run", {})
        if matches[0].get("expired") is not False or source.get("id") != run["id"] or source.get("head_sha") != commit:
            raise ValueError("compositor runtime artifact provenance differs")


def safe_file(root: Path, name: str, label: str) -> Path:
    if not isinstance(name, str) or Path(name).name != name:
        raise ValueError(f"{label} name is unsafe")
    path = root / name
    if not path.is_file() or path.is_symlink():
        raise ValueError(f"{label} is missing or unsafe")
    return path


def stage_runtime_module():
    stage_path = ROOT / "packaging/linux/compositor-runtime/stage_runtime.py"
    original_sys_path = sys.path.copy()
    try:
        sys.path.insert(0, str(stage_path.parent))
        spec = importlib.util.spec_from_file_location("compositor_stage_runtime", stage_path)
        module = importlib.util.module_from_spec(spec)
        assert spec.loader is not None
        spec.loader.exec_module(module)
        return module
    finally:
        sys.path[:] = original_sys_path


def session_descriptor_text(receipt: dict) -> str:
    runtime_id = receipt["runtime_id"]
    session_name = "GNOME" if receipt["desktop"] == "GNOME" else "Plasma"
    launcher = f"/usr/lib/copypaste/compositor-runtime/bin/copypaste-compositor-session-{runtime_id}"
    return (
        "[Desktop Entry]\n"
        f"Name={session_name} (CopyPaste Clipboard)\n"
        "Comment=User-selected CopyPaste compositor session\n"
        f"Exec={launcher}\n"
        "Type=Application\n"
        "DesktopNames=CopyPaste\n"
    )


def binding(root: Path, desktop: str, architecture: str, commit: str, producer_run_id: str) -> dict:
    root = root.resolve(strict=True)
    if root.is_symlink() or not root.is_dir():
        raise ValueError("compositor runtime artifact root is unsafe")
    producer = read_json(root / "compositor-runtime-producer-receipt.json", "producer receipt")
    family, family_version, distro, distro_version, package_format = runtime_identity(desktop, architecture)
    required = {"schema", "commit", "producer_run_id", "source_run_id", "version", "architecture", "desktop", "family", "distribution", "format", "runtime_receipt", "package", "source", "glibc_floor", "runtime_id", "upstream_licenses"}
    if set(producer) != required or producer.get("schema") != 1:
        raise ValueError("producer receipt schema is invalid")
    if (producer.get("commit") != commit or str(producer.get("producer_run_id")) != producer_run_id
            or producer.get("architecture") != architecture or producer.get("desktop") != desktop
            or producer.get("family") != family or producer.get("format") != package_format
            or producer.get("distribution") != {"id": distro, "version": distro_version}
            or not isinstance(producer.get("version"), str) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", producer["version"])
            or not isinstance(producer.get("runtime_id"), str) or not re.fullmatch(r"[a-z0-9][a-z0-9.-]{1,63}", producer["runtime_id"])
            or not isinstance(producer.get("glibc_floor"), str) or not re.fullmatch(r"[0-9]+\.[0-9]+", producer["glibc_floor"])):
        raise ValueError("producer receipt identity differs from the qualification target")
    source = producer["source"]
    expected_source_keys = {"revision", "patch_sha256", "shell_revision"} if desktop == "GNOME" else {"revision", "patch_sha256"}
    if not isinstance(source, dict) or set(source) != expected_source_keys or not isinstance(source["revision"], str) or not SHA256.fullmatch(source.get("patch_sha256", "")):
        raise ValueError("producer receipt source provenance is invalid")
    verify_baseline_source(desktop, source)
    runtime = producer["runtime_receipt"]
    package = producer["package"]
    for label, record in (("runtime receipt", runtime), ("package", package)):
        if not isinstance(record, dict) or set(record) != {"name", "sha256", "size_bytes"} or not SHA256.fullmatch(record.get("sha256", "")) or type(record.get("size_bytes")) is not int or record["size_bytes"] <= 0:
            raise ValueError(f"producer {label} record is invalid")
    runtime_path = safe_file(root, runtime["name"], "runtime receipt")
    package_path = safe_file(root, package["name"], "companion package")
    if runtime_path.stat().st_size != runtime["size_bytes"] or sha256(runtime_path) != runtime["sha256"] or package_path.stat().st_size != package["size_bytes"] or sha256(package_path) != package["sha256"]:
        raise ValueError("producer receipt does not bind downloaded runtime bytes")
    if package_path.suffix != "." + package_format or package_path.name != f"copypaste-compositor-runtime-{producer['runtime_id']}-v{producer['version']}-linux-{architecture}.{package_format}":
        raise ValueError("companion package filename differs from producer receipt")
    checksum_path = safe_file(root, package_path.name + ".sha256", "companion package checksum")
    if checksum_path.read_text(encoding="utf-8") != f"{package['sha256']}  {package_path.name}\n":
        raise ValueError("companion package checksum file differs")
    licenses = producer["upstream_licenses"]
    if not isinstance(licenses, list) or not licenses:
        raise ValueError("producer receipt has no upstream license records")
    for license_meta in licenses:
        if not isinstance(license_meta, dict) or set(license_meta) != {"name", "sha256", "size_bytes"}:
            raise ValueError("producer license metadata is invalid")
        license_path = safe_file(root, license_meta["name"], "upstream license")
        if license_meta != {"name": license_path.name, "sha256": sha256(license_path), "size_bytes": license_path.stat().st_size}:
            raise ValueError("producer license bytes differ")
    module = stage_runtime_module()
    receipt = module.read_receipt(runtime_path)
    module.validate_receipt(receipt)
    runtime_input = root / "runtime"
    if not runtime_input.is_dir() or runtime_input.is_symlink():
        raise ValueError("immutable runtime payload is missing or unsafe")
    module.validate_payload(runtime_input, receipt)
    if any(receipt.get(key) != producer.get(key) for key in ("runtime_id", "architecture", "desktop", "distribution", "source", "glibc_floor")):
        raise ValueError("immutable runtime receipt differs from producer receipt")
    verify_baseline_source(desktop, receipt["source"])
    return {"schema": 1, "producer_run_id": producer_run_id, "commit": commit, "runtime_id": producer["runtime_id"], "desktop": desktop, "architecture": architecture, "distribution": producer["distribution"], "format": package_format, "package": package, "runtime_receipt": runtime}


def verify_installed(binding_value: dict, installed_root: Path) -> None:
    required = {"schema", "producer_run_id", "commit", "runtime_id", "desktop", "architecture", "distribution", "format", "package", "runtime_receipt"}
    if set(binding_value) != required or binding_value.get("schema") != 1:
        raise ValueError("runtime binding is invalid")
    runtime_id = binding_value["runtime_id"]
    receipt_path = installed_root / "usr/share/copypaste/compositor-runtime" / f"{runtime_id}.receipt.json"
    if not receipt_path.is_file() or receipt_path.is_symlink() or sha256(receipt_path) != binding_value["runtime_receipt"]["sha256"]:
        raise ValueError("installed compositor runtime receipt differs from authenticated input")
    runtime = installed_root / "usr/lib/copypaste/compositor-runtime" / runtime_id
    launcher = installed_root / "usr/lib/copypaste/compositor-runtime/bin" / f"copypaste-compositor-session-{runtime_id}"
    descriptor = installed_root / "usr/share/wayland-sessions" / f"copypaste-{runtime_id}.desktop"
    if not runtime.is_dir() or runtime.is_symlink() or not launcher.is_file() or launcher.is_symlink() or launcher.stat().st_mode & 0o111 == 0 or not descriptor.is_file() or descriptor.is_symlink():
        raise ValueError("installed compositor runtime launcher is incomplete")
    module = stage_runtime_module()
    receipt = module.read_receipt(receipt_path)
    module.validate_receipt(receipt)
    if any(receipt.get(key) != binding_value.get(key) for key in ("runtime_id", "desktop", "architecture", "distribution")):
        raise ValueError("installed runtime receipt identity differs from authenticated input")
    module.validate_payload(runtime, receipt)
    if launcher.read_text(encoding="utf-8") != module.launcher_text(receipt, "/usr/lib/copypaste/compositor-runtime"):
        raise ValueError("installed compositor launcher differs from authenticated deterministic launcher")
    if descriptor.read_text(encoding="utf-8") != session_descriptor_text(receipt):
        raise ValueError("installed session descriptor differs from authenticated deterministic descriptor")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    source = commands.add_parser("source")
    source.add_argument("--run", required=True, type=Path); source.add_argument("--artifacts", required=True, type=Path)
    source.add_argument("--repository", required=True); source.add_argument("--commit", required=True)
    artifact = commands.add_parser("artifact")
    artifact.add_argument("--root", required=True, type=Path); artifact.add_argument("--desktop", required=True, choices=("GNOME", "KDE")); artifact.add_argument("--architecture", required=True, choices=("x86_64", "aarch64")); artifact.add_argument("--commit", required=True); artifact.add_argument("--producer-run-id", required=True); artifact.add_argument("--output", required=True, type=Path)
    installed = commands.add_parser("installed")
    installed.add_argument("--binding", required=True, type=Path); installed.add_argument("--root", required=True, type=Path)
    args = parser.parse_args()
    if args.command == "source":
        verify_source(read_json(args.run, "runtime producer run"), read_json(args.artifacts, "runtime producer artifacts"), args.repository, args.commit)
    elif args.command == "artifact":
        value = binding(args.root, args.desktop, args.architecture, args.commit, args.producer_run_id)
        args.output.write_text(json.dumps(value, sort_keys=True) + "\n", encoding="utf-8")
    else:
        verify_installed(read_json(args.binding, "runtime binding"), args.root)


if __name__ == "__main__":
    main()
