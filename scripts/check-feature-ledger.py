#!/usr/bin/env python3
import copy
import json
import pathlib
import re
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
LEDGER = ROOT / "docs/feature-ledger.json"
HANDLER = ROOT / "crates/copypaste-ui/src-tauri/src/lib.rs"
FORBIDDEN = re.compile(r"\b(?:todo|tbd|waiv(?:e|ed|er)|placeholder)\b", re.I)
CLOUD_STATES = {"unconfigured", "signed-out", "signed-in", "sync-with-skips", "offline-error", "signed-out-again"}
CLOUD_RELEASE = {
    "release-android-cloud-evidence",
    "release-android-hardware-evidence",
    "release-macos-cloud-evidence",
    "release-macos-native-evidence",
}
PERFORMANCE_PLATFORMS = {"android", "macos"}
SCENARIO_SUFFIXES = {".py", ".ps1", ".sh"}


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


def repo_file(root, value):
    if not isinstance(value, str) or not value:
        raise ValueError("evidence path is missing")
    relative = pathlib.PurePosixPath(value)
    if relative.is_absolute() or ".." in relative.parts or "\\" in value:
        raise ValueError("evidence path must stay inside the repository")
    try:
        file = (root / pathlib.Path(*relative.parts)).resolve(strict=True)
        file.relative_to(root.resolve(strict=True))
    except (OSError, ValueError):
        raise ValueError(f"evidence file does not exist: {value}") from None
    if not file.is_file():
        raise ValueError(f"evidence path is not a file: {value}")
    return file, relative


def artifact_matches(document, platform, scenario, p95_ms):
    if document.get("platform") != platform:
        return False, "measurement artifact names the wrong platform"
    records = document.get("measurements")
    if not isinstance(records, list):
        records = [document]
    for record in records:
        if not isinstance(record, dict):
            continue
        name = record.get("scenario")
        if isinstance(name, dict):
            name = name.get("name")
        value = record.get("p95_ms")
        if value is None and isinstance(record.get("scenario"), dict):
            value = record["scenario"].get("p95_ms")
        samples = record.get("samples_ms")
        measured = isinstance(samples, list) and samples and all(
            not isinstance(sample, bool) and isinstance(sample, (int, float)) and sample >= 0
            for sample in samples
        )
        if name == scenario and value == p95_ms and measured:
            return True, ""
    return False, "measurement artifact has no matching measured p95 samples"


