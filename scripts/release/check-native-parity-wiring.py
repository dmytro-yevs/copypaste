#!/usr/bin/env python3
"""Verify that native release evidence reaches the final artifact gate."""

import pathlib
import sys

import yaml

import native_evidence_wiring


ROOT = pathlib.Path(__file__).resolve().parents[2]


def load_release():
    path = ROOT / ".github" / "workflows" / "release.yml"
    with path.open(encoding="utf-8") as stream:
        return yaml.safe_load(stream) or {}


def main():
    release = load_release()
    errors = native_evidence_wiring.contract_errors(release)
    for error in errors:
        print(f"FAIL|{error}|")
    if not errors:
        print("PASS|three native receipts are bound to signed release artifacts|")
    if "--self-test" in sys.argv:
        errors.extend("self-test failure" for _ in range(native_evidence_wiring.self_test(release)))
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
