import os
import pathlib
import re
import shlex

import yaml


PLATFORMS = {"android", "macos", "windows"}
VISUAL_PLATFORMS = {"android", "macos"}
SCENARIO_SUFFIXES = {".py", ".ps1", ".sh"}
STATE = re.compile(r"[a-z0-9][a-z0-9_-]*\Z")


def repo_file(root, value):
    if not isinstance(value, str) or not value:
        raise ValueError("evidence path is missing")
    relative = pathlib.PurePosixPath(value.removeprefix("./"))
    if relative.is_absolute() or ".." in relative.parts or "\\" in value:
        raise ValueError("evidence path must stay inside the repository")
    try:
        file = (root / pathlib.Path(*relative.parts)).resolve(strict=True)
        file.relative_to(root.resolve(strict=True))
    except (OSError, ValueError):
        raise ValueError(f"evidence file does not exist: {value}") from None
    if not file.is_file():
        raise ValueError(f"evidence path is not a file: {value}")
    return file, relative


def string_list_errors(value, label, pattern=None):
    if not isinstance(value, list) or not value:
        return [f"{label} must be a nonempty list"]
    errors = []
    if any(not isinstance(item, str) or not item.strip() for item in value):
        errors.append(f"{label} entries must be nonempty strings")
        return errors
    if len(value) != len(set(value)):
        errors.append(f"{label} entries must be unique")
    if pattern and any(not pattern.fullmatch(item) for item in value):
        errors.append(f"{label} entries have an invalid identifier")
    return errors


def evidence_state_ids(platform, value):
    if not isinstance(value, list):
        return []
    if platform in VISUAL_PLATFORMS:
        return [
            item["state"] for item in value
            if isinstance(item, dict) and isinstance(item.get("state"), str)
        ]
    return [item for item in value if isinstance(item, str)]


def visual_state_errors(value, label):
    if not isinstance(value, list) or not value:
        return [f"{label} must be a nonempty list"]
    errors = []
    states = []
    paths = []
    required = {"state", "screenshot", "accessibility"}
    for item in value:
        if not isinstance(item, dict) or set(item) != required:
            errors.append(f"{label} entries must name state, screenshot, and accessibility")
            continue
        state = item["state"]
        if not isinstance(state, str) or not STATE.fullmatch(state):
            errors.append(f"{label} entries have an invalid identifier")
        else:
            states.append(state)
        for field in ("screenshot", "accessibility"):
            path = item[field]
            if not isinstance(path, str) or not path:
                errors.append(f"{label} {field} paths must be nonempty strings")
            else:
                paths.append(path)
    if len(states) != len(set(states)):
        errors.append(f"{label} states must be unique")
    if len(paths) != len(set(paths)):
        errors.append(f"{label} artifact paths must be unique")
    return errors


def receipt_expectations(document, uploads):
    expected = {platform: [] for platform in sorted(PLATFORMS)}
    artifact_paths = {platform: set() for platform in VISUAL_PLATFORMS}
    features = document.get("features") if isinstance(document, dict) else None
    for feature in features if isinstance(features, list) else []:
        if not isinstance(feature, dict) or feature.get("status") != "product":
            continue
        feature_id = feature.get("id")
        native = feature.get("native")
        for platform in sorted(PLATFORMS):
            record = native.get(platform) if isinstance(native, dict) else None
            states = record.get("evidence_states") if isinstance(record, dict) else None
            for state_record in states if isinstance(states, list) else []:
                state = (
                    state_record.get("state")
                    if platform in VISUAL_PLATFORMS and isinstance(state_record, dict)
                    else state_record
                )
                if isinstance(feature_id, str) and isinstance(state, str):
                    expectation = {"feature_id": feature_id, "state": state}
                    if platform in VISUAL_PLATFORMS:
                        producers = uploads.get(record.get("release_artifact"), [])
                        roots = producers[0]["roots"] if len(producers) == 1 else []
                        screenshot = _relative_to_upload(
                            _artifact_path(state_record.get("screenshot"), {".png"}), roots
                        )
                        accessibility = _relative_to_upload(
                            _artifact_path(
                                state_record.get("accessibility"),
                                {".json", ".log", ".txt", ".xml"},
                            ),
                            roots,
                        )
                        if screenshot is None or accessibility is None:
                            raise ValueError("feature-state evidence is outside its release artifact")
                        for path in (screenshot.as_posix(), accessibility.as_posix()):
                            if path in artifact_paths[platform]:
                                raise ValueError(
                                    f"{platform} feature-state evidence reuses an artifact path: {path}"
                                )
                            artifact_paths[platform].add(path)
                        expectation.update({
                            "screenshot": screenshot.as_posix(),
                            "accessibility": accessibility.as_posix(),
                        })
                    expected[platform].append(expectation)
    for states in expected.values():
        states.sort(key=lambda value: (value["feature_id"], value["state"]))
    return expected


