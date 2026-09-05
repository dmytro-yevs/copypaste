import copy
import json
import os
import pathlib
import shlex
import subprocess
import tempfile

from native_evidence_policy import load_policy, schema_document


ROOT = pathlib.Path(__file__).resolve().parents[2]
POLICY = load_policy()
RELEASE_ARTIFACTS = {
    platform: requirement["release_artifact"]
    for platform, requirement in POLICY["platforms"].items()
}
QUALIFIED_ARTIFACTS = {
    "macos": (
        "macos-app-arm64",
        "artifacts/qualified-artifacts/macos",
        "CopyPaste-v${{ needs.version.outputs.version }}-macos-arm64.dmg",
    ),
    "android": (
        "android",
        "artifacts/qualified-artifacts/android",
        "CopyPaste-v${{ needs.version.outputs.version }}-android.apk",
    ),
    "windows": (
        "windows-x86_64",
        "artifacts/qualified-artifacts/windows",
        "CopyPaste-v${{ needs.version.outputs.version }}-windows-x86_64-setup.exe",
    ),
}


def steps(job):
    return job.get("steps") or []


def commands(job):
    return "\n".join(str(step.get("run") or "") for step in steps(job))


def downloads(job):
    return {
        (step.get("with") or {}).get("name")
        for step in steps(job)
        if str(step.get("uses") or "").startswith("actions/download-artifact")
    }


def download_destinations(job):
    return {
        (step.get("with") or {}).get("name"): (step.get("with") or {}).get("path")
        for step in steps(job)
        if str(step.get("uses") or "").startswith("actions/download-artifact")
    }


def exact_ledger_gate_steps(job):
    expected = (
        "python3", "scripts/check-feature-ledger.py", "--require-complete",
        "--version", "$RELEASE_VERSION",
    )
    matches = []
    for step in steps(job):
        for line in str(step.get("run") or "").splitlines():
            try:
                argv = tuple(shlex.split(line, comments=True))
            except ValueError:
                continue
            if argv == expected:
                matches.append(step)
    return matches


def qualification_contract(release):
    errors = []
    triggers = release.get(True) or release.get("on") or {}
    inputs = ((triggers.get("workflow_dispatch") or {}).get("inputs") or {})
    qualify_input = inputs.get("qualify") or {}
    version = (release.get("jobs") or {}).get("version") or {}
    outputs = version.get("outputs") or {}
    resolve = next((step for step in steps(version) if step.get("id") == "resolve"), {})
    resolve_body = str(resolve.get("run") or "")
    resolve_env = str(resolve.get("env") or {})

    if qualify_input.get("type") != "boolean" or qualify_input.get("default") is not False:
        errors.append("release qualification must be an explicit false-by-default boolean input")
    if outputs.get("qualify") != "${{ steps.resolve.outputs.qualify }}":
        errors.append("release version job must expose the canonical qualification output")
    for value in (
        'echo "qualify=$qualify"',
        '>> "$GITHUB_OUTPUT"',
        'if [[ "$publish" == "true" ]]; then',
        "qualify=true",
        "true|false)",
    ):
        if value not in resolve_body:
            errors.append("release mode resolver must validate and emit qualification state")
            break
    if "INPUT_QUALIFY" not in resolve_env or "${{ inputs.qualify }}" not in resolve_env:
        errors.append("release mode resolver must receive the explicit qualification input")

    jobs = release.get("jobs") or {}
    for name in (
        "android-upgrade-fixture",
        "android-cloud-evidence",
        "android-smoke",
        "android-smoke-api33",
        "native-parity",
    ):
        if (jobs.get(name) or {}).get("if") != "needs.version.outputs.qualify == 'true'":
            errors.append(f"{name} must run for canonical release qualification")

    windows = jobs.get("windows") or {}
    signed = next(
        (step for step in steps(windows) if step.get("name") == "Build signed Windows release package"),
        {},
    )
    unsigned = next(
        (step for step in steps(windows) if step.get("name") == "Build unsigned Windows release package"),
        {},
    )
    prepare = next(
        (step for step in steps(windows) if step.get("name") == "Prepare Windows release signing"),
        {},
    )
    cleanup = next(
        (step for step in steps(windows) if step.get("name") == "Remove Windows signing material"),
        {},
    )
    if (
        signed.get("if") != "needs.version.outputs.qualify == 'true'"
        or unsigned.get("if") != "needs.version.outputs.qualify != 'true'"
        or prepare.get("if") != "needs.version.outputs.qualify == 'true'"
        or cleanup.get("if") != "always() && needs.version.outputs.qualify == 'true'"
        or "needs.version.outputs.qualify" not in commands(windows)
    ):
        errors.append("Windows qualification must sign, verify, and clean up the release package")

    publish = jobs.get("publish") or {}
    if publish.get("if") != "needs.version.outputs.publish == 'true'":
        errors.append("only publication may use the publish output")
    writers = [
        name for name, job in jobs.items()
        if "write" in str(job.get("permissions") or "")
    ]
    if writers != ["publish"]:
        errors.append("qualification must not widen repository contents permissions")
    return errors


