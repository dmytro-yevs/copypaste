#!/usr/bin/env python3
import copy
import json
import pathlib
import re
import shlex
import sys
import tempfile

from feature_ledger_evidence import (
    STATE,
    native_errors,
    release_gate_errors,
    repo_file,
    string_list_errors,
)

ROOT = pathlib.Path(__file__).resolve().parents[1]
LEDGER = ROOT / "docs/feature-ledger.json"
HANDLER = ROOT / "crates/copypaste-ui/src-tauri/src/lib.rs"
FORBIDDEN = re.compile(r"\b(?:todo|tbd|waiv(?:e|ed|er)|placeholder)\b", re.I)
CLOUD_STATES = {"unconfigured", "signed-out", "signed-in", "sync-with-skips", "offline-error", "signed-out-again"}
CLOUD_RELEASE = {
    "release-android-api33-smoke-evidence",
    "release-android-cloud-evidence",
    "release-android-smoke-evidence",
    "release-macos-cloud-evidence",
    "release-macos-native-evidence",
    "release-windows-native-evidence",
}
CLOUD_UI_TEST = "npm --prefix crates/copypaste-ui run test:cloud"
CLOUD_UI_SCRIPT = "vitest run src/features/settings/patterns/CloudSyncSettings.test.tsx"
CLOUD_UI_TEST_FILE = "crates/copypaste-ui/src/features/settings/patterns/CloudSyncSettings.test.tsx"
SHIPPED_PLATFORMS = {"android", "macos", "windows"}
PERFORMANCE_PLATFORMS = SHIPPED_PLATFORMS
REQUIRED_RELEASE = {
    "release-android-smoke-evidence",
    "release-macos-native-evidence",
    "release-windows-native-evidence",
}
SCENARIO_SUFFIXES = {".py", ".ps1", ".sh"}
TEST_RUNNERS = {"cargo", "npm", "python", "python3", "pwsh", "bash", "./gradlew"}


def fail(message):
    print(f"feature-ledger: {message}", file=sys.stderr)
    return 1


def shipped_commands(source):
    block = source.split("tauri::generate_handler![", 1)[1].split("]", 1)[0]
    return set(re.findall(r"^\s*(?:[a-z_]+::)+([a-z_]+),", block, re.MULTILINE))


def contract_errors(shipped, classified):
    errors = []
    duplicates = sorted({name for name in classified if classified.count(name) > 1})
    missing = sorted(shipped - set(classified))
    unknown = sorted(set(classified) - shipped)
    if duplicates:
        errors.append("contracts classified more than once: " + ", ".join(duplicates))
    if missing:
        errors.append("unclassified Tauri commands: " + ", ".join(missing))
    if unknown:
        errors.append("ledger contracts not shipped: " + ", ".join(unknown))
    return errors


def cloud_errors(feature, root=ROOT):
    errors = []
    for platform in ("android", "macos"):
        scenario = feature.get("native", {}).get(platform, {})
        verified = scenario.get("evidence_states", [])
        unverified = scenario.get("unverified_states", [])
        states = {
            state for state in verified + unverified if isinstance(state, str)
        } if isinstance(verified, list) and isinstance(unverified, list) else set()
        if states != CLOUD_STATES:
            errors.append(f"cloud-account: {platform} evidence coverage must be {sorted(CLOUD_STATES)}")
        if "cloud-evidence.sh" not in scenario.get("scenario", ""):
            errors.append(f"cloud-account: {platform} must use a dedicated cloud evidence scenario")
        script = root / scenario.get("scenario", "").split()[0].removeprefix("./")
        if not script.is_file():
            errors.append(f"cloud-account: {platform} scenario does not exist")
    windows = feature.get("native", {}).get("windows", {})
    verified = windows.get("evidence_states", [])
    unverified = windows.get("unverified_states", [])
    windows_states = {
        state for state in verified + unverified if isinstance(state, str)
    } if isinstance(verified, list) and isinstance(unverified, list) else set()
    if windows_states != CLOUD_STATES:
        errors.append(f"cloud-account: windows evidence coverage must be {sorted(CLOUD_STATES)}")
    if "windows-native-evidence.ps1" not in windows.get("scenario", ""):
        errors.append("cloud-account: windows must use the native release evidence scenario")
    if feature.get("ui_tests") != [CLOUD_UI_TEST]:
        errors.append(f"cloud-account: ui_tests must run {CLOUD_UI_TEST}")
    try:
        package_file, _ = repo_file(root, "crates/copypaste-ui/package.json")
        package = json.loads(package_file.read_text(encoding="utf-8"))
        if package.get("scripts", {}).get("test:cloud") != CLOUD_UI_SCRIPT:
            errors.append("cloud-account: test:cloud must run the focused lifecycle test")
    except (ValueError, OSError, json.JSONDecodeError) as error:
        errors.append(f"cloud-account: focused UI test script is unavailable: {error}")
    try:
        repo_file(root, CLOUD_UI_TEST_FILE)
    except ValueError as error:
        errors.append(f"cloud-account: focused UI test is unavailable: {error}")
    if set(feature.get("release_evidence", [])) != CLOUD_RELEASE:
        errors.append(f"cloud-account: release_evidence must be {sorted(CLOUD_RELEASE)}")
    return errors


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
        return [f"{feature_id}: performance must distinguish android, macos, and windows"]
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