def receipt_expectation_tokens(document, uploads):
    expected = receipt_expectations(document, uploads)
    tokens = []
    for platform in sorted(expected):
        for record in expected[platform]:
            token = f'{platform}:{record["feature_id"]}={record["state"]}'
            if platform in {"android", "macos"}:
                token += (
                    f',screenshot={record["screenshot"]}'
                    f',accessibility={record["accessibility"]}'
                )
            tokens.append(token)
    return tokens


def _scenario(root, value):
    if not isinstance(value, str) or not value.strip():
        raise ValueError("scenario must be an executable command")
    try:
        argv = shlex.split(value)
    except ValueError as error:
        raise ValueError(f"scenario command is invalid: {error}") from None
    if not argv or any(token in {"&&", ";", "|", ">", ">>"} for token in argv):
        raise ValueError("scenario must name one executable without shell composition")
    file, relative = repo_file(root, argv[0])
    if file.suffix.lower() not in SCENARIO_SUFFIXES:
        raise ValueError("scenario must be an executable .sh, .py, or .ps1 file")
    if file.suffix.lower() != ".ps1" and not os.access(file, os.X_OK):
        raise ValueError(f"scenario is not executable: {relative.as_posix()}")
    return file, relative


def workflow_contract(root):
    workflow, _ = repo_file(root, ".github/workflows/release.yml")
    try:
        document = yaml.safe_load(workflow.read_text(encoding="utf-8"))
    except (OSError, yaml.YAMLError) as error:
        raise ValueError(f"release workflow is not readable YAML: {error}") from None
    jobs = document.get("jobs") if isinstance(document, dict) else None
    if not isinstance(jobs, dict):
        raise ValueError("release workflow has no jobs")
    downloads = set()
    for job_name, job in jobs.items():
        for step in (job or {}).get("steps") or []:
            if str(step.get("uses") or "").startswith("actions/download-artifact"):
                name = (step.get("with") or {}).get("name")
                if isinstance(name, str):
                    downloads.add(name)
    artifacts = {}
    for job_name, job in jobs.items():
        job = job or {}
        for step in job.get("steps") or []:
            if not str(step.get("uses") or "").startswith("actions/upload-artifact"):
                continue
            settings = step.get("with") or {}
            name = settings.get("name")
            path = settings.get("path")
            if not isinstance(name, str) or not isinstance(path, str):
                continue
            roots = []
            for line in path.splitlines():
                candidate = line.strip()
                if candidate and not any(mark in candidate for mark in "*?!${}"):
                    roots.append(pathlib.PurePosixPath(candidate.removeprefix("./")))
            artifacts.setdefault(name, []).append(
                {
                    "job": job_name,
                    "roots": roots,
                    "strict": settings.get("if-no-files-found") == "error" or name in downloads,
                }
            )
    return artifacts


