import collections
import re
import shlex


def job_steps(job):
    return job.get("steps") or []


def windows_workspace_shards_hold(jobs, expected_packages, rust_version):
    test_names = ("windows-test-core", "windows-test-services", "windows-test-apps")
    names = ("windows-clippy", *test_names, "windows-native-test")
    shards = {name: jobs.get(name) or {} for name in names}
    bodies = {
        name: "\n".join(str(step.get("run") or "") for step in job_steps(job))
        for name, job in shards.items()
    }
    caches = {
        name: [
            step
            for step in job_steps(job)
            if str(step.get("uses") or "").startswith("Swatinem/rust-cache")
        ]
        for name, job in shards.items()
    }
    selected = []
    valid_commands = True
    for name in test_names:
        commands = []
        for line in bodies[name].splitlines():
            try:
                argv = tuple(shlex.split(line.strip()))
            except ValueError:
                continue
            if len(argv) >= 3 and argv[:3] == ("cargo", "+" + rust_version, "test"):
                commands.append(argv)
        if len(commands) != 1:
            valid_commands = False
            continue
        command = commands[0]
        if "--locked" not in command or any(
            flag in command
            for flag in (
                "--workspace",
                "--exclude",
                "--no-default-features",
                "--features",
                "--all-features",
            )
        ):
            valid_commands = False
        selected += [
            command[index + 1]
            for index, token in enumerate(command[:-1])
            if token in ("-p", "--package")
        ]
    cache_keys = [
        str((caches[name][0].get("with") or {}).get("shared-key", ""))
        for name in names
        if len(caches[name]) == 1
    ]
    clippy = "cargo +{} clippy --workspace --all-targets --locked -- -D warnings".format(
        rust_version
    )
    workspace_test = "cargo +{} test --workspace --locked".format(rust_version)
    return (
        shards["windows-clippy"].get("timeout-minutes") == 20
        and shards["windows-native-test"].get("timeout-minutes") == 20
        and all(shards[name].get("timeout-minutes") == 15 for name in test_names)
        and all(len(caches[name]) == 1 for name in names)
        and len(cache_keys) == len(names) == len(set(cache_keys))
        and all(
            (caches[name][0].get("with") or {}).get("save-if") is True
            for name in (*test_names, "windows-native-test")
        )
        and valid_commands
        and set(selected) == expected_packages
        and len(selected) == len(expected_packages)
        and clippy in bodies["windows-clippy"]
        and "pairing_presentation::windows::refusal_tests" in bodies["windows-native-test"]
        and "crypto::keystore::" in bodies["windows-native-test"]
        and "test:native-parity" in bodies["windows-native-test"]
        and all(workspace_test not in body for body in bodies.values())
    )


def command_invocations(source):
    invocations = []
    for line in source.splitlines():
        lexer = shlex.shlex(line, posix=True)
        lexer.commenters = "#"
        lexer.whitespace_split = True
        try:
            words = list(lexer)
        except ValueError:
            continue
        if (
            len(words) >= 4
            and words[:2] == ["python3", "scripts/ci/run-gates.py"]
            and words[2] in ("--gate", "--profile")
        ):
            invocations.append((words[2][2:], words[3]))
    return invocations


def portable_gate_contract_holds(registry, profile_name, local_source, ci_jobs):
    gates = registry.get("gates") or {}
    profile = (registry.get("profiles") or {}).get(profile_name)
    if not isinstance(profile, list) or len(profile) != len(set(profile)):
        return False
    if set(profile) - set(gates):
        return False
    if command_invocations(local_source) != [("profile", profile_name)]:
        return False
    ci_invocations = []
    for job in ci_jobs.values():
        for step in job_steps(job):
            ci_invocations.extend(command_invocations(str(step.get("run") or "")))
    selected = [value for kind, value in ci_invocations if kind == "gate"]
    if collections.Counter(selected) != collections.Counter(profile):
        return False
    file_size = (gates.get("file-size-budget") or {}).get("commands")
    ledger = (gates.get("feature-ledger") or {}).get("commands")
    return file_size == [
        ["bash", "scripts/check-file-size-gate.sh", "--self-test"],
        ["bash", "scripts/check-file-size-gate.sh"],
    ] and ledger == [
        ["python3", "scripts/check-feature-ledger.py", "--self-test"],
        ["python3", "scripts/check-feature-ledger.py"],
    ]


def ci_rust_toolchain_holds(ci_doc, rust_version):
    failures = []
    if str((ci_doc.get("env") or {}).get("MSRV")) != rust_version:
        failures.append("workflow MSRV")
    for job_name, job in (ci_doc.get("jobs") or {}).items():
        pinned = (job.get("env") or {}).get("RUSTUP_TOOLCHAIN")
        if pinned is not None and str(pinned) != rust_version:
            failures.append("{} RUSTUP_TOOLCHAIN".format(job_name))
        for step in job_steps(job):
            action = str(step.get("uses") or "")
            selected = (step.get("with") or {}).get("toolchain")
            if action.startswith("dtolnay/rust-toolchain") and selected is not None:
                value = str(selected)
                if "${{" not in value and value != rust_version:
                    failures.append("{} toolchain action".format(job_name))
            for version in re.findall(r"\bcargo\s+\+(\d+\.\d+(?:\.\d+)?)\b", str(step.get("run") or "")):
                if version != rust_version:
                    failures.append("{} cargo +{}".format(job_name, version))
    return not failures, failures
