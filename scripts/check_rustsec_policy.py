#!/usr/bin/env python3
import argparse
import datetime
import json
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_POLICY = ROOT / "config/rustsec-exceptions.json"
ENFORCED_WARNINGS = {"notice", "unsound"}
REQUIRED_FIELDS = {
    "advisory",
    "aliases",
    "package",
    "version",
    "kind",
    "owner",
    "expires",
    "affected_targets",
    "unaffected_targets",
    "decision",
}


def audit_findings(report):
    findings = []
    for item in report.get("vulnerabilities", {}).get("list", []):
        findings.append(("vulnerability", item))
    for kind, items in report.get("warnings", {}).items():
        if kind in ENFORCED_WARNINGS:
            findings.extend((kind, item) for item in items)
    return findings


def evaluate(report, policy, today, root=ROOT):
    errors = []
    exceptions = {}
    for entry in policy.get("exceptions", []):
        missing = sorted(REQUIRED_FIELDS - entry.keys())
        advisory = entry.get("advisory", "<missing>")
        if missing:
            errors.append(f"{advisory}: missing fields: {', '.join(missing)}")
            continue
        if advisory in exceptions:
            errors.append(f"{advisory}: duplicate exception")
            continue
        try:
            expiry = datetime.date.fromisoformat(entry["expires"])
        except (TypeError, ValueError):
            errors.append(f"{advisory}: expires must be an ISO date")
            continue
        decision = root / entry["decision"]
        if not entry["owner"] or not decision.is_file():
            errors.append(f"{advisory}: owner and decision must resolve")
            continue
        if expiry < today:
            errors.append(f"{advisory}: exception expired on {expiry.isoformat()}")
            continue
        exceptions[advisory] = entry

    used = set()
    accepted = []
    for kind, finding in audit_findings(report):
        advisory = finding["advisory"]
        package = finding["package"]
        advisory_id = advisory["id"]
        entry = exceptions.get(advisory_id)
        if entry is None:
            errors.append(f"{advisory_id}: {package['name']} {package['version']} is not accepted")
            continue
        mismatches = []
        for field, actual in (
            ("kind", kind),
            ("package", package["name"]),
            ("version", package["version"]),
        ):
            if entry[field] != actual:
                mismatches.append(f"{field}={actual}")
        missing_aliases = sorted(set(entry["aliases"]) - set(advisory.get("aliases", [])))
        if missing_aliases:
            mismatches.append("aliases=" + ",".join(advisory.get("aliases", [])))
        if mismatches:
            errors.append(f"{advisory_id}: exception mismatch ({'; '.join(mismatches)})")
            continue
        used.add(advisory_id)
        accepted.append(entry)
        if advisory_id == "RUSTSEC-2024-0429":
            variant = root / "vendor/glib/src/variant_iter.rs"
            try:
                patched = "&mut p" in variant.read_text(encoding="utf-8")
            except OSError:
                patched = False
            if not patched:
                errors.append(
                    f"{advisory_id}: vendor/glib lost the VariantStrIter mutability patch"
                )

    for advisory_id in sorted(exceptions.keys() - used):
        errors.append(f"{advisory_id}: exception is stale; the advisory was not detected")
    return errors, accepted


def load_json(path):
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def run_audit():
    try:
        result = subprocess.run(
            ["cargo-audit", "audit", "--json"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        raise RuntimeError(f"could not run cargo-audit: {error}") from error
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        detail = result.stderr.strip() or "cargo-audit produced no JSON"
        raise RuntimeError(detail) from error


def glib_vendor_errors(root):
    variant = root / "vendor/glib/src/variant_iter.rs"
    if not variant.is_file():
        return [
            "vendor/glib is missing; restore the RUSTSEC-2024-0429 VariantStrIter patch"
        ]
    try:
        text = variant.read_text(encoding="utf-8")
    except OSError:
        return ["vendor/glib/src/variant_iter.rs could not be read"]
    if "&mut p" not in text:
        return [
            "vendor/glib lost the VariantStrIter mutability patch (RUSTSEC-2024-0429)"
        ]
    return []


def target_errors(entries):
    errors = []
    cargo = os.environ.get("CARGO", "cargo")
    for entry in entries:
        package = f"{entry['package']}@{entry['version']}"
        expectations = [
            (target, True) for target in entry["affected_targets"]
        ] + [
            (target, False) for target in entry["unaffected_targets"]
        ]
        for target, expected in expectations:
            result = subprocess.run(
                [cargo, "tree", "-i", package, "--target", target, "--locked"],
                cwd=ROOT,
                capture_output=True,
                text=True,
                check=False,
            )
            if result.returncode:
                errors.append(f"{entry['advisory']}: cargo tree failed for {target}")
                continue
            present = bool(result.stdout.strip())
            if present != expected:
                state = "present" if present else "absent"
                errors.append(f"{entry['advisory']}: {package} is unexpectedly {state} for {target}")
    return errors


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--policy", type=pathlib.Path, default=DEFAULT_POLICY)
    parser.add_argument("--report", type=pathlib.Path)
    args = parser.parse_args()
    try:
        policy = load_json(args.policy)
        report = load_json(args.report) if args.report else run_audit()
        errors, accepted = evaluate(report, policy, datetime.datetime.now(datetime.UTC).date())
        if not errors:
            errors.extend(target_errors(accepted))
        errors.extend(glib_vendor_errors(ROOT))
    except (OSError, ValueError, RuntimeError) as error:
        print(f"rustsec-policy: {error}", file=sys.stderr)
        return 1
    for entry in accepted:
        aliases = ", ".join(entry["aliases"])
        print(
            f"rustsec-policy: accepted {entry['advisory']} ({aliases}) for "
            f"{entry['package']} {entry['version']} until {entry['expires']} by {entry['owner']}"
        )
    for error in errors:
        print(f"rustsec-policy: {error}", file=sys.stderr)
    return bool(errors)


if __name__ == "__main__":
    sys.exit(main())
