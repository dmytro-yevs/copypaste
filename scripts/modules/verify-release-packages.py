#!/usr/bin/env python3
"""Verify exact native receipts before making module packages public."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

from catalog import REQUIRED_TARGETS, build_catalog


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
    if module["id"] not in {"copypaste.ocr", "copypaste.semantic-search", "copypaste.supabase"}:
        raise ValueError("No native qualification contract exists for this module")
    verify_receipts(args.directory, module, args.commit, args.run_id)
    print(f"Verified exact signed {module['id']} package receipts for macOS, Windows, and Android")


def verify_receipts(directory, module, commit, run_id):
    for platform, architecture in [("macos", "aarch64"), ("windows", "x86_64"), ("android", "x86_64")]:
        name = f"CopyPasteModule-{module['id']}-v{module['version']}-{platform}-{architecture}.cpmodule"
        package = directory / name
        receipt = json.loads((directory / (name + ".receipt.json")).read_text())
        if (receipt["commit"] != commit or receipt["run_id"] != run_id
                or receipt["target"] != {"platform": platform, "architecture": architecture}
                or receipt["module_id"] != module["id"] or receipt["module_version"] != module["version"]
                or receipt["package_sha256"] != hashlib.sha256(package.read_bytes()).hexdigest()
                or receipt["package_size_bytes"] != package.stat().st_size
                or receipt["cases_passed"] != 3 or not receipt["signature_verified"]
                or not receipt["removal_completed_after_restart"]):
            raise ValueError(f"Native evidence does not qualify the exact {platform}/{architecture} package")


if __name__ == "__main__":
    main()
