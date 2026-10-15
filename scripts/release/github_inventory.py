"""Read complete GitHub run inventories without treating API errors as missing data."""

import json
import re
import subprocess


def inventory(repository, run_id, kind):
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
    return {"total_count": len(records), kind: records}
