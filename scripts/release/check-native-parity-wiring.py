#!/usr/bin/env python3
import copy
import pathlib
import shlex
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


def uploads(job):
    return {
        (step.get("with") or {}).get("name")
        for step in steps(job)
        if str(step.get("uses") or "").startswith("actions/upload-artifact")
    }


def needs(job):
    value = job.get("needs") or []
    return {value} if isinstance(value, str) else set(value)


def prebuilt_sidecar_contract(jobs, package_job, sidecar_job, artifact):
    package = jobs.get(package_job) or {}
    sidecars = jobs.get(sidecar_job) or {}
    sidecar_commands = commands(sidecars)
    package_commands = commands(package)
    return (
        sidecar_job in needs(package)
        and artifact in uploads(sidecars)
        and artifact in downloads(package)
        and "--target x86_64-pc-windows-msvc -p copypaste-cli -p copypaste-daemon" in sidecar_commands
        and 'architecture = "x86_64"' in sidecar_commands
        and "-PrebuiltSidecarsDirectory artifacts/windows-sidecars" in package_commands
    )


def windows_signing_prepare(job):
    return next(
        (step for step in steps(job) if "-Operation Prepare" in str(step.get("run") or "")),
        {},
    )


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


def parsed_commands(job):
    parsed = []
    for step_index, step in enumerate(steps(job)):
        for line_index, line in enumerate(str(step.get("run") or "").splitlines()):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            try:
                argv = tuple(shlex.split(line, posix=True))
            except ValueError:
                continue
            parsed.append((step_index, line_index, argv))
    return parsed


