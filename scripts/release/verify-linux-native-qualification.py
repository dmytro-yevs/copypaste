#!/usr/bin/env python3
"""Verify full Linux desktop evidence against the exact release artifacts."""

import argparse
import hashlib
import json
import re
from pathlib import Path, PurePosixPath
from typing import Optional


ARCHITECTURES = ("x86_64", "aarch64")
DESKTOPS = ("GNOME", "KDE")
SESSIONS = ("x11", "wayland")
FORMATS = ("AppImage", "deb", "rpm")
ASSERTIONS = {
    "package_install", "package_upgrade", "desktop_uri_icon", "daemon_cli_gui",
    "clipboard_text", "clipboard_html", "clipboard_rtf", "clipboard_png",
    "clipboard_tiff", "clipboard_file", "privacy_confidential",
    "privacy_excluded_app", "privacy_private_mode", "quick_paste_hotkey",
    "quick_paste_focus_restore", "tray_window_notification",
    "encrypted_restart_persistence", "pairing_sync", "modules",
}
X11_KEYBOARD_ASSERTION = "native_x11_keyboard_input"
WAYLAND_KEYBOARD_ASSERTION = "portal_keyboard_grant"
FIRST_INSTALL_ASSERTIONS = (ASSERTIONS - {"package_upgrade"}) | {"clean_install_baseline"}
EVIDENCE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
TRACE_PLACEHOLDERS = {
    "external-gtk-clipboard",
    "desktop entry URI handler and icon verified",
}
MODULE_IDS = ("copypaste.ocr", "copypaste.semantic-search", "copypaste.supabase")
IPC_OPERATIONS = {"list", "install", "set_preferences", "set_enabled", "invoke", "remove"}


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(block)
    return hasher.hexdigest()


def regular_file(root: Path, name: str) -> Path:
    parts = PurePosixPath(name).parts if isinstance(name, str) else ()
    if not parts or any(part in (".", "..") or not EVIDENCE_NAME.fullmatch(part) for part in parts):
        raise ValueError("evidence attachment has an unsafe name")
    path = root / name
    try:
        resolved = path.resolve(strict=True)
    except FileNotFoundError as error:
        raise ValueError(f"evidence attachment is missing: {name}") from error
    if root.resolve() not in resolved.parents or path.is_symlink() or not resolved.is_file():
        raise ValueError(f"evidence attachment is not a regular root file: {name}")
    return resolved


def expected_packages(artifacts: Path, version: str, architecture: str) -> dict[str, dict]:
    expected = {}
    for format_name in FORMATS:
        name = f"CopyPaste-v{version}-linux-{architecture}.{format_name}"
        path = artifacts / name
        if not path.is_file() or path.is_symlink():
            raise ValueError(f"exact Linux artifact is missing or unsafe: {name}")
        expected[name] = {"sha256": digest(path), "size_bytes": path.stat().st_size}
    return expected


def assertion_set(upgrade_mode: object, session: object) -> set[str]:
    if upgrade_mode == "prior_release":
        assertions = ASSERTIONS
    elif upgrade_mode == "first_install_baseline":
        assertions = FIRST_INSTALL_ASSERTIONS
    else:
        raise ValueError("evidence upgrade mode is invalid")
    if session == "x11":
        return assertions | {X11_KEYBOARD_ASSERTION}
    if session == "wayland":
        return assertions | {WAYLAND_KEYBOARD_ASSERTION}
    raise ValueError("evidence session is invalid")


def verify_compositor_runtime(binding, receipt, native_format):
    required = {"schema", "producer_run_id", "commit", "runtime_id", "desktop", "architecture", "distribution", "format", "package", "runtime_receipt"}
    environment = receipt.get("environment")
    if (not isinstance(binding, dict) or set(binding) != required or binding.get("schema") != 1
            or binding.get("commit") != receipt.get("commit") or binding.get("desktop") != receipt.get("desktop")
            or binding.get("architecture") != receipt.get("architecture") or not isinstance(environment, dict)
            or binding.get("distribution") != environment.get("distribution") or binding.get("format") != native_format
            or not isinstance(binding.get("producer_run_id"), str) or not re.fullmatch(r"[1-9][0-9]*", binding["producer_run_id"])
            or not isinstance(binding.get("runtime_id"), str) or not binding["runtime_id"]):
        raise ValueError("compositor runtime binding differs from its scenario")
    for field in ("package", "runtime_receipt"):
        item = binding.get(field)
        if (not isinstance(item, dict) or set(item) != {"name", "sha256", "size_bytes"}
                or not isinstance(item["name"], str) or not EVIDENCE_NAME.fullmatch(item["name"])
                or not isinstance(item["sha256"], str) or not re.fullmatch(r"[0-9a-f]{64}", item["sha256"])
                or not isinstance(item["size_bytes"], int) or item["size_bytes"] <= 0):
            raise ValueError("compositor runtime binding artifact is invalid")


