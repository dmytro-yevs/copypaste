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


RELEASE_ENVIRONMENT = "release"
DEFAULT_BRANCH_DISPATCH = ("github.event_name != 'workflow_dispatch' || "
                           "github.ref == format('refs/heads/{0}', github.event.repository.default_branch)")
# A pushed v* tag already starts the full release, so a manual run on one adds no exposure.
RELEASE_REF_DISPATCH = DEFAULT_BRANCH_DISPATCH + " || startsWith(github.ref, 'refs/tags/v')"
# The marketplace publisher replaces the live catalog and never needs a release commit.
DEFAULT_BRANCH_ONLY = ("module-marketplace.yml",)
PULL_REQUEST_EVENTS = ("pull_request", "pull_request_target", "merge_group")
UPDATER_SIGNER = "scripts/release/updater-signer"


def conjuncts(condition):
    """Return the operands that must all hold for a job condition, without outer parentheses."""
    text = str(condition or "").strip()
    if text.startswith("${{") and text.endswith("}}"):
        text = text[3:-2].strip()
    while True:
        depth, quoted, closed, operators = 0, False, None, []
        for index, character in enumerate(text):
            if character == "'":
                quoted = not quoted
            elif not quoted:
                depth += (character == "(") - (character == ")")
                if depth == 0 and character == ")" and closed is None:
                    closed = index
                if depth == 0 and text[index:index + 2] in ("&&", "||"):
                    operators.append((index, text[index:index + 2]))
        if not text.startswith("(") or closed != len(text) - 1:
            break
        text = text[1:-1].strip()
    if not operators or any(operator == "||" for _, operator in operators):
        return [text]
    starts = [0, *(index + 2 for index, _ in operators)]
    ends = [*(index for index, _ in operators), len(text)]
    return [operand for start, end in zip(starts, ends) for operand in conjuncts(text[start:end])]


def reads_secrets(fragment):
    """Whether any expression in a workflow fragment uses the secrets context."""
    return any(re.search(r"\bsecrets\b", expression)
               for expression in re.findall(r"\$\{\{.*?\}\}", json.dumps(fragment), re.DOTALL))


def privileged(job):
    """Whether a job can sign or publish: it reads or forwards secrets, or may write repository contents."""
    permissions = job.get("permissions")
    return ("secrets" in job or reads_secrets(job)
            or (isinstance(permissions, dict) and permissions.get("contents") == "write"))


def refuses_untrusted_dispatch(jobs, key, guards, visiting=()):
    """Whether a manual run on a ref no guard allows skips this job, directly or through a job it needs."""
    job = jobs[key]
    required = conjuncts(job.get("if"))
    if set(guards) & set(required):
        return True
    needs = job.get("needs", [])
    # A status function lets a job run after a skipped dependency unless it also requires that success.
    survives_skips = re.search(r"\b(always|cancelled|failure)\s*\(", str(job.get("if", "")))
    return any(parent in jobs and parent not in visiting
               and (not survives_skips or f"needs.{parent}.result == 'success'" in required)
               and refuses_untrusted_dispatch(jobs, parent, guards, (*visiting, key))
               for parent in ([needs] if isinstance(needs, str) else needs))


def release_boundary_errors(documents):
    """Keep signing and publishing inside the release environment and away from pull-request code."""
    def events(document):
        triggers = document.get("on", {})
        return [triggers] if isinstance(triggers, str) else list(triggers)

    errors = []
    # Every workflow a pull request runs, directly or through reusable workflow calls.
    reachable = {name for name, document in documents.items() if set(events(document)) & set(PULL_REQUEST_EVENTS)}
    pending = list(reachable)
    while pending:
        for job in documents[pending.pop()]["jobs"].values():
            callee = Path(job.get("uses", "")).name
            if job.get("uses", "").startswith("./.github/workflows/") and callee in set(documents) - reachable:
                reachable.add(callee)
                pending.append(callee)
    for name, document in documents.items():
        jobs = document["jobs"]
        if reads_secrets(document.get("env")):
            errors.append(f"{name}: workflow-level env would hand a secret to every job")
        for key, job in jobs.items():
            if not privileged(job):
                continue
            if name in reachable:
                errors.append(f"{name}: {key} would hand release secrets to pull-request code")
                continue
            if "runs-on" in job and job.get("environment") != RELEASE_ENVIRONMENT:
                errors.append(f"{name}: {key} must sign or publish in the {RELEASE_ENVIRONMENT} environment")
            strict = name in DEFAULT_BRANCH_ONLY
            guards = (DEFAULT_BRANCH_DISPATCH,) if strict else (DEFAULT_BRANCH_DISPATCH, RELEASE_REF_DISPATCH)
            if "workflow_dispatch" in events(document) and not refuses_untrusted_dispatch(jobs, key, guards):
                errors.append(f"{name}: {key} must refuse a manual run outside the default branch"
                              + ("" if strict else " and v* release tags"))
    return errors


def signer_errors(root):
    """Require the updater signer to come from the committed, integrity-pinned lockfile."""
    script = root / "scripts/release/sign-update-artifact.sh"
    if not script.is_file():
        return []
    errors = []
    text = script.read_text(encoding="utf-8")
    if "npm ci --ignore-scripts" not in text or re.search(r"\bnpx\b|\bnpm\s+(install|i|exec|x)\b", text):
        errors.append("updater signer must be installed with npm ci --ignore-scripts from the committed lockfile")
    try:
        manifest = json.loads((root / UPDATER_SIGNER / "package.json").read_text(encoding="utf-8"))
        packages = json.loads((root / UPDATER_SIGNER / "package-lock.json").read_text(encoding="utf-8"))["packages"]
    except (OSError, ValueError, KeyError):
        return [*errors, "updater signer manifest or lockfile is missing or unreadable"]
    pinned = manifest.get("dependencies", {})
    if (not pinned or set(manifest) & {"devDependencies", "optionalDependencies", "scripts"}
            or any(not re.fullmatch(r"\d+\.\d+\.\d+", str(version)) for version in pinned.values())
            or packages.get("", {}).get("dependencies") != pinned):
        errors.append("updater signer dependencies must be exact versions recorded in the lockfile")
    for name, entry in packages.items():
        if name and (not str(entry.get("resolved", "")).startswith("https://registry.npmjs.org/")
                     or not str(entry.get("integrity", "")).startswith("sha512-") or entry.get("hasInstallScript")):
            errors.append(f"updater signer lockfile entry lacks a pinned integrity or runs install scripts: {name}")
    return errors


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
            if use.startswith("actions/upload-artifact@"):
                days = step.get("with", {}).get("retention-days")
                if type(days) is not int or not 1 <= days <= 90:
                    errors.append(f"{key}: artifact retention must be an explicit literal 1..90 days")
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
    for workflow, document in documents.items():
        limits = contract.get("artifact_retention_days", {}).get(workflow, {})
        for key, job in document.get("jobs", {}).items():
            for step in job.get("steps", []):
                if not step.get("uses", "").startswith("actions/upload-artifact@"):
                    continue
                if key not in limits:
                    errors.append(f"{workflow}: {key} artifact retention has no reviewed classification")
                elif step.get("with", {}).get("retention-days") != limits[key]:
                    errors.append(f"{workflow}: {key} artifact retention differs from its reviewed contract")
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
    errors.extend(release_boundary_errors(documents))
    errors.extend(signer_errors(root))
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
