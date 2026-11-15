"""Read complete GitHub run inventories without treating API errors as missing data."""

import json
import argparse
from datetime import datetime
import os
import re
import subprocess


def latest_artifacts(payload, attempt):
    records = payload.get("artifacts", [])
    if payload.get("total_count") != len(records):
        raise ValueError("artifact inventory is incomplete")
    groups = {}
    for record in records:
        name = record.get("name")
        if not isinstance(name, str) or not name:
            raise ValueError("artifact name is invalid")
        groups.setdefault(name, []).append(record)
    selected = []
    for name, entries in groups.items():
        if len(entries) == 1:
            selected.append(entries[0])
            continue
        if type(attempt) is not int or attempt < 2:
            raise ValueError("duplicate artifacts outside a workflow rerun: " + name)
        dates = []
        for record in entries:
            try:
                value = re.sub(r"(\.\d{6})\d+", r"\1", record["created_at"])
                date = datetime.fromisoformat(value.replace("Z", "+00:00"))
                if date.utcoffset() is None:
                    raise ValueError("artifact time has no timezone")
                dates.append(date)
            except (KeyError, TypeError, ValueError) as error:
                raise ValueError("artifact retry identity lacks a valid creation time") from error
        if len(set(dates)) != len(dates):
            raise ValueError("artifact retry creation times are ambiguous: " + name)
        # IDs are not chronological. Never fall back to an older unexpired
        # artifact when the newest version is expired or fails verification.
        selected.append(entries[dates.index(max(dates))])
    return {"total_count": len(selected), "artifacts": selected,
            "observed_artifact_count": len(records), "source_run_attempt": attempt}


def inventory(repository, run_id, kind, *, latest=True, run_info=None):
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("invalid repository")
    if not re.fullmatch(r"[1-9][0-9]*", str(run_id)) or kind not in ("run", "jobs", "artifacts"):
        raise ValueError("invalid run inventory request")
    endpoint = f"repos/{repository}/actions/runs/{run_id}"
    if kind == "run":
        return json.loads(subprocess.check_output(["gh", "api", endpoint], text=True))
    pages = json.loads(subprocess.check_output(["gh", "api", "--paginate", "--slurp",
                                              f"{endpoint}/{kind}?per_page=100"], text=True))
    records = [record for page in pages for record in page[kind]]
    if not pages or any(page["total_count"] != len(records) for page in pages):
        raise ValueError("GitHub inventory changed or is incomplete")
    payload = {"total_count": len(records), kind: records}
    if kind == "artifacts" and latest:
        duplicate = len({record.get("name") for record in records}) != len(records)
        run = run_info or (inventory(repository, run_id, "run") if duplicate else {})
        return latest_artifacts(payload, run.get("run_attempt", 1))
    # GitHub's latest job inventory carries retained successful jobs forward
    # after a failed-job rerun. Do not union stale successes from filter=all.
    return payload


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY"), required=not os.environ.get("GITHUB_REPOSITORY"))
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--kind", required=True, choices=("run", "jobs", "artifacts"))
    args = parser.parse_args()
    print(json.dumps(inventory(args.repository, args.run_id, args.kind)))


if __name__ == "__main__":
    main()