def test_command_errors(value, label, root=ROOT):
    errors = string_list_errors(value, label)
    if errors:
        return errors
    for command in value:
        try:
            argv = shlex.split(command)
        except ValueError:
            errors.append(f"{label} contains an invalid command")
            continue
        if not re.search(r"(?:^|[^a-z])(?:test|check)[a-z0-9:_-]*", command, re.I):
            errors.append(f"{label} command does not run a test or check: {command}")
            continue
        candidates = [argv[0]] if argv else []
        if argv and argv[0] == "cd" and "&&" in argv:
            at = argv.index("&&")
            candidates = argv[at + 1:at + 2]
        if not candidates:
            errors.append(f"{label} command has no executable: {command}")
            continue
        runner = candidates[0]
        if runner in TEST_RUNNERS:
            continue
        if runner.startswith("./"):
            try:
                repo_file(root, runner)
                continue
            except ValueError:
                pass
        errors.append(f"{label} command has no recognized executable: {command}")
    return errors


def feature_shape_errors(feature, root=ROOT):
    feature_id = feature.get("id", "<missing id>")
    errors = []
    if not isinstance(feature_id, str) or not STATE.fullmatch(feature_id):
        errors.append(f"{feature_id}: id must be a lowercase identifier")
    errors.extend(string_list_errors(feature.get("contracts"), f"{feature_id}: contracts", re.compile(r"[a-z][a-z0-9_]*\Z")))
    errors.extend(test_command_errors(feature.get("backend_tests"), f"{feature_id}: backend_tests", root))
    errors.extend(test_command_errors(feature.get("ui_tests"), f"{feature_id}: ui_tests", root))
    errors.extend(string_list_errors(feature.get("accessibility_states"), f"{feature_id}: accessibility_states", STATE))
    errors.extend(string_list_errors(feature.get("failure_states"), f"{feature_id}: failure_states", STATE))
    errors.extend(string_list_errors(feature.get("release_evidence"), f"{feature_id}: release_evidence", STATE))
    return errors


def platform_errors(feature, root=ROOT, require_complete=False):
    feature_id = feature.get("id", "<missing id>")
    errors = []
    values = feature.get("release_evidence", [])
    release_evidence = {value for value in values if isinstance(value, str)} if isinstance(values, list) else set()
    missing_release = sorted(REQUIRED_RELEASE - release_evidence)
    if missing_release:
        errors.append(f"{feature_id}: release evidence missing {', '.join(missing_release)}")
    native, pending = native_errors(feature, root, require_complete)
    errors.extend(native)
    errors.extend(performance_errors(feature, root))
    return errors, pending


