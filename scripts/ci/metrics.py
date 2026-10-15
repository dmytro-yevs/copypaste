#!/usr/bin/env python3
"""Collect paginated GitHub job timings; keep unsuccessful and unknown-cache cohorts separate."""

import argparse
from collections import Counter, defaultdict
from datetime import datetime, timezone
import json
import math
from pathlib import Path
import subprocess

from workflow_yaml import read_workflow


def seconds(start, end):
    if not start or not end:
        return None
    return max(0, (datetime.fromisoformat(end.replace("Z", "+00:00"))
                   - datetime.fromisoformat(start.replace("Z", "+00:00"))).total_seconds())


def percentile(values, fraction):
    return sorted(values)[max(0, math.ceil(len(values) * fraction) - 1)] if values else None


def graph(root):
    workflows = {}
    for path in sorted((root / ".github/workflows").glob("*.yml")):
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
    if run["path"] == ".github/workflows/release.yml":
        mode = "build" if "android" in by_name else "reuse-or-qualification"
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
    parser.add_argument("--limit", type=int, default=10)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if not 1 <= args.limit <= 100:
        parser.error("limit must be between 1 and 100")
    topology = graph(args.root)
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
        if args.archive:
            raw = json.loads((args.archive / f"{workflow}.runs.json").read_text())["workflow_runs"]
            results = [measure(run, json.loads((args.archive / f"{run['id']}.jobs.json").read_text()))
                       for run in raw if (args.archive / f"{run['id']}.jobs.json").exists()]
        else:
            results = collect(args.repository, workflow, args.limit)
        report["workflows"][workflow] = {"runs": results, "cohorts": summarize(results),
                                        "conclusions": dict(Counter(run["conclusion"] for run in results))}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote {sum(len(w['runs']) for w in report['workflows'].values())} measured runs to {args.output}")


if __name__ == "__main__":
    main()
