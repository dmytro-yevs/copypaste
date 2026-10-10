#!/usr/bin/env python3
"""Accept exact Linux module artifacts from a completed module workflow run."""

import argparse
import json
from pathlib import Path


ARCHITECTURES = ("x86_64", "aarch64")


def artifact_name(workflow: str, module: str, architecture: str) -> str:
    prefix = "ocr" if workflow == ".github/workflows/ocr-module.yml" else f"provider-{module}"
    return f"{prefix}-linux-{architecture}"


def verify(run: dict, jobs: dict, artifacts: dict, repository: str, commit: str, workflow: str, module: str) -> None:
    if (
        type(run.get("id")) is not int
        or run.get("head_sha") != commit
        or run.get("head_repository", {}).get("full_name") != repository
        or run.get("path") != workflow
        or run.get("event") != "workflow_dispatch"
        or run.get("status") != "completed"
        or run.get("conclusion") != "success"
    ):
        raise ValueError("Module artifact source is not a successful dispatch at the exact product commit")
    entries = jobs.get("jobs", [])
    if jobs.get("total_count") != len(entries):
        raise ValueError("Module artifact source job inventory is incomplete")
    successful_linux_jobs = {
        architecture: any(
            architecture in str(job.get("name", ""))
            and "linux" in str(job.get("name", "")).lower()
            and job.get("status") == "completed"
            and job.get("conclusion") == "success"
            for job in entries
        )
        for architecture in ARCHITECTURES
    }
    if not all(successful_linux_jobs.values()):
        raise ValueError("Module artifact source did not pass every Linux architecture job")

    expected = {artifact_name(workflow, module, architecture) for architecture in ARCHITECTURES}
    found = {
        artifact.get("name"): artifact
        for artifact in artifacts.get("artifacts", [])
        if artifact.get("name") in expected
    }
    if set(found) != expected:
        raise ValueError("Module artifact source lacks both exact Linux architecture artifacts")
    for name, artifact in found.items():
        source = artifact.get("workflow_run", {})
        if artifact.get("expired") is not False or source.get("id") != run["id"] or source.get("head_sha") != commit:
            raise ValueError(f"Module artifact provenance differs: {name}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("run", "jobs", "artifacts"):
        parser.add_argument(f"--{name}", required=True, type=Path)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--workflow", required=True, choices=(
        ".github/workflows/provider-module.yml", ".github/workflows/ocr-module.yml",
    ))
    parser.add_argument("--module", required=True, choices=("supabase", "semantic-search", "ocr"))
    args = parser.parse_args()
    if (args.workflow.endswith("ocr-module.yml")) != (args.module == "ocr"):
        raise ValueError("Module does not match its source workflow")
    verify(
        *(json.loads(path.read_text()) for path in (args.run, args.jobs, args.artifacts)),
        args.repository, args.commit, args.workflow, args.module,
    )
    print("verified exact Linux module artifact source run")


if __name__ == "__main__":
    main()
