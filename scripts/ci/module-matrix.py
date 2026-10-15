#!/usr/bin/env python3
"""Select existing module targets without weakening standalone publication coverage."""

import argparse
import json
import os
from pathlib import Path


def matrix(contract, kind, linux_only, publish):
    if contract.get("schema") != 1:
        raise ValueError("unsupported module platform schema")
    if linux_only and publish:
        raise ValueError("Linux qualification producers cannot publish incomplete module matrices")
    rows = contract["desktop"][kind]
    identities = [(row["platform"], row["architecture"]) for row in rows]
    if len(set(identities)) != len(rows):
        raise ValueError("duplicate platform target")
    selected = [row for row in rows if not linux_only or row["platform"] == "linux"]
    if not selected:
        raise ValueError("required module platform matrix is empty")
    return selected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kind", required=True, choices=("provider", "ocr"))
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    rows = matrix(json.loads((root / "config/module-platforms.json").read_text()), args.kind,
                  os.environ.get("LINUX_ONLY") == "true", os.environ.get("REQUESTED_PUBLISH") == "true")
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        output.write("desktop_matrix=" + json.dumps(rows, separators=(",", ":")) + "\n")


if __name__ == "__main__":
    main()
