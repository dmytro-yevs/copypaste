#!/usr/bin/env python3
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
LEDGER = ROOT / "docs/feature-ledger.json"
HANDLER = ROOT / "crates/copypaste-ui/src-tauri/src/lib.rs"
FORBIDDEN = re.compile(r"\b(?:todo|tbd|waiv(?:e|ed|er)|placeholder)\b", re.I)
CLOUD_STATES = {"unconfigured", "signed-out", "signed-in", "sync-with-skips", "offline-error", "signed-out-again"}
CLOUD_RELEASE = {"release-android-cloud-evidence", "release-macos-cloud-evidence"}


def fail(message):
    print(f"feature-ledger: {message}", file=sys.stderr)
    return 1


def cloud_errors(feature):
    errors = []
    for platform in ("android", "macos"):
        scenario = feature.get("native", {}).get(platform, {})
        states = set(scenario.get("evidence_states", []))
        if states != CLOUD_STATES:
            errors.append(f"cloud-account: {platform} evidence_states must be {sorted(CLOUD_STATES)}")
        if "cloud-evidence.sh" not in scenario.get("scenario", ""):
            errors.append(f"cloud-account: {platform} must use a dedicated cloud evidence scenario")
        script = ROOT / scenario.get("scenario", "").split()[0].removeprefix("./")
        if not script.is_file():
            errors.append(f"cloud-account: {platform} scenario does not exist")
    if set(feature.get("release_evidence", [])) != CLOUD_RELEASE:
        errors.append(f"cloud-account: release_evidence must be {sorted(CLOUD_RELEASE)}")
    return errors


def self_test():
    feature = {
        "native": {
            "android": {"scenario": "./scripts/release/android-cloud-evidence.sh", "evidence_states": list(CLOUD_STATES)},
            "macos": {"scenario": "./scripts/release/macos-cloud-evidence.sh", "evidence_states": list(CLOUD_STATES)},
        },
        "release_evidence": list(CLOUD_RELEASE),
    }
    checks = [("complete native cloud evidence passes", not cloud_errors(feature))]
    feature["native"]["android"]["evidence_states"].remove("offline-error")
    checks.append(("a missing native cloud state fails", bool(cloud_errors(feature))))
    for description, held in checks:
        print(f"{'PASS' if held else 'FAIL'}|self-test: {description}|")
    return 0 if all(held for _, held in checks) else 1


def main():
    if "--self-test" in sys.argv:
        return self_test()
    raw = LEDGER.read_text(encoding="utf-8")
    if FORBIDDEN.search(raw):
        return fail("completion records may not contain TODOs, waivers, or placeholders")
    try:
        document = json.loads(raw)
    except json.JSONDecodeError as error:
        return fail(str(error))
    if document.get("schema_version") != 1:
        return fail("unsupported schema_version")

    source = HANDLER.read_text(encoding="utf-8")
    block = source.split("tauri::generate_handler![", 1)[1].split("]", 1)[0]
    shipped = set(re.findall(r"commands::[a-z_]+::([a-z_]+)", block))
    classified = []
    errors = []
    required = {"backend_tests", "ui_tests", "accessibility_states", "failure_states", "performance", "native", "release_evidence"}
    for feature in document.get("features", []):
        feature_id = feature.get("id", "<missing id>")
        missing = sorted(required - feature.keys())
        if missing:
            errors.append(f"{feature_id}: missing {', '.join(missing)}")
        if feature.get("status") not in {"product", "removed"}:
            errors.append(f"{feature_id}: status must be product or removed")
        classified.extend(feature.get("contracts", []))
        if feature.get("status") == "product":
            release_evidence = feature.get("release_evidence", [])
            if "release-android-hardware-evidence" not in release_evidence:
                errors.append(f"{feature_id}: release evidence missing physical Android smoke")
            for platform in ("android", "macos"):
                scenario = feature.get("native", {}).get(platform, {})
                for field in ("scenario", "screenshot", "ax_log"):
                    if not scenario.get(field):
                        errors.append(f"{feature_id}: {platform} missing {field}")
            perf = feature.get("performance", {})
            if not isinstance(perf.get("p95_ms"), int) or perf.get("p95_ms", 0) <= 0 or not perf.get("measurement"):
                errors.append(f"{feature_id}: performance needs a positive p95_ms and measurement")
            for state in ("restart", "offline"):
                if state not in feature.get("failure_states", []):
                    errors.append(f"{feature_id}: failure_states missing {state}")
            if feature_id == "cloud-account":
                errors.extend(cloud_errors(feature))

    duplicates = sorted({name for name in classified if classified.count(name) > 1})
    missing = sorted(shipped - set(classified))
    unknown = sorted(set(classified) - shipped)
    if duplicates:
        errors.append("contracts classified more than once: " + ", ".join(duplicates))
    if missing:
        errors.append("unclassified Tauri commands: " + ", ".join(missing))
    if unknown:
        errors.append("ledger contracts not shipped: " + ", ".join(unknown))
    if errors:
        return fail("\nfeature-ledger: ".join(errors))
    print(f"feature-ledger: {len(document['features'])} features, {len(shipped)} Tauri commands classified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
