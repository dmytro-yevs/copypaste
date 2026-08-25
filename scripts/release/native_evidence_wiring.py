import copy
import json
import pathlib

from native_evidence_policy import load_policy, schema_document


ROOT = pathlib.Path(__file__).resolve().parents[2]
POLICY = load_policy()
RELEASE_ARTIFACTS = {
    platform: requirement["release_artifact"]
    for platform, requirement in POLICY["platforms"].items()
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


def physical_android_contract(release):
    jobs = release.get("jobs") or {}
    hardware = jobs.get("android-hardware") or {}
    upload = [
        step
        for step in steps(hardware)
        if (step.get("with") or {}).get("name") == RELEASE_ARTIFACTS["android"]
    ]
    valid = (
        set(hardware.get("runs-on") or []) == {"self-hosted", "linux", "ARM64", "android-device"}
        and "android-smoke-release.sh" in commands(hardware)
        and "ro.kernel.qemu" in commands(hardware)
        and "ANDROID_SERIAL" in commands(hardware)
        and "android" in downloads(hardware)
        and len(upload) == 1
        and (upload[0].get("with") or {}).get("if-no-files-found") == "error"
    )
    return valid, "the publication receipt must come from the labelled physical-device runner"


def contract_errors(release, projected_schema=None):
    errors = []
    jobs = release.get("jobs") or {}
    gate = jobs.get("native-parity") or {}
    publish = jobs.get("publish") or {}
    if not {"macos", "android-hardware", "windows"} <= set(gate.get("needs") or []):
        errors.append("native parity must wait for all three shipped platforms")
    if not {"native-parity", "windows"} <= set(publish.get("needs") or []):
        errors.append("publication must wait for Windows and native parity")
    if not set(RELEASE_ARTIFACTS.values()) <= downloads(gate):
        errors.append("native parity must download all three release receipts")

    gate_commands = commands(gate)
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
    hardware_valid, _ = physical_android_contract(release)
    if not hardware_valid:
        errors.append("physical Android publication evidence must run on labelled hardware and fail closed")

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
    android_schema["then"]["properties"]["environment"]["const"] = "emulator"
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
        "missing physical Android platform dependency fails",
        lambda value: value["jobs"]["native-parity"]["needs"].remove("android-hardware"),
        "all three shipped platforms",
    )
    rejected(
        "emulator receipt substituted for physical Android fails",
        lambda value: next(
            step for step in value["jobs"]["native-parity"]["steps"]
            if (step.get("with") or {}).get("name") == RELEASE_ARTIFACTS["android"]
        )["with"].update({"name": "release-android-smoke-evidence"}),
        "all three release receipts",
    )
    rejected(
        "physical Android runner without its device label fails",
        lambda value: value["jobs"]["android-hardware"].update({"runs-on": ["self-hosted", "linux", "ARM64"]}),
        "labelled hardware",
    )
    for label, held in fixtures:
        print(f"{'PASS' if held else 'FAIL'}|{label}|{'fixture passed unexpectedly' if not held else ''}")
    return sum(not held for _, held in fixtures)
