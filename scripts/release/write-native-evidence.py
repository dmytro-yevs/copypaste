#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile

from native_evidence_policy import load_policy
from png_evidence import validate_png


POLICY = load_policy()
KINDS = set(POLICY["artifact_kinds"])
PLATFORMS = POLICY["platforms"]
FEATURE_STATE_PATTERN = re.compile(r"^[a-z0-9][a-z0-9_-]*$")


def parser():
    result = argparse.ArgumentParser()
    result.add_argument("--output", required=True)
    result.add_argument("--platform", choices=tuple(PLATFORMS), required=True)
    result.add_argument(
        "--environment",
        choices=tuple(POLICY["environments"]),
        required=True,
    )
    result.add_argument("--os-version", required=True)
    result.add_argument("--architecture", required=True)
    result.add_argument("--commit", default=os.environ.get("GITHUB_SHA"))
    result.add_argument("--run-id", default=os.environ.get("GITHUB_RUN_ID"))
    result.add_argument("--scenario")
    result.add_argument("--elapsed-ms", type=int, required=True)
    result.add_argument("--budget-ms", type=int)
    result.add_argument("--assertion", action="append", default=[])
    result.add_argument("--artifact", action="append", default=[])
    result.add_argument("--feature-state", action="append", default=[])
    return result


def artifact_record(root, value):
    try:
        kind, relative = value.split("=", 1)
    except ValueError as error:
        raise ValueError("artifact must use KIND=RELATIVE_PATH") from error
    if kind not in KINDS:
        raise ValueError(f"unknown artifact kind {kind}")
    relative_path = pathlib.PurePosixPath(relative)
    if relative_path.is_absolute() or ".." in relative_path.parts or "\\" in relative:
        raise ValueError("artifact paths must stay relative to the receipt")
    try:
        file = (root / pathlib.Path(*relative_path.parts)).resolve(strict=True)
    except OSError:
        raise ValueError("artifact cannot be read") from None
    try:
        file.relative_to(root.resolve(strict=True))
    except ValueError:
        raise ValueError("artifact escapes its evidence directory") from None
    if not file.is_file() or file.stat().st_size == 0:
        raise ValueError("artifact must be a non-empty regular file")
    if kind == "screenshot":
        validate_png(file)
    return {
        "kind": kind,
        "path": relative_path.as_posix(),
        "sha256": hashlib.sha256(file.read_bytes()).hexdigest(),
        "bytes": file.stat().st_size,
    }


def feature_state_record(value, requirement, artifacts):
    parts = value.split(",")
    try:
        feature_id, state = parts[0].split("=", 1)
    except ValueError:
        raise ValueError("feature state must use FEATURE_ID=STATE") from None
    if not FEATURE_STATE_PATTERN.fullmatch(feature_id) or not FEATURE_STATE_PATTERN.fullmatch(state):
        raise ValueError("feature state identifiers are invalid")
    bindings = {}
    for part in parts[1:]:
        try:
            kind, relative = part.split("=", 1)
        except ValueError:
            raise ValueError("feature state artifact must use KIND=RELATIVE_PATH") from None
        if kind in bindings:
            raise ValueError("feature state repeats an artifact binding")
        bindings[kind] = relative
    required = set(requirement["feature_state_artifacts"])
    if set(bindings) != required:
        raise ValueError("feature state artifact bindings contradict native evidence policy")
    record = {"feature_id": feature_id, "state": state}
    for kind, relative in bindings.items():
        artifact = next(
            (
                candidate for candidate in artifacts
                if candidate["kind"] == kind and candidate["path"] == relative
            ),
            None,
        )
        if artifact is None:
            raise ValueError(f"feature state {kind} does not name a declared artifact")
        record[kind] = {
            "path": artifact["path"],
            "sha256": artifact["sha256"],
            "bytes": artifact["bytes"],
        }
    return record


