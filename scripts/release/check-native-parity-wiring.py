#!/usr/bin/env python3
import copy
import pathlib
import sys

import yaml


ROOT = pathlib.Path(__file__).resolve().parents[2]


def load(name):
    with (ROOT / ".github" / "workflows" / name).open(encoding="utf-8") as stream:
        return yaml.safe_load(stream)


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


def requirements_ci_install_at(command):
    needle = "install --requirement requirements-ci.txt"
    at = command.find(needle)
    if at < 0:
        return -1
    # Quoted venv pip (`"$RUNNER_TEMP/ci-python/bin/pip" install`) is still pip.
    if "pip" not in command[:at]:
        return -1
    return at


def step_installs_requirements(step):
    return requirements_ci_install_at(str(step.get("run") or "")) >= 0


def installs_requirements_before(job, marker):
    installed = False
    for step in steps(job):
        command = "\n".join((
            str(step.get("run") or ""),
            str((step.get("with") or {}).get("script") or ""),
        ))
        marker_at = command.find(marker)
        install_at = requirements_ci_install_at(command)
        if marker_at >= 0:
            return installed or (install_at >= 0 and install_at < marker_at)
        if install_at >= 0:
            installed = True
    return False


def contract_errors(release, nightly, ci):
    errors = []
    jobs = release.get("jobs") or {}
    gate = jobs.get("native-parity") or {}
    windows = jobs.get("windows") or {}
    publish = jobs.get("publish") or {}

    if not {"macos", "android-smoke", "windows"} <= set(gate.get("needs") or []):
        errors.append("native parity must wait for all three shipped platforms")
    if not {"native-parity", "windows"} <= set(publish.get("needs") or []):
        errors.append("publication must wait for Windows and native parity")
    required_receipts = {
        "release-macos-native-evidence",
        "release-android-smoke-evidence",
        "release-windows-native-evidence",
    }
    if not required_receipts <= downloads(gate):
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
    ):
        errors.append("release gate must validate exactly three run-bound native receipts")

    windows_commands = commands(windows)
    if (
        "build-windows.ps1" not in windows_commands
        or "-Unsigned" not in windows_commands
        or "ExpectedSignature $signature" not in windows_commands
        or "windows-native-evidence.ps1" not in windows_commands
        or "-PackageDirectory artifacts/windows-x86_64" not in windows_commands
    ):
        errors.append("Windows release must distinguish signed and unsigned installed evidence")
    windows_env = windows.get("env") or {}
    signing_env = {
        "WINDOWS_TIMESTAMP_URL",
        "TAURI_UPDATER_PUBLIC_KEY",
        "TAURI_UPDATER_ENDPOINT",
    }
    private_signing_env = {"TAURI_SIGNING_PRIVATE_KEY", "TAURI_SIGNING_PRIVATE_KEY_PASSWORD"}
    prepare_step = next(
        (step for step in steps(windows) if step.get("name") == "Prepare Windows release signing"),
        {},
    )
    prepare_env = prepare_step.get("env") or {}
    prepare_commands = str(prepare_step.get("run") or "")
    cleanup_step = next(
        (step for step in steps(windows) if step.get("name") == "Remove Windows signing material"),
        {},
    )
    signed_build = next(
        (step for step in steps(windows) if step.get("name") == "Build signed Windows release package"),
        {},
    )
    unsigned_build = next(
        (step for step in steps(windows) if step.get("name") == "Build unsigned Windows release package"),
        {},
    )
    if (
        not signing_env <= set(windows_env)
        or not {"WINDOWS_SIGNING_CERTIFICATE_BASE64", "WINDOWS_SIGNING_CERTIFICATE_PASSWORD"} <= set(prepare_env)
        or "releases/download/v${{ needs.version.outputs.version }}" not in str(windows_env.get("WINDOWS_RELEASE_BASE_URL") or "")
    ):
        errors.append("signed Windows publication must declare every certificate, updater, timestamp, and release URL input")
    if (
        "windows-sign.ps1" not in prepare_commands
        or "-Operation Prepare" not in prepare_commands
        or "-PersistEnvironment" not in prepare_commands
        or "-SmokeSign" not in prepare_commands
        or cleanup_step.get("if") != "always() && needs.version.outputs.publish == 'true'"
        or "-Operation Cleanup" not in str(cleanup_step.get("run") or "")
        or "Import-PfxCertificate" in windows_commands
        or "Cert:\\" in windows_commands
    ):
        errors.append("Windows signing must use the shared prepare, smoke, and cleanup lifecycle without certificate stores")
    if (
        private_signing_env & set(windows_env)
        or not private_signing_env <= set(signed_build.get("env") or {})
        or private_signing_env & set(unsigned_build.get("env") or {})
        or signed_build.get("if") != "needs.version.outputs.publish == 'true'"
        or unsigned_build.get("if") != "needs.version.outputs.publish != 'true'"
    ):
        errors.append("Tauri private signing inputs must be scoped only to the signed Windows build step")
    windows_uploads = {
        (step.get("with") or {}).get("name"): (step.get("with") or {})
        for step in steps(windows)
        if str(step.get("uses") or "").startswith("actions/upload-artifact")
    }
    for name in ("windows-x86_64", "release-windows-native-evidence"):
        if (windows_uploads.get(name) or {}).get("if-no-files-found") != "error":
            errors.append(f"{name} upload must fail closed")

    publish_commands = commands(publish)
    if "windows-x86_64" not in downloads(publish):
        errors.append("publication must download the Windows release artifact")
    for asset in ("dist/*.exe", "dist/*.exe.sig", "dist/latest.json", "dist/SHA256SUMS"):
        if asset not in publish_commands:
            errors.append(f"publication is missing Windows asset {asset}")

    nightly_jobs = nightly.get("jobs") or {}
    nightly_windows = nightly_jobs.get("windows") or {}
    nightly_commands = commands(nightly_windows)
    triggers = nightly.get(True) or nightly.get("on") or {}
    windows_input = ((triggers.get("workflow_dispatch") or {}).get("inputs") or {}).get("windows_evidence") or {}
    if windows_input.get("type") != "boolean" or windows_input.get("default") is not False:
        errors.append("nightly Windows evidence must remain an explicit manual option")
    if (
        "inputs.windows_evidence" not in str(nightly_windows.get("if") or "")
        or "build-windows.ps1 -Unsigned" not in nightly_commands
        or "windows-native-evidence.ps1" not in nightly_commands
        or "--require windows" not in nightly_commands
    ):
        errors.append("requested nightly Windows evidence must exercise and gate an installed package")

    ci_jobs = ci.get("jobs") or {}
    requirements_producers = (
        (jobs.get("macos") or {}, "smoke-macos-dmg.sh", "release macOS evidence"),
        (jobs.get("android-smoke") or {}, "android-release-emulator-legs.sh", "release Android API 36 evidence"),
        (jobs.get("android-smoke-api33") or {}, "android-smoke-release.sh", "release Android API 33 evidence"),
        (windows, "windows-native-evidence.ps1", "release Windows evidence"),
        (nightly_windows, "windows-native-evidence.ps1", "nightly Windows evidence"),
        (ci_jobs.get("frontend") or {}, "npm test", "CI frontend tests"),
        (ci_jobs.get("windows-test") or {}, "npm test", "CI Windows frontend tests"),
        (ci_jobs.get("windows-package") or {}, "windows-native-evidence.ps1", "CI Windows evidence"),
        (ci_jobs.get("release-pipeline") or {}, "scripts/release/check.sh", "CI release self-tests"),
    )
    for job, marker, label in requirements_producers:
        if not installs_requirements_before(job, marker):
            errors.append(f"{label} must install requirements-ci.txt before its producer")

    android_uploads = [
        step
        for step in steps(jobs.get("android-smoke") or {})
        if (step.get("with") or {}).get("name") == "release-android-smoke-evidence"
    ]
    if len(android_uploads) != 1 or (android_uploads[0].get("with") or {}).get("if-no-files-found") != "error":
        errors.append("Android emulator evidence upload must fail closed")
    return errors


