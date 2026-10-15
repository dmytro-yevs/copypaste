#!/usr/bin/env python3
"""Collect paginated GitHub job timings; keep unsuccessful and unknown-cache cohorts separate."""

import argparse
import base64
from collections import Counter, defaultdict
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import re
import subprocess

from workflow_yaml import parse_workflow, read_workflow


def seconds(start, end):
    if not start or not end:
        return None
    def timestamp(value):
        # Actions logs have seven fractional digits; older Python accepts six.
        value = re.sub(r"(\.\d{6})\d+", r"\1", value)
        return datetime.fromisoformat(value.replace("Z", "+00:00"))
    return max(0, (timestamp(end) - timestamp(start)).total_seconds())


def percentile(values, fraction):
    return sorted(values)[max(0, math.ceil(len(values) * fraction) - 1)] if values else None


def cache_observations(log):
    """Measure cache toolkit outcomes, excluding echoed commands and double-counted keys."""
    result = {"restored": 0, "missed": 0, "incomplete_restores": 0,
              "restore_s": 0, "restored_bytes": 0,
              "coverage": "actions/cache toolkit log markers; Gradle task cache hits are separate"}
    pending = None
    for line in log.splitlines():
        match = re.match(r"^(\d{4}-\d\d-\d\dT\S+Z) (Cache .*)$", line)
        if not match:
            continue
        timestamp, message = match.groups()
        if message.startswith("Cache hit for:"):
            if pending:
                result["incomplete_restores"] += 1
            pending = timestamp
        elif message.startswith("Cache not found for input keys:"):
            result["missed"] += 1
        elif message.startswith("Cache Size:") and pending:
            size = re.search(r"\(([0-9]+) B\)", message)
            if size:
                result["restored_bytes"] += int(size.group(1))
        elif message == "Cache restored successfully" and pending:
            result["restored"] += 1
            result["restore_s"] += seconds(pending, timestamp)
            pending = None
    if pending:
        result["incomplete_restores"] += 1
    attempts = result["restored"] + result["missed"] + result["incomplete_restores"]
    complete = attempts and not result["incomplete_restores"]
    result["observed_hit_rate"] = result["restored"] / attempts if complete else None
    result["restore_s"] = round(result["restore_s"], 3) if complete else None
    if result["incomplete_restores"]:
        # Parallel or differently formatted cache clients cannot be paired
        # reliably from these markers. This is missing telemetry, not a miss.
        result["restored_bytes"] = None
        result["coverage"] += "; unpaired/parallel markers: transfer totals and hit rate are unavailable"
    return result


def collect_cache_logs(repository, run):
    for job in run["jobs"]:
        if job["conclusion"] == "skipped":
            continue
        result = subprocess.run(["gh", "api", "--allow-escape-sequences",
                                 f"repos/{repository}/actions/jobs/{job['id']}/logs"], capture_output=True, text=True)
        if result.returncode:
            job["cache"] = {"coverage": "log unavailable", "observed_hit_rate": None}
        else:
            job["cache"] = cache_observations(result.stdout)
            phases = defaultdict(float)
            for line in result.stdout.splitlines():
                match = re.match(r"^\d{4}-\d\d-\d\dT\S+Z COPYPASTE_CI_TIMING (.*)$", line)
                if match:
                    value = json.loads(match.group(1))
                    if value.get("exit_code") == 0 and isinstance(value.get("duration_s"), (int, float)):
                        phases[value["phase"]] += value["duration_s"]
            job["phases_s"] = dict(phases)
    # Keep unknown runs unknown: these log markers do not cover every compiler
    # and Gradle task cache, and cannot establish a wholly cold or warm build.


def graph(root):
    workflows = {}
    for path in sorted(path for path in (root / ".github/workflows").iterdir() if path.suffix in (".yml", ".yaml")):
        data = read_workflow(path)
        workflows[path.name] = {
            "name": data["name"], "triggers": data.get("on"),
            "jobs": {key: {
                "name": job.get("name", key),
                "needs": ([job["needs"]] if isinstance(job.get("needs"), str)
                          else job.get("needs", [])),
                "workflow": job.get("uses"), "condition": job.get("if"),
                "runner": job.get("runs-on"), "matrix": job.get("strategy", {}).get("matrix"),
            } for key, job in data["jobs"].items()},
        }
    return workflows