def exact_command_occurrences(job, expected):
    return [
        (step_index, line_index)
        for step_index, line_index, argv in parsed_commands(job)
        if argv == expected
    ]


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
    signing_prepare = windows_signing_prepare(windows)
    certificate_env = signing_prepare.get("env") or {}
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
        or not {"WINDOWS_SIGNING_CERTIFICATE_BASE64", "WINDOWS_SIGNING_CERTIFICATE_PASSWORD"} <= set(certificate_env)
        or "releases/download/v${{ needs.version.outputs.version }}" not in str(windows_env.get("WINDOWS_RELEASE_BASE_URL") or "")
    ):
        errors.append("signed Windows publication must declare every certificate, updater, timestamp, and release URL input")
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
        or "build-windows.ps1" not in nightly_commands
        or "-Unsigned" not in nightly_commands
        or "windows-native-evidence.ps1" not in nightly_commands
        or "--require windows" not in nightly_commands
    ):
        errors.append("requested nightly Windows evidence must exercise and gate an installed package")
    if not prebuilt_sidecar_contract(
        nightly_jobs, "windows", "windows-sidecars", "windows-nightly-sidecars"
    ):
        errors.append("nightly Windows packaging must consume an x86_64 prebuilt sidecar artifact")

    ci_jobs = ci.get("jobs") or {}
    ci_frontend = ci_jobs.get("frontend") or {}
    ci_windows_tests = [
        ci_jobs.get(name) or {}
        for name in ("windows-test-core", "windows-test-services", "windows-test-apps")
    ]
    ci_windows_native_test = ci_jobs.get("windows-native-test") or {}
    windows_parity = "npm run test:native-parity"
    windows_parity_steps = exact_command_occurrences(
        ci_windows_native_test, ("npm", "run", "test:native-parity")
    )
    windows_npm_ci_steps = exact_command_occurrences(ci_windows_native_test, ("npm", "ci"))
    windows_test_timeouts = [job.get("timeout-minutes") for job in ci_windows_tests]
    windows_native_test_timeout = ci_windows_native_test.get("timeout-minutes")
    if (
        len(ci_windows_tests) != 3
        or any(not isinstance(timeout, int) or isinstance(timeout, bool) or not 0 < timeout <= 15
               for timeout in windows_test_timeouts)
        or not isinstance(windows_native_test_timeout, int)
        or isinstance(windows_native_test_timeout, bool)
        or not 0 < windows_native_test_timeout <= 20
    ):
        errors.append("CI Windows workspace test shards must be bounded to at most 15 minutes")
    if (
        len(windows_parity_steps) != 1
        or len(windows_npm_ci_steps) != 1
        or windows_npm_ci_steps[0] > windows_parity_steps[0]
    ):
        errors.append("CI Windows native-parity tests must run exactly once after npm ci")
    frontend_contract = (
        ("npm ci", ("npm", "ci")),
        ("npm run build", ("npm", "run", "build")),
        ("npm test", ("npm", "test")),
    )
    frontend_positions = [
        exact_command_occurrences(ci_frontend, expected)
        for _, expected in frontend_contract
    ]
    if any(len(positions) != 1 for positions in frontend_positions):
        errors.append("CI Linux frontend must run exactly one npm ci, npm run build, and full npm test")
    elif [positions[0] for positions in frontend_positions] != sorted(
        positions[0] for positions in frontend_positions
    ):
        errors.append("CI Linux frontend must run npm ci, npm run build, and npm test in dependency order")

    windows_npm_commands = [
        argv
        for _, _, argv in parsed_commands(ci_windows_native_test)
        if argv[:1] == ("npm",)
    ]
    allowed_windows_npm = {
        ("npm", "ci"),
        ("npm", "run", "test:native-parity"),
    }
    if any(argv not in allowed_windows_npm for argv in windows_npm_commands):
        errors.append("CI Windows tests must own only explicit native-parity frontend coverage")
    if not prebuilt_sidecar_contract(
        ci_jobs, "windows-package", "windows-sidecars", "windows-ci-sidecars"
    ):
        errors.append("CI Windows packaging must consume an x86_64 prebuilt sidecar artifact")
    ci_windows_package = ci_jobs.get("windows-package") or {}
    installed_evidence_steps = [
        step
        for step in steps(ci_windows_package)
        if step.get("id") == "installed-evidence"
        and "windows-native-evidence.ps1" in str(step.get("run") or "")
    ]
    launch_diagnostic_uploads = [
        step
        for step in steps(ci_windows_package)
        if (step.get("with") or {}).get("name")
        == "windows-ci-launch-failure-diagnostics"
    ]
    if len(installed_evidence_steps) != 1 or len(launch_diagnostic_uploads) != 1:
        errors.append("CI Windows evidence failure must upload one launch diagnostic artifact")
    else:
        diagnostic_upload = launch_diagnostic_uploads[0]
        diagnostic_with = diagnostic_upload.get("with") or {}
        if (
            diagnostic_upload.get("uses")
            != "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02"
            or diagnostic_upload.get("if")
            != "failure() && steps.installed-evidence.outcome == 'failure'"
            or diagnostic_with.get("path")
            != "artifacts/windows-ci-native/failure-diagnostics"
            or diagnostic_with.get("if-no-files-found") != "error"
            or diagnostic_with.get("retention-days") != 7
        ):
            errors.append(
                "CI Windows launch diagnostics must be pinned, retained seven days, failure-only, and fail closed"
            )
    if not prebuilt_sidecar_contract(
        jobs, "windows", "windows-sidecars", "windows-release-sidecars"
    ):
        errors.append("release Windows packaging must consume an x86_64 prebuilt sidecar artifact")
    requirements_producers = (
        (jobs.get("macos") or {}, "smoke-macos-dmg.sh", "release macOS evidence"),
        (jobs.get("android-smoke") or {}, "android-release-emulator-legs.sh", "release Android API 36 evidence"),
        (jobs.get("android-smoke-api33") or {}, "android-smoke-release.sh", "release Android API 33 evidence"),
        (windows, "windows-native-evidence.ps1", "release Windows evidence"),
        (nightly_windows, "windows-native-evidence.ps1", "nightly Windows evidence"),
        (ci_frontend, "npm test", "CI frontend tests"),
        (ci_windows_native_test, windows_parity, "CI Windows native-parity tests"),
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

    def move_install_after_test(value, job_name, marker):
        job_steps = value["jobs"][job_name]["steps"]
        install = next(step for step in job_steps if step_installs_requirements(step))
        job_steps.remove(install)
        test_index = next(
            index for index, step in enumerate(job_steps)
            if marker in str(step.get("run") or "")
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

    def remove_prepare_certificate(value):
        windows_signing_prepare(value["jobs"]["windows"])["env"].pop(
            "WINDOWS_SIGNING_CERTIFICATE_BASE64"
        )

    def rejected(label, mutation, expected):
        nonlocal failures
        fixture = copy.deepcopy(release)
        mutation(fixture)
        held = any(expected in error for error in contract_errors(fixture, nightly, ci))
        print(f"{'PASS' if held else 'FAIL'}|{label}|{'fixture passed unexpectedly' if not held else ''}")
        failures += 0 if held else 1

    def rejected_ci(label, job_name, marker, expected):
        nonlocal failures
        fixture = copy.deepcopy(ci)
        move_install_after_test(fixture, job_name, marker)
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

    def rejected_nightly_dropped(label, job_name, expected):
        nonlocal failures
        fixture = copy.deepcopy(nightly)
        fixture["jobs"].pop(job_name)
        held = any(expected in error for error in contract_errors(release, fixture, ci))
        print(f"{'PASS' if held else 'FAIL'}|{label}|{'fixture passed unexpectedly' if not held else ''}")
        failures += 0 if held else 1

    def rejected_ci_mutation(label, mutation, expected):
        nonlocal failures
        held = True
        detail = ""
        for fixture_factory in (
            lambda: copy.deepcopy(ci),
            lambda: combined_frontend_fixture(ci),
        ):
            try:
                fixture = fixture_factory()
                mutation(fixture)
                rejected = any(
                    expected in error
                    for error in contract_errors(release, nightly, fixture)
                )
            except Exception as error:
                rejected = False
                detail = f"fixture mutation failed: {error}"
            if not rejected:
                held = False
                detail = detail or "fixture passed unexpectedly"
                break
        print(f"{'PASS' if held else 'FAIL'}|{label}|{detail}")
        failures += 0 if held else 1

    def frontend_command(value, command):
        job = value["jobs"]["frontend"]
        occurrences = exact_command_occurrences(job, tuple(shlex.split(command)))
        if len(occurrences) != 1:
            raise ValueError(
                f"expected one parsed {command!r} command, found {len(occurrences)}"
            )
        return occurrences[0]

    def edit_frontend_lines(value, step_index, edit):
        step = value["jobs"]["frontend"]["steps"][step_index]
        run = str(step.get("run") or "")
        lines = run.splitlines()
        edit(lines)
        step["run"] = "\n".join(lines) + ("\n" if run.endswith("\n") else "")

    def delete_frontend_command(value, command):
        step_index, line_index = frontend_command(value, command)
        edit_frontend_lines(value, step_index, lambda lines: lines.pop(line_index))

    def duplicate_frontend_command(value, command):
        step_index, line_index = frontend_command(value, command)
        edit_frontend_lines(
            value,
            step_index,
            lambda lines: lines.insert(line_index + 1, lines[line_index]),
        )

    def replace_frontend_command(value, command, replacement):
        step_index, line_index = frontend_command(value, command)

        def replace(lines):
            indentation = lines[line_index][:-len(lines[line_index].lstrip())]
            lines[line_index] = indentation + replacement

        edit_frontend_lines(value, step_index, replace)

    def swap_frontend_commands(value, first, second):
        job_steps = value["jobs"]["frontend"]["steps"]
        first_step, first_line = frontend_command(value, first)
        second_step, second_line = frontend_command(value, second)
        if first_step == second_step:
            def swap(lines):
                lines[first_line], lines[second_line] = (
                    lines[second_line],
                    lines[first_line],
                )

            edit_frontend_lines(value, first_step, swap)
            return
        first_lines = str(job_steps[first_step].get("run") or "").splitlines()
        second_lines = str(job_steps[second_step].get("run") or "").splitlines()
        first_lines[first_line], second_lines[second_line] = (
            second_lines[second_line],
            first_lines[first_line],
        )
        job_steps[first_step]["run"] = "\n".join(first_lines)
        job_steps[second_step]["run"] = "\n".join(second_lines)

    def combined_frontend_fixture(value):
        fixture = copy.deepcopy(value)
        for command in reversed(("npm ci", "npm run build", "npm test")):
            delete_frontend_command(fixture, command)
        fixture["jobs"]["frontend"]["steps"].append(
            {"run": "npm ci\nnpm run build\nnpm test"}
        )
        return fixture

    try:
        combined = combined_frontend_fixture(ci)
        combined_errors = contract_errors(release, nightly, combined)
        combined_held = not combined_errors
        combined_detail = "; ".join(combined_errors)
    except Exception as error:
        combined_held = False
        combined_detail = f"fixture setup failed: {error}"
    print(
        f"{'PASS' if combined_held else 'FAIL'}|"
        "combined multiline Linux frontend commands pass|"
        f"{combined_detail}"
    )
    failures += 0 if combined_held else 1

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
        "missing signing preparation certificate input fails",
        remove_prepare_certificate,
        "declare every certificate, updater, timestamp, and release URL input",
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
        "npm test",
        "CI frontend tests must install requirements-ci.txt",
    )
    rejected_ci(
        "Windows parity test before requirements install fails",
        "windows-native-test",
        "npm run test:native-parity",
        "CI Windows native-parity tests must install requirements-ci.txt",
    )
    rejected_ci_mutation(
        "missing Windows parity command fails",
        lambda value: next(
            step
            for step in value["jobs"]["windows-native-test"]["steps"]
            if "npm run test:native-parity" in str(step.get("run") or "")
        ).update({"run": "npm ci"}),
        "must run exactly once after npm ci",
    )
    rejected_ci_mutation(
        "Windows parity command before npm ci fails",
        lambda value: next(
            step
            for step in value["jobs"]["windows-native-test"]["steps"]
            if "npm run test:native-parity" in str(step.get("run") or "")
        ).update({"run": "npm run test:native-parity\nnpm ci"}),
        "must run exactly once after npm ci",
    )
    rejected_ci_mutation(
        "Windows test timeout above budget fails",
        lambda value: value["jobs"]["windows-test-core"].update({"timeout-minutes": 16}),
        "bounded to at most 15 minutes",
    )
    rejected_ci_mutation(
        "missing Linux frontend install fails",
        lambda value: delete_frontend_command(value, "npm ci"),
        "must run exactly one npm ci, npm run build, and full npm test",
    )
    rejected_ci_mutation(
        "duplicate Linux frontend test fails",
        lambda value: duplicate_frontend_command(value, "npm test"),
        "must run exactly one npm ci, npm run build, and full npm test",
    )
    rejected_ci_mutation(
        "renamed Linux frontend build fails",
        lambda value: replace_frontend_command(value, "npm run build", "npm run bundle"),
        "must run exactly one npm ci, npm run build, and full npm test",
    )
    rejected_ci_mutation(
        "narrowed Linux frontend test fails",
        lambda value: replace_frontend_command(
            value, "npm test", "npm test -- src/App.test.tsx"
        ),
        "must run exactly one npm ci, npm run build, and full npm test",
    )
    rejected_ci_mutation(
        "Linux frontend build before install fails",
        lambda value: swap_frontend_commands(value, "npm ci", "npm run build"),
        "in dependency order",
    )
    rejected_ci_mutation(
        "Linux frontend test before build fails",
        lambda value: swap_frontend_commands(value, "npm run build", "npm test"),
        "in dependency order",
    )
    rejected_ci_mutation(
        "missing Windows launch diagnostics upload fails",
        lambda value: value["jobs"]["windows-package"]["steps"].__setitem__(
            slice(None),
            [
                step
                for step in value["jobs"]["windows-package"]["steps"]
                if (step.get("with") or {}).get("name")
                != "windows-ci-launch-failure-diagnostics"
            ],
        ),
        "must upload one launch diagnostic artifact",
    )
    rejected_ci_mutation(
        "unaddressable Windows evidence step fails",
        lambda value: next(
            step
            for step in value["jobs"]["windows-package"]["steps"]
            if step.get("id") == "installed-evidence"
        ).pop("id"),
        "must upload one launch diagnostic artifact",
    )
    rejected_ci_mutation(
        "always-uploaded Windows launch diagnostics fail",
        lambda value: next(
            step
            for step in value["jobs"]["windows-package"]["steps"]
            if (step.get("with") or {}).get("name")
            == "windows-ci-launch-failure-diagnostics"
        ).update({"if": "always()"}),
        "must be pinned, retained seven days, failure-only, and fail closed",
    )
    rejected_ci_mutation(
        "broad Windows launch diagnostics path fails",
        lambda value: next(
            step
            for step in value["jobs"]["windows-package"]["steps"]
            if (step.get("with") or {}).get("name")
            == "windows-ci-launch-failure-diagnostics"
        )["with"].update({"path": "artifacts"}),
        "must be pinned, retained seven days, failure-only, and fail closed",
    )
    rejected_ci_mutation(
        "unpinned Windows launch diagnostics action fails",
        lambda value: next(
            step
            for step in value["jobs"]["windows-package"]["steps"]
            if (step.get("with") or {}).get("name")
            == "windows-ci-launch-failure-diagnostics"
        ).update({"uses": "actions/upload-artifact@v4"}),
        "must be pinned, retained seven days, failure-only, and fail closed",
    )
    rejected_ci_mutation(
        "wrong Windows launch diagnostics retention fails",
        lambda value: next(
            step
            for step in value["jobs"]["windows-package"]["steps"]
            if (step.get("with") or {}).get("name")
            == "windows-ci-launch-failure-diagnostics"
        )["with"].update({"retention-days": 8}),
        "must be pinned, retained seven days, failure-only, and fail closed",
    )
    rejected_ci_dropped(
        "a CI Windows job that vanished fails",
        "windows-package",
        "CI Windows evidence must install requirements-ci.txt",
    )
    rejected_ci_dropped(
        "missing CI Windows sidecar shard fails",
        "windows-sidecars",
        "CI Windows packaging must consume an x86_64 prebuilt sidecar artifact",
    )
    rejected_nightly_dropped(
        "missing nightly Windows sidecar shard fails",
        "windows-sidecars",
        "nightly Windows packaging must consume an x86_64 prebuilt sidecar artifact",
    )
    rejected(
        "missing release Windows sidecar shard fails",
        lambda value: value["jobs"].pop("windows-sidecars"),
        "release Windows packaging must consume an x86_64 prebuilt sidecar artifact",
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