def verify_trace(root: Path, receipt: dict, architecture: str, desktop: str, session: str, version: str, commit: str, source_run_id: str, artifact_run_id: Optional[str], expected_assertions: set[str]) -> None:
    name = f"linux-native-{architecture}-{desktop.lower()}-{session}.trace.json"
    path = regular_file(root, name)
    trace = json.loads(path.read_text(encoding="utf-8"))
    identity = {
        "schema": 1, "version": version, "commit": commit,
        "source_run_id": str(source_run_id), "architecture": architecture,
        "desktop": desktop, "session": session,
    }
    if {key: trace.get(key) for key in identity} != identity:
        raise ValueError(f"scenario trace provenance differs: {name}")
    if artifact_run_id is not None and str(trace.get("artifact_run_id")) != str(artifact_run_id):
        raise ValueError(f"scenario trace artifact provenance differs: {name}")
    commands = trace.get("commands")
    if not isinstance(commands, list) or not commands:
        raise ValueError(f"scenario trace has no executed commands: {name}")
    covered = set()
    for command in commands:
        if not isinstance(command, dict) or command.get("returncode") != 0:
            raise ValueError(f"scenario trace has a failed command: {name}")
        argv = command.get("argv")
        assertions = command.get("assertions")
        if not isinstance(argv, list) or not argv or not all(isinstance(value, str) and value and len(value) <= 256 and "\n" not in value and "\r" not in value for value in argv):
            raise ValueError(f"scenario trace command is invalid: {name}")
        if argv[:2] == ["unix-ipc", "modules"]:
            raise ValueError(f"scenario trace must use typed IPC records: {name}")
        if any(value in TRACE_PLACEHOLDERS for value in argv) or argv[:2] == ["sh", "-ceu"]:
            raise ValueError(f"scenario trace command is a placeholder: {name}")
        if not isinstance(assertions, list) or not assertions or not all(item in expected_assertions for item in assertions):
            raise ValueError(f"scenario trace assertions are invalid: {name}")
        covered.update(assertions)
    ipc = trace.get("ipc")
    if not isinstance(ipc, list):
        raise ValueError(f"scenario trace has no typed IPC results: {name}")
    required = {
        "endpoint_category", "method", "operation", "request_sha256",
        "response_sha256", "success", "assertions",
    }
    module_rows = []
    for row in ipc:
        if not isinstance(row, dict) or not required <= set(row) or set(row) - (required | {"module_id", "enabled"}):
            raise ValueError(f"scenario typed IPC record is invalid: {name}")
        if (row["endpoint_category"] != "gui_owned_unix_socket" or row["method"] != "modules"
                or row["operation"] not in IPC_OPERATIONS or row["success"] is not True):
            raise ValueError(f"scenario typed IPC result is invalid: {name}")
        if not all(isinstance(row[key], str) and re.fullmatch(r"[0-9a-f]{64}", row[key]) for key in ("request_sha256", "response_sha256")):
            raise ValueError(f"scenario typed IPC digests are invalid: {name}")
        module_id = row.get("module_id")
        if row["operation"] == "list":
            if module_id is not None or "enabled" in row:
                raise ValueError(f"module list IPC record has lifecycle data: {name}")
        elif module_id not in MODULE_IDS:
            raise ValueError(f"module IPC record has an unknown module identity: {name}")
        if "enabled" in row and (row["operation"] != "set_enabled" or not isinstance(row["enabled"], bool)):
            raise ValueError(f"module IPC enabled state is invalid: {name}")
        assertions = row["assertions"]
        if not isinstance(assertions, list) or not assertions or not all(item in expected_assertions for item in assertions):
            raise ValueError(f"scenario typed IPC assertions are invalid: {name}")
        covered.update(assertions)
        if "modules" in assertions:
            module_rows.append(row)
    if "modules" in expected_assertions:
        if not module_rows:
            raise ValueError(f"scenario trace has no module lifecycle IPC evidence: {name}")
        for module_id in MODULE_IDS:
            rows = [row for row in module_rows if row.get("module_id") == module_id]
            if {row["operation"] for row in rows} < {"install", "invoke", "remove", "set_enabled"}:
                raise ValueError(f"scenario trace module lifecycle is incomplete for {module_id}: {name}")
            if {row.get("enabled") for row in rows if row["operation"] == "set_enabled"} != {True, False}:
                raise ValueError(f"scenario trace module enable lifecycle is incomplete for {module_id}: {name}")
        if not any(row.get("module_id") == "copypaste.semantic-search" and row["operation"] == "set_preferences" for row in module_rows):
            raise ValueError(f"scenario trace semantic setup is missing: {name}")
    if covered != expected_assertions:
        raise ValueError(f"scenario trace does not execute every assertion: {name}")
    installation = trace.get("installation")
    required_installation = {"formats", "packages", "appimage_extract_argv", "native_install_argv"}
    if not isinstance(installation, dict) or set(installation) != required_installation:
        raise ValueError(f"scenario trace has no actual package installation record: {name}")
    formats = installation["formats"]
    if formats not in (["AppImage", "deb"], ["AppImage", "rpm"]) or formats != receipt.get("installed_formats"):
        raise ValueError(f"scenario trace installation formats differ: {name}")
    expected_packages = {item.get("name"): item for item in receipt.get("packages", []) if isinstance(item, dict)}
    selected = {package_name for package_name in expected_packages if package_name.rsplit(".", 1)[-1] in formats}
    reported = installation["packages"]
    reported_map = {item.get("name"): item for item in reported if isinstance(item, dict)} if isinstance(reported, list) else {}
    if len(reported_map) != len(reported) or set(reported_map) != selected:
        raise ValueError(f"scenario trace installation packages differ: {name}")
    for package_name, item in reported_map.items():
        format_name = package_name.rsplit(".", 1)[-1]
        source_path = item.get("path")
        if (not isinstance(source_path, str) or not Path(source_path).is_absolute() or Path(source_path).name != package_name
                or item != {"format": format_name, "path": source_path, **expected_packages[package_name]}):
            raise ValueError(f"scenario trace installation digest differs: {name}")
    appimage = next(item for item in reported if item["format"] == "AppImage")
    native = next(item for item in reported if item["format"] != "AppImage")
    appimage_argv = installation["appimage_extract_argv"]
    native_argv = installation["native_install_argv"]
    if appimage_argv != ["env", "APPIMAGE_EXTRACT_AND_RUN=1", appimage["path"], "--appimage-extract"]:
        raise ValueError(f"scenario trace installation commands are invalid: {name}")
    expected_native_argv = (["sudo", "apt-get", "install", "--yes", native["path"]]
                            if native["format"] == "deb" else ["sudo", "dnf", "--assumeyes", "install", native["path"]])
    if native_argv != expected_native_argv:
        raise ValueError(f"scenario trace native package manager command differs: {name}")
    if not any(command["argv"] == appimage_argv and "package_install" in command["assertions"] for command in commands):
        raise ValueError(f"scenario trace AppImage installation command was not executed: {name}")
    if not any(command["argv"] == native_argv and "package_install" in command["assertions"] for command in commands):
        raise ValueError(f"scenario trace native installation command was not executed: {name}")
    runtimes = trace.get("runtimes")
    if not isinstance(runtimes, list) or len(runtimes) != 2 or [row.get("format") if isinstance(row, dict) else None for row in runtimes] != formats:
        raise ValueError(f"scenario trace has incomplete runtime records: {name}")
    expected_names = {"copypaste", "copypaste-daemon", "copypaste-cli"}
    baseline = None
    for runtime in runtimes:
        if set(runtime) != {"format", "gui_owned_daemon", "executables"} or runtime["gui_owned_daemon"] is not True:
            raise ValueError(f"scenario runtime record is invalid: {name}")
        executables = runtime["executables"]
        if not isinstance(executables, dict) or set(executables) != expected_names:
            raise ValueError(f"scenario runtime executable inventory is incomplete: {name}")
        identity = {}
        for executable, item in executables.items():
            if (not isinstance(item, dict) or set(item) != {"path", "sha256", "size_bytes"}
                    or not isinstance(item["path"], str) or not Path(item["path"]).is_absolute()
                    or Path(item["path"]).name != executable or not isinstance(item["size_bytes"], int)
                    or item["size_bytes"] <= 0 or not isinstance(item["sha256"], str)
                    or not re.fullmatch(r"[0-9a-f]{64}", item["sha256"])):
                raise ValueError(f"scenario runtime executable provenance is invalid: {name}")
            if ((runtime["format"] == "AppImage" and "/squashfs-root/usr/lib/copypaste/" not in item["path"])
                    or (runtime["format"] != "AppImage" and item["path"] != f"/usr/lib/copypaste/{executable}")):
                raise ValueError(f"scenario runtime record does not identify its installed format: {name}")
            identity[executable] = (item["sha256"], item["size_bytes"])
        if baseline is None:
            baseline = identity
        elif identity != baseline:
            raise ValueError(f"portable and installed-native runtime executables differ: {name}")
    if receipt.get("trace") != {"name": name, "sha256": digest(path), "size_bytes": path.stat().st_size}:
        raise ValueError(f"receipt does not bind its scenario trace: {name}")