def source_graph(repository, commit, workflow):
    """Read source workflow YAML as data; never check out or execute source-run code."""
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("invalid source commit")
    result = {}

    def read(name):
        if name in result:
            return
        if not re.fullmatch(r"[A-Za-z0-9_-]+\.ya?ml", name):
            raise ValueError("invalid source workflow name")
        content = api(repository, f"contents/.github/workflows/{name}?ref={commit}")
        data = parse_workflow(base64.b64decode(content["content"]).decode("utf-8"))
        result[name] = {"name": data["name"], "triggers": data.get("on"), "jobs": {}}
        for key, job in data["jobs"].items():
            needs = job.get("needs", [])
            result[name]["jobs"][key] = {"name": job.get("name", key),
                "needs": [needs] if isinstance(needs, str) else needs,
                "workflow": job.get("uses"), "matrix": job.get("strategy", {}).get("matrix"),
                "runner": job.get("runs-on"), "condition": job.get("if")}
            if job.get("uses", "").startswith("./.github/workflows/"):
                read(Path(job["uses"]).name)
    read(workflow)
    return result


def compare(before, after):
    def cohorts(report):
        result = defaultdict(list)
        for workflow, data in report["workflows"].items():
            for run in data["runs"]:
                if run["conclusion"] == "success" and run["wall_clock_s"] is not None:
                    result[(workflow, run["event"], run["mode"], run["cache_cohort"])].append(run["wall_clock_s"])
        return result
    previous, current = cohorts(before), cohorts(after)
    comparisons = []
    for key in sorted(set(previous) | set(current)):
        left, right = previous[key], current[key]
        comparable = key[3] in ("cold", "warm") and len(left) >= 3 and len(right) >= 3
        old, new = percentile(left, .5), percentile(right, .5)
        comparisons.append({"workflow": key[0], "event": key[1], "mode": key[2], "cache": key[3],
            "before_n": len(left), "after_n": len(right), "before_p50_s": old, "after_p50_s": new,
            "before_p95_s": percentile(left, .95), "after_p95_s": percentile(right, .95),
            "comparable": comparable, "improvement_percent": round(100 * (old - new) / old, 2) if comparable and old else None,
            "limitation": None if comparable else "Require matching classified cache cohorts and at least three successful samples per side."})
    return comparisons


def measure(run, inventory):
    jobs = inventory["jobs"]
    if inventory["total_count"] != len(jobs):
        raise ValueError("incomplete job inventory")
    entries = []
    for job in jobs:
        steps = [{"name": step["name"], "conclusion": step.get("conclusion"),
                  "duration_s": seconds(step.get("started_at"), step.get("completed_at"))}
                 for step in job.get("steps", [])]
        entries.append({"id": job["id"], "name": job["name"],
                        "conclusion": job.get("conclusion"),
                        "duration_s": seconds(job.get("started_at"), job.get("completed_at")),
                        "scheduling_delay_s": seconds(job.get("created_at"), job.get("started_at")),
                        "started_at": job.get("started_at"), "completed_at": job.get("completed_at"),
                        "steps": steps})
    executed = [job for job in entries if job["conclusion"] != "skipped" and job["started_at"]]
    completed = [job["completed_at"] for job in executed if job["completed_at"]]
    failures = [job["completed_at"] for job in executed
                if job["conclusion"] == "failure" and job["completed_at"]]
    by_name = {job["name"]: job for job in executed}
    mode = "validation"
    if run["path"] == ".github/workflows/ci.yml" and "rust-and-supabase" not in by_name:
        mode = "documentation-or-incomplete"
    if run["path"] == ".github/workflows/release.yml":
        mode = "build" if "android" in by_name else "reuse-or-qualification"
        if any(name.startswith("Qualify current Linux artifacts / ") for name in by_name):
            mode = "integrated-build"
        if "publish" in by_name:
            mode += "-publish"
    return {
        "id": run["id"], "url": run["html_url"], "commit": run["head_sha"],
        "attempt": run.get("run_attempt", 1), "event": run["event"], "mode": mode,
        "conclusion": run.get("conclusion"), "created_at": run["created_at"],
        "wall_clock_s": seconds(run["created_at"], max(completed)) if completed else None,
        "initial_queue_s": seconds(run["created_at"], min(j["started_at"] for j in executed)) if executed else None,
        "first_failed_job_signal_s": seconds(run["created_at"], min(failures)) if failures else None,
        "job_execution_s": sum(job["duration_s"] or 0 for job in executed),
        "cache_cohort": "unknown", "jobs": entries,
    }


