"""Release invariants for the three artifacts people install.

These checks intentionally verify outcomes and provenance, rather than the
number or names of build jobs. A release must still qualify macOS, Android,
and Windows receipts against the exact artifacts it publishes.
"""

import copy
import json
import os
import pathlib
import subprocess
import tempfile

from native_evidence_policy import load_policy, schema_document


ROOT = pathlib.Path(__file__).resolve().parents[2]
POLICY = load_policy()
RECEIPT_ARTIFACTS = {
    platform: requirement["release_artifact"]
    for platform, requirement in POLICY["platforms"].items()
}


def steps(job):
    return job.get("steps") or []


def commands(job):
    return "\n".join(str(step.get("run") or "") for step in steps(job))


def needs(job):
    value = job.get("needs") or []
    return {value} if isinstance(value, str) else set(value)


def downloads(job):
    return {
        (step.get("with") or {}).get("name")
        for step in steps(job)
        if str(step.get("uses") or "").startswith("actions/download-artifact")
    }


def uploaders(jobs, artifact):
    return [
        (name, job)
        for name, job in jobs.items()
        if artifact in {
            (step.get("with") or {}).get("name")
            for step in steps(job)
            if str(step.get("uses") or "").startswith("actions/upload-artifact")
        }
    ]


def finalizers(jobs):
    return [
        (name, job)
        for name, job in jobs.items()
        if "check:native-parity" in commands(job)
    ]


def validation_steps(job):
    return [step for step in steps(job) if "check:native-parity" in str(step.get("run") or "")]


def resolver_job(jobs):
    return next(
        (job for job in jobs.values()
         if (job.get("outputs") or {}).get("qualify") == "${{ steps.resolve.outputs.qualify }}"),
        {},
    )


def resolve_mode(release, *, event_name, ref_name, version, publish, qualify, metadata_version):
    version_job = resolver_job(release.get("jobs") or {})
    resolver = next((step for step in steps(version_job) if step.get("id") == "resolve"), {})
    script = str(resolver.get("run") or "")
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
        result = subprocess.run(["bash", "-c", script], env=environment, text=True, capture_output=True, check=False)
        values = {}
        if output.exists():
            for line in output.read_text(encoding="utf-8").splitlines():
                key, value = line.split("=", 1)
                values[key] = value
        return result, values


def qualification_errors(release):
    jobs = release.get("jobs") or {}
    version = resolver_job(jobs)
    resolver = next((step for step in steps(version) if step.get("id") == "resolve"), {})
    source = commands(version)
    errors = []
    if version.get("outputs", {}).get("qualify") != "${{ steps.resolve.outputs.qualify }}":
        errors.append("release version job must expose qualification state")
    if not {"INPUT_PUBLISH", "INPUT_QUALIFY"} <= set((resolver.get("env") or {})):
        errors.append("release resolver must receive publish and qualification inputs")
    if "publish" not in source or "qualify=true" not in source:
        errors.append("publication must imply artifact qualification")
    publishers = [job for job in jobs.values() if "publish-github-release.sh" in commands(job)]
    if len(publishers) != 1 or ".outputs.publish" not in str(publishers[0]):
        errors.append("only the final release job may publish a qualified artifact")
    writers = [name for name, job in jobs.items() if "write" in str(job.get("permissions") or "")]
    if len(writers) > 1:
        errors.append("qualification must not widen repository write permissions")
    return errors


def signing_errors(jobs):
    errors = []
    android_jobs = [job for job in jobs.values() if "apksigner" in str(job)]
    windows_jobs = [job for job in jobs.values() if "build-windows.ps1" in str(job) or "windows-sign.ps1" in str(job)]
    android_source = "\n".join(str(job) for job in android_jobs)
    windows_source = "\n".join(str(job) for job in windows_jobs)
    if "ANDROID_KEYSTORE_BASE64" not in android_source or "apksigner" not in android_source:
        errors.append("release must sign and verify the Android APK")
    if "WINDOWS_SIGNING_CERTIFICATE_BASE64" not in windows_source or "TAURI_SIGNING_PRIVATE_KEY" not in windows_source:
        errors.append("release must sign the Windows installer and updater")
    private = {"TAURI_SIGNING_PRIVATE_KEY", "TAURI_SIGNING_PRIVATE_KEY_PASSWORD"}
    if any(private & set((job.get("env") or {})) for job in jobs.values()):
        errors.append("Windows private signing inputs must be scoped to signing steps")
    return errors


def android_receipt_errors(job):
    runner = next(
        (step for step in steps(job) if "android-emulator-runner" in str(step.get("uses") or "")),
        {},
    )
    runner_inputs = runner.get("with") or {}
    source = str(job)
    if (
        runner_inputs.get("api-level") != "36"
        or runner_inputs.get("arch") != "x86_64"
        or (
            "COPYPASTE_SMOKE_PROFILE=critical" not in source
            and (job.get("env") or {}).get("COPYPASTE_SMOKE_PROFILE") != "critical"
        )
        or "android-release-emulator-legs.sh" not in source
    ):
        return ["Android release receipt must come from the critical signed API 36 emulator smoke"]
    return []


