#!/usr/bin/env python3
"""Run one Linux desktop scenario and write its exact-artifact receipt.

The driver is a repository-controlled executable.  It writes one JSON object
per line to stdout after each command it ran; this program owns the receipt,
package digests, trace, and attachment digests so a workflow input cannot
manufacture a successful matrix row.
"""

import argparse
import hashlib
import json
import os
import re
import subprocess
from pathlib import Path


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
SAFE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
TRACE_PLACEHOLDERS = {
    "external-gtk-clipboard",
    "desktop entry URI handler and icon verified",
}
IPC_PREFIX = "COPYPASTE_QUALIFICATION_IPC "
INSTALL_PREFIX = "COPYPASTE_QUALIFICATION_INSTALL "
MODULE_IDS = ("copypaste.ocr", "copypaste.semantic-search", "copypaste.supabase")
IPC_OPERATIONS = {"list", "install", "set_preferences", "set_enabled", "invoke", "remove"}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def artifact_inventory(directory: Path, version: str, architecture: str) -> list[dict]:
    inventory = []
    for extension in FORMATS:
        path = directory / f"CopyPaste-v{version}-linux-{architecture}.{extension}"
        if not path.is_file() or path.is_symlink():
            raise ValueError(f"missing exact {architecture} package: {path.name}")
        inventory.append({"name": path.name, "sha256": sha256(path), "size_bytes": path.stat().st_size})
    return inventory


def command_rows(stdout: str, expected_assertions: set[str]) -> list[dict]:
    rows = []
    for line in stdout.splitlines():
        if not line:
            continue
        if line.startswith(IPC_PREFIX) or line.startswith(INSTALL_PREFIX):
            continue
        if not line.startswith("COPYPASTE_QUALIFICATION_COMMAND "):
            raise ValueError("scenario driver emitted unstructured output")
        row = json.loads(line.removeprefix("COPYPASTE_QUALIFICATION_COMMAND "))
        if not isinstance(row, dict) or set(row) != {"argv", "returncode", "assertions"}:
            raise ValueError("scenario driver emitted an invalid command trace")
        if row["returncode"] != 0 or not isinstance(row["argv"], list) or not row["argv"]:
            raise ValueError("scenario driver reported a failed command")
        if not all(isinstance(value, str) and value and len(value) <= 256 and "\n" not in value and "\r" not in value for value in row["argv"]):
            raise ValueError("scenario driver emitted unsafe command arguments")
        if row["argv"][:2] == ["unix-ipc", "modules"]:
            raise ValueError("typed IPC must use a typed IPC trace record")
        if any(value in TRACE_PLACEHOLDERS for value in row["argv"]) or row["argv"][:2] == ["sh", "-ceu"]:
            raise ValueError("scenario driver emitted a placeholder command trace")
        if not isinstance(row["assertions"], list) or not row["assertions"] or not all(item in expected_assertions for item in row["assertions"]):
            raise ValueError("scenario driver emitted invalid assertion coverage")
        rows.append(row)
    if not rows:
        raise ValueError("scenario driver produced no executed-command trace")
    return rows


def ipc_rows(stdout: str, expected_assertions: set[str]) -> list[dict]:
    rows = []
    required = {
        "endpoint_category", "method", "operation", "request_sha256",
        "response_sha256", "success", "assertions",
    }
    for line in stdout.splitlines():
        if not line.startswith(IPC_PREFIX):
            continue
        row = json.loads(line.removeprefix(IPC_PREFIX))
        if not isinstance(row, dict) or not required <= set(row) or set(row) - (required | {"module_id", "enabled"}):
            raise ValueError("scenario driver emitted an invalid typed IPC trace")
        if (row["endpoint_category"] != "gui_owned_unix_socket" or row["method"] != "modules"
                or row["operation"] not in IPC_OPERATIONS or row["success"] is not True):
            raise ValueError("scenario driver emitted an invalid typed IPC result")
        if not all(isinstance(row[name], str) and re.fullmatch(r"[0-9a-f]{64}", row[name]) for name in ("request_sha256", "response_sha256")):
            raise ValueError("scenario driver emitted invalid typed IPC digests")
        module_id = row.get("module_id")
        if row["operation"] == "list":
            if module_id is not None or "enabled" in row:
                raise ValueError("module list IPC trace has unexpected lifecycle data")
        elif module_id not in MODULE_IDS:
            raise ValueError("module IPC trace has an unknown module identity")
        if "enabled" in row and (row["operation"] != "set_enabled" or not isinstance(row["enabled"], bool)):
            raise ValueError("module IPC trace has an invalid enabled state")
        assertions = row["assertions"]
        if not isinstance(assertions, list) or not assertions or not all(item in expected_assertions for item in assertions):
            raise ValueError("scenario driver emitted invalid typed IPC assertion coverage")
        rows.append(row)
    return rows


