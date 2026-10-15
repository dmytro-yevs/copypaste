#!/usr/bin/env python3
"""Enforce repository CI contracts on parsed YAML; actionlint owns expression syntax."""

import argparse
import json
from pathlib import Path
import re

from workflow_yaml import read_workflow


def dependencies(jobs):
    graph = {}
    for key, job in jobs.items():
        needs = job.get("needs", [])
        graph[key] = [needs] if isinstance(needs, str) else needs
        if not isinstance(graph[key], list) or set(graph[key]) - set(jobs):
            raise ValueError(f"{key}: unknown job dependency")
    active, done = set(), set()

    def visit(key):
        if key in active:
            raise ValueError(f"{key}: circular job dependency")
        if key in done:
            return
        active.add(key)
        for parent in graph[key]:
            visit(parent)
        active.remove(key)
        done.add(key)

    for key in graph:
        visit(key)
    return graph


def validate(document, path, root):
    errors = []
    jobs = document.get("jobs", {})
    if not jobs:
        return ["workflow has no jobs"]
    try:
        dependencies(jobs)
    except ValueError as error:
        errors.append(str(error))
    permissions = document.get("permissions")
    if not isinstance(permissions, dict) or any(value != "read" for value in permissions.values()):
        errors.append("workflow permissions must be explicit read-only; grant writes per job")
    triggers = document.get("on", {})
    if isinstance(triggers, str):
        triggers = [triggers]
    if "pull_request_target" in triggers:
        errors.append("pull_request_target is forbidden for repository code execution")
    concurrency = document.get("concurrency", {})
    if any(name in path.name for name in ("release", "module", "compositor", "qualification")):
        if concurrency.get("cancel-in-progress") not in (None, "false"):
            errors.append("artifact producers/publication must not cancel an active run")
    for key, job in jobs.items():
        if job.get("continue-on-error") not in (None, "false"):
            errors.append(f"{key}: required jobs cannot ignore failures")
        if "runs-on" in job:
            timeout = job.get("timeout-minutes")
            if not isinstance(timeout, int) or not 1 <= timeout <= 120:
                errors.append(f"{key}: runner jobs require an explicit 1..120 minute timeout")
        steps = job.get("steps", [])
        for step in [job, *steps]:
            use = step.get("uses", "")
            if not use:
                continue
            if use.startswith("./"):
                if not (root / use).exists():
                    errors.append(f"{key}: local action/workflow does not exist: {use}")
            elif not re.fullmatch(r"[^@]+@[0-9a-f]{40}", use):
                errors.append(f"{key}: action/workflow must be pinned to a full commit SHA: {use}")
    return errors


def check(root):
    errors = []
    documents = {}
    for path in sorted((root / ".github/workflows").glob("*.yml")):
        try:
            documents[path.name] = read_workflow(path)
            errors.extend(f"{path.name}: {error}" for error in validate(documents[path.name], path, root))
        except (ValueError, TypeError) as error:
            errors.append(f"{path.name}: {error}")
    contract = json.loads((root / "config/ci-policy.json").read_text())
    for workflow, names in contract["required_jobs"].items():
        jobs = documents.get(workflow, {}).get("jobs", {})
        for key, name in names.items():
            if key not in jobs or jobs[key].get("name", key) != name:
                errors.append(f"{workflow}: required job identity changed: {key} / {name}")
    for path in (root / ".github/actions").glob("*/action.yml"):
        for step in read_workflow(path).get("runs", {}).get("steps", []):
            use = step.get("uses", "")
            if use and not use.startswith("./") and not re.fullmatch(r"[^@]+@[0-9a-f]{40}", use):
                errors.append(f"{path}: unpinned composite dependency: {use}")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    errors = check(args.root)
    if errors:
        raise SystemExit("\n".join(errors))
    print("CI policy contracts passed")


if __name__ == "__main__":
    main()