def self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory)
        (root / "scripts").mkdir()
        (root / ".github/workflows").mkdir(parents=True)
        for platform in PERFORMANCE_PLATFORMS:
            producer = root / f"scripts/{platform}.sh"
            producer.write_text(
                f'cloud_latency_record "$OUT" ready "$elapsed" 42\n'
                f'cloud_latency_write "$OUT" "$OUT/latency.json" {platform}\n',
                encoding="utf-8",
            )
            producer.chmod(0o755)
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
        "ui_tests": [CLOUD_UI_TEST],
        "native": {
            "android": {"scenario": "./scripts/release/android-cloud-evidence.sh", "evidence_states": list(CLOUD_STATES)},
            "macos": {"scenario": "./scripts/release/macos-cloud-evidence.sh", "evidence_states": list(CLOUD_STATES)},
            "windows": {
                "scenario": "./scripts/release/windows-native-evidence.ps1",
                "evidence_states": ["unconfigured"],
                "unverified_states": sorted(CLOUD_STATES - {"unconfigured"}),
            },
        },
        "release_evidence": list(CLOUD_RELEASE),
    }
    checks.append(("complete native cloud evidence passes", not cloud_errors(cloud)))
    cloud["ui_tests"] = ["npm --prefix crates/copypaste-ui test -- CloudStep"]
    checks.append(("a filtered command that can skip the cloud UI test fails", bool(cloud_errors(cloud))))
    cloud["ui_tests"] = [CLOUD_UI_TEST]
    cloud["native"]["android"]["evidence_states"].remove("offline-error")
    checks.append(("a missing native cloud state fails", bool(cloud_errors(cloud))))
    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory)
        (root / "scripts").mkdir()
        (root / ".github/workflows").mkdir(parents=True)
        artifact_names = {
            "android": "release-android-smoke-evidence",
            "macos": "release-macos-native-evidence",
            "windows": "release-windows-native-evidence",
        }
        jobs = []
        for platform, artifact_name in artifact_names.items():
            producer = root / f"scripts/{platform}.sh"
            producer.write_text(
                'touch "$OUT/screenshot.png"\nprintf "{}" > "$OUT/ax.json"\n',
                encoding="utf-8",
            )
            producer.chmod(0o755)
            jobs.append(
                f"  {platform}:\n"
                f"    steps:\n"
                f"      - run: ./scripts/{platform}.sh\n"
                f"      - uses: actions/upload-artifact@v4\n"
                f"        with:\n"
                f"          name: {artifact_name}\n"
                f"          path: artifacts/{platform}\n"
                f"          if-no-files-found: error\n"
            )
        (root / ".github/workflows/release.yml").write_text(
            "jobs:\n" + "".join(jobs)
            + "  native-parity:\n"
            + "    steps:\n"
            + "      - run: python3 -m pip install --requirement requirements-ci.txt\n"
            + "      - run: python3 scripts/check-feature-ledger.py --require-complete\n",
            encoding="utf-8",
        )
        product = {
            "id": "fixture",
            "native": {
                platform: {
                    "scenario": f"./scripts/{platform}.sh",
                    "evidence_status": "verified",
                    "screenshot": f"artifacts/{platform}/screenshot.png",
                    "ax_log": f"artifacts/{platform}/ax.json",
                    "evidence_states": ["ready"],
                    "release_artifact": artifact_names[platform],
                }
                for platform in SHIPPED_PLATFORMS
            },
            "performance": {platform: {"status": "uncredited"} for platform in SHIPPED_PLATFORMS},
            "release_evidence": list(REQUIRED_RELEASE),
        }

        def platform_rejects(probe, message, complete=False):
            return any(message in error for error in platform_errors(probe, root, complete)[0])

        checks.append(("all three shipped platform records pass", not platform_errors(product, root)[0]))
        probe = copy.deepcopy(product)
        del probe["native"]["windows"]
        checks.append(("a missing Windows platform fails", platform_rejects(probe, "must distinguish")))
        probe = copy.deepcopy(product)
        del probe["native"]["windows"]["evidence_states"]
        checks.append(("missing platform evidence states fail", platform_rejects(probe, "nonempty list")))
        probe = copy.deepcopy(product)
        probe["native"]["windows"]["screenshot"] = "artifacts/generic/screenshot.png"
        checks.append(("an artifact outside its upload fails", platform_rejects(probe, "outside the uploaded")))
        probe = copy.deepcopy(product)
        probe["native"]["windows"]["ax_log"] = "artifacts/windows/invented.json"
        checks.append(("an unproduced artifact path fails", platform_rejects(probe, "not produced")))
        probe = copy.deepcopy(product)
        probe["native"]["windows"]["scenario"] = "manual evidence review"
        checks.append(("prose-only platform evidence fails", platform_rejects(probe, "does not exist")))
        probe = copy.deepcopy(product)
        (root / "scripts/windows.sh").chmod(0o644)
        checks.append(("a non-executable platform scenario fails", platform_rejects(probe, "not executable")))
        (root / "scripts/windows.sh").chmod(0o755)
        probe = copy.deepcopy(product)
        probe["release_evidence"].remove("release-windows-native-evidence")
        checks.append(("missing Windows release evidence fails", platform_rejects(probe, "release-windows-native-evidence")))
        probe = copy.deepcopy(product)
        probe["native"]["macos"] = {
            "scenario": "./scripts/macos.sh",
            "evidence_status": "pending",
            "unverified_states": ["ready"],
        }
        errors, pending = platform_errors(probe, root)
        checks.append(("honest pending evidence is visible", not errors and pending == ["fixture/macos/ready"]))
        checks.append(("release completion rejects pending evidence", platform_rejects(probe, "release evidence is pending", True)))
        probe["native"]["macos"]["screenshot"] = "artifacts/macos/screenshot.png"
        checks.append(("pending evidence cannot cite a fake artifact", platform_rejects(probe, "cannot cite unproduced")))
        probe = copy.deepcopy(product)
        del probe["performance"]["windows"]
        checks.append(("missing Windows performance record fails", platform_rejects(probe, "android, macos, and windows")))
        checks.append(("release requires complete feature evidence", not release_gate_errors(root)))
        workflow = root / ".github/workflows/release.yml"
        workflow.write_text(
            workflow.read_text(encoding="utf-8").replace(" --require-complete", ""),
            encoding="utf-8",
        )
        checks.append(("a prose-only release ledger check fails", bool(release_gate_errors(root))))
    shape = {
        "id": "fixture",
        "contracts": ["status"],
        "backend_tests": ["cargo test -p fixture"],
        "ui_tests": ["npm test"],
        "accessibility_states": ["ready"],
        "failure_states": ["restart", "offline"],
        "release_evidence": ["release-fixture-evidence"],
    }
    checks.append(("required feature fields accept structured values", not feature_shape_errors(shape)))
    probe = copy.deepcopy(shape)
    probe["ui_tests"] = ["manual QA passed"]
    checks.append(("prose cannot stand in for a UI test", bool(feature_shape_errors(probe))))
    probe = copy.deepcopy(shape)
    probe["accessibility_states"] = []
    checks.append(("empty accessibility coverage fails", bool(feature_shape_errors(probe))))
    handler = """tauri::generate_handler![
        commands::history::copy_item,
        updater::update_status,
    ]"""
    shipped = shipped_commands(handler)
    checks.append(("sibling-module Tauri commands are extracted", shipped == {"copy_item", "update_status"}))
    checks.append(("an omitted sibling-module command fails", any("update_status" in error for error in contract_errors(shipped, ["copy_item"]))))
    checks.append(("a misclassified sibling-module command fails", bool(contract_errors(shipped, ["copy_item", "update_state"]))))
    for description, held in checks:
        print(f"{'PASS' if held else 'FAIL'}|self-test: {description}|")
    return 0 if all(held for _, held in checks) else 1


