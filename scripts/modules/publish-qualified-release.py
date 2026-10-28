#!/usr/bin/env python3
"""Promote exact application-qualified module bytes through a draft release."""

import argparse
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import time
import tomllib

sys.path.insert(0, str(Path(__file__).parent))
from catalog import REQUIRED_TARGETS


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "publish_application_release", ROOT / "scripts/release/publish-release.py"
)
publisher = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publisher)
RECEIPT_SPEC = importlib.util.spec_from_file_location(
    "verify_release_packages", ROOT / "scripts/modules/verify-release-packages.py"
)
receipt_verifier = importlib.util.module_from_spec(RECEIPT_SPEC)
RECEIPT_SPEC.loader.exec_module(receipt_verifier)


def module_tag(manifest_path: Path) -> str:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    module_id = manifest.get("id")
    version = manifest.get("version")
    if module_id not in {"copypaste.ocr", "copypaste.semantic-search", "copypaste.sms-codes", "copypaste.supabase"}:
        raise ValueError("unrecognized first-party module")
    if not isinstance(version, str) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("invalid module release version")
    return f"module-{module_id}-v{version}"


def validate_release(release: dict, tag: str, commit: str) -> None:
    if (release.get("tag_name") != tag or release.get("target_commitish") != commit
            or release.get("prerelease") is not False or release.get("draft") not in (True, False)
            or type(release.get("id")) is not int or release["id"] <= 0):
        raise ValueError("module release tag or source commit differs from qualification")


def qualified_paths(directory: Path, module: dict) -> list[Path]:
    paths = sorted(directory.iterdir())
    if not paths or any(not path.is_file() or path.is_symlink() for path in paths):
        raise ValueError("qualified module files are missing or invalid")
    package_names = {
        f"CopyPasteModule-{module['id']}-v{module['version']}-{platform}-{architecture}.cpmodule"
        for platform, architecture in REQUIRED_TARGETS if platform in module["supported_platforms"]
    }
    receipt_names = {
        f"CopyPasteModule-{module['id']}-v{module['version']}-{platform}-{architecture}.cpmodule.receipt.json"
        for platform, architecture in receipt_verifier.receipt_targets(module)
    }
    if {path.name for path in paths} != package_names | receipt_names:
        raise ValueError("qualified module release contains an unexpected artifact")
    return paths


def release_by_id(repository: str, release_id: int) -> dict:
    return json.loads(subprocess.check_output([
        "gh", "api", f"repos/{repository}/releases/{release_id}",
    ], text=True))


def wait_for_created_release(repository: str, tag: str) -> dict:
    # GitHub can acknowledge draft creation before listing that draft.
    for attempt in range(10):
        release = publisher.find_release(repository, tag)
        if release is not None:
            return release
        if attempt < 9:
            time.sleep(1)
    raise ValueError("created draft module release is not visible")


def publish(repository: str, directory: Path, manifest: Path, commit: str, run_id: str) -> None:
    if repository != "dmytro-yevs/copypaste" or not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("invalid qualified repository or commit")
    if not re.fullmatch(r"[1-9][0-9]*", run_id):
        raise ValueError("invalid qualification run")
    module = json.loads(manifest.read_text())
    tag = module_tag(manifest)
    application_version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    if module["app_versions"] != f">={application_version}, <2.0.0":
        raise ValueError("module source does not bind the current application release")
    qualified_paths(directory, module)
    subprocess.run([
        sys.executable, str(ROOT / "scripts/modules/verify-release-packages.py"),
        "--directory", str(directory), "--commit", commit, "--run-id", run_id,
        "--release-tag", tag,
    ], check=True)
    release = publisher.find_release(repository, tag)
    if release is None:
        notes = f"{module['title']} module for CopyPaste {application_version}."
        subprocess.run([
            "gh", "release", "create", tag, "--repo", repository, "--target", commit,
            "--latest=false", "--draft", "--title", f"{module['title']} {module['version']}",
            "--notes", notes,
        ], check=True)
        release = wait_for_created_release(repository, tag)
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
