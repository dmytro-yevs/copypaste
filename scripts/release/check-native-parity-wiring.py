#!/usr/bin/env python3
import pathlib
import sys

import yaml


ROOT = pathlib.Path(__file__).resolve().parents[2]
FAIL = 0


def record(held, description, detail):
    global FAIL
    print(f"{'PASS' if held else 'FAIL'}|{description}|{detail if not held else ''}")
    if not held:
        FAIL += 1


def load(name):
    with (ROOT / ".github" / "workflows" / name).open(encoding="utf-8") as stream:
        return yaml.safe_load(stream)


def steps(job):
    return job.get("steps") or []


release = load("release.yml")
jobs = release.get("jobs") or {}
gate = jobs.get("native-parity") or {}
publish = jobs.get("publish") or {}
gate_needs = set(gate.get("needs") or [])
publish_needs = set(publish.get("needs") or [])
record(
    {"macos", "android-hardware"} <= gate_needs,
    "native parity waits for macOS and physical Android evidence",
    f"native-parity needs {sorted(gate_needs)}",
)
record(
    "native-parity" in publish_needs,
    "publication waits for the native parity gate",
    f"publish needs {sorted(publish_needs)}",
)

downloads = {
    (step.get("with") or {}).get("name")
    for step in steps(gate)
    if str(step.get("uses") or "").startswith("actions/download-artifact")
}
record(
    {"release-macos-native-evidence", "release-android-hardware-evidence"} <= downloads,
    "native parity downloads both release evidence artifacts",
    f"downloads {sorted(str(item) for item in downloads)}",
)

gate_commands = "\n".join(str(step.get("run") or "") for step in steps(gate))
record(
    "--require macos,android" in gate_commands
    and "--run-id ${{ github.run_id }}" in gate_commands
    and "artifacts/native-parity/macos/native-evidence.json" in gate_commands
    and "artifacts/native-parity/android/release-android-hardware/native-evidence.json"
    in gate_commands
    and gate_commands.count("native-evidence.json") == 2,
    "release gate requires exactly the two native receipts",
    "expected the platform set, workflow run ID, and exact macOS and Android receipt paths",
)

hardware_uploads = [
    step for step in steps(jobs.get("android-hardware") or {})
    if str(step.get("uses") or "").startswith("actions/upload-artifact")
]
record(
    len(hardware_uploads) == 1
    and (hardware_uploads[0].get("with") or {}).get("if-no-files-found") == "error",
    "physical Android evidence upload fails closed",
    "the hardware artifact may be absent without failing",
)

nightly = load("native-nightly.yml")
triggers = nightly.get(True) or nightly.get("on") or {}
windows_input = ((triggers.get("workflow_dispatch") or {}).get("inputs") or {}).get("windows_evidence") or {}
windows = (nightly.get("jobs") or {}).get("windows") or {}
windows_commands = "\n".join(str(step.get("run") or "") for step in steps(windows))
record(
    windows_input.get("type") == "boolean" and windows_input.get("default") is False,
    "Windows evidence is opt-in on manual dispatch",
    f"windows_evidence input is {windows_input!r}",
)
record(
    "inputs.windows_evidence" in str(windows.get("if") or "")
    and "--require windows" in windows_commands
    and "--run-id ${{ github.run_id }}" in windows_commands,
    "requested Windows evidence runs its own receipt gate",
    "the Windows job is not conditional or does not validate its run-bound receipt",
)

for script in ("macos-native-evidence.sh", "android-smoke-release.sh"):
    body = (ROOT / "scripts" / "release" / script).read_text(encoding="utf-8")
    record(
        "write-native-evidence.py" in body,
        f"{script} writes a native evidence receipt",
        "the producer cannot feed the gate",
    )

sys.exit(1 if FAIL else 0)
