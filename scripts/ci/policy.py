#!/usr/bin/env python3
"""Enforce repository CI contracts on parsed YAML; actionlint owns expression syntax."""

import argparse
import json
from pathlib import Path
import re

from workflow_yaml import read_workflow


def queue_errors(concurrency):
    if not isinstance(concurrency, dict) or "queue" not in concurrency:
        return []
    if concurrency["queue"] not in ("single", "max"):
        return ["concurrency queue must be the literal single or max"]
    if concurrency["queue"] == "max" and concurrency.get("cancel-in-progress") not in (None, False, "false"):
        return ["queue: max requires non-cancelling concurrency"]
    return []


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
    errors.extend(queue_errors(concurrency))
    if isinstance(concurrency, str):
        concurrency = {"group": concurrency}
    if any(name in path.name for name in ("release", "module", "compositor", "qualification")):
        if concurrency.get("cancel-in-progress") not in (None, False, "false"):
            errors.append("artifact producers/publication must not cancel an active run")
    for key, job in jobs.items():
        errors.extend(f"{key}: {error}" for error in queue_errors(job.get("concurrency", {})))
        if job.get("continue-on-error") not in (None, False, "false"):
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
            if use.startswith("actions/download-artifact@"):
                errors.append(f"{key}: use the verified artifact downloader to bind immutable IDs and ZIP digests")
            if use.startswith("./"):
                if not (root / use).exists():
                    errors.append(f"{key}: local action/workflow does not exist: {use}")
            elif not re.fullmatch(r"[^@]+@[0-9a-f]{40}", use):
                errors.append(f"{key}: action/workflow must be pinned to a full commit SHA: {use}")
    return errors


def check(root):
    errors = []
    documents = {}
    for path in sorted(path for path in (root / ".github/workflows").iterdir() if path.suffix in (".yml", ".yaml")):
        try:
            documents[path.name] = read_workflow(path)
            errors.extend(f"{path.name}: {error}" for error in validate(documents[path.name], path, root))
        except (ValueError, TypeError) as error:
            errors.append(f"{path.name}: {error}")
    contract = json.loads((root / "config/ci-policy.json").read_text())
    try:
        dependencies({name: {"needs": [Path(job["uses"]).name for job in data["jobs"].values()
                                      if job.get("uses", "").startswith("./.github/workflows/")]}
                      for name, data in documents.items()})
    except ValueError as error:
        errors.append(f"reusable workflow graph: {error}")
    levels = {"none": 0, "read": 1, "write": 2}
    for workflow, document in documents.items():
        for key, caller in document["jobs"].items():
            target = caller.get("uses", "")
            if not target.startswith("./.github/workflows/") or Path(target).name not in documents:
                continue
            child = documents[Path(target).name]
            ceiling = caller.get("permissions", document.get("permissions", {}))
            if not isinstance(ceiling, dict):
                continue
            for child_id, job in child["jobs"].items():
                requested = job.get("permissions", child.get("permissions", {}))
                if not isinstance(requested, dict):
                    continue
                for permission, level in requested.items():
                    if levels.get(level, 0) > levels.get(ceiling.get(permission, "none"), 0):
                        errors.append(f"{workflow}: {key} does not permit {target}/{child_id} {permission}: {level}")
    for workflow, names in contract["required_jobs"].items():
        jobs = documents.get(workflow, {}).get("jobs", {})
        for key, name in names.items():
            if key not in jobs or jobs[key].get("name", key) != name:
                errors.append(f"{workflow}: required job identity changed: {key} / {name}")
    for workflow, jobs in contract.get("required_dependencies", {}).items():
        actual = documents.get(workflow, {}).get("jobs", {})
        for key, parents in jobs.items():
            needs = actual.get(key, {}).get("needs", [])
            needs = [needs] if isinstance(needs, str) else needs
            if set(parents) - set(needs):
                errors.append(f"{workflow}: {key} lost required dependency gates")
    for workflow, concurrency in contract.get("required_concurrency", {}).items():
        if documents.get(workflow, {}).get("concurrency") != concurrency:
            errors.append(f"{workflow}: artifact producer concurrency differs from its reviewed contract")
    for workflow, jobs in contract.get("required_job_concurrency", {}).items():
        for key, concurrency in jobs.items():
            if documents.get(workflow, {}).get("jobs", {}).get(key, {}).get("concurrency") != concurrency:
                errors.append(f"{workflow}: {key} publication concurrency differs from its reviewed contract")
    for workflow, guards in contract.get("qualification_guards", {}).items():
        for key, guard in guards.items():
            if documents.get(workflow, {}).get("jobs", {}).get(key, {}).get("if") != guard:
                errors.append(f"{workflow}: {key} qualification guard differs from its reviewed contract")
    for workflow in ("ci.yml", "ci-configuration.yml"):
        triggers = documents.get(workflow, {}).get("on", {})
        for event in ("pull_request", "merge_group"):
            if event not in triggers or (isinstance(triggers[event], dict)
                                         and any(key in triggers[event] for key in ("paths", "paths-ignore"))):
                errors.append(f"{workflow}: required checks must run for every {event}")
    for path in [*(root / ".github/actions").glob("*/action.yml"), *(root / ".github/actions").glob("*/action.yaml")]:
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
