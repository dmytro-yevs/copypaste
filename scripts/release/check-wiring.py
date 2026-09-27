#!/usr/bin/env python3
"""Check the few CI contracts that catch product regressions cheaply."""

import copy
import pathlib
import sys

import yaml

from ci_contract import ci_rust_toolchain_holds, critical_pr_errors, commands, job_steps


ROOT = pathlib.Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github" / "workflows"


def load(name):
    with (WORKFLOWS / name).open(encoding="utf-8") as stream:
        return yaml.safe_load(stream) or {}


def critical_smoke_step(job):
    for step in job_steps(job):
        if "android-emulator-runner" not in str(step.get("uses") or ""):
            continue
        if "android-emulator-legs.sh" not in str((step.get("with") or {}).get("script") or ""):
            continue
        profile = (step.get("env") or {}).get("COPYPASTE_SMOKE_PROFILE")
        if profile == "critical" or profile == "${{ inputs.mode == 'full' && 'full' || 'critical' }}":
            return step
    return None


def android_smoke_errors(workflow):
    jobs = workflow.get("jobs") or {}
    build_jobs = [job for job in jobs.values() if "npm run tauri -- android build --debug --apk" in commands(job)]
    smoke_jobs = [job for job in jobs.values() if critical_smoke_step(job) is not None]
    errors = []
    if len(build_jobs) != 1:
        errors.append("Android CI must build one debug APK")
    if len(smoke_jobs) != 1:
        errors.append("Android CI must run one critical emulator smoke")
    return errors


def security_errors(supply_chain):
    source = str(supply_chain)
    if "check-secret-scan.sh --scan" not in source or "gitleaks" not in source:
        return ["CI must retain secret scanning in the active security job"]
    if "cargo deny" not in source or "check_rustsec_policy.py" not in source:
        return ["CI must retain Rust dependency policy checks"]
    return []


def report(errors):
    for error in errors:
        print(f"FAIL|{error}|")
    if not errors:
        print("PASS|critical CI wiring is present and fail closed|")


def self_test(ci, android, supply_chain, rust_version):
    fixtures = []

    broken = copy.deepcopy(ci)
    for step in broken["jobs"]["rust"]["steps"]:
        if "cargo +1.96 test --workspace --locked" in str(step.get("run") or ""):
            step["run"] = "true"
    fixtures.append(("missing core Rust test fails", bool(critical_pr_errors(broken))))

    broken = copy.deepcopy(ci)
    for step in broken["jobs"]["windows-native"]["steps"]:
        if "npx vitest run" in str(step.get("run") or ""):
            step["run"] = "npm ci"
    fixtures.append(("missing Windows product UI test fails", bool(critical_pr_errors(broken))))

    broken = copy.deepcopy(android)
    for job in (broken.get("jobs") or {}).values():
        step = critical_smoke_step(job)
        if step is not None:
            step.setdefault("env", {})["COPYPASTE_SMOKE_PROFILE"] = "full"
    fixtures.append(("noncritical Android smoke fails", bool(android_smoke_errors(broken))))

    broken = copy.deepcopy(supply_chain)
    broken["jobs"] = {}
    fixtures.append(("missing secret scan fails", bool(security_errors(broken))))

    stale = copy.deepcopy(ci)
    for job in (stale.get("jobs") or {}).values():
        for step in job_steps(job):
            if "cargo +1.96" in str(step.get("run") or ""):
                step["run"] = str(step["run"]).replace("cargo +1.96", "cargo +0.0")
    held, _ = ci_rust_toolchain_holds(stale, rust_version)
    fixtures.append(("stale Rust toolchain fails", not held))

    failures = 0
    for label, held in fixtures:
        print(f"{'PASS' if held else 'FAIL'}|self-test: {label}|")
        failures += not held
    return failures


def main():
    ci = load("ci.yml")
    android = load("android-emulator.yml")
    supply_chain = load("supply-chain.yml")
    rust_version = "1.96"
    errors = critical_pr_errors(ci) + android_smoke_errors(android) + security_errors(supply_chain)
    toolchain_holds, details = ci_rust_toolchain_holds(ci, rust_version)
    if not toolchain_holds:
        errors.extend(f"CI toolchain mismatch: {detail}" for detail in details)
    report(errors)
    if "--self-test" in sys.argv:
        errors.extend("self-test failure" for _ in range(self_test(ci, android, supply_chain, rust_version)))
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
