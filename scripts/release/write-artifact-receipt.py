#!/usr/bin/env python3
import argparse
import hashlib
import json
from pathlib import Path


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--platform", required=True, choices=("macos", "android", "windows"))
    parser.add_argument("--artifact", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    artifact = args.artifact.resolve(strict=True)
    if not artifact.is_file() or artifact.stat().st_size == 0:
        raise SystemExit("qualified artifact must be a nonempty file")
    if len(args.commit) != 40 or any(character not in "0123456789abcdef" for character in args.commit):
        raise SystemExit("commit must be a lowercase 40-character SHA")

    receipt = {
        "schema": 1,
        "platform": args.platform,
        "version": args.version,
        "commit": args.commit,
        "run_id": args.run_id,
        "artifact": {
            "name": artifact.name,
            "bytes": artifact.stat().st_size,
            "sha256": sha256(artifact),
        },
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