def main():
    args = parser().parse_args()
    requirement = PLATFORMS[args.platform]
    if not args.commit or len(args.commit) != 40 or any(c not in "0123456789abcdef" for c in args.commit):
        raise SystemExit("write-native-evidence: --commit must be a lowercase 40-character Git SHA")
    if not args.run_id:
        raise SystemExit("write-native-evidence: --run-id is required outside GitHub Actions")
    if args.elapsed_ms < 0:
        raise SystemExit("write-native-evidence: latency values are invalid")
    if args.elapsed_ms > requirement["budget_ms"]:
        raise SystemExit(f"write-native-evidence: {args.platform} latency exceeds native evidence policy")
    if args.environment != requirement["environment"]:
        raise SystemExit(f"write-native-evidence: {args.platform} release evidence requires {requirement['environment']}")
    if args.scenario is not None and args.scenario != requirement["scenario"]:
        raise SystemExit(f"write-native-evidence: {args.platform} scenario contradicts native evidence policy")
    if args.budget_ms is not None and args.budget_ms != requirement["budget_ms"]:
        raise SystemExit(f"write-native-evidence: {args.platform} budget contradicts native evidence policy")
    if args.assertion and set(args.assertion) != set(requirement["assertions"]):
        raise SystemExit(f"write-native-evidence: {args.platform} assertions contradict native evidence policy")
    if not args.artifact:
        raise SystemExit("write-native-evidence: artifacts are required")

    output = pathlib.Path(args.output)
    try:
        output.parent.mkdir(parents=True, exist_ok=True)
    except OSError:
        raise SystemExit("write-native-evidence: evidence directory is unavailable") from None
    try:
        artifacts = [artifact_record(output.parent, item) for item in args.artifact]
    except ValueError as error:
        raise SystemExit(f"write-native-evidence: {error}") from None
    artifact_policy = requirement["artifacts"]
    kinds = [artifact["kind"] for artifact in artifacts]
    allowed = set(artifact_policy["required"]) | set(artifact_policy["optional"])
    missing = set(artifact_policy["required"]) - set(kinds)
    unexpected = set(kinds) - allowed
    duplicates = {kind for kind in kinds if kinds.count(kind) > 1} - set(artifact_policy["repeatable"])
    if missing or unexpected or duplicates:
        raise SystemExit("write-native-evidence: artifact set contradicts native evidence policy")

    try:
        feature_states = [
            feature_state_record(value, requirement, artifacts)
            for value in args.feature_state
        ]
    except ValueError as error:
        raise SystemExit(f"write-native-evidence: {error}") from None
    identities = {(item["feature_id"], item["state"]) for item in feature_states}
    if len(identities) != len(feature_states):
        raise SystemExit("write-native-evidence: duplicate feature state")
    screenshot_identities = [
        item["screenshot"]["sha256"] for item in feature_states if "screenshot" in item
    ]
    if len(screenshot_identities) != len(set(screenshot_identities)):
        raise SystemExit("write-native-evidence: feature states reuse screenshot evidence")

    receipt = {
        "schema_version": POLICY["receipt_schema_version"],
        "platform": args.platform,
        "environment": args.environment,
        "os_version": args.os_version,
        "architecture": args.architecture,
        "source": {"commit": args.commit, "run_id": args.run_id},
        "scenario": {
            "name": requirement["scenario"],
            "elapsed_ms": args.elapsed_ms,
            "budget_ms": requirement["budget_ms"],
        },
        "assertions": requirement["assertions"],
        "artifacts": artifacts,
    }
    if feature_states:
        receipt["feature_states"] = feature_states
    try:
        with tempfile.NamedTemporaryFile("w", dir=output.parent, delete=False, encoding="utf-8") as temp:
            json.dump(receipt, temp, indent=2)
            temp.write("\n")
            temporary = pathlib.Path(temp.name)
        temporary.replace(output)
    except OSError:
        raise SystemExit("write-native-evidence: receipt could not be written") from None


