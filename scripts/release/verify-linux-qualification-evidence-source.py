#!/usr/bin/env python3
"""Accept an exact standalone or qualification-only parent evidence run."""
import argparse
import json
from pathlib import Path


BASE = ["Verify exact artifact sources", "Assemble verified Linux native qualification",
        "x86_64 GNOME x11", "x86_64 GNOME wayland", "x86_64 KDE x11", "x86_64 KDE wayland",
        "aarch64 GNOME x11", "aarch64 GNOME wayland", "aarch64 KDE x11", "aarch64 KDE wayland"]
PREFIX = "Run exact staged Linux native qualification / "
PARENT_SKIPPED = {"macos", "android", "windows",
                  "Build opt-in compositor runtime companions", "Verify staged compositor runtime companions",
                  "Sign and bind opt-in compositor runtime companions", "Linux full parity evidence gate", "qualify", "publish"}


def verify(run, jobs, repository, commit):
    if (run.get("head_sha") != commit or run.get("head_repository", {}).get("full_name") != repository
            or run.get("status") != "completed" or run.get("conclusion") != "success"):
        raise ValueError("evidence run provenance differs")
    direct = run.get("path") == ".github/workflows/linux-native-qualification.yml" and run.get("event") == "workflow_dispatch"
    parent = run.get("path") == ".github/workflows/release.yml" and run.get("event") == "workflow_dispatch"
    if not (direct or parent):
        raise ValueError("evidence run is not an accepted qualification workflow")
    entries = jobs.get("jobs", [])
    if jobs.get("total_count") != len(entries):
        raise ValueError("evidence job inventory is incomplete")
    successful = {job.get("name") for job in entries if job.get("status") == "completed" and job.get("conclusion") == "success"}
    prefix = PREFIX if parent else ""
    if not {prefix + name for name in BASE} <= successful:
        raise ValueError("evidence run did not pass every native qualification job")
    if parent:
        by_name = {job.get("name"): job for job in entries}
        if not PARENT_SKIPPED <= set(by_name) or any(by_name[name].get("conclusion") != "skipped" for name in PARENT_SKIPPED):
            raise ValueError("qualification-only parent did not skip rebuild and publication jobs")
        linux_builds = [job for job in entries if str(job.get("name", "")).startswith("Linux build")]
        if not linux_builds or any(job.get("status") != "completed" or job.get("conclusion") != "skipped" for job in linux_builds):
            raise ValueError("qualification-only parent did not skip every Linux build job")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", required=True, type=Path); parser.add_argument("--jobs", required=True, type=Path)
    parser.add_argument("--repository", required=True); parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    verify(json.loads(args.run.read_text()), json.loads(args.jobs.read_text()), args.repository, args.commit)


if __name__ == "__main__": main()