def verify(root: Path, artifacts: Path, version: str, commit: str, source_run_id: str, artifact_run_id: Optional[str] = None) -> None:
    root = root.resolve(strict=True)
    artifacts = artifacts.resolve(strict=True)
    seen = set()
    installed_coverage = {(architecture, format_name): 0 for architecture in ARCHITECTURES for format_name in FORMATS}
    for architecture in ARCHITECTURES:
        expected = expected_packages(artifacts, version, architecture)
        for desktop in DESKTOPS:
            for session in SESSIONS:
                name = f"linux-native-{architecture}-{desktop.lower()}-{session}.json"
                receipt_path = regular_file(root, name)
                receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
                identity = (receipt.get("architecture"), receipt.get("desktop"), receipt.get("session"))
                if identity != (architecture, desktop, session):
                    raise ValueError(f"invalid evidence identity: {name}")
                if (receipt.get("schema"), receipt.get("version"), receipt.get("commit"), str(receipt.get("source_run_id"))) != (1, version, commit, str(source_run_id)):
                    raise ValueError(f"evidence provenance differs: {name}")
                if artifact_run_id is not None and str(receipt.get("artifact_run_id")) != str(artifact_run_id):
                    raise ValueError(f"evidence artifact provenance differs: {name}")
                installed_formats = receipt.get("installed_formats")
                environment = receipt.get("environment")
                if not isinstance(installed_formats, list) or set(installed_formats) not in ({"AppImage", "deb"}, {"AppImage", "rpm"}):
                    raise ValueError(f"evidence installed formats are invalid: {name}")
                if not isinstance(environment, dict) or not isinstance(environment.get("distribution"), str) or not isinstance(environment.get("distribution_version"), str):
                    raise ValueError(f"evidence runtime environment is invalid: {name}")
                native_format = next(format_name for format_name in installed_formats if format_name != "AppImage")
                if (native_format == "deb" and environment["distribution"] not in {"ubuntu", "debian"}) or (native_format == "rpm" and environment["distribution"] != "fedora"):
                    raise ValueError(f"evidence native package manager differs from runtime: {name}")
                trace_path = regular_file(root, f"linux-native-{architecture}-{desktop.lower()}-{session}.trace.json")
                trace = json.loads(trace_path.read_text(encoding="utf-8"))
                if receipt.get("compositor_runtime") != trace.get("compositor_runtime"):
                    raise ValueError(f"compositor runtime binding differs from trace: {name}")
                verify_compositor_runtime(receipt.get("compositor_runtime"), receipt, native_format)
                for format_name in installed_formats:
                    installed_coverage[(architecture, format_name)] += 1
                expected_assertions = assertion_set(receipt.get("upgrade_mode"), session)
                assertions = receipt.get("assertions")
                if not isinstance(assertions, dict) or set(assertions) != expected_assertions or any(value is not True for value in assertions.values()):
                    raise ValueError(f"evidence assertions are incomplete: {name}")
                packages = receipt.get("packages")
                if not isinstance(packages, list) or len(packages) != len(expected):
                    raise ValueError(f"evidence package inventory is incomplete: {name}")
                package_map = {package.get("name"): package for package in packages if isinstance(package, dict)}
                if len(package_map) != len(packages) or set(package_map) != set(expected):
                    raise ValueError(f"evidence package inventory does not match its architecture: {name}")
                for package_name, package in package_map.items():
                    if package != {"name": package_name, **expected[package_name]}:
                        raise ValueError(f"evidence package does not bind exact artifact bytes: {name}")
                evidence = receipt.get("evidence")
                if not isinstance(evidence, list) or not evidence:
                    raise ValueError(f"evidence attachments are missing: {name}")
                evidence_names = set()
                for item in evidence:
                    if not isinstance(item, dict) or set(item) != {"name", "sha256", "size_bytes"}:
                        raise ValueError(f"evidence attachment metadata is invalid: {name}")
                    path = regular_file(root, item["name"])
                    if item["name"] in evidence_names or item["sha256"] != digest(path) or item["size_bytes"] != path.stat().st_size:
                        raise ValueError(f"evidence attachment differs from its receipt: {name}")
                    evidence_names.add(item["name"])
                verify_trace(root, receipt, architecture, desktop, session, version, commit, source_run_id, artifact_run_id, expected_assertions)
                seen.add(identity)
    if len(seen) != len(ARCHITECTURES) * len(DESKTOPS) * len(SESSIONS):
        raise ValueError("native qualification matrix is incomplete")
    if any(count < 2 for count in installed_coverage.values()):
        raise ValueError("native package formats were not installed in two desktop sessions")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--artifacts", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--source-run-id", required=True)
    parser.add_argument("--artifact-run-id")
    args = parser.parse_args()
    verify(args.root, args.artifacts, args.version, args.commit, args.source_run_id, args.artifact_run_id)
    print("verified full exact-artifact Linux desktop qualification")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
