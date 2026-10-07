#!/usr/bin/env python3
"""Prepare a marketplace update without publishing or changing any release."""
import base64
import json
import os
from pathlib import Path
import re
import subprocess
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from catalog import REPOSITORY, build_catalog, verify_signature


def download_optional(url, maximum_bytes):
    try:
        with urlopen(Request(url, headers={"User-Agent": "CopyPaste marketplace publisher"}), timeout=30) as response:
            data = response.read(maximum_bytes + 1)
    except HTTPError as error:
        if error.code == 404:
            return None
        raise
    if len(data) > maximum_bytes:
        raise ValueError("Published catalog metadata is too large")
    return data


def main():
    repository = os.environ["GITHUB_REPOSITORY"]
    if repository != "dmytro-yevs/copypaste":
        raise ValueError("Marketplace publication is restricted to the first-party repository")
    tag = os.environ["MODULE_RELEASE_TAG"]
    if not re.fullmatch(r"module-copypaste\.[a-z0-9.-]+-v[0-9]+\.[0-9]+\.[0-9]+", tag):
        raise ValueError("Invalid qualified module release tag")
    result = subprocess.run([
        "gh", "api", f"repos/{repository}/releases/tags/{tag}",
    ], check=True, capture_output=True, text=True)
    release = json.loads(result.stdout)
    if release["draft"] or release["prerelease"]:
        raise ValueError("Marketplace packages must already be in a published stable module release")
    packages = Path("dist/packages")
    packages.mkdir(parents=True, exist_ok=False)
    subprocess.run([
        "gh", "release", "download", tag, "--repo", repository,
        "--pattern", "*.cpmodule", "--dir", str(packages),
    ], check=True)
    previous_bytes = download_optional(f"{REPOSITORY}/modules/modules.json", 2 * 1024 * 1024)
    previous = None
    if previous_bytes is not None:
        signature = download_optional(f"{REPOSITORY}/modules/modules.json.sig", 64 * 1024)
        if signature is None:
            raise ValueError("The existing marketplace catalog is missing its signature")
        verify_signature(previous_bytes, base64.b64decode(signature.strip(), validate=True), "modules.json")
        previous = json.loads(previous_bytes)
    else:
        # Only a confirmed missing release permits creation. Network or auth
        # failures must never turn an existing catalog into a fresh catalog.
        result = subprocess.run([
            "gh", "api", "-i", f"repos/{repository}/releases/tags/modules",
        ], capture_output=True, text=True)
        if result.returncode == 0:
            raise ValueError("The marketplace release exists but its catalog is missing")
        if not re.search(r"^HTTP/\S+ 404\b", result.stdout, re.MULTILINE):
            raise ValueError("Could not confirm the marketplace release is absent")
        Path("dist/create-marketplace-release").touch()
    catalog = build_catalog(packages.glob("*.cpmodule"), tag, previous)
    Path("dist/modules.json").write_text(json.dumps(catalog, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    print(f"Prepared {len(catalog['modules'])} marketplace modules")


if __name__ == "__main__":
    main()
