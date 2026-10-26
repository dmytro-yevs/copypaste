#!/usr/bin/env python3
"""Read a prior Linux version from an authenticated production receipt."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import runpy


RELEASE = runpy.run_path(str(Path(__file__).with_name("linux_release_contract.py")))


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def version(receipt_path: Path, run_id: str, commit: str) -> str:
    if not re.fullmatch(r"[1-9][0-9]*", run_id) or not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("prior Linux producer identity is invalid")
    resolved = receipt_path.resolve(strict=True)
    if receipt_path.is_symlink() or not resolved.is_file():
        raise ValueError("prior Linux production receipt is unsafe")
    try:
        receipt = json.loads(resolved.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ValueError("prior Linux production receipt is invalid") from error
    value = receipt.get("version")
    if (receipt.get("schema") != 2 or receipt.get("platform") != "linux"
            or receipt.get("commit") != commit or str(receipt.get("run_id")) != run_id
            or receipt.get("inventory_complete") is not True or not isinstance(value, str)
            or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", value)):
        raise ValueError("prior Linux production receipt does not bind its producer identity")
    expected = {
        f"CopyPaste-v{value}-linux-{architecture}.{extension}"
        for architecture in RELEASE["ARCHITECTURES"]
        for extension in RELEASE["PUBLISHED_FORMATS"]
    }
    expected.update(
        f"CopyPaste-v{value}-linux-{architecture}.runtime-baseline.json"
        for architecture in RELEASE["ARCHITECTURES"]
    )
    expected.update(f"{name}{suffix}" for name in tuple(expected) if name.endswith((".AppImage", ".deb", ".rpm")) for suffix in (".sig", ".sha256"))
    expected.update(
        f"CopyPaste-v{value}-linux-{architecture}.rpm.installation.json"
        for architecture in RELEASE["ARCHITECTURES"]
    )
    records = receipt.get("artifacts")
    if not isinstance(records, list) or len(records) != len(expected):
        raise ValueError("prior Linux production receipt inventory is incomplete")
    names = set()
    for record in records:
        if not isinstance(record, dict):
            raise ValueError("prior Linux production receipt record is invalid")
        name = record.get("name")
        if (not isinstance(name, str) or Path(name).name != name or name in names
                or not isinstance(record.get("bytes"), int) or record["bytes"] <= 0
                or not re.fullmatch(r"[0-9a-f]{64}", str(record.get("sha256", "")))):
            raise ValueError("prior Linux production receipt record is invalid")
        artifact = resolved.parent / name
        if (not artifact.is_file() or artifact.is_symlink() or artifact.stat().st_size != record["bytes"]
                or sha256(artifact) != record["sha256"]):
            raise ValueError("prior Linux artifact differs from its receipt")
        names.add(name)
    if names != expected:
        raise ValueError("prior Linux production receipt does not cover the release inventory")
    return value


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    print(version(args.receipt, args.run_id, args.commit))


if __name__ == "__main__":
    main()
