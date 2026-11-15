#!/usr/bin/env python3
"""Publish qualified files without replacing an existing release asset."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess


def missing_assets(directory: Path, release: dict, *, allow_draft=False) -> list[Path]:
    if (release.get("draft") is not False and not (allow_draft and release.get("draft") is True)) or release.get("prerelease") is not False:
        raise ValueError("existing release is not a published stable release")
    paths = sorted(directory.iterdir())
    if not paths or any(not path.is_file() or path.is_symlink() for path in paths):
        raise ValueError("qualified release files are missing or invalid")
    assets = release.get("assets", [])
    expected = {path.name for path in paths}
    if any(asset.get("name") not in expected for asset in assets):
        raise ValueError("published release contains an unqualified asset")
    missing = []
    for path in paths:
        matching = [item for item in assets if item.get("name") == path.name]
        if not matching:
            missing.append(path)
            continue
        digest = "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
        if (
            len(matching) != 1
            or matching[0].get("size") != path.stat().st_size
            or matching[0].get("digest") != digest
        ):
            raise ValueError(f"published asset differs from qualification: {path.name}")
    return missing


def find_release(repository: str, tag: str) -> dict | None:
    # An API failure must never be interpreted as an absent release.
    releases = json.loads(subprocess.check_output([
        "gh", "api", "--paginate", "--slurp",
        f"repos/{repository}/releases?per_page=100",
    ], text=True))
    matching = [
        release for page in releases for release in page
        if release.get("tag_name") == tag
    ]
    if len(matching) > 1:
        raise ValueError("release identity is ambiguous")
    return matching[0] if matching else None


def publish(args) -> None:
    if (
        not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", args.version)
        or not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", args.repository)
    ):
        raise ValueError("invalid stable release identity")
    tag = f"v{args.version}"
    release = find_release(args.repository, tag)
    if release:
        paths = missing_assets(args.artifacts, release, allow_draft=True)
        if paths:
            subprocess.run([
                "gh", "release", "upload", tag, "--repo", args.repository,
                *(str(path) for path in paths),
            ], check=True)
    else:
        paths = missing_assets(args.artifacts, {
            "draft": False, "prerelease": False, "assets": [],
        })
        subprocess.run([
            "gh", "release", "create", tag, "--repo", args.repository,
            "--verify-tag", "--draft", "--title", f"CopyPaste {args.version}",
            "--notes-file", str(args.notes), *(str(path) for path in paths),
        ], check=True)
        release = find_release(args.repository, tag)
    if not release or type(release.get("id")) is not int or release["id"] <= 0:
        raise ValueError("release has no unique API identity")
    # The tag endpoint exposes published releases only. A draft must be read
    # through its immutable API ID, including after an interrupted upload.
    endpoint = f"repos/{args.repository}/releases/{release['id']}"
    release = json.loads(subprocess.check_output([
        "gh", "api", endpoint,
    ], text=True))
    if release.get("tag_name") != tag or missing_assets(args.artifacts, release, allow_draft=True):
        raise ValueError("release publication is incomplete")
    if release["draft"]:
        subprocess.run(["gh", "release", "edit", tag, "--repo", args.repository,
                        "--draft=false", "--latest"], check=True)
        release = json.loads(subprocess.check_output([
            "gh", "api", endpoint,
        ], text=True))
        if release.get("tag_name") != tag or missing_assets(args.artifacts, release):
            raise ValueError("release promotion did not complete")
    print(f"verified complete stable release {tag}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--artifacts", required=True, type=Path)
    parser.add_argument("--notes", required=True, type=Path)
    publish(parser.parse_args())


if __name__ == "__main__":
    main()