def main():
    if "--self-test" in sys.argv:
        return self_test()
    require_complete = "--require-complete" in sys.argv
    raw = LEDGER.read_text(encoding="utf-8")
    if FORBIDDEN.search(raw):
        return fail("completion records may not contain TODOs, waivers, or placeholders")
    try:
        document = json.loads(raw)
    except json.JSONDecodeError as error:
        return fail(str(error))
    if document.get("schema_version") != 2:
        return fail("unsupported schema_version")

    shipped = shipped_commands(HANDLER.read_text(encoding="utf-8"))
    classified = []
    errors = []
    pending = []
    errors.extend(release_gate_errors(ROOT))
    required = {
        "contracts", "backend_tests", "ui_tests", "accessibility_states", "failure_states",
        "performance", "native", "release_evidence",
    }
    features = document.get("features")
    if not isinstance(features, list) or not features:
        return fail("features must be a nonempty list")
    feature_ids = [feature.get("id") for feature in features if isinstance(feature, dict)]
    valid_feature_ids = [feature_id for feature_id in feature_ids if isinstance(feature_id, str)]
    if len(valid_feature_ids) != len(set(valid_feature_ids)):
        errors.append("feature ids must be unique")
    for feature in features:
        if not isinstance(feature, dict):
            errors.append("feature records must be objects")
            continue
        feature_id = feature.get("id", "<missing id>")
        missing = sorted(required - feature.keys())
        if missing:
            errors.append(f"{feature_id}: missing {', '.join(missing)}")
        if feature.get("status") not in {"product", "removed"}:
            errors.append(f"{feature_id}: status must be product or removed")
        errors.extend(feature_shape_errors(feature))
        if isinstance(feature.get("contracts"), list):
            classified.extend(contract for contract in feature["contracts"] if isinstance(contract, str))
        if feature.get("status") == "product":
            native, feature_pending = platform_errors(feature, require_complete=require_complete)
            errors.extend(native)
            pending.extend(feature_pending)
            for state in ("restart", "offline"):
                if state not in feature.get("failure_states", []):
                    errors.append(f"{feature_id}: failure_states missing {state}")
            if feature_id == "cloud-account":
                errors.extend(cloud_errors(feature))

    errors.extend(contract_errors(shipped, classified))
    if errors:
        return fail("\nfeature-ledger: ".join(errors))
    if pending:
        print(f"feature-ledger: PENDING native evidence: {', '.join(sorted(pending))}")
    print(f"feature-ledger: {len(features)} features, {len(shipped)} Tauri commands classified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