def self_test():
    with tempfile.TemporaryDirectory() as directory:
        from PIL import Image, ImageDraw

        root = pathlib.Path(directory)
        (root / "accessibility.txt").write_text("named native accessibility node\n")
        (root / "measurement.json").write_text('{"elapsed_ms":1}\n')
        good = root / "good.png"
        image = Image.new("RGB", (2, 2), (220, 38, 38))
        image.putpixel((1, 1), (255, 255, 255))
        image.save(good)
        black = root / "black.png"
        Image.new("RGB", (8, 8), "black").save(black)
        white = root / "white.png"
        Image.new("RGB", (8, 8), "white").save(white)
        near_black = root / "near-black-checker.png"
        image = Image.new("RGB", (100, 100), "black")
        for y in range(100):
            for x in range(100):
                value = 8 + ((x + y) % 2)
                image.putpixel((x, y), (value, value, value))
        image.save(near_black)
        sparse = root / "isolated-one-percent.png"
        image = Image.new("RGB", (100, 100), "black")
        for y in range(5, 100, 10):
            for x in range(5, 100, 10):
                image.putpixel((x, y), (255, 255, 255))
        image.save(sparse)
        transparent = root / "transparent-hidden-rgb.png"
        image = Image.new("RGBA", (100, 100), (255, 0, 0, 0))
        ImageDraw.Draw(image).rectangle((50, 0, 99, 99), fill=(0, 255, 0, 0))
        image.save(transparent)
        chunk_corrupt = bytearray(good.read_bytes())
        chunk_corrupt[chunk_corrupt.index(b"IDAT") + 4] ^= 0xFF
        fixtures = {
            "missing.png": None,
            "empty.png": b"",
            "signature-truncated.png": good.read_bytes()[:24],
            "chunk-corrupt.png": bytes(chunk_corrupt),
            "black.png": black.read_bytes(),
            "white.png": white.read_bytes(),
            "near-black-checker.png": near_black.read_bytes(),
            "isolated-one-percent.png": sparse.read_bytes(),
            "transparent-hidden-rgb.png": transparent.read_bytes(),
            "good.png": good.read_bytes(),
        }
        common = [
            sys.executable, __file__, "--platform", "android",
            "--environment", "physical-device", "--os-version", "API 33",
            "--architecture", "x86_64", "--commit", "a" * 40,
            "--run-id", "self-test", "--elapsed-ms", "1",
        ]
        for name, content in fixtures.items():
            screenshot = root / name
            if content is not None:
                screenshot.write_bytes(content)
            receipt = root / f"{name}.json"
            result = subprocess.run(
                common + [
                    "--output", os.fspath(receipt),
                    "--artifact", f"screenshot={name}",
                    "--artifact", "accessibility=accessibility.txt",
                    "--artifact", "measurement=measurement.json",
                    "--feature-state",
                    f"devices=scan-pairing-code,screenshot={name},accessibility=accessibility.txt",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            if name == "good.png":
                if result.returncode != 0 or not receipt.is_file():
                    raise SystemExit(f"valid PNG self-test failed: {result.stderr.strip()}")
            elif result.returncode == 0 or receipt.exists():
                raise SystemExit(f"{name} produced a native evidence receipt")
        emulator = list(common)
        emulator[emulator.index("physical-device")] = "emulator"
        emulator_receipt = root / "emulator.json"
        result = subprocess.run(
            emulator + [
                "--output", os.fspath(emulator_receipt),
                "--artifact", "screenshot=good.png",
                "--artifact", "accessibility=accessibility.txt",
                "--artifact", "measurement=measurement.json",
                "--feature-state",
                "devices=scan-pairing-code,screenshot=good.png,accessibility=accessibility.txt",
            ],
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode == 0 or emulator_receipt.exists():
            raise SystemExit("emulator produced a physical Android publication receipt")
        label_only_receipt = root / "label-only.json"
        result = subprocess.run(
            common + [
                "--output", os.fspath(label_only_receipt),
                "--artifact", "screenshot=good.png",
                "--artifact", "accessibility=accessibility.txt",
                "--artifact", "measurement=measurement.json",
                "--feature-state", "devices=scan-pairing-code",
            ],
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode == 0 or label_only_receipt.exists():
            raise SystemExit("label-only Android feature state produced a receipt")
        second = root / "second.png"
        image = Image.new("RGB", (2, 2), (24, 120, 220))
        image.putpixel((0, 1), (240, 180, 20))
        image.save(second)
        (root / "second-accessibility.txt").write_text("second named state\n")
        multi_receipt = root / "multiple-states.json"
        result = subprocess.run(
            common + [
                "--output", os.fspath(multi_receipt),
                "--artifact", "screenshot=good.png",
                "--artifact", "accessibility=accessibility.txt",
                "--artifact", "screenshot=second.png",
                "--artifact", "accessibility=second-accessibility.txt",
                "--artifact", "measurement=measurement.json",
                "--feature-state",
                "devices=scan-pairing-code,screenshot=good.png,accessibility=accessibility.txt",
                "--feature-state",
                "history=populated,screenshot=second.png,accessibility=second-accessibility.txt",
            ],
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0 or not multi_receipt.is_file():
            raise SystemExit(f"distinct feature-state artifacts failed: {result.stderr.strip()}")
        emitted = json.loads(multi_receipt.read_text())
        if len(emitted.get("feature_states", [])) != 2:
            raise SystemExit("multiple feature states were not retained")
    print("native evidence writer self-test passed")


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        self_test()
    else:
        main()
