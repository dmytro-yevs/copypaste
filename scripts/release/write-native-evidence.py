#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
import pathlib
import re
import stat
import subprocess
import sys
import tempfile
from types import SimpleNamespace

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
    result.add_argument("--qualified-artifact", required=True)
    result.add_argument("--qualified-artifact-identity", required=True)
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


def file_identity(metadata):
    return {
        "device": metadata.st_dev,
        "inode": metadata.st_ino,
        "bytes": metadata.st_size,
        "mtime_ns": metadata.st_mtime_ns,
        "ctime_ns": metadata.st_ctime_ns,
    }


def path_and_handle_identity_match(path_metadata, handle_metadata, *, windows=None):
    path_identity = file_identity(path_metadata)
    handle_identity = file_identity(handle_metadata)
    if windows is None:
        windows = os.name == "nt"
    # CPython 3.12 aliases Windows path ctime to creation time, while fstat
    # retains raw ChangeTime. Handle-to-handle comparisons still use ctime.
    path_birthtime = getattr(path_metadata, "st_birthtime_ns", None)
    handle_birthtime = getattr(handle_metadata, "st_birthtime_ns", None)
    if not windows or not isinstance(path_birthtime, int) or not isinstance(handle_birthtime, int):
        return path_identity == handle_identity
    return (
        path_identity["device"] == handle_identity["device"]
        and path_identity["inode"] == handle_identity["inode"]
        and path_identity["bytes"] == handle_identity["bytes"]
        and path_identity["mtime_ns"] == handle_identity["mtime_ns"]
        and path_birthtime == handle_birthtime
    )


def qualified_artifact_open_flags():
    return os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_BINARY", 0)


def qualified_artifact_snapshot(value):
    file = pathlib.Path(value)
    try:
        before = file.lstat()
    except OSError:
        raise ValueError("qualified artifact cannot be read") from None
    if file.is_symlink() or not stat.S_ISREG(before.st_mode) or before.st_size < 1:
        raise ValueError("qualified artifact must be a non-empty regular non-symlink file")
    digest = hashlib.sha256()
    descriptor = None
    try:
        descriptor = os.open(file, qualified_artifact_open_flags())
        opened = os.fstat(descriptor)
        if not path_and_handle_identity_match(before, opened):
            raise ValueError("qualified artifact changed while opening")
        while chunk := os.read(descriptor, 1024 * 1024):
            digest.update(chunk)
        if file_identity(os.fstat(descriptor)) != file_identity(opened):
            raise ValueError("qualified artifact changed while hashing")
        if not path_and_handle_identity_match(file.lstat(), opened):
            raise ValueError("qualified artifact changed while hashing")
    except OSError:
        raise ValueError("qualified artifact cannot be read") from None
    finally:
        if descriptor is not None:
            try:
                os.close(descriptor)
            except OSError:
                pass
    return {
        "record": {
            "name": file.name,
            "sha256": digest.hexdigest(),
            "bytes": opened.st_size,
        },
        "identity": file_identity(opened),
    }


