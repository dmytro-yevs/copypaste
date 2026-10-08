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
    parser.add_argument("--artifact", required=True, type=Path, action="append")
    parser.add_argument("--version", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    artifacts = [path.resolve(strict=True) for path in args.artifact]
    if any(not path.is_file() or path.stat().st_size == 0 for path in artifacts):
        raise SystemExit("qualified artifacts must be nonempty files")
    if len({path.name for path in artifacts}) != len(artifacts):
        raise SystemExit("qualified artifact names must be unique")
    if len(args.commit) != 40 or any(character not in "0123456789abcdef" for character in args.commit):
        raise SystemExit("commit must be a lowercase 40-character SHA")

    receipt = {
        "schema": 1 if len(artifacts) == 1 else 2,
        "platform": args.platform,
        "version": args.version,
        "commit": args.commit,
        "run_id": args.run_id,
    }
    metadata = [
        {"name": path.name, "bytes": path.stat().st_size, "sha256": sha256(path)}
        for path in artifacts
    ]
    if len(metadata) == 1:
        receipt["artifact"] = metadata[0]
    else:
        receipt["artifacts"] = metadata
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