def resolve_mode(release, *, event_name, ref_name, version, publish, qualify, metadata_version):
    job = (release.get("jobs") or {}).get("version") or {}
    step = next((candidate for candidate in steps(job) if candidate.get("id") == "resolve"), {})
    script = str(step.get("run") or "")
    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory)
        output = root / "github-output"
        node_dir = root / "bin"
        node_dir.mkdir()
        node = node_dir / "node"
        node.write_text("#!/bin/sh\nprintf '%s\\n' \"$RELEASE_TEST_METADATA_VERSION\"\n", encoding="utf-8")
        node.chmod(0o755)
        environment = os.environ.copy()
        environment.update({
            "EVENT_NAME": event_name,
            "GITHUB_REF_NAME": ref_name,
            "INPUT_VERSION": version,
            "INPUT_PUBLISH": publish,
            "INPUT_QUALIFY": qualify,
            "RELEASE_TEST_METADATA_VERSION": metadata_version,
            "GITHUB_OUTPUT": str(output),
            "PATH": f"{node_dir}{os.pathsep}{environment.get('PATH', '')}",
        })
        result = subprocess.run(
            ["bash", "-c", script],
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        values = {}
        if output.exists():
            for line in output.read_text(encoding="utf-8").splitlines():
                key, value = line.split("=", 1)
                values[key] = value
        return result, values


def emulator_android_contract(release):
    jobs = release.get("jobs") or {}
    smoke = jobs.get("android-smoke") or {}
    runner = next(
        (step for step in steps(smoke)
         if str(step.get("uses") or "").startswith("reactivecircus/android-emulator-runner")),
        {},
    )
    upload = [
        step
        for step in steps(smoke)
        if (step.get("with") or {}).get("name") == RELEASE_ARTIFACTS["android"]
    ]
    valid = (
        "android-hardware" not in jobs
        and (runner.get("with") or {}).get("api-level") == "36"
        and (runner.get("with") or {}).get("arch") == "x86_64"
        and "android-release-emulator-legs.sh" in str((runner.get("with") or {}).get("script") or "")
        and "sha256sum --check" in commands(smoke)
        and "android" in downloads(smoke)
        and len(upload) == 1
        and (upload[0].get("with") or {}).get("if-no-files-found") == "error"
    )
    return valid, "the qualified Android receipt must come from the signed API 36 emulator job"


def contract_errors(release, projected_schema=None):
    errors = []
    errors.extend(qualification_contract(release))
    jobs = release.get("jobs") or {}
    gate = jobs.get("native-parity") or {}
    publish = jobs.get("publish") or {}
    if "continue-on-error" in gate:
        errors.append("native parity must not continue after a failed complete-evidence gate")
    if not {"macos", "android-smoke", "windows"} <= set(gate.get("needs") or []):
        errors.append("native parity must wait for all three shipped platforms")
    if not {"native-parity", "windows"} <= set(publish.get("needs") or []):
        errors.append("publication must wait for Windows and native parity")
    if not set(RELEASE_ARTIFACTS.values()) <= downloads(gate):
        errors.append("native parity must download all three release receipts")
    qualified_downloads = download_destinations(gate)
    expected_downloads = {
        artifact: destination
        for artifact, destination, _ in QUALIFIED_ARTIFACTS.values()
    }
    if any(qualified_downloads.get(artifact) != destination
           for artifact, destination in expected_downloads.items()):
        errors.append("native parity must download each qualified product artifact to its own path")

    gate_commands = commands(gate)
    ledger_gate_steps = exact_ledger_gate_steps(gate)
    if len(ledger_gate_steps) != 1:
        errors.append("native parity must use one exact version-bound complete-evidence gate")
    elif "if" in ledger_gate_steps[0]:
        errors.append("native parity complete-evidence gate must run unconditionally")
    elif "continue-on-error" in ledger_gate_steps[0]:
        errors.append("native parity complete-evidence gate must not continue after failure")
    elif (ledger_gate_steps[0].get("env") or {}).get("RELEASE_VERSION") != "${{ needs.version.outputs.version }}":
        errors.append("native parity must bind the complete-evidence gate to the resolved version")
    receipt_paths = (
        "artifacts/native-parity/macos/native-evidence.json",
        "artifacts/native-parity/android/native-evidence.json",
        "artifacts/native-parity/windows/native-evidence.json",
    )
    if (
        "--require macos,android,windows" not in gate_commands
        or "--run-id ${{ github.run_id }}" not in gate_commands
        or any(path not in gate_commands for path in receipt_paths)
        or gate_commands.count("native-evidence.json") != 3
        or "--receipt-expectations" not in gate_commands
        or "--expect-feature-state" not in gate_commands
    ):
        errors.append("release gate must validate exactly three run-bound native receipts and ledger states")
    qualified_selectors = [
        f"--qualified-artifact {platform}=${{{{ github.workspace }}}}/{destination}/{filename}"
        for platform, (_, destination, filename) in QUALIFIED_ARTIFACTS.items()
    ]
    if (
        gate_commands.count("--qualified-artifact") != len(qualified_selectors)
        or any(selector not in gate_commands for selector in qualified_selectors)
    ):
        errors.append("native parity must bind each receipt to one exact qualified product artifact")
    emulator_valid, _ = emulator_android_contract(release)
    if not emulator_valid:
        errors.append("canonical Android publication evidence must run on the signed API 36 emulator and fail closed")

    if projected_schema is None:
        schema_path = ROOT / "crates" / "copypaste-ui" / "scripts" / "native-parity-evidence.schema.json"
        try:
            projected_schema = json.loads(schema_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            projected_schema = None
    if projected_schema != schema_document(POLICY):
        errors.append("native evidence schema must be the current policy projection")
    return errors


def self_test(release):
    fixtures = []

    def mode_holds(event_name, ref_name, version, publish, qualify, expected):
        result, values = resolve_mode(
            release,
            event_name=event_name,
            ref_name=ref_name,
            version=version,
            publish=publish,
            qualify=qualify,
            metadata_version="2.0.0-alpha.33",
        )
        return result.returncode == 0 and values == expected

    fixtures.extend((
        (
            "tag release publishes and qualifies",
            mode_holds(
                "push", "v2.0.0-alpha.33", "", "false", "false",
                {"version": "2.0.0-alpha.33", "publish": "true", "qualify": "true"},
            ),
        ),
        (
            "publish dispatch implies qualification",
            mode_holds(
                "workflow_dispatch", "", "v2.0.0-alpha.33", "true", "false",
                {"version": "2.0.0-alpha.33", "publish": "true", "qualify": "true"},
            ),
        ),
        (
            "qualification dispatch remains non-publishing",
            mode_holds(
                "workflow_dispatch", "", "2.0.0-alpha.33", "false", "true",
                {"version": "2.0.0-alpha.33", "publish": "false", "qualify": "true"},
            ),
        ),
        (
            "build-only dispatch skips qualification",
            mode_holds(
                "workflow_dispatch", "", "2.0.0-alpha.33", "false", "false",
                {"version": "2.0.0-alpha.33", "publish": "false", "qualify": "false"},
            ),
        ),
    ))

    with tempfile.TemporaryDirectory() as directory:
        sentinel = pathlib.Path(directory) / "mode-input-executed"
        hostile = f"true; touch {sentinel}"
        result, values = resolve_mode(
            release,
            event_name="workflow_dispatch",
            ref_name="",
            version="2.0.0-alpha.33",
            publish=hostile,
            qualify="false",
            metadata_version="2.0.0-alpha.33",
        )
        fixtures.append((
            "adversarial release mode input fails without execution",
            result.returncode != 0 and not values and not sentinel.exists(),
        ))
    result, values = resolve_mode(
        release,
        event_name="pull_request",
        ref_name="",
        version="2.0.0-alpha.33",
        publish="false",
        qualify="false",
        metadata_version="2.0.0-alpha.33",
    )
    fixtures.append((
        "unsupported release event fails closed",
        result.returncode != 0 and not values,
    ))

    def rejected(label, mutate, expected):
        fixture = copy.deepcopy(release)
        mutate(fixture)
        held = any(expected in error for error in contract_errors(fixture))
        fixtures.append((label, held))

    stale_schema = copy.deepcopy(schema_document(POLICY))
    android_schema = next(
        condition for condition in stale_schema["allOf"]
        if condition["if"]["properties"]["platform"]["const"] == "android"
    )
    android_schema["then"]["properties"]["environment"]["const"] = "physical-device"
    fixtures.append((
        "stale native evidence schema fails",
        any("current policy projection" in error for error in contract_errors(release, stale_schema)),
    ))
    rejected(
        "missing feature-state receipt expectations fails",
        lambda value: next(
            step for step in value["jobs"]["native-parity"]["steps"]
            if "--receipt-expectations" in str(step.get("run") or "")
        ).update({"run": "npm run check:native-parity -- --require macos,android,windows"}),
        "native receipts and ledger states",
    )
    rejected(
        "unbound complete-evidence gate fails",
        lambda value: next(
            step for step in value["jobs"]["native-parity"]["steps"]
            if "check-feature-ledger.py" in str(step.get("run") or "")
        ).update({"run": "python3 scripts/check-feature-ledger.py --require-complete"}),
        "exact version-bound complete-evidence gate",
    )
    rejected(
        "changed complete-evidence version binding fails",
        lambda value: next(
            step for step in value["jobs"]["native-parity"]["steps"]
            if "check-feature-ledger.py" in str(step.get("run") or "")
        )["env"].update({"RELEASE_VERSION": "${{ github.ref_name }}"}),
        "bind the complete-evidence gate to the resolved version",
    )
    rejected(
        "conditional complete-evidence gate fails",
        lambda value: next(
            step for step in value["jobs"]["native-parity"]["steps"]
            if "check-feature-ledger.py" in str(step.get("run") or "")
        ).update({"if": False}),
        "must run unconditionally",
    )
    rejected(
        "continuing complete-evidence gate fails",
        lambda value: next(
            step for step in value["jobs"]["native-parity"]["steps"]
            if "check-feature-ledger.py" in str(step.get("run") or "")
        ).update({"continue-on-error": True}),
        "must not continue after failure",
    )
    rejected(
        "continuing native-parity job fails",
        lambda value: value["jobs"]["native-parity"].update({"continue-on-error": True}),
        "must not continue after a failed complete-evidence gate",
    )
    rejected(
        "missing canonical Android emulator dependency fails",
        lambda value: value["jobs"]["native-parity"]["needs"].remove("android-smoke"),
        "all three shipped platforms",
    )
    rejected(
        "physical receipt substituted for canonical Android emulator fails",
        lambda value: next(
            step for step in value["jobs"]["native-parity"]["steps"]
            if (step.get("with") or {}).get("name") == RELEASE_ARTIFACTS["android"]
        )["with"].update({"name": "release-android-physical-evidence"}),
        "all three release receipts",
    )
    rejected(
        "canonical Android receipt must use API 36 emulator",
        lambda value: next(
            step for step in value["jobs"]["android-smoke"]["steps"]
            if str(step.get("uses") or "").startswith("reactivecircus/android-emulator-runner")
        )["with"].update({"api-level": "33"}),
        "signed API 36 emulator",
    )
    rejected(
        "qualification cannot skip canonical Android emulator evidence",
        lambda value: value["jobs"]["android-smoke"].update(
            {"if": "needs.version.outputs.publish == 'true'"}),
        "android-smoke must run for canonical release qualification",
    )
    rejected(
        "qualification cannot become publication",
        lambda value: value["jobs"]["publish"].update(
            {"if": "needs.version.outputs.qualify == 'true'"}),
        "only publication may use the publish output",
    )
    rejected(
        "missing qualified Android artifact download fails",
        lambda value: value["jobs"]["native-parity"]["steps"].__setitem__(
            slice(None),
            [
                step for step in value["jobs"]["native-parity"]["steps"]
                if (step.get("with") or {}).get("name") != "android"
            ],
        ),
        "qualified product artifact",
    )
    rejected(
        "missing qualified Windows selector fails",
        lambda value: next(
            step for step in value["jobs"]["native-parity"]["steps"]
            if "--qualified-artifact windows=" in str(step.get("run") or "")
        ).update({"run": next(
            step for step in value["jobs"]["native-parity"]["steps"]
            if "--qualified-artifact windows=" in str(step.get("run") or "")
        )["run"].replace(
            "--qualified-artifact windows=", "--unbound-artifact windows=")}),
        "exact qualified product artifact",
    )
    for label, held in fixtures:
        print(f"{'PASS' if held else 'FAIL'}|{label}|{'fixture passed unexpectedly' if not held else ''}")
    return sum(not held for _, held in fixtures)
