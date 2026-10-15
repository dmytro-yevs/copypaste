"""Resolve upgrade inputs from published bytes, preserving their original producer identity."""

import hashlib
import json
from pathlib import Path
import re
import runpy
import subprocess

from github_inventory import inventory


def version(value):
    match = re.fullmatch(r"v?([0-9]+)\.([0-9]+)\.([0-9]+)", value)
    return tuple(map(int, match.groups())) if match else None


def previous(releases, target):
    current = version(target)
    if current is None:
        raise ValueError("invalid target version")
    candidates = [release for release in releases if release.get("draft") is False
                  and release.get("prerelease") is False and version(release.get("tag_name", ""))
                  and release["tag_name"].startswith("v")
                  and version(release["tag_name"]) < current]
    candidates = [release for release in candidates if any(
        asset.get("name", "").startswith(f"CopyPaste-{release['tag_name']}-linux-")
        and asset["name"].endswith(".deb") for asset in release.get("assets", []))]
    if not candidates:
        return None
    return max(candidates, key=lambda release: version(release["tag_name"]))


def verify_published_receipt(receipt, release, root, commit):
    if (receipt.get("platform") != "linux" or receipt.get("commit") != commit
            or receipt.get("version") != release["tag_name"][1:]):
        raise ValueError("previous Linux receipt differs from the published release")
    assets = {item["name"]: item for item in release["assets"]}
    checked = set()
    for record in receipt.get("artifacts", []):
        name = record.get("name", "")
        if not name.endswith((".deb", ".AppImage", ".rpm")):
            continue
        if Path(name).name != name or name in checked:
            raise ValueError("unsafe or duplicated prior package identity")
        checked.add(name)
        path = root / name
        asset = assets.get(name, {})
        if (not path.is_file() or path.is_symlink() or path.stat().st_size != record.get("bytes")
                or asset.get("size") != record.get("bytes")
                or asset.get("digest") != "sha256:" + record.get("sha256", "")
                or hashlib.sha256(path.read_bytes()).hexdigest() != record.get("sha256")):
            raise ValueError("previous package differs from published bytes: " + name)
    if not any(name.endswith(".deb") for name in checked) or not any(name.endswith(".AppImage") for name in checked):
        raise ValueError("previous published Linux package set is incomplete")


def resolve(repository, target, temporary):
    pages = json.loads(subprocess.check_output(["gh", "api", "--paginate", "--slurp",
                                              f"repos/{repository}/releases?per_page=100"], text=True))
    release = previous([item for page in pages for item in page], target)
    if release is None:
        return ""
    tag = release["tag_name"]
    subprocess.run(["git", "fetch", "--quiet", "--no-tags", "origin", f"refs/tags/{tag}:refs/tags/{tag}"], check=True)
    commit = subprocess.check_output(["git", "rev-parse", tag + "^{commit}"], text=True).strip()
    pages = json.loads(subprocess.check_output(["gh", "api", "--paginate", "--slurp",
        f"repos/{repository}/actions/workflows/release.yml/runs?head_sha={commit}&status=completed&per_page=100"], text=True))
    for run in (run for page in pages for run in page["workflow_runs"]):
        if (run.get("head_repository", {}).get("full_name") != repository or run.get("head_sha") != commit
                or run.get("path") != ".github/workflows/release.yml"
                or run.get("event") not in ("push", "workflow_dispatch")):
            continue
        jobs = inventory(repository, run["id"], "jobs")
        passed = {job["name"] for job in jobs["jobs"] if job.get("conclusion") == "success" and job.get("status") == "completed"}
        if not {"Linux build (x86_64)", "preflight"} <= passed:
            continue
        artifacts = inventory(repository, run["id"], "artifacts")
        matches = [item for item in artifacts["artifacts"] if item["name"] == "production-linux-x86_64" and not item["expired"]]
        if len(matches) != 1 or matches[0].get("workflow_run", {}).get("head_sha") != commit:
            continue
        verifier = runpy.run_path(str(Path(__file__).with_name("verify-linux-artifact-source-run.py")))["verify"]
        verifier(run, jobs, artifacts, repository, commit)
        destination = temporary / f"previous-linux-{run['id']}"
        destination.mkdir(parents=True)
        # The source envelope is authenticated before extracting the archive.
        subprocess.run(["gh", "run", "download", str(run["id"]), "--repo", repository,
                        "--name", "production-linux-x86_64", "--dir", str(destination)], check=True)
        receipt = json.loads((destination / "production-receipt.json").read_text())
        # Matching the public GitHub asset digest proves which same-commit build
        # was actually published, without downloading unrelated platform packages.
        try:
            verify_published_receipt(receipt, release, destination, commit)
        except ValueError:
            continue
        origin = str(receipt["run_id"])
        if origin != str(run["id"]):
            raise ValueError("previous receipt does not bind its verified producer")
        print(f"Resolved published upgrade baseline {tag} to exact Linux producer {origin}")
        return origin
    raise ValueError(f"Published Linux baseline {tag} has no retained authenticated qualification artifacts; supply a verified previous_linux_artifact_run_id")