def artifact_errors(root, platform, credit, evidence):
    try:
        file, _ = repo_file(root, evidence.get("path"))
    except ValueError as error:
        return [str(error)]
    if file.suffix.lower() != ".json":
        return ["measurement artifact must be JSON, not prose"]
    try:
        document = json.loads(file.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return ["measurement artifact is not readable JSON"]
    if not isinstance(document, dict):
        return ["measurement artifact must be a JSON object"]
    held, message = artifact_matches(document, platform, credit["scenario"], credit["p95_ms"])
    return [] if held else [message]


def scenario_errors(root, platform, credit, evidence):
    errors = []
    try:
        producer, producer_path = repo_file(root, evidence.get("path"))
    except ValueError as error:
        return [str(error)]
    if producer.suffix.lower() not in SCENARIO_SUFFIXES:
        errors.append("reproducible evidence must be an executable scenario, not prose")
        return errors
    source = producer.read_text(encoding="utf-8")
    scenario = re.escape(credit["scenario"])
    p95_ms = credit["p95_ms"]
    if not any(re.search(rf"\b{scenario}\b.*\b{p95_ms}\b", line) for line in source.splitlines()):
        errors.append("scenario no longer records the credited p95")
    platform_patterns = (
        rf"--platform\s+[\"']?{platform}\b",
        rf"cloud_latency_write[^\n]*\b{platform}\b",
    )
    if not any(re.search(pattern, source) for pattern in platform_patterns):
        errors.append("scenario records evidence for the wrong platform")

    execution = evidence.get("executed_by")
    if not isinstance(execution, list) or not execution:
        errors.append("scenario has no release execution chain")
        return errors
    invoked = producer_path
    final = None
    for value in execution:
        try:
            wiring, wiring_path = repo_file(root, value)
        except ValueError as error:
            errors.append(str(error))
            return errors
        body = wiring.read_text(encoding="utf-8")
        target = invoked.as_posix()
        if target not in body and f"./{target}" not in body:
            errors.append(f"{value} does not execute {target}")
            return errors
        invoked = wiring_path
        final = wiring_path
    if final is None or final.suffix.lower() not in {".yml", ".yaml"} or final.parts[:2] != (".github", "workflows"):
        errors.append("scenario execution chain does not end in a workflow")
    return errors


def performance_errors(feature, root=ROOT):
    feature_id = feature.get("id", "<missing id>")
    performance = feature.get("performance")
    if not isinstance(performance, dict) or set(performance) != PERFORMANCE_PLATFORMS:
        return [f"{feature_id}: performance must distinguish android and macos"]
    errors = []
    for platform in sorted(PERFORMANCE_PLATFORMS):
        credit = performance[platform]
        label = f"{feature_id}: {platform} performance"
        if credit == {"status": "uncredited"}:
            continue
        if not isinstance(credit, dict) or credit.get("status") != "credited":
            errors.append(f"{label} status must be credited or uncredited")
            continue
        if set(credit) != {"status", "scenario", "p95_ms", "evidence"}:
            errors.append(f"{label} credited fields are incomplete or unknown")
            continue
        p95_ms = credit.get("p95_ms")
        if isinstance(p95_ms, bool) or not isinstance(p95_ms, int) or p95_ms <= 0:
            errors.append(f"{label} p95_ms must be a positive integer")
            continue
        if not isinstance(credit.get("scenario"), str) or not credit["scenario"]:
            errors.append(f"{label} scenario is missing")
            continue
        evidence = credit.get("evidence")
        if not isinstance(evidence, dict):
            errors.append(f"{label} evidence is missing")
            continue
        kind = evidence.get("kind")
        expected = {"kind", "path"} if kind == "artifact" else {"kind", "path", "executed_by"}
        if kind not in {"artifact", "scenario"} or set(evidence) != expected:
            errors.append(f"{label} evidence must be an artifact or executed scenario")
            continue
        validator = artifact_errors if kind == "artifact" else scenario_errors
        errors.extend(f"{label}: {message}" for message in validator(root, platform, credit, evidence))
    return errors


def self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory)
        (root / "scripts").mkdir()
        (root / ".github/workflows").mkdir(parents=True)
        for platform in PERFORMANCE_PLATFORMS:
            (root / f"scripts/{platform}.sh").write_text(
                f'cloud_latency_record "$OUT" ready "$elapsed" 42\n'
                f'cloud_latency_write "$OUT" "$OUT/latency.json" {platform}\n',
                encoding="utf-8",
            )
            (root / f".github/workflows/{platform}.yml").write_text(
                f"run: ./scripts/{platform}.sh\n", encoding="utf-8"
            )
        feature = {
            "id": "fixture",
            "performance": {
                platform: {
                    "status": "credited",
                    "scenario": "ready",
                    "p95_ms": 42,
                    "evidence": {
                        "kind": "scenario",
                        "path": f"scripts/{platform}.sh",
                        "executed_by": [f".github/workflows/{platform}.yml"],
                    },
                }
                for platform in PERFORMANCE_PLATFORMS
            },
        }
        def rejects(probe, message):
            return any(message in error for error in performance_errors(probe, root))

        checks = [("valid reproducible evidence passes", not performance_errors(feature, root))]
        probe = copy.deepcopy(feature)
        probe["performance"]["android"]["evidence"]["path"] = "scripts/missing.sh"
        checks.append(("a missing evidence file fails", rejects(probe, "does not exist")))
        probe = copy.deepcopy(feature)
        probe["performance"]["android"]["scenario"] = "stale"
        checks.append(("a stale scenario fails", rejects(probe, "no longer records")))
        probe = copy.deepcopy(feature)
        probe["performance"]["android"]["evidence"]["path"] = "scripts/macos.sh"
        checks.append(("evidence from the wrong platform fails", rejects(probe, "wrong platform")))
        probe = copy.deepcopy(feature)
        probe["performance"]["android"]["p95_ms"] = 0
        checks.append(("a nonpositive p95 fails", rejects(probe, "positive integer")))
        (root / ".github/workflows/android.yml").write_text("run: echo configured\n", encoding="utf-8")
        checks.append(("an unexecuted scenario fails", rejects(feature, "does not execute")))
        for platform in PERFORMANCE_PLATFORMS:
            artifact = {"platform": platform, "scenario": "ready", "p95_ms": 42, "samples_ms": [40, 42]}
            (root / f"{platform}.json").write_text(json.dumps(artifact), encoding="utf-8")
            feature["performance"][platform]["evidence"] = {
                "kind": "artifact", "path": f"{platform}.json"
            }
        checks.append(("valid measurement artifacts pass", not performance_errors(feature, root)))
        (root / "android.json").write_text(
            json.dumps({"platform": "android", "scenario": "ready", "p95_ms": 42}),
            encoding="utf-8",
        )
        checks.append(("configured-only artifacts fail", rejects(feature, "p95 samples")))
        (root / "claim.md").write_text("ready p95 42 on android", encoding="utf-8")
        feature["performance"]["android"]["evidence"] = {"kind": "artifact", "path": "claim.md"}
        checks.append(("prose-only evidence fails", rejects(feature, "not prose")))

    cloud = {
        "native": {
            "android": {"scenario": "./scripts/release/android-cloud-evidence.sh", "evidence_states": list(CLOUD_STATES)},
            "macos": {"scenario": "./scripts/release/macos-cloud-evidence.sh", "evidence_states": list(CLOUD_STATES)},
        },
        "release_evidence": list(CLOUD_RELEASE),
    }
    checks.append(("complete native cloud evidence passes", not cloud_errors(cloud)))
    cloud["native"]["android"]["evidence_states"].remove("offline-error")
    checks.append(("a missing native cloud state fails", bool(cloud_errors(cloud))))
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
            for state in ("restart", "offline"):
                if state not in feature.get("failure_states", []):
                    errors.append(f"{feature_id}: failure_states missing {state}")
            if feature_id == "cloud-account":
                errors.extend(cloud_errors(feature))
            errors.extend(performance_errors(feature))

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
