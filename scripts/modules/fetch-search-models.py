#!/usr/bin/env python3
"""Stage checksum-pinned model fixtures for explicit native qualification."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
from urllib.request import urlopen


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    document = json.loads(args.sources.read_text())
    for model in document["models"]:
        for item in model["files"]:
            path = args.destination / model["id"] / item["path"]
            path.parent.mkdir(parents=True, exist_ok=True)
            if not path.exists():
                with urlopen(item["url"], timeout=60) as source, path.open("wb") as output:
                    shutil.copyfileobj(source, output, 1024 * 1024)
            digest = hashlib.sha256()
            with path.open("rb") as source:
                for block in iter(lambda: source.read(1024 * 1024), b""):
                    digest.update(block)
            if path.stat().st_size != item["size_bytes"] or digest.hexdigest() != item["sha256"]:
                raise ValueError(f"Invalid pinned qualification model: {model['id']}/{item['path']}")
    shutil.copyfile(args.sources, args.destination / "search-models.json")
    print("Verified both pinned semantic model profiles")


if __name__ == "__main__":
    main()
