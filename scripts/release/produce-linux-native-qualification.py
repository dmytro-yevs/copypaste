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
    "portal_keyboard_grant",
}
FIRST_INSTALL_ASSERTIONS = (ASSERTIONS - {"package_upgrade"}) | {"clean_install_baseline"}
SAFE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")


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
        if not line.startswith("COPYPASTE_QUALIFICATION_COMMAND "):
            raise ValueError("scenario driver emitted unstructured output")
        row = json.loads(line.removeprefix("COPYPASTE_QUALIFICATION_COMMAND "))
        if not isinstance(row, dict) or set(row) != {"argv", "returncode", "assertions"}:
            raise ValueError("scenario driver emitted an invalid command trace")
        if row["returncode"] != 0 or not isinstance(row["argv"], list) or not row["argv"]:
            raise ValueError("scenario driver reported a failed command")
        if not all(isinstance(value, str) and value and len(value) <= 256 and "\n" not in value and "\r" not in value for value in row["argv"]):
            raise ValueError("scenario driver emitted unsafe command arguments")
        if not isinstance(row["assertions"], list) or not row["assertions"] or not all(item in expected_assertions for item in row["assertions"]):
            raise ValueError("scenario driver emitted invalid assertion coverage")
        rows.append(row)
    if not rows:
        raise ValueError("scenario driver produced no executed-command trace")
    if {item for row in rows for item in row["assertions"]} != expected_assertions:
        raise ValueError("scenario driver did not execute every required assertion")
    return rows


def attachment(path: Path) -> dict:
    if not path.is_file() or path.is_symlink() or not SAFE_NAME.fullmatch(path.name):
        raise ValueError(f"unsafe evidence attachment: {path}")
    return {"name": path.name, "sha256": sha256(path), "size_bytes": path.stat().st_size}


def produce(args: argparse.Namespace) -> Path:
    if args.architecture not in ARCHITECTURES or args.desktop not in DESKTOPS or args.session not in SESSIONS:
        raise ValueError("unsupported Linux qualification matrix coordinate")
    if not re.fullmatch(r"[0-9a-f]{40}", args.commit):
        raise ValueError("commit must be a full lowercase Git SHA")
    if not re.fullmatch(r"[1-9][0-9]*", args.source_run_id):
        raise ValueError("source run ID is invalid")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    packages = artifact_inventory(args.artifacts.resolve(), args.version, args.architecture)
    upgrade_mode = "prior_release" if args.previous_artifacts is not None else "first_install_baseline"
    if upgrade_mode == "prior_release":
        if args.previous_version is None:
            raise ValueError("prior-release qualification requires a previous version")
        previous = artifact_inventory(args.previous_artifacts.resolve(), args.previous_version, args.architecture)
        expected_assertions = ASSERTIONS
    else:
        if args.previous_version is not None:
            raise ValueError("first-install baseline cannot name a prior version")
        previous = []
        expected_assertions = FIRST_INSTALL_ASSERTIONS
    driver = args.driver.resolve()
    if not driver.is_file() or not os.access(driver, os.X_OK):
        raise ValueError("repository-controlled scenario driver is missing or not executable")
    result = subprocess.run(
        [str(driver), "--artifacts", str(args.artifacts.resolve()), "--version", args.version,
         "--architecture", args.architecture, "--desktop", args.desktop, "--session", args.session,
         "--evidence-dir", str(output)] + (
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
    commands = command_rows(result.stdout, expected_assertions)
    trace_name = f"linux-native-{args.architecture}-{args.desktop.lower()}-{args.session}.trace.json"
    trace_path = output / trace_name
    trace_path.write_text(json.dumps({
        "schema": 1, "version": args.version, "commit": args.commit, "source_run_id": args.source_run_id,
        "artifact_run_id": args.artifact_run_id,
        "architecture": args.architecture, "desktop": args.desktop, "session": args.session,
        "upgrade_mode": upgrade_mode,
        "commands": commands,
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
    args = parser.parse_args()
    print(produce(args))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
