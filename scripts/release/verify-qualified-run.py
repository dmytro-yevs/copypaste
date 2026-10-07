#!/usr/bin/env python3
"""Accept a completed production qualification at the immutable release tag."""

import argparse
import json
from pathlib import Path


def verify(run: dict, jobs: dict, artifacts: dict, repository: str, commit: str) -> None:
    if type(run.get("id")) is not int or run["id"] <= 0:
        raise ValueError("qualification source has no valid run identity")
    if (
        run.get("head_sha") != commit
        or run.get("head_repository", {}).get("full_name") != repository
    ):
        raise ValueError("qualification source does not match the release tag and repository")
    if (
        run.get("path") != ".github/workflows/release.yml"
        or run.get("event") not in ("push", "workflow_dispatch")
    ):
        raise ValueError("qualification source is not the production release workflow")
    if run.get("status") != "completed" or run.get("conclusion") != "success":
        raise ValueError("qualification source did not complete successfully")
    entries = jobs.get("jobs", [])
    if jobs.get("total_count") != len(entries):
        raise ValueError("qualification job inventory is incomplete")
    for name in ("preflight", "macos", "android", "windows", "qualify"):
        matching = [job for job in entries if job.get("name") == name]
        if (
            len(matching) != 1
            or matching[0].get("status") != "completed"
            or matching[0].get("conclusion") != "success"
        ):
            raise ValueError(f"required qualification job did not pass: {name}")
    matching = [
        item for item in artifacts.get("artifacts", [])
        if item.get("name") == "production-qualified"
    ]
    if len(matching) != 1 or matching[0].get("expired") is not False:
        raise ValueError("qualified artifact is missing, ambiguous, or expired")
    source = matching[0].get("workflow_run", {})
    if source.get("id") != run.get("id") or source.get("head_sha") != commit:
        raise ValueError("qualified artifact provenance does not match the source run")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("run", "jobs", "artifacts"):
        parser.add_argument(f"--{name}", required=True, type=Path)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    verify(
        *(json.loads(path.read_text()) for path in (args.run, args.jobs, args.artifacts)),
        args.repository,
        args.commit,
    )
    print("verified successful native qualification at the immutable release tag")


if __name__ == "__main__":
    main()