def trace_rows(stdout: str, expected_assertions: set[str]) -> tuple[list[dict], list[dict]]:
    commands = command_rows(stdout, expected_assertions)
    ipc = ipc_rows(stdout, expected_assertions)
    covered = {item for row in commands + ipc for item in row["assertions"]}
    if covered != expected_assertions:
        raise ValueError("scenario driver did not execute every required assertion")
    if "modules" in expected_assertions:
        module_rows = [row for row in ipc if "modules" in row["assertions"]]
        if not module_rows:
            raise ValueError("module qualification has no typed IPC trace")
        for module_id in MODULE_IDS:
            operations = [row for row in module_rows if row.get("module_id") == module_id]
            if {row["operation"] for row in operations} < {"install", "invoke", "remove", "set_enabled"}:
                raise ValueError(f"module qualification lifecycle is incomplete for {module_id}")
            if {row.get("enabled") for row in operations if row["operation"] == "set_enabled"} != {True, False}:
                raise ValueError(f"module qualification did not enable and disable {module_id}")
        if not any(row.get("module_id") == "copypaste.semantic-search" and row["operation"] == "set_preferences" for row in module_rows):
            raise ValueError("module qualification did not configure semantic search through typed IPC")
    return commands, ipc


def installation_report(stdout: str, expected_packages: list[dict], artifacts: Path) -> dict:
    records = [json.loads(line.removeprefix(INSTALL_PREFIX)) for line in stdout.splitlines() if line.startswith(INSTALL_PREFIX)]
    if len(records) != 1:
        raise ValueError("scenario driver must report exactly one actual package installation")
    report = records[0]
    required = {"formats", "packages", "appimage_extract_argv", "native_install_argv"}
    if not isinstance(report, dict) or set(report) != required:
        raise ValueError("scenario driver emitted an invalid package installation report")
    formats = report["formats"]
    if formats not in (["AppImage", "deb"], ["AppImage", "rpm"]):
        raise ValueError("actual package installation report has invalid formats")
    expected = {item["name"]: item for item in expected_packages}
    selected = {name for name in expected if name.rsplit(".", 1)[-1] in formats}
    packages = report["packages"]
    package_map = {item.get("name"): item for item in packages if isinstance(item, dict)} if isinstance(packages, list) else {}
    if len(package_map) != len(packages) or set(package_map) != selected:
        raise ValueError("actual package installation report does not bind exact installed artifacts")
    for name, item in package_map.items():
        format_name = name.rsplit(".", 1)[-1]
        expected_path = str((artifacts / name).resolve())
        if item != {"format": format_name, "path": expected_path, **expected[name]}:
            raise ValueError("actual package installation report has an invalid artifact digest")
    appimage = next(item for item in packages if item["format"] == "AppImage")
    native = next(item for item in packages if item["format"] != "AppImage")
    appimage_path = appimage["path"]
    native_path = native["path"]
    if report["appimage_extract_argv"] != ["env", "APPIMAGE_EXTRACT_AND_RUN=1", appimage_path, "--appimage-extract"]:
        raise ValueError("actual package installation report has no exact AppImage extraction command")
    expected_native = (
        ["sudo", "apt-get", "install", "--yes", native_path]
        if native["format"] == "deb" else ["sudo", "dnf", "--assumeyes", "install", native_path]
    )
    if report["native_install_argv"] != expected_native:
        raise ValueError("actual package installation report has no exact native package command")
    return report


def scenario_assertions(upgrade_mode: str, session: str) -> set[str]:
    assertions = ASSERTIONS if upgrade_mode == "prior_release" else FIRST_INSTALL_ASSERTIONS
    if session == "x11":
        return assertions | {X11_KEYBOARD_ASSERTION}
    if session == "wayland":
        return assertions | {WAYLAND_KEYBOARD_ASSERTION}
    raise ValueError("unsupported session assertion contract")


def attachment(path: Path) -> dict:
    if not path.is_file() or path.is_symlink() or not SAFE_NAME.fullmatch(path.name):
        raise ValueError(f"unsafe evidence attachment: {path}")
    return {"name": path.name, "sha256": sha256(path), "size_bytes": path.stat().st_size}


def runtime_environment() -> dict:
    values = {}
    os_release = Path("/etc/os-release")
    if not os_release.is_file():
        return {"distribution": "unknown", "distribution_version": "unknown"}
    for line in os_release.read_text(encoding="utf-8").splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            values[key] = value.strip('"')
    return {"distribution": values.get("ID"), "distribution_version": values.get("VERSION_ID")}


def staged_directory(path: Path, label: str) -> Path:
    try:
        resolved = path.resolve(strict=True)
    except FileNotFoundError as error:
        raise ValueError(f"{label} staging directory is missing") from error
    if path.is_symlink() or not resolved.is_dir():
        raise ValueError(f"{label} staging directory is unsafe")
    return resolved


