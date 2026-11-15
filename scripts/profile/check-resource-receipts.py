#!/usr/bin/env python3
"""Check native resource receipts after their release provenance is verified.

This offline gate never substitutes functional tests or an incomplete budget
for native qualification. Receipts contain simultaneous process-family samples.
"""
import argparse
import json
import math
from pathlib import Path
import re


def number(value):
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def check(contract, receipts, commit):
    failures, missing = [], []
    seen, artifacts = set(), {}
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("expected commit must be a full lowercase Git SHA")
    if contract.get("schema_version") != 1:
        raise ValueError("unsupported resource contract")
    for receipt in receipts:
        platform, scenario = receipt.get("platform"), receipt.get("scenario")
        key = (platform, scenario)
        label = f"{platform}/{scenario}"
        if platform not in contract["platforms"] or scenario not in contract["required_scenarios"]:
            failures.append(f"{label}: unknown platform/scenario")
            continue
        if key in seen:
            failures.append(f"{label}: duplicate scenario receipt")
        seen.add(key)
        budget = contract["platforms"][platform]
        if (receipt.get("commit") != commit or receipt.get("dirty") is not False
                or receipt.get("build_mode") != "release"
                or receipt.get("budget_version") != contract["budget_version"]):
            failures.append(f"{label}: commit/build/budget identity mismatch")
        artifact = receipt.get("artifact_sha256", "")
        if not isinstance(artifact, str) or not re.fullmatch(r"[0-9a-f]{64}", artifact):
            failures.append(f"{label}: missing artifact digest")
        elif platform in artifacts and artifacts[platform] != artifact:
            failures.append(f"{label}: scenarios used different artifacts")
        else:
            artifacts[platform] = artifact
        for field in ("device", "os_version", "fixture_sha256", "signing_identity", "provenance_receipt"):
            if not isinstance(receipt.get(field), str) or not receipt[field].strip():
                missing.append(f"{label}: missing {field}")
        if receipt.get("controlled") is not True or receipt.get("process_coverage_complete") is not True:
            missing.append(f"{label}: uncontrolled workload or incomplete process coverage")
        if platform == "android" and receipt.get("physical_device") is not True:
            missing.append(f"{label}: physical Android evidence required")
        if receipt.get("memory_metric") != budget["memory_metric"]:
            failures.append(f"{label}: wrong memory metric")
        samples = receipt.get("samples")
        if not isinstance(samples, list) or len(samples) < contract["minimum_samples"]:
            missing.append(f"{label}: insufficient samples")
            continue
        previous = -1
        totals = []
        for sample in samples:
            elapsed = sample.get("elapsed_seconds")
            processes = sample.get("processes")
            if not number(elapsed) or elapsed <= previous:
                failures.append(f"{label}: non-monotonic sample time")
                break
            previous = elapsed
            if not isinstance(processes, list) or not processes:
                failures.append(f"{label}: empty process-family sample")
                break
            identities = set()
            total = 0
            for process in processes:
                identity = process.get("identity")
                value = process.get("memory_bytes")
                if (not isinstance(identity, str) or not identity or identity in identities
                        or not number(value)):
                    failures.append(f"{label}: invalid/duplicate process identity or memory")
                    break
                identities.add(identity)
                total += value
            else:
                totals.append(total)
                continue
            break
        limit_key = "ocr_ai_peak_limit_bytes" if scenario == "ocr_ai_peak" else "ordinary_memory_limit_bytes"
        limit = budget.get(limit_key)
        if not number(limit) or limit == 0:
            missing.append(f"{label}: unresolved {limit_key}")
        elif totals and max(totals) >= limit:
            failures.append(f"{label}: combined memory {max(totals)} is not below {limit}")
        metrics = receipt.get("metrics", {})
        if not isinstance(metrics, dict):
            failures.append(f"{label}: invalid metrics")
            continue
        required = ["cpu_percent", "wakeups_per_second", "disk_bytes_per_second", "network_bytes_per_second"]
        if scenario == "recovery":
            required.append("recovery_seconds")
        if scenario == "sustained":
            required.append("sustained_growth_bytes")
        for metric in required:
            threshold, value = budget.get(metric + "_limit"), metrics.get(metric)
            if not number(threshold):
                missing.append(f"{label}: unresolved {metric} budget")
            if not number(value):
                missing.append(f"{label}: missing {metric} measurement")
            elif number(threshold) and value > threshold:
                failures.append(f"{label}: {metric} exceeds budget")
    for platform in contract["platforms"]:
        for scenario in contract["required_scenarios"]:
            if (platform, scenario) not in seen:
                missing.append(f"{platform}/{scenario}: missing native receipt")
    return {"status": "failed" if failures else "incomplete" if missing else "passed",
            "failures": failures, "missing": missing}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--contract", type=Path, default=Path(__file__).resolve().parents[2] / "config/runtime-resource-budgets.json")
    parser.add_argument("--commit", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("receipts", nargs="*", type=Path)
    args = parser.parse_args()
    try:
        result = check(json.loads(args.contract.read_text()),
                       [json.loads(path.read_text()) for path in args.receipts], args.commit)
    except (ValueError, KeyError, TypeError, AttributeError, OSError) as error:
        result = {"status": "failed", "failures": [f"Invalid input: {error}"], "missing": []}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(result["status"])
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