def summarize(runs):
    cohorts = defaultdict(list)
    for run in runs:
        cohorts[(run["mode"], run["cache_cohort"], run["conclusion"])].append(run)
    return [{"mode": key[0], "cache": key[1], "conclusion": key[2], "count": len(items),
             "wall_clock_p50_s": percentile([r["wall_clock_s"] for r in items if r["wall_clock_s"] is not None], .5),
             "wall_clock_p95_s": percentile([r["wall_clock_s"] for r in items if r["wall_clock_s"] is not None], .95)}
            for key, items in sorted(cohorts.items())]


def critical_path(topology, workflow, jobs):
    """Expand local reusable workflows; matrix rows contribute their longest execution."""
    nodes = {}

    def expand(filename, prefix="", identity=""):
        groups = {}
        definition = topology[filename]["jobs"]
        for key, job in definition.items():
            node_id = identity + key
            label = prefix + job["name"]
            called = job.get("workflow")
            if called and called.startswith("./.github/workflows/"):
                groups[key] = expand(Path(called).name, label + " / ", node_id + "/")
                continue
            # Expressions are placeholders in labels, not executable programs.
            pieces = re.split(r"(\$\{\{.*?\}\})", label)
            pattern = "".join(".*" if part.startswith("${{") else re.escape(part) for part in pieces)
            if job.get("matrix") and "${{" not in label:
                pattern += r"(?: \(.*\))?"
            matches = [item for item in jobs if re.fullmatch(pattern, item["name"])
                       and item["conclusion"] != "skipped"]
            longest = max(matches, key=lambda item: item["duration_s"] or 0, default={})
            nodes[node_id] = {"name": longest.get("name", label), "weight": longest.get("duration_s") or 0, "needs": []}
            groups[key] = ([node_id], [node_id])
        depended_on = set()
        for key, job in definition.items():
            for parent in job["needs"]:
                depended_on.add(parent)
                for node in groups[key][0]:
                    nodes[node]["needs"].extend(groups[parent][1])
        roots = [node for key, group in groups.items() if not definition[key]["needs"] for node in group[0]]
        leaves = [node for key, group in groups.items() if key not in depended_on for node in group[1]]
        return roots, leaves

    expand(workflow)
    memo, active = {}, set()

    def visit(key):
        if key in active:
            raise ValueError("workflow dependency cycle")
        if key in memo:
            return memo[key]
        active.add(key)
        node = nodes[key]
        duration, path = max((visit(parent) for parent in node["needs"]), default=(0, []), key=lambda value: value[0])
        memo[key] = duration + node["weight"], path + ([node["name"]] if node["weight"] else [])
        active.remove(key)
        return memo[key]

    duration, path = max((visit(key) for key in nodes), default=(0, []), key=lambda value: value[0])
    return {"execution_s": duration, "jobs": path,
            "method": "Longest dependency path of observed job execution; excludes queue/setup gaps between jobs and uses longest matrix row."}


def markdown(report):
    lines = ["# CI timing report", "", "Cache cohorts remain unknown unless cache telemetry is available.", "",
             "| Workflow / run | Conclusion | Wall clock (s) | Initial queue (s) | Job execution sum (s) |",
             "| --- | --- | ---: | ---: | ---: |"]
    for workflow, data in report["workflows"].items():
        for run in data["runs"]:
            lines.append(f"| [{workflow} / {run['id']}]({run['url']}) | {run['conclusion']} | {run['wall_clock_s']} | {run['initial_queue_s']} | {run['job_execution_s']} |")
            critical = run.get("critical_path", {})
            if critical.get("jobs"):
                lines += ["", f"Run {run['id']} execution critical path ({critical['execution_s']}s): " + " → ".join(critical["jobs"]), ""]
            budget = run.get("budget", {})
            if budget.get("exceeded"):
                lines += ["", f"Performance review: {run['wall_clock_s']}s exceeds the {budget['limit_s']}s investigation threshold.", ""]
            for job in run["jobs"]:
                if job.get("phases_s"):
                    lines.append(f"- {job['name']}: " + ", ".join(f"{phase} {duration:.1f}s" for phase, duration in job["phases_s"].items()))
    return "\n".join(lines) + "\n"


def api(repository, endpoint, paginated=False):
    command = ["gh", "api"]
    if paginated:
        command += ["--paginate", "--slurp"]
    return json.loads(subprocess.check_output([*command, f"repos/{repository}/{endpoint}"], text=True))


