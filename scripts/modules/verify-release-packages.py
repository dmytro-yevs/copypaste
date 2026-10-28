#!/usr/bin/env python3
"""Verify exact native receipts before making module packages public."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

from catalog import LINUX_RELEASE, build_catalog


LEGACY_PLATFORMS = ["macos", "windows", "android"]
NATIVE_RECEIPT_TARGETS = {
    "macos": [("macos", "aarch64")],
    "windows": [("windows", "x86_64")],
    "linux": [("linux", architecture) for architecture in LINUX_RELEASE["ARCHITECTURES"]],
    "android": [("android", "x86_64")],
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--release-tag", required=True)
    args = parser.parse_args()
    packages = list(args.directory.glob("*.cpmodule"))
    # Package authentication also checks all file hashes and platform parity.
    module = build_catalog(packages, args.release_tag)["modules"][0]
    if module["id"] not in {"copypaste.ocr", "copypaste.semantic-search", "copypaste.sms-codes", "copypaste.supabase"}:
        raise ValueError("No native qualification contract exists for this module")
    verify_receipts(args.directory, module, args.commit, args.run_id)
    print(f"Verified exact signed {module['id']} package receipts for every shipped native platform")


def receipt_targets(module):
    platforms = module.get("supported_platforms", LEGACY_PLATFORMS)
    if not isinstance(platforms, list) or not platforms or any(platform not in NATIVE_RECEIPT_TARGETS for platform in platforms):
        raise ValueError("Invalid module receipt platforms")
    return [target for platform in platforms for target in NATIVE_RECEIPT_TARGETS[platform]]


def verify_receipts(directory, module, commit, run_id):
    expected_app_version = None
    if module["id"] == "copypaste.sms-codes":
        matched = re.fullmatch(r">=([0-9]+\.[0-9]+\.[0-9]+), <2\.0\.0", module.get("app_versions", ""))
        if matched is None:
            raise ValueError("SMS Codes must bind one stable application version")
        expected_app_version = matched.group(1)
    for platform, architecture in receipt_targets(module):
        name = f"CopyPasteModule-{module['id']}-v{module['version']}-{platform}-{architecture}.cpmodule"
        package = directory / name
        receipt = json.loads((directory / (name + ".receipt.json")).read_text())
        if (receipt["commit"] != commit or receipt["run_id"] != run_id
                or receipt["target"] != {"platform": platform, "architecture": architecture}
                or receipt["module_id"] != module["id"] or receipt["module_version"] != module["version"]
                or receipt["package_sha256"] != hashlib.sha256(package.read_bytes()).hexdigest()
                or receipt["package_size_bytes"] != package.stat().st_size
                or receipt["cases_passed"] != 3 or not receipt["signature_verified"]
                or receipt["restart_required"] is not False
                or not receipt["removal_completed_after_restart"]):
            raise ValueError(f"Native evidence does not qualify the exact {platform}/{architecture} package")
        if expected_app_version is not None and (
                receipt.get("app_version") != expected_app_version
                or receipt.get("environment") != "android-emulator"
                or receipt.get("api") != "36"
                or receipt.get("internet_permission") is not False):
            raise ValueError("SMS Codes requires an isolated API 36 Android emulator receipt")
        if platform == "linux" and not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", receipt.get("glibc_floor", "")):
            raise ValueError(f"Linux native evidence is missing the package glibc ABI floor for {architecture}")
        uid = receipt.get("effective_uid")
        if platform == "linux" and (not isinstance(uid, int) or uid <= 0 or not receipt.get("network_namespace_isolated")):
            raise ValueError(f"Linux native evidence did not run under the runner identity in an isolated network namespace for {architecture}")


if __name__ == "__main__":
    main()
