#!/usr/bin/env python3
"""Discover already-qualified exact-source artifacts before a tagged publication."""

import json
import os
from pathlib import Path
import re
import runpy
import subprocess

from github_inventory import inventory


def eligible(run, jobs, artifacts, repository, commit, *, linux, compositor):
    # The full Linux origin/byte checks still run in the existing preflight
    # recovery path, after this authenticated source envelope is selected.
    verifier = runpy.run_path(str(Path(__file__).with_name("verify-qualified-run.py")))["verify"]
    verifier(run, jobs, artifacts, repository, commit)
    passed = {job["name"] for job in jobs["jobs"] if job.get("status") == "completed" and job.get("conclusion") == "success"}
    if linux and "Linux full parity evidence gate" not in passed:
        raise ValueError("candidate lacks native Linux qualification")
    if compositor and "Sign and bind opt-in compositor runtime companions" not in passed:
        raise ValueError("candidate lacks qualified runtime companions")


def discover(repository, commit, *, linux=True, compositor=True):
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("invalid qualification source commit")
    pages = json.loads(subprocess.check_output(["gh", "api", "--paginate", "--slurp",
        f"repos/{repository}/actions/workflows/release.yml/runs?head_sha={commit}&status=completed&per_page=100"], text=True))
    for run in (run for page in pages for run in page["workflow_runs"]):
        if run.get("conclusion") not in ("success", "failure", "cancelled"):
            continue
        jobs = inventory(repository, run["id"], "jobs")
        artifacts = inventory(repository, run["id"], "artifacts")
        try:
            eligible(run, jobs, artifacts, repository, commit, linux=linux, compositor=compositor)
        except ValueError:
            continue
        return str(run["id"])
    return ""


def main():
    source = os.environ.get("EXPLICIT_QUALIFIED_RUN_ID", "")
    if source and not re.fullmatch(r"[1-9][0-9]*", source):
        raise ValueError("qualified_run_id must be a positive run ID")
    if not source and (os.environ.get("REQUESTED_PUBLISH") == "true" or os.environ.get("GITHUB_REF_TYPE") == "tag"):
        source = discover(os.environ["GITHUB_REPOSITORY"], os.environ["GITHUB_SHA"],
                          linux=Path("packaging/linux/release-contract.json").exists(),
                          compositor=Path(".github/workflows/compositor-runtime.yml").exists())
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        output.write("qualified_run_id=" + source + "\n")
    print("Selected previously qualified source: " + (source or "none; build and qualify this source"))


if __name__ == "__main__":
    main()