def collect(repository, workflow, limit):
    runs = api(repository, f"actions/workflows/{workflow}/runs?per_page={limit}")["workflow_runs"]
    results = []
    for run in runs:
        if run["status"] != "completed":
            continue
        pages = api(repository, f"actions/runs/{run['id']}/attempts/{run['run_attempt']}/jobs?per_page=100", True)
        jobs = [job for page in pages for job in page["jobs"]]
        results.append(measure(run, {"jobs": jobs, "total_count": len(jobs)}))
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", default="dmytro-yevs/copypaste")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--archive", type=Path, help="Read previously collected API JSON without network access")
    parser.add_argument("--workflow", action="append", help="Workflow filename; defaults to every workflow")
    parser.add_argument("--run-id", type=int, help="Collect one completed source run")
    parser.add_argument("--summary", type=Path, help="Write a Markdown timing summary")
    parser.add_argument("--compare", type=Path, help="Compare matching successful cohorts with a prior report")
    parser.add_argument("--cache-logs", "--logs", action="store_true", help="Measure cache and phase telemetry from complete job logs")
    parser.add_argument("--limit", type=int, default=10)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if not 1 <= args.limit <= 100:
        parser.error("limit must be between 1 and 100")
    topology = graph(args.root)
    budgets = json.loads((args.root / "config/ci-performance-budgets.json").read_text())["wall_clock_seconds"]
    single = None
    if args.run_id:
        run = api(args.repository, f"actions/runs/{args.run_id}")
        if run["status"] != "completed":
            raise ValueError("final timing requires a completed source run")
        workflow = Path(run["path"]).name
        if workflow not in topology:
            raise ValueError("source workflow is not registered in the local topology")
        pages = api(args.repository, f"actions/runs/{run['id']}/attempts/{run['run_attempt']}/jobs?per_page=100", True)
        jobs = [job for page in pages for job in page["jobs"]]
        single = measure(run, {"total_count": len(jobs), "jobs": jobs})
        topology.update(source_graph(args.repository, run["head_sha"], workflow))
        args.workflow = [workflow]
    report = {"schema": 1, "repository": args.repository,
              "collected_at": datetime.now(timezone.utc).isoformat(),
              "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=args.root, text=True).strip(),
              "limitations": ["Cache state is unknown without per-cache log evidence; unknown is never called warm or cold.",
                              "Job scheduling delay may include dependency waiting; it is not runner queue time.",
                              "Wall clock ends at the final job, not the mutable workflow updated_at timestamp.",
                              "Cancelled and failed runs do not measure successful lifecycle performance.",
                              "Historical modes are inferred from executed job names; compare exact inputs before claiming improvement."],
              "dependency_graph": topology, "workflows": {}}
    for workflow in args.workflow or topology:
        if single:
            results = [single]
        elif args.archive:
            raw = json.loads((args.archive / f"{workflow}.runs.json").read_text())["workflow_runs"]
            results = [measure(run, json.loads((args.archive / f"{run['id']}.jobs.json").read_text()))
                       for run in raw if (args.archive / f"{run['id']}.jobs.json").exists()]
        else:
            results = collect(args.repository, workflow, args.limit)
        for run in results:
            run["critical_path"] = critical_path(topology, workflow, run["jobs"])
            run["critical_path"]["source_topology_verified"] = bool(single) or run["commit"] == report["source_commit"]
            if args.cache_logs:
                collect_cache_logs(args.repository, run)
            limit = budgets.get(workflow)
            eligible = run["conclusion"] == "success" and run["mode"] != "documentation-or-incomplete"
            run["budget"] = {"limit_s": limit, "eligible": eligible,
                             "exceeded": bool(eligible and limit and run["wall_clock_s"] and run["wall_clock_s"] > limit)}
        report["workflows"][workflow] = {"runs": results, "cohorts": summarize(results),
                                        "conclusions": dict(Counter(run["conclusion"] for run in results))}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    if args.compare:
        report["comparison"] = compare(json.loads(args.compare.read_text()), report)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    summary = args.summary or (Path(os.environ["GITHUB_STEP_SUMMARY"]) if os.environ.get("GITHUB_STEP_SUMMARY") else None)
    if summary:
        with summary.open("a", encoding="utf-8") as output:
            output.write(markdown(report))
    print(f"Wrote {sum(len(w['runs']) for w in report['workflows'].values())} measured runs to {args.output}")


if __name__ == "__main__":
    main()
