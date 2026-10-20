#!/usr/bin/env python3
"""Conservative component selection. Unknown paths and invalid Git ranges require full CI."""

import argparse
import json
import os
from pathlib import Path
import subprocess


def select(paths):
    selected = {"native": False}
    for path in paths:
        if not path or path.startswith("/") or ".." in Path(path).parts:
            return {"native": True}
        if path in ("README.md", "LICENSE", "LICENSE-MIT", "LICENSE-APACHE"):
            continue
        selected["native"] = True
    return selected


def changed_paths(base, head):
    if not base or set(base) == {"0"}:
        raise ValueError("missing base revision")
    # --end-of-options prevents refs from being interpreted as command options.
    base_sha = subprocess.check_output(["git", "rev-parse", "--verify", "--end-of-options", base + "^{commit}"], text=True).strip()
    head_sha = subprocess.check_output(["git", "rev-parse", "--verify", "--end-of-options", head + "^{commit}"], text=True).strip()
    return subprocess.check_output(["git", "diff", "--name-only", "-z", base_sha, head_sha]).decode("utf-8").split("\0")[:-1]


def verify_results(needs, native, trusted_pr, is_pr=False):
    if needs.get("changes", {}).get("result") != "success":
        raise ValueError("change detection did not pass")
    for key, result in needs.items():
        if key == "changes":
            continue
        expected = "success" if native else "skipped"
        if key in ("compositor-runtime-metadata", "compositor-runtime-producer") and not trusted_pr:
            expected = "skipped"
        if key == "linux-pr-release-candidate" and not is_pr:
            expected = "skipped"
        if result.get("result") != expected:
            raise ValueError(f"{key}: expected {expected}, got {result.get('result')}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    if args.verify:
        verify_results(json.loads(os.environ["NEEDS"]), os.environ["NATIVE"] == "true",
                       os.environ["TRUSTED_PR"] == "true", os.environ["IS_PR"] == "true")
        print("All selected CI jobs passed")
        return
    try:
        paths = changed_paths(os.environ.get("BASE_SHA", ""), os.environ.get("HEAD_SHA", "HEAD"))
        result = select(paths)
    except (ValueError, UnicodeError, subprocess.CalledProcessError) as error:
        print(f"Full validation required: {error}")
        result = {"native": True}
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        for name, enabled in result.items():
            output.write(f"{name}={str(enabled).lower()}\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