def self_test(release, nightly, ci):
    failures = 0

    def move_install_after_test(value, job_name):
        job_steps = value["jobs"][job_name]["steps"]
        install = next(step for step in job_steps if step_installs_requirements(step))
        job_steps.remove(install)
        test_index = next(
            index for index, step in enumerate(job_steps)
            if "npm test" in str(step.get("run") or "")
        )
        job_steps.insert(test_index + 1, install)

    def remove_download(value):
        job_steps = value["jobs"]["native-parity"]["steps"]
        job_steps[:] = [
            step
            for step in job_steps
            if (step.get("with") or {}).get("name") != "release-windows-native-evidence"
        ]

    def remove_asset(value):
        release_step = next(
            step for step in value["jobs"]["publish"]["steps"]
            if step.get("name") == "Create GitHub Release"
        )
        release_step["run"] = release_step["run"].replace("dist/latest.json", "")

    def rejected(label, mutation, expected):
        nonlocal failures
        fixture = copy.deepcopy(release)
        mutation(fixture)
        held = any(expected in error for error in contract_errors(fixture, nightly, ci))
        print(f"{'PASS' if held else 'FAIL'}|{label}|{'fixture passed unexpectedly' if not held else ''}")
        failures += 0 if held else 1

    def rejected_ci(label, job_name, expected):
        nonlocal failures
        fixture = copy.deepcopy(ci)
        move_install_after_test(fixture, job_name)
        held = any(expected in error for error in contract_errors(release, nightly, fixture))
        print(f"{'PASS' if held else 'FAIL'}|{label}|{'fixture passed unexpectedly' if not held else ''}")
        failures += 0 if held else 1

    # Splitting the Windows job into two means either half can now go missing
    # on its own, which no fixture above would notice.
    def rejected_ci_dropped(label, job_name, expected):
        nonlocal failures
        fixture = copy.deepcopy(ci)
        fixture["jobs"].pop(job_name)
        held = any(expected in error for error in contract_errors(release, nightly, fixture))
        print(f"{'PASS' if held else 'FAIL'}|{label}|{'fixture passed unexpectedly' if not held else ''}")
        failures += 0 if held else 1

    rejected(
        "missing Windows platform dependency fails",
        lambda value: value["jobs"]["native-parity"]["needs"].remove("windows"),
        "all three shipped platforms",
    )
    rejected(
        "missing Windows evidence download fails",
        remove_download,
        "all three release receipts",
    )
    rejected(
        "missing Windows publish asset fails",
        remove_asset,
        "dist/latest.json",
    )
    rejected(
        "job-wide Tauri private signing input fails",
        lambda value: value["jobs"]["windows"]["env"].update({"TAURI_SIGNING_PRIVATE_KEY": "leaked"}),
        "scoped only to the signed Windows build step",
    )
    rejected(
        "unsigned build Tauri private signing input fails",
        lambda value: next(step for step in value["jobs"]["windows"]["steps"] if step.get("name") == "Build unsigned Windows release package").setdefault("env", {}).update({"TAURI_SIGNING_PRIVATE_KEY_PASSWORD": "leaked"}),
        "scoped only to the signed Windows build step",
    )
    rejected(
        "missing signed-step Tauri private signing input fails",
        lambda value: next(step for step in value["jobs"]["windows"]["steps"] if step.get("name") == "Build signed Windows release package")["env"].pop("TAURI_SIGNING_PRIVATE_KEY_PASSWORD"),
        "scoped only to the signed Windows build step",
    )
    rejected(
        "missing requirements install fails",
        lambda value: value["jobs"]["windows"]["steps"].__setitem__(
            slice(None),
            [step for step in value["jobs"]["windows"]["steps"] if not step_installs_requirements(step)],
        ),
        "release Windows evidence must install requirements-ci.txt",
    )
    rejected(
        "missing Android producer requirements install fails",
        lambda value: value["jobs"]["android-smoke-api33"]["steps"].__setitem__(
            slice(None),
            [step for step in value["jobs"]["android-smoke-api33"]["steps"] if not step_installs_requirements(step)],
        ),
        "release Android API 33 evidence must install requirements-ci.txt",
    )
    rejected(
        "missing macOS producer requirements install fails",
        lambda value: value["jobs"]["macos"]["steps"].__setitem__(
            slice(None),
            [step for step in value["jobs"]["macos"]["steps"] if not step_installs_requirements(step)],
        ),
        "release macOS evidence must install requirements-ci.txt",
    )
    rejected_ci(
        "frontend test before requirements install fails",
        "frontend",
        "CI frontend tests must install requirements-ci.txt",
    )
    rejected_ci(
        "Windows frontend test before requirements install fails",
        "windows-test",
        "CI Windows frontend tests must install requirements-ci.txt",
    )
    rejected_ci_dropped(
        "a CI Windows job that vanished fails",
        "windows-package",
        "CI Windows evidence must install requirements-ci.txt",
    )
    return failures


release = load("release.yml")
nightly = load("native-nightly.yml")
ci = load("ci.yml")
errors = contract_errors(release, nightly, ci)
for error in errors:
    print(f"FAIL|{error}|")
if not errors:
    print("PASS|all shipped-platform release wiring is fail closed|")

for script in ("macos-native-evidence.sh", "android-smoke-release.sh", "windows-native-evidence.ps1"):
    body = (ROOT / "scripts" / "release" / script).read_text(encoding="utf-8")
    if "write-native-evidence.py" not in body:
        errors.append(f"{script} does not write a native evidence receipt")
        print(f"FAIL|{errors[-1]}|")

if "--self-test" in sys.argv:
    errors.extend("self-test failure" for _ in range(self_test(release, nightly, ci)))
sys.exit(1 if errors else 0)
