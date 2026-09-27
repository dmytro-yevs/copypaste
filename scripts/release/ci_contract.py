"""Small, behavior-based checks for the PR workflow.

The release workflow qualifies installable artifacts. PR CI catches the cheap
regressions first: core Rust, frontend, Android Kotlin, and native boundaries.
"""

import re


def job_steps(job):
    return job.get("steps") or []


def commands(job):
    return "\n".join(str(step.get("run") or "") for step in job_steps(job))


def has_command(job, pattern):
    return re.search(pattern, commands(job), re.MULTILINE) is not None


def ci_rust_toolchain_holds(ci_doc, rust_version):
    failures = []
    for job_name, job in (ci_doc.get("jobs") or {}).items():
        pinned = (job.get("env") or {}).get("RUSTUP_TOOLCHAIN")
        if pinned is not None and str(pinned) != rust_version:
            failures.append(f"{job_name} RUSTUP_TOOLCHAIN")
        for version in re.findall(r"\bcargo\s+\+(\d+\.\d+(?:\.\d+)?)\b", commands(job)):
            if version != rust_version:
                failures.append(f"{job_name} cargo +{version}")
    return not failures, failures


def critical_pr_errors(ci):
    jobs = ci.get("jobs") or {}
    errors = []

    rust = jobs.get("rust") or {}
    if not has_command(rust, r"cargo\s+\+\S+\s+test\s+--workspace\s+--locked"):
        errors.append("PR CI must run the locked core Rust workspace tests")
    if "verify-schema.sh" not in commands(rust):
        errors.append("PR CI must verify the Supabase schema and RLS")

    frontend = jobs.get("frontend") or {}
    frontend_commands = commands(frontend)
    for command in ("npm ci", "npm run build", "npm test"):
        if command not in frontend_commands:
            errors.append(f"PR CI frontend must run {command}")

    android = jobs.get("android-kotlin") or {}
    if "check-android-manifest.sh" not in commands(android) or "testArm64DebugUnitTest" not in commands(android):
        errors.append("PR CI must run Android manifest and Kotlin unit checks")

    macos = jobs.get("macos-native") or {}
    if "build-macos-app.sh" not in commands(macos):
        errors.append("PR CI must build the macOS native app")
    macos_commands = commands(macos)
    if "macos-native-evidence.sh" not in macos_commands:
        errors.append("PR CI must run the macOS product UI smoke")
    macos_steps = str(job_steps(macos))
    if "NSPasteboard smoke" not in macos_steps or "Keychain smoke" not in macos_steps:
        errors.append("PR CI must run macOS pasteboard and keychain API smokes")

    windows = jobs.get("windows-native") or {}
    windows_commands = commands(windows)
    for marker in (
        "pairing_presentation::windows::refusal_tests",
        "crypto::keystore::",
        "npm run test:native-parity",
        "npx vitest run",
        "tests/smoke.e2e.test.ts",
        "tests/history-render.e2e.test.ts",
        "tests/sensitive.e2e.test.ts",
        "tests/windows-surfaces.windows.e2e.test.ts",
    ):
        if marker not in windows_commands:
            errors.append("PR CI must run Windows pairing, DPAPI, native, and product UI checks")
            break

    return errors
