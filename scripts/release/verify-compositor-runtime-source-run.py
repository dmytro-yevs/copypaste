#!/usr/bin/env python3
"""Verify a completed, exact-source compositor runtime producer run."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


EXPECTED = {
    f"copypaste-compositor-runtime-{desktop}-{family}-{distribution}-{architecture}"
    for desktop, family, distribution in (
        ("gnome", "46", "ubuntu24.04"),
        ("kde", "6.0", "fedora40"),
    )
    for architecture in ("x86_64", "aarch64")
}

EXPECTED_JOBS = {
    f"{desktop} {family} {distribution} ({architecture})"
    for desktop, family, distribution in (
        ("GNOME", "46", "Ubuntu 24.04"),
        ("KDE", "6.0", "Fedora 40"),
    )
    for architecture in ("x86_64", "aarch64")
}


def verify(run: dict, jobs: dict, artifacts: dict, repository: str, commit: str) -> None:
    direct = run.get("path") == ".github/workflows/compositor-runtime.yml" and run.get("event") == "workflow_dispatch"
    trusted_ci = (run.get("path") == ".github/workflows/ci.yml" and run.get("event") == "pull_request"
                  and isinstance(run.get("pull_requests"), list) and len(run["pull_requests"]) == 1)
    if (
        type(run.get("id")) is not int
        or run.get("head_sha") != commit
        or run.get("head_repository", {}).get("full_name") != repository
        or not (direct or trusted_ci)
        or run.get("status") != "completed"
        or run.get("conclusion") != "success"
    ):
        raise ValueError("compositor runtime source is not a successful trusted dispatch at the exact commit")
    entries = jobs.get("jobs", [])
    if jobs.get("total_count") != len(entries):
        raise ValueError("compositor runtime source job inventory is incomplete")
    passed = {
        job.get("name")
        for job in entries
        if job.get("status") == "completed" and job.get("conclusion") == "success"
    }
    required_jobs = EXPECTED_JOBS if direct else {"Compositor runtime metadata", *EXPECTED_JOBS}
    if not all(any(name == expected or name.endswith(f" / {expected}") for name in passed) for expected in required_jobs):
        raise ValueError("compositor runtime source did not pass every production runtime job")
    names = {
        artifact.get("name")
        for artifact in artifacts.get("artifacts", [])
        if artifact.get("expired") is False
    }
    if names != EXPECTED:
        raise ValueError("compositor runtime source artifact inventory is incomplete or ambiguous")
    for artifact in artifacts.get("artifacts", []):
        if artifact.get("name") in EXPECTED:
            source = artifact.get("workflow_run", {})
            if source.get("id") != run["id"] or source.get("head_sha") != commit:
                raise ValueError("compositor runtime artifact provenance differs from its source run")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("run", "jobs", "artifacts"):
        parser.add_argument(f"--{name}", required=True, type=Path)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    verify(
        *(json.loads(path.read_text(encoding="utf-8")) for path in (args.run, args.jobs, args.artifacts)),
        args.repository,
        args.commit,
    )
    print("verified exact compositor runtime source run")


if __name__ == "__main__":
    main()
