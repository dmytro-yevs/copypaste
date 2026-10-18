#!/usr/bin/env python3
"""Promote exact application-qualified module bytes through a draft release."""

import argparse
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "publish_application_release", ROOT / "scripts/release/publish-release.py"
)
publisher = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publisher)


def module_tag(manifest_path: Path) -> str:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    module_id = manifest.get("id")
    version = manifest.get("version")
    if module_id not in {"copypaste.ocr", "copypaste.semantic-search", "copypaste.supabase"}:
        raise ValueError("unrecognized first-party module")
    if not isinstance(version, str) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("invalid module release version")
    return f"module-{module_id}-v{version}"


def validate_release(release: dict, tag: str, commit: str) -> None:
    if (release.get("tag_name") != tag or release.get("target_commitish") != commit
            or release.get("prerelease") is not False or release.get("draft") not in (True, False)
            or type(release.get("id")) is not int or release["id"] <= 0):
        raise ValueError("module release tag or source commit differs from qualification")


def release_by_id(repository: str, release_id: int) -> dict:
    return json.loads(subprocess.check_output([
        "gh", "api", f"repos/{repository}/releases/{release_id}",
    ], text=True))


def publish(repository: str, directory: Path, manifest: Path, commit: str, run_id: str) -> None:
    if repository != "dmytro-yevs/copypaste" or not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("invalid qualified repository or commit")
    if not re.fullmatch(r"[1-9][0-9]*", run_id):
        raise ValueError("invalid qualification run")
    tag = module_tag(manifest)
    application_version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    if json.loads(manifest.read_text())["app_versions"] != f">={application_version}, <2.0.0":
        raise ValueError("module source does not bind the current application release")
    subprocess.run([
        sys.executable, str(ROOT / "scripts/modules/verify-release-packages.py"),
        "--directory", str(directory), "--commit", commit, "--run-id", run_id,
        "--release-tag", tag,
    ], check=True)
    release = publisher.find_release(repository, tag)
    if release is None:
        subprocess.run([
            "gh", "release", "create", tag, "--repo", repository, "--target", commit,
            "--latest=false", "--draft", "--title", f"CopyPaste {tag}",
            "--notes", (f"Optional first-party module for CopyPaste {application_version}. "
                        f"Exact production-signed packages from qualified run {run_id}, "
                        "with native receipts for macOS, Windows, Linux x86_64, and Android."),
        ], check=True)
        release = publisher.find_release(repository, tag)
    if release is None:
        raise ValueError("draft module release was not created")
    validate_release(release, tag, commit)
    release = release_by_id(repository, release["id"])
    validate_release(release, tag, commit)
    missing = publisher.missing_assets(directory, release, allow_draft=True)
    if missing and not release["draft"]:
        raise ValueError("published module release is incomplete")
    for path in missing:
        subprocess.run(["gh", "release", "upload", tag, str(path), "--repo", repository], check=True)
        release = release_by_id(repository, release["id"])
        validate_release(release, tag, commit)
        publisher.missing_assets(directory, release, allow_draft=True)
    if publisher.missing_assets(directory, release, allow_draft=True):
        raise ValueError("module release is incomplete")
    if release["draft"]:
        subprocess.run(["gh", "release", "edit", tag, "--repo", repository,
                        "--draft=false", "--latest=false"], check=True)
        release = release_by_id(repository, release["id"])
        validate_release(release, tag, commit)
        if release["draft"] or publisher.missing_assets(directory, release):
            raise ValueError("module release promotion did not complete")
    print(f"verified published module release {tag} from run {run_id}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--run-id", required=True)
    args = parser.parse_args()
    publish(args.repository, args.directory, args.manifest, args.commit, args.run_id)


if __name__ == "__main__":
    main()
