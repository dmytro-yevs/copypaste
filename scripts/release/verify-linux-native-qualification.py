#!/usr/bin/env python3
"""Verify full Linux desktop evidence against the exact release artifacts."""

import argparse
import hashlib
import json
import re
from pathlib import Path
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
    "portal_keyboard_grant",
}
FIRST_INSTALL_ASSERTIONS = (ASSERTIONS - {"package_upgrade"}) | {"clean_install_baseline"}
EVIDENCE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(block)
    return hasher.hexdigest()


def regular_file(root: Path, name: str) -> Path:
    if not isinstance(name, str) or not EVIDENCE_NAME.fullmatch(name):
        raise ValueError("evidence attachment has an unsafe name")
    path = root / name
    try:
        resolved = path.resolve(strict=True)
    except FileNotFoundError as error:
        raise ValueError(f"evidence attachment is missing: {name}") from error
    if resolved.parent != root.resolve() or path.is_symlink() or not resolved.is_file():
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


def assertion_set(upgrade_mode: object) -> set[str]:
    if upgrade_mode == "prior_release":
        return ASSERTIONS
    if upgrade_mode == "first_install_baseline":
        return FIRST_INSTALL_ASSERTIONS
    raise ValueError("evidence upgrade mode is invalid")


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
        if not isinstance(assertions, list) or not assertions or not all(item in expected_assertions for item in assertions):
            raise ValueError(f"scenario trace assertions are invalid: {name}")
        covered.update(assertions)
    if covered != expected_assertions:
        raise ValueError(f"scenario trace does not execute every assertion: {name}")
    if receipt.get("trace") != {"name": name, "sha256": digest(path), "size_bytes": path.stat().st_size}:
        raise ValueError(f"receipt does not bind its scenario trace: {name}")


def verify(root: Path, artifacts: Path, version: str, commit: str, source_run_id: str, artifact_run_id: Optional[str] = None) -> None:
    root = root.resolve(strict=True)
    artifacts = artifacts.resolve(strict=True)
    seen = set()
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
                expected_assertions = assertion_set(receipt.get("upgrade_mode"))
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
