#!/usr/bin/env python3
"""Accept exact Linux build artifacts from a prior release workflow run."""

import argparse
import json
import os
import runpy
from pathlib import Path


RELEASE = runpy.run_path(str(Path(__file__).with_name("linux_release_contract.py")))
PIPELINE = runpy.run_path(str(Path(__file__).with_name("current_pipeline.py")))


def verify(run: dict, jobs: dict, artifacts: dict, repository: str, commit: str, *, current_run_id=None) -> None:
    if (
        type(run.get("id")) is not int or run.get("head_sha") != commit
        or run.get("head_repository", {}).get("full_name") != repository
        or run.get("path") != ".github/workflows/release.yml"
        or run.get("event") not in ("push", "workflow_dispatch")
        or (run.get("status") != "completed" and not PIPELINE["current_release"](run, current_run_id))
    ):
        raise ValueError("Linux artifact source is not a completed release workflow at the tagged commit")
    entries = jobs.get("jobs", [])
    if jobs.get("total_count") != len(entries):
        raise ValueError("Linux artifact source job inventory is incomplete")
    expected = {"preflight", *(f"Linux build ({architecture})" for architecture in RELEASE["ARCHITECTURES"])}
    passed = {
        job.get("name") for job in entries
        if job.get("status") == "completed" and job.get("conclusion") == "success"
    }
    if not expected <= passed:
        raise ValueError("Linux artifact source did not pass all native build jobs")
    names = {artifact.get("name") for artifact in artifacts.get("artifacts", []) if artifact.get("expired") is False}
    if not {f"production-linux-{architecture}" for architecture in RELEASE["ARCHITECTURES"]} <= names:
        raise ValueError("Linux artifact source lacks required unexpired architecture artifacts")
    for artifact in artifacts.get("artifacts", []):
        if artifact.get("name") in {f"production-linux-{architecture}" for architecture in RELEASE["ARCHITECTURES"]}:
            source = artifact.get("workflow_run", {})
            if source.get("id") != run["id"] or source.get("head_sha") != commit:
                raise ValueError("Linux artifact source artifact provenance differs")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("run", "jobs", "artifacts"):
        parser.add_argument(f"--{name}", required=True, type=Path)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    verify(
        *(json.loads(path.read_text()) for path in (args.run, args.jobs, args.artifacts)),
        args.repository, args.commit,
        current_run_id=os.environ.get("GITHUB_RUN_ID"),
    )
    print("verified exact Linux artifact source run")


if __name__ == "__main__":
    main()
