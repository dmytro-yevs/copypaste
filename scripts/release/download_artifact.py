#!/usr/bin/env python3
"""Download immutable artifact IDs, verify ZIP digests, and extract safe files."""

import argparse
import fnmatch
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import tempfile
import zipfile

from github_inventory import inventory, latest_artifacts


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            value.update(chunk)
    return "sha256:" + value.hexdigest()


def extract(archive, destination):
    destination.mkdir(parents=True, exist_ok=True)
    root = destination.resolve()
    seen = set()
    with zipfile.ZipFile(archive) as source:
        for item in source.infolist():
            name = PurePosixPath(item.filename)
            if (name.is_absolute() or ".." in name.parts or "\\" in item.filename
                    or any(ord(character) < 32 or ord(character) == 127 for character in item.orig_filename)
                    or ":" in item.filename or not name.parts):
                raise ValueError("unsafe artifact archive path")
            target = root.joinpath(*name.parts)
            if not target.resolve().is_relative_to(root) or stat.S_ISLNK(item.external_attr >> 16):
                raise ValueError("artifact archive contains a symlink or escaping path")
            if item.is_dir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            identity = name.as_posix().casefold()
            if identity in seen:
                raise ValueError("artifact archive contains duplicate paths")
            seen.add(identity)
            target.parent.mkdir(parents=True, exist_ok=True)
            with source.open(item) as contents, target.open("wb") as output:
                shutil.copyfileobj(contents, output)


def merge(source, destination):
    destination.mkdir(parents=True, exist_ok=True)
    root = destination.resolve()
    for path in source.rglob("*"):
        target = root / path.relative_to(source)
        if not target.resolve().is_relative_to(root) or target.is_symlink():
            raise ValueError("artifact destination contains an unsafe symlink")
        if path.is_dir():
            target.mkdir(parents=True, exist_ok=True)
        elif target.exists():
            if not target.is_file() or digest(path) != digest(target):
                raise ValueError("artifact merge contains conflicting files: " + str(path.relative_to(source)))
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, target)


def download(repository, run_id, destination, *, name=None, pattern=None, artifact_ids=None, merge_multiple=False, commit=None):
    if sum(bool(value) for value in (name, pattern, artifact_ids)) > 1:
        raise ValueError("select artifacts by name, pattern, or IDs, not multiple selectors")
    run = inventory(repository, run_id, "run")
    if (type(run.get("id")) is not int or run["id"] != int(run_id)
            or not re.fullmatch(r"[0-9a-f]{40}", str(run.get("head_sha", "")))
            or (run.get("head_repository") or {}).get("full_name", "").lower() != repository.lower()
            or (commit and run.get("head_sha") != commit)):
        raise ValueError("artifact run repository or source commit differs")
    if str(run_id) != os.environ.get("GITHUB_RUN_ID") and run.get("status") != "completed":
        raise ValueError("external artifact source must have completed")
    raw = inventory(repository, run_id, "artifacts", latest=False)
    if artifact_ids:
        records = [record for record in raw["artifacts"] if record.get("id") in artifact_ids]
        if len(records) != len(set(artifact_ids)):
            raise ValueError("an immutable artifact ID is missing or ambiguous")
    else:
        records = latest_artifacts(raw, run.get("run_attempt", 1))["artifacts"]
        records = [record for record in records if (not name or record["name"] == name)
                   and (not pattern or fnmatch.fnmatchcase(record["name"], pattern))]
    if not records:
        raise ValueError("required artifacts are missing")
    references = []
    for record in records:
        source = record.get("workflow_run", {})
        if (type(record.get("id")) is not int or record["id"] <= 0 or record.get("expired") is not False
                or source.get("id") != int(run_id) or source.get("head_sha") != run.get("head_sha")
                or not re.fullmatch(r"sha256:[0-9a-f]{64}", record.get("digest", ""))):
            raise ValueError("artifact provenance, expiry, or digest is invalid")
        if (Path(record["name"]).name != record["name"] or record["name"] in (".", "..")
                or "\\" in record["name"] or ":" in record["name"]
                or any(ord(character) < 32 or ord(character) == 127 for character in record["name"])):
            raise ValueError("artifact name is unsafe")
        with tempfile.TemporaryDirectory(prefix="copypaste-artifact-") as directory:
            temporary = Path(directory)
            archive = temporary / "artifact.zip"
            with archive.open("wb") as output:
                subprocess.run(["gh", "api", f"repos/{repository}/actions/artifacts/{record['id']}/zip"], stdout=output, check=True)
            if digest(archive) != record["digest"]:
                raise ValueError("artifact ZIP SHA-256 differs from its immutable reference")
            extract(archive, temporary / "files")
            target = destination if name or merge_multiple or (artifact_ids and len(records) == 1) else destination / record["name"]
            merge(temporary / "files", target)
        references.append({key: record.get(key) for key in ("id", "name", "digest", "created_at")})
    print("Verified immutable artifact references: " + json.dumps(references))
    return references


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY"), required=not os.environ.get("GITHUB_REPOSITORY"))
    parser.add_argument("--run-id", default=os.environ.get("SOURCE_RUN_ID") or os.environ.get("GITHUB_RUN_ID"), required=not (os.environ.get("SOURCE_RUN_ID") or os.environ.get("GITHUB_RUN_ID")))
    parser.add_argument("--name", default=os.environ.get("ARTIFACT_NAME") or None)
    parser.add_argument("--pattern", default=os.environ.get("ARTIFACT_PATTERN") or None)
    parser.add_argument("--artifact-id", type=int, action="append")
    parser.add_argument("--path", type=Path, default=Path(os.environ.get("DOWNLOAD_PATH") or "."))
    parser.add_argument("--merge-multiple", action="store_true", default=os.environ.get("MERGE_MULTIPLE") == "true")
    parser.add_argument("--commit", default=os.environ.get("EXPECTED_COMMIT") or None)
    args = parser.parse_args()
    download(args.repository, args.run_id, args.path, name=args.name, pattern=args.pattern,
             artifact_ids=args.artifact_id, merge_multiple=args.merge_multiple, commit=args.commit)


if __name__ == "__main__":
    main()