def _run_commands(step):
    command = str(step.get("run") or "") if isinstance(step, dict) else ""
    for line in command.splitlines():
        try:
            argv = shlex.split(line.strip(), comments=True)
        except ValueError:
            continue
        if argv:
            yield argv


def _installs_requirements(argv):
    return (
        argv[:4] in (["python3", "-m", "pip", "install"], ["python", "-m", "pip", "install"])
        and "--requirement" in argv
        and "requirements-ci.txt" in argv
    )


def ledger_dependency_errors(root):
    errors = []
    workflows = root / ".github/workflows"
    for path in sorted([*workflows.glob("*.yml"), *workflows.glob("*.yaml")]):
        try:
            document = yaml.safe_load(path.read_text(encoding="utf-8"))
        except (OSError, yaml.YAMLError):
            continue
        jobs = document.get("jobs") if isinstance(document, dict) else None
        job_items = jobs.items() if isinstance(jobs, dict) else []
        for job_name, job in job_items:
            installed = False
            for step in (job or {}).get("steps") or []:
                for argv in _run_commands(step):
                    installed = installed or _installs_requirements(argv)
                    if len(argv) >= 2 and argv[1] == "scripts/check-feature-ledger.py" and not installed:
                        errors.append(
                            f"{path.name}/{job_name} runs feature ledger before requirements-ci.txt is installed"
                        )
    return errors


def release_gate_errors(root):
    workflow, _ = repo_file(root, ".github/workflows/release.yml")
    try:
        document = yaml.safe_load(workflow.read_text(encoding="utf-8"))
    except (OSError, yaml.YAMLError) as error:
        return [f"release workflow is not readable YAML: {error}"]
    gate = ((document.get("jobs") or {}).get("native-parity") or {}) if isinstance(document, dict) else {}
    if "continue-on-error" in gate:
        return ["release native-parity must not continue after a failed complete-evidence gate"]
    installed = False
    matches = []
    for step in gate.get("steps") or []:
        for argv in _run_commands(step):
            if _installs_requirements(argv):
                installed = True
            if argv == [
                "python3", "scripts/check-feature-ledger.py", "--require-complete",
                "--version", "$RELEASE_VERSION",
            ]:
                matches.append(step)
                if not installed:
                    return ["release feature-ledger gate runs before its declared Python dependencies"]
    if len(matches) != 1:
        return ["release native-parity must use one exact version-bound complete-evidence gate"]
    if "if" in matches[0]:
        return ["release native-parity complete-evidence gate must run unconditionally"]
    if "continue-on-error" in matches[0]:
        return ["release native-parity complete-evidence gate must not continue after failure"]
    if (matches[0].get("env") or {}).get("RELEASE_VERSION") != "${{ needs.version.outputs.version }}":
        return ["release native-parity must bind the complete-evidence gate to the resolved version"]
    return []


def _artifact_path(value, suffixes):
    if not isinstance(value, str) or not value:
        raise ValueError("runtime evidence path is missing")
    path = pathlib.PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts or "\\" in value:
        raise ValueError("runtime evidence path must stay inside the artifact")
    if path.suffix.lower() not in suffixes:
        raise ValueError(f"runtime evidence must use {', '.join(sorted(suffixes))}")
    return path


def _relative_to_upload(path, roots):
    for root in roots:
        try:
            return path.relative_to(root)
        except ValueError:
            continue
    return None


