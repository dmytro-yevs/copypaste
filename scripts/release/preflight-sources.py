#!/usr/bin/env python3
"""Validate externally supplied release dependencies before native compilation."""

import json
import os
from pathlib import Path
import runpy

from github_inventory import inventory
from previous_release import resolve


def require_secrets(environment, *, build, publish):
    required = []
    if build:
        required += ["ANDROID_KEYSTORE_BASE64", "ANDROID_KEYSTORE_PASSWORD", "ANDROID_KEY_ALIAS",
                     "ANDROID_KEY_PASSWORD", "TAURI_SIGNING_PRIVATE_KEY",
                     "WINDOWS_SIGNING_CERTIFICATE_BASE64", "WINDOWS_SIGNING_CERTIFICATE_PASSWORD",
                     "WINDOWS_TIMESTAMP_URL"]
    if publish:
        required.append("HOMEBREW_TAP_TOKEN")
    missing = [name for name in required if not environment.get(name)]
    if missing:
        raise ValueError("Missing release signing/publication configuration: " + ", ".join(missing))


def main():
    root = Path(__file__).parent
    environment = os.environ
    require_secrets(environment, build=environment["BUILD"] == "true", publish=environment["PUBLISH"] == "true")
    repository, commit = environment["GITHUB_REPOSITORY"], environment["RELEASE_COMMIT"]
    if environment.get("LINUX_EVIDENCE_RUN_ID") and not environment.get("LINUX_ARTIFACT_RUN_ID"):
        raise ValueError("External Linux evidence requires its original linux_artifact_run_id")
    if environment["BUILD"] == "true":
        source_module = runpy.run_path(str(root / "stage-linux-module-qualification.py"))["source_module"]
        for name in ("supabase", "semantic-search", "ocr"):
            source_module(root.parents[1], name, environment["RELEASE_VERSION"])
    snapshots = {}
    for module in ("supabase", "semantic-search", "ocr"):
        run_id = environment.get(module.replace("-", "_").upper() + "_MODULE_RUN_ID")
        if not run_id:
            continue
        values = [inventory(repository, run_id, kind) for kind in ("run", "jobs", "artifacts")]
        workflow = ".github/workflows/ocr-module.yml" if module == "ocr" else ".github/workflows/provider-module.yml"
        runpy.run_path(str(root / "verify-linux-module-source-run.py"))["verify"](*values, repository, commit, workflow, module)
        snapshots[module] = values[-1]
    run_id = environment.get("LINUX_EVIDENCE_RUN_ID")
    if run_id:
        runpy.run_path(str(root / "verify-linux-qualification-evidence-source.py"))["verify"](
            inventory(repository, run_id, "run"), inventory(repository, run_id, "jobs"), repository, commit)
    runtime = environment.get("COMPOSITOR_RUNTIME_RUN_ID")
    if runtime:
        values = [inventory(repository, runtime, kind) for kind in ("run", "jobs", "artifacts")]
        runpy.run_path(str(root / "verify-compositor-runtime-source-run.py"))["verify"](*values, repository, commit)
    previous = environment.get("PREVIOUS_LINUX_ARTIFACT_RUN_ID", "")
    if previous:
        values = [inventory(repository, previous, kind) for kind in ("run", "jobs", "artifacts")]
        runpy.run_path(str(root / "verify-linux-artifact-source-run.py"))["verify"](*values, repository, values[0]["head_sha"])
    elif environment["BUILD"] == "true" and environment.get("LINUX_REQUIRED") == "true":
        previous = resolve(repository, environment["RELEASE_VERSION"], Path(environment["RUNNER_TEMP"]))
    with open(environment["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        output.write("previous_linux_artifact_run_id=" + previous + "\n")
    output = Path(environment["RUNNER_TEMP"]) / "release-source-artifacts.json"
    output.write_text(json.dumps(snapshots, indent=2) + "\n", encoding="utf-8")
    print("External dependency identities and required signing configuration are valid")


if __name__ == "__main__":
    main()
