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
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--run-id", required=True)
    args = parser.parse_args()

    for platform in ("macos", "android", "windows"):
        receipt_path = args.root / platform / "production-receipt.json"
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        if receipt.get("schema") != 1 or receipt.get("platform") != platform:
            raise SystemExit(f"invalid {platform} receipt identity")
        for key, expected in (
            ("version", args.version),
            ("commit", args.commit),
            ("run_id", args.run_id),
        ):
            if str(receipt.get(key)) != expected:
                raise SystemExit(f"{platform} receipt has wrong {key}")
        metadata = receipt.get("artifact")
        if not isinstance(metadata, dict):
            raise SystemExit(f"{platform} receipt has no artifact metadata")
        artifact = args.root / platform / str(metadata.get("name", ""))
        if not artifact.is_file():
            raise SystemExit(f"{platform} qualified artifact is missing")
        if artifact.stat().st_size != metadata.get("bytes"):
            raise SystemExit(f"{platform} artifact size changed")
        if sha256(artifact) != metadata.get("sha256"):
            raise SystemExit(f"{platform} artifact digest changed")
    print("verified production artifacts for macOS, Android, and Windows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
