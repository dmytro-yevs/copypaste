"""Bind in-progress producers to this exact release run, never to another active run."""


def current_release(run, current_run_id):
    return (type(run.get("id")) is int and run["id"] > 0
            and str(run["id"]) == str(current_run_id)
            and run.get("path") == ".github/workflows/release.yml"
            and run.get("event") in ("push", "workflow_dispatch")
            # GitHub reports the entire run as queued while a reusable child is
            # queued, even when other jobs (including this verifier) are running.
            and run.get("status") in ("queued", "in_progress", "pending", "waiting"))


def successful_jobs(jobs):
    entries = jobs.get("jobs", [])
    if jobs.get("total_count") != len(entries):
        raise ValueError("producer job inventory is incomplete")
    names = [job.get("name") for job in entries]
    if len(names) != len(set(names)):
        raise ValueError("producer job inventory is ambiguous")
    return {job["name"] for job in entries
            if job.get("status") == "completed" and job.get("conclusion") == "success"}