def contract_errors(release, projected_schema=None):
    jobs = release.get("jobs") or {}
    errors = qualification_errors(release) + signing_errors(jobs)
    producers = {}
    for platform, artifact in RECEIPT_ARTIFACTS.items():
        matches = uploaders(jobs, artifact)
        if len(matches) != 1:
            errors.append(f"release must upload one {platform} native receipt")
        else:
            producers[platform] = matches[0]

    finalizer_matches = finalizers(jobs)
    if len(finalizer_matches) != 1:
        errors.append("release must have one fail-closed native receipt finalizer")
        return errors
    finalizer_name, finalizer = finalizer_matches[0]
    if "continue-on-error" in finalizer:
        errors.append("native receipt finalizer must not continue after failure")
    for platform, (producer_name, _) in producers.items():
        if producer_name not in needs(finalizer):
            errors.append(f"native receipt finalizer must wait for {platform} evidence")
    if not set(RECEIPT_ARTIFACTS.values()) <= downloads(finalizer):
        errors.append("native receipt finalizer must download all three receipts")
    validation = validation_steps(finalizer)
    if len(validation) != 1:
        errors.append("native receipt finalizer must run one validation step")
        validation_source = ""
    else:
        validation_step = validation[0]
        validation_source = str(validation_step.get("run") or "")
        if "if" in validation_step or "continue-on-error" in validation_step:
            errors.append("native receipt validation must not be skipped or continued")
    required = ("--require macos,android,windows", "--commit", "github.sha", "--run-id", "github.run_id")
    if any(value not in validation_source for value in required) or validation_source.count("--evidence") != 3:
        errors.append("native receipt finalizer must validate all three run-bound receipts")
    for platform in RECEIPT_ARTIFACTS:
        if f"--qualified-artifact {platform}=" not in validation_source:
            errors.append("native receipt finalizer must bind every receipt to its build artifact")
            break
    if "sha256" not in commands(finalizer):
        errors.append("native receipt finalizer must verify qualified artifact hashes")
    if "android" in producers:
        errors.extend(android_receipt_errors(producers["android"][1]))

    if projected_schema is None:
        schema_path = ROOT / "crates" / "copypaste-ui" / "scripts" / "native-parity-evidence.schema.json"
        try:
            projected_schema = json.loads(schema_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            projected_schema = None
    if projected_schema != schema_document(POLICY):
        errors.append("native evidence schema must match the current evidence policy")
    return errors


def self_test(release):
    fixtures = []

    def rejects(label, mutate, expected):
        fixture = copy.deepcopy(release)
        mutate(fixture)
        fixtures.append((label, any(expected in error for error in contract_errors(fixture))))

    def mode_holds(event_name, ref_name, version, publish, qualify, expected):
        result, values = resolve_mode(release, event_name=event_name, ref_name=ref_name, version=version, publish=publish, qualify=qualify, metadata_version="2.0.0-alpha.34")
        return result.returncode == 0 and values == expected

    fixtures.extend((
        ("tag release publishes and qualifies", mode_holds("push", "v2.0.0-alpha.34", "", "false", "false", {"version": "2.0.0-alpha.34", "publish": "true", "qualify": "true"})),
        ("qualification dispatch remains non-publishing", mode_holds("workflow_dispatch", "", "2.0.0-alpha.34", "false", "true", {"version": "2.0.0-alpha.34", "publish": "false", "qualify": "true"})),
    ))

    jobs = release.get("jobs") or {}
    finalizer_name, _ = finalizers(jobs)[0]
    android_name, _ = uploaders(jobs, RECEIPT_ARTIFACTS["android"])[0]
    windows_name, _ = uploaders(jobs, RECEIPT_ARTIFACTS["windows"])[0]
    rejects("missing Windows evidence dependency fails", lambda value: value["jobs"][finalizer_name]["needs"].remove(windows_name), "wait for windows evidence")
    rejects("missing Android receipt download fails", lambda value: value["jobs"][finalizer_name]["steps"].__setitem__(slice(None), [step for step in steps(value["jobs"][finalizer_name]) if (step.get("with") or {}).get("name") != RECEIPT_ARTIFACTS["android"]]), "download all three receipts")
    rejects("unbound Windows artifact fails", lambda value: next(step for step in steps(value["jobs"][finalizer_name]) if "--qualified-artifact windows=" in str(step.get("run") or "")).update({"run": commands(value["jobs"][finalizer_name]).replace("--qualified-artifact windows=", "--unbound-artifact windows=")}), "bind every receipt")
    rejects("skipped receipt validation fails", lambda value: validation_steps(value["jobs"][finalizer_name])[0].update({"if": False}), "must not be skipped")
    rejects("continuing receipt validation fails", lambda value: validation_steps(value["jobs"][finalizer_name])[0].update({"continue-on-error": True}), "must not be skipped")
    rejects("unbound receipt commit fails", lambda value: validation_steps(value["jobs"][finalizer_name])[0].update({"run": str(validation_steps(value["jobs"][finalizer_name])[0]["run"]).replace("--commit", "--unbound-commit")}), "all three run-bound receipts")
    rejects("wrong Android evidence platform fails", lambda value: next(step for step in steps(value["jobs"][android_name]) if "android-emulator-runner" in str(step.get("uses") or ""))["with"].update({"api-level": "33"}), "signed API 36")
    rejects("missing Windows signing input fails", lambda value: value["jobs"][windows_name]["steps"].__setitem__(slice(None), [step for step in steps(value["jobs"][windows_name]) if "TAURI_SIGNING_PRIVATE_KEY" not in str(step)]), "sign the Windows installer")

    failures = 0
    for label, held in fixtures:
        print(f"{'PASS' if held else 'FAIL'}|{label}|")
        failures += not held
    return failures