def _verified_errors(feature_id, platform, record, release_evidence, uploads):
    label = f"{feature_id}: {platform}"
    errors = []
    artifact_name = record.get("release_artifact")
    if not isinstance(artifact_name, str) or artifact_name not in release_evidence:
        errors.append(f"{label} release_artifact is not owned by the feature")
        return errors
    producers = uploads.get(artifact_name, [])
    if not producers:
        return [f"{label} release_artifact does not exist in release.yml"]
    if len(producers) != 1:
        return [f"{label} release_artifact has multiple producers"]
    if not any(producer["strict"] for producer in producers):
        errors.append(f"{label} release_artifact may be absent without failing release")
    if platform in VISUAL_PLATFORMS:
        for state in record.get("evidence_states", []):
            if not isinstance(state, dict):
                continue
            state_label = f"{label} {state.get('state', '<missing state>')}"
            try:
                screenshot = _artifact_path(state.get("screenshot"), {".png"})
                accessibility = _artifact_path(
                    state.get("accessibility"), {".json", ".log", ".txt", ".xml"}
                )
            except ValueError as error:
                errors.append(f"{state_label} {error}")
                continue
            for field, path in (("screenshot", screenshot), ("accessibility", accessibility)):
                relative = _relative_to_upload(path, producers[0]["roots"])
                if relative is None:
                    errors.append(
                        f"{state_label} {field} is outside the uploaded release artifact"
                    )
    return errors


def native_errors(feature, root, require_complete=False, uploads=None):
    feature_id = feature.get("id", "<missing id>")
    native = feature.get("native")
    if not isinstance(native, dict) or set(native) != PLATFORMS:
        return [f"{feature_id}: native must distinguish android, macos, and windows"], []
    if uploads is None:
        try:
            uploads = workflow_contract(root)
        except ValueError as error:
            return [str(error)], []
    release_evidence = feature.get("release_evidence")
    release_names = {
        name for name in release_evidence if isinstance(name, str)
    } if isinstance(release_evidence, list) else set()
    errors = []
    pending = []
    allowed = {"scenario", "evidence_status", "evidence_states", "unverified_states", "release_artifact"}
    for platform in sorted(PLATFORMS):
        record = native[platform]
        label = f"{feature_id}: {platform}"
        if not isinstance(record, dict):
            errors.append(f"{label} evidence record must be an object")
            continue
        unknown = set(record) - allowed
        if unknown:
            errors.append(f"{label} evidence has unknown fields: {', '.join(sorted(unknown))}")
        status = record.get("evidence_status")
        if status not in {"verified", "partial", "pending"}:
            errors.append(f"{label} evidence_status must be verified, partial, or pending")
            continue
        try:
            _scenario(root, record.get("scenario"))
        except ValueError as error:
            errors.append(f"{label} {error}")
            continue
        verified = record.get("evidence_states")
        unverified = record.get("unverified_states")
        if status in {"verified", "partial"}:
            if platform in VISUAL_PLATFORMS:
                errors.extend(visual_state_errors(verified, f"{label} evidence_states"))
            else:
                errors.extend(string_list_errors(verified, f"{label} evidence_states", STATE))
        elif "evidence_states" in record:
            errors.append(f"{label} pending evidence cannot claim evidence_states")
        if status in {"partial", "pending"}:
            errors.extend(string_list_errors(unverified, f"{label} unverified_states", STATE))
            if isinstance(unverified, list):
                pending.extend(f"{feature_id}/{platform}/{state}" for state in unverified)
        elif "unverified_states" in record:
            errors.append(f"{label} verified evidence cannot contain unverified_states")
        verified_ids = evidence_state_ids(platform, verified)
        if isinstance(unverified, list) and set(verified_ids) & {
            state for state in unverified if isinstance(state, str)
        }:
            errors.append(f"{label} verified and unverified states overlap")
        if status == "pending":
            forbidden = {"release_artifact"} & set(record)
            if forbidden:
                errors.append(f"{label} pending evidence cannot cite unproduced artifacts")
        else:
            errors.extend(
                _verified_errors(feature_id, platform, record, release_names, uploads)
            )
    for name in release_names:
        if name not in uploads:
            errors.append(f"{feature_id}: release_evidence does not exist in release.yml: {name}")
    if require_complete and pending:
        errors.append(f"{feature_id}: release evidence is pending: {', '.join(sorted(pending))}")
    return errors, pending
