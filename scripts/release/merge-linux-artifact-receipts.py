#!/usr/bin/env python3
"""Merge per-architecture Linux receipts into one release qualification receipt."""

import argparse
import json
from pathlib import Path


IDENTITY_KEYS = ("platform", "version", "commit", "run_id")


def merge(receipts: list[Path]) -> dict:
    if len(receipts) != 2:
        raise ValueError("Linux qualification requires exactly two architecture receipts")
    payloads = [json.loads(path.read_text(encoding="utf-8")) for path in receipts]
    first = payloads[0]
    if first.get("schema") != 2 or first.get("platform") != "linux":
        raise ValueError("invalid Linux receipt identity")
    for payload in payloads[1:]:
        if payload.get("schema") != 2 or any(
            payload.get(key) != first.get(key) for key in IDENTITY_KEYS
        ):
            raise ValueError("Linux receipt identity differs between architectures")
    artifacts = [artifact for payload in payloads for artifact in payload.get("artifacts", [])]
    names = [artifact.get("name") for artifact in artifacts if isinstance(artifact, dict)]
    if len(names) != len(artifacts) or len(set(names)) != len(names):
        raise ValueError("Linux receipt metadata is invalid or ambiguous")
    return {
        "schema": 2,
        **{key: first[key] for key in IDENTITY_KEYS},
        "artifacts": artifacts,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--receipt", action="append", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    merged = merge(args.receipt)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(merged, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