def qualified_artifact_identity(value):
    try:
        identity = json.loads(value)
    except json.JSONDecodeError:
        raise ValueError("qualified artifact identity is invalid") from None
    if (
        not isinstance(identity, dict)
        or set(identity) != {"record", "identity"}
        or not isinstance(identity["record"], dict)
        or set(identity["record"]) != {"name", "sha256", "bytes"}
        or not isinstance(identity["identity"], dict)
        or set(identity["identity"]) != {"device", "inode", "bytes", "mtime_ns", "ctime_ns"}
        or not isinstance(identity["record"]["name"], str)
        or not identity["record"]["name"]
        or not re.fullmatch(r"[0-9a-f]{64}", identity["record"]["sha256"])
        or not isinstance(identity["record"]["bytes"], int)
        or identity["record"]["bytes"] < 1
        or any(not isinstance(item, int) for item in identity["identity"].values())
    ):
        raise ValueError("qualified artifact identity is invalid")
    return identity


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
        qualified_snapshot = qualified_artifact_snapshot(args.qualified_artifact)
        if qualified_snapshot != qualified_artifact_identity(args.qualified_artifact_identity):
            raise ValueError("qualified artifact changed after capture")
        qualified_artifact = qualified_snapshot["record"]
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
        "qualified_artifact": qualified_artifact,
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
        qualified = root / "qualified.apk"
        qualified.write_bytes(b"qualified release artifact\n")
        qualified_identity = json.dumps(qualified_artifact_snapshot(qualified), separators=(",", ":"))
        path_metadata = SimpleNamespace(
            st_dev=1,
            st_ino=2,
            st_size=3,
            st_mtime_ns=4,
            st_ctime_ns=5,
            st_birthtime_ns=6,
        )
        handle_metadata = SimpleNamespace(
            st_dev=1,
            st_ino=2,
            st_size=3,
            st_mtime_ns=4,
            st_ctime_ns=7,
            st_birthtime_ns=6,
        )
        if not path_and_handle_identity_match(path_metadata, handle_metadata, windows=True):
            raise SystemExit("Windows path and handle creation times did not match")
        for field, value in (
            ("st_dev", 8),
            ("st_ino", 8),
            ("st_size", 8),
            ("st_mtime_ns", 8),
            ("st_birthtime_ns", 8),
        ):
            mismatch = SimpleNamespace(**vars(handle_metadata))
            setattr(mismatch, field, value)
            if path_and_handle_identity_match(path_metadata, mismatch, windows=True):
                raise SystemExit(f"Windows path and handle mismatch accepted for {field}")
        legacy_path_metadata = SimpleNamespace(**vars(path_metadata))
        legacy_handle_metadata = SimpleNamespace(**vars(handle_metadata))
        del legacy_path_metadata.st_birthtime_ns
        del legacy_handle_metadata.st_birthtime_ns
        if path_and_handle_identity_match(legacy_path_metadata, legacy_handle_metadata, windows=True):
            raise SystemExit("Windows legacy path and handle change-time mismatch accepted")
        raw_handle_change = SimpleNamespace(**vars(handle_metadata))
        raw_handle_change.st_ctime_ns = 8
        if file_identity(handle_metadata) == file_identity(raw_handle_change):
            raise SystemExit("Windows raw handle change time was not retained")
        original_fstat = os.fstat
        fstat_calls = 0

        def change_handle_time_after_hash(descriptor):
            nonlocal fstat_calls
            metadata = original_fstat(descriptor)
            fstat_calls += 1
            if fstat_calls == 2:
                return SimpleNamespace(
                    st_dev=metadata.st_dev,
                    st_ino=metadata.st_ino,
                    st_size=metadata.st_size,
                    st_mtime_ns=metadata.st_mtime_ns,
                    st_ctime_ns=metadata.st_ctime_ns + 1,
                )
            return metadata

        os.fstat = change_handle_time_after_hash
        try:
            try:
                qualified_artifact_snapshot(qualified)
            except ValueError as error:
                if str(error) != "qualified artifact changed while hashing":
                    raise
            else:
                raise SystemExit("qualified artifact raw handle change was accepted")
        finally:
            os.fstat = original_fstat
        if fstat_calls != 2:
            raise SystemExit("qualified artifact raw handle change fixture was not used")
        binary_bytes = b"release\r\nartifact\x1aafter-eof\r\n"
        binary = root / "binary-qualified.apk"
        binary.write_bytes(binary_bytes)
        binary_snapshot = qualified_artifact_snapshot(binary)
        if binary_snapshot["record"]["sha256"] != hashlib.sha256(binary_bytes).hexdigest():
            raise SystemExit("qualified artifact binary bytes changed while hashing")
        original_binary = getattr(os, "O_BINARY", None)
        os.O_BINARY = 0x4000
        try:
            if not qualified_artifact_open_flags() & os.O_BINARY:
                raise SystemExit("qualified artifact open flags omit O_BINARY")
        finally:
            if original_binary is None:
                del os.O_BINARY
            else:
                os.O_BINARY = original_binary
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
            "--environment", "emulator", "--os-version", "API 33",
            "--architecture", "x86_64", "--commit", "a" * 40,
            "--run-id", "self-test", "--elapsed-ms", "1",
            "--qualified-artifact", os.fspath(qualified),
            "--qualified-artifact-identity", qualified_identity,
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
        physical = list(common)
        physical[physical.index("emulator")] = "physical-device"
        physical_receipt = root / "physical.json"
        result = subprocess.run(
            physical + [
                "--output", os.fspath(physical_receipt),
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
        if result.returncode == 0 or physical_receipt.exists():
            raise SystemExit("physical Android produced an emulator publication receipt")
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
        invalid_qualified = {
            "missing.apk": root / "missing.apk",
            "empty.apk": root / "empty.apk",
            "directory.apk": root / "directory.apk",
            "linked.apk": root / "linked.apk",
        }
        invalid_qualified["empty.apk"].write_bytes(b"")
        invalid_qualified["directory.apk"].mkdir()
        invalid_qualified["linked.apk"].symlink_to(qualified)
        option_index = common.index("--qualified-artifact")
        for name, artifact in invalid_qualified.items():
            rejected_common = common[:option_index + 1] + [os.fspath(artifact)]
            receipt = root / f"invalid-qualified-{name}.json"
            result = subprocess.run(
                rejected_common + [
                    "--output", os.fspath(receipt),
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
            if result.returncode == 0 or receipt.exists():
                raise SystemExit(f"{name} produced a native evidence receipt")
        replacement = root / "replacement.apk"
        replacement.write_bytes(b"x" * qualified.stat().st_size)
        replacement.replace(qualified)
        receipt = root / "replaced-qualified.json"
        result = subprocess.run(
            common + [
                "--output", os.fspath(receipt),
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
        if result.returncode == 0 or receipt.exists():
            raise SystemExit("same-size qualified artifact replacement produced a receipt")
        qualified.write_bytes(b"qualified release artifact\n")
        replacement.unlink(missing_ok=True)
        qualified_identity = json.dumps(qualified_artifact_snapshot(qualified), separators=(",", ":"))
        common[common.index("--qualified-artifact-identity") + 1] = qualified_identity
        original_read = os.read
        replacement = root / "during-hash.apk"
        replacement.write_bytes(b"y" * qualified.stat().st_size)
        replaced = False

        def replace_during_hash(descriptor, size):
            nonlocal replaced
            chunk = original_read(descriptor, size)
            if chunk and not replaced:
                replacement.replace(qualified)
                replaced = True
            return chunk

        os.read = replace_during_hash
        try:
            try:
                qualified_artifact_snapshot(qualified)
            except ValueError:
                pass
            else:
                raise SystemExit("qualified artifact replacement during hashing was accepted")
        finally:
            os.read = original_read
        if not replaced:
            raise SystemExit("qualified artifact hash race fixture did not replace the file")
    print("native evidence writer self-test passed")


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        self_test()
    elif len(sys.argv) == 3 and sys.argv[1] == "--capture-qualified-artifact":
        try:
            print(json.dumps(qualified_artifact_snapshot(sys.argv[2]), separators=(",", ":")))
        except ValueError as error:
            raise SystemExit(f"write-native-evidence: {error}") from None
    else:
        main()
