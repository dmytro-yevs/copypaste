#!/usr/bin/env python3
import argparse
import base64
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile

from png_evidence import validate_png


KINDS = {"screenshot", "accessibility", "measurement", "test-log", "diagnostic-log", "feature-evidence"}


def parser():
    result = argparse.ArgumentParser()
    result.add_argument("--output", required=True)
    result.add_argument("--platform", choices=("macos", "android", "windows"), required=True)
    result.add_argument(
        "--environment",
        choices=("hosted-runner", "emulator", "physical-device"),
        required=True,
    )
    result.add_argument("--os-version", required=True)
    result.add_argument("--architecture", required=True)
    result.add_argument("--commit", default=os.environ.get("GITHUB_SHA"))
    result.add_argument("--run-id", default=os.environ.get("GITHUB_RUN_ID"))
    result.add_argument("--scenario", required=True)
    result.add_argument("--elapsed-ms", type=int, required=True)
    result.add_argument("--budget-ms", type=int, required=True)
    result.add_argument("--assertion", action="append", default=[])
    result.add_argument("--artifact", action="append", default=[])
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


def main():
    args = parser().parse_args()
    if not args.commit or len(args.commit) != 40 or any(c not in "0123456789abcdef" for c in args.commit):
        raise SystemExit("write-native-evidence: --commit must be a lowercase 40-character Git SHA")
    if not args.run_id:
        raise SystemExit("write-native-evidence: --run-id is required outside GitHub Actions")
    if args.elapsed_ms < 0 or args.budget_ms < 1:
        raise SystemExit("write-native-evidence: latency values are invalid")
    if not args.assertion or not args.artifact:
        raise SystemExit("write-native-evidence: assertions and artifacts are required")

    output = pathlib.Path(args.output)
    try:
        output.parent.mkdir(parents=True, exist_ok=True)
    except OSError:
        raise SystemExit("write-native-evidence: evidence directory is unavailable") from None
    try:
        artifacts = [artifact_record(output.parent, item) for item in args.artifact]
    except ValueError as error:
        raise SystemExit(f"write-native-evidence: {error}") from None

    receipt = {
        "schema_version": 1,
        "platform": args.platform,
        "environment": args.environment,
        "os_version": args.os_version,
        "architecture": args.architecture,
        "source": {"commit": args.commit, "run_id": args.run_id},
        "scenario": {
            "name": args.scenario,
            "elapsed_ms": args.elapsed_ms,
            "budget_ms": args.budget_ms,
        },
        "assertions": args.assertion,
        "artifacts": artifacts,
    }
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
        root = pathlib.Path(directory)
        good = root / "good.png"
        good.write_bytes(base64.b64decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
        ))
        chunk_corrupt = bytearray(good.read_bytes())
        chunk_corrupt[chunk_corrupt.index(b"IDAT") + 4] ^= 0xFF
        fixtures = {
            "empty.png": b"",
            "signature-truncated.png": good.read_bytes()[:24],
            "chunk-corrupt.png": bytes(chunk_corrupt),
            "good.png": good.read_bytes(),
        }
        common = [
            sys.executable, __file__, "--platform", "android",
            "--environment", "emulator", "--os-version", "API 33",
            "--architecture", "x86_64", "--commit", "a" * 40,
            "--run-id", "self-test", "--scenario", "release-webview-ready",
            "--elapsed-ms", "1", "--budget-ms", "2", "--assertion", "passed",
        ]
        for name, content in fixtures.items():
            screenshot = root / name
            screenshot.write_bytes(content)
            receipt = root / f"{name}.json"
            result = subprocess.run(
                common + ["--output", os.fspath(receipt), "--artifact", f"screenshot={name}"],
                check=False,
                capture_output=True,
                text=True,
            )
            if name == "good.png":
                if result.returncode != 0 or not receipt.is_file():
                    raise SystemExit(f"valid PNG self-test failed: {result.stderr.strip()}")
            elif result.returncode == 0 or receipt.exists():
                raise SystemExit(f"{name} produced a native evidence receipt")
    print("native evidence writer self-test passed")


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        self_test()
    else:
        main()
