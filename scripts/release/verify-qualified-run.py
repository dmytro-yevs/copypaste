#!/usr/bin/env python3
"""Accept a completed production qualification at the immutable release tag."""

import argparse
import json
import runpy
from pathlib import Path
from typing import Optional, Tuple


def verify(
    run: dict,
    jobs: dict,
    artifacts: dict,
    repository: str,
    commit: str,
    *,
    require_linux: bool = False,
    require_compositor_runtime: bool = False,
    linux_origin: Optional[Tuple[dict, dict, dict, dict]] = None,
) -> None:
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
    if require_linux:
        required_linux_jobs = {"Linux full parity evidence gate"}
        passed_linux_jobs = {
            job.get("name")
            for job in entries
            if job.get("status") == "completed" and job.get("conclusion") == "success"
        }
        missing = required_linux_jobs - passed_linux_jobs
        if missing:
            raise ValueError(f"required Linux qualification jobs did not pass: {', '.join(sorted(missing))}")
        builds = {f"Linux build ({architecture})" for architecture in ("x86_64", "aarch64")}
        if not builds <= passed_linux_jobs:
            skipped_builds = [
                job for job in entries
                if isinstance(job.get("name"), str) and job["name"].startswith("Linux build (")
            ]
            if not skipped_builds or any(
                job.get("status") != "completed" or job.get("conclusion") != "skipped"
                for job in skipped_builds
            ):
                raise ValueError("Linux artifact recovery requires explicitly skipped build jobs")
            if linux_origin is None:
                raise ValueError("Linux builds were skipped without a verified artifact origin")
            origin_run, origin_jobs, origin_artifacts, receipt = linux_origin
            verifier = runpy.run_path(str(Path(__file__).with_name("verify-linux-artifact-source-run.py")))
            verifier["verify"](origin_run, origin_jobs, origin_artifacts, repository, commit)
            if receipt.get("platform") != "linux" or str(receipt.get("run_id")) != str(origin_run.get("id")):
                raise ValueError("Linux receipt does not bind the verified artifact origin")
    if require_compositor_runtime:
        if not require_linux:
            raise ValueError("compositor runtime qualification requires Linux qualification")
        matching_runtime = [
            job for job in entries
            if job.get("name") == "Sign and bind opt-in compositor runtime companions"
        ]
        if (
            len(matching_runtime) != 1
            or matching_runtime[0].get("status") != "completed"
            or matching_runtime[0].get("conclusion") != "success"
        ):
            raise ValueError("required compositor runtime qualification job did not pass")
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
    parser.add_argument("--require-linux", action="store_true")
    parser.add_argument("--require-compositor-runtime", action="store_true")
    parser.add_argument("--linux-origin-run", type=Path)
    parser.add_argument("--linux-origin-jobs", type=Path)
    parser.add_argument("--linux-origin-artifacts", type=Path)
    parser.add_argument("--linux-receipt", type=Path)
    args = parser.parse_args()
    origin_paths = (args.linux_origin_run, args.linux_origin_jobs, args.linux_origin_artifacts, args.linux_receipt)
    if any(origin_paths) and not all(origin_paths):
        raise ValueError("Linux artifact origin requires complete run, jobs, artifacts, and receipt evidence")
    verify(
        *(json.loads(path.read_text()) for path in (args.run, args.jobs, args.artifacts)),
        args.repository,
        args.commit,
        require_linux=args.require_linux,
        require_compositor_runtime=args.require_compositor_runtime,
        linux_origin=tuple(json.loads(path.read_text()) for path in origin_paths) if all(origin_paths) else None,
    )
    print("verified successful native qualification at the immutable release tag")


if __name__ == "__main__":
    main()