def produce(args: argparse.Namespace) -> Path:
    if args.architecture not in ARCHITECTURES or args.desktop not in DESKTOPS or args.session not in SESSIONS:
        raise ValueError("unsupported Linux qualification matrix coordinate")
    if not re.fullmatch(r"[0-9a-f]{40}", args.commit):
        raise ValueError("commit must be a full lowercase Git SHA")
    if not re.fullmatch(r"[1-9][0-9]*", args.source_run_id):
        raise ValueError("source run ID is invalid")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    module_artifacts = staged_directory(args.module_artifacts, "module artifact")
    module_fixtures = staged_directory(args.module_fixtures, "module fixture")
    packages = artifact_inventory(args.artifacts.resolve(), args.version, args.architecture)
    upgrade_mode = "prior_release" if args.previous_artifacts is not None else "first_install_baseline"
    if upgrade_mode == "prior_release":
        if args.previous_version is None:
            raise ValueError("prior-release qualification requires a previous version")
        previous = artifact_inventory(args.previous_artifacts.resolve(), args.previous_version, args.architecture)
        expected_assertions = scenario_assertions(upgrade_mode, args.session)
    else:
        if args.previous_version is not None:
            raise ValueError("first-install baseline cannot name a prior version")
        previous = []
        expected_assertions = scenario_assertions(upgrade_mode, args.session)
    driver = args.driver.resolve()
    if not driver.is_file() or not os.access(driver, os.X_OK):
        raise ValueError("repository-controlled scenario driver is missing or not executable")
    result = subprocess.run(
        [str(driver), "--artifacts", str(args.artifacts.resolve()), "--version", args.version,
         "--architecture", args.architecture, "--desktop", args.desktop, "--session", args.session,
         "--evidence-dir", str(output), "--module-artifacts", str(module_artifacts),
         "--module-fixtures", str(module_fixtures)] + (
            ["--previous-artifacts", str(args.previous_artifacts.resolve()), "--previous-version", args.previous_version]
            if upgrade_mode == "prior_release" else ["--first-install-baseline"]
        ),
        text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False,
    )
    log_name = f"linux-native-{args.architecture}-{args.desktop.lower()}-{args.session}.driver.log"
    log_path = output / log_name
    log_path.write_text(result.stdout, encoding="utf-8")
    if result.returncode:
        raise RuntimeError(f"native scenario driver failed; full output: {log_path}")
    commands, ipc = trace_rows(result.stdout, expected_assertions)
    installation = installation_report(result.stdout, packages, args.artifacts.resolve())
    for argv in (installation["appimage_extract_argv"], installation["native_install_argv"]):
        if not any(row["argv"] == argv and "package_install" in row["assertions"] for row in commands):
            raise ValueError("actual package installation report lacks its executed command trace")
    installed_formats = installation["formats"]
    trace_name = f"linux-native-{args.architecture}-{args.desktop.lower()}-{args.session}.trace.json"
    trace_path = output / trace_name
    trace_path.write_text(json.dumps({
        "schema": 1, "version": args.version, "commit": args.commit, "source_run_id": args.source_run_id,
        "artifact_run_id": args.artifact_run_id,
        "architecture": args.architecture, "desktop": args.desktop, "session": args.session,
        "upgrade_mode": upgrade_mode,
        "installed_formats": installed_formats,
        "installation": installation,
        "environment": runtime_environment(),
        "commands": commands,
        "ipc": ipc,
    }, indent=2) + "\n", encoding="utf-8")
    attachments = [attachment(log_path), attachment(trace_path)]
    for path in sorted(output.iterdir()):
        if path in (log_path, trace_path):
            continue
        if path.is_file() and not path.is_symlink():
            attachments.append(attachment(path))
    receipt_name = f"linux-native-{args.architecture}-{args.desktop.lower()}-{args.session}.json"
    receipt_path = output / receipt_name
    receipt_path.write_text(json.dumps({
        "schema": 1, "version": args.version, "commit": args.commit, "source_run_id": args.source_run_id,
        "artifact_run_id": args.artifact_run_id,
        "architecture": args.architecture, "desktop": args.desktop, "session": args.session,
        "upgrade_mode": upgrade_mode,
        "installed_formats": installed_formats,
        "environment": runtime_environment(),
        "packages": packages,
        "previous_packages": previous,
        "assertions": {name: True for name in sorted(expected_assertions)},
        "trace": attachment(trace_path),
        "evidence": attachments,
    }, indent=2) + "\n", encoding="utf-8")
    return receipt_path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts", required=True, type=Path)
    parser.add_argument("--previous-artifacts", type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--previous-version")
    parser.add_argument("--architecture", required=True)
    parser.add_argument("--desktop", required=True)
    parser.add_argument("--session", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--source-run-id", required=True)
    parser.add_argument("--artifact-run-id", required=True)
    parser.add_argument("--driver", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--module-artifacts", required=True, type=Path)
    parser.add_argument("--module-fixtures", required=True, type=Path)
    args = parser.parse_args()
    print(produce(args))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
