#!/usr/bin/env python3
"""Fetch checksum-pinned PP-OCRv5 ONNX assets for an offline OCR package."""
import argparse
import ast
import hashlib
import json
from pathlib import Path
import tempfile
import urllib.request


def digest(path):
    hasher = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            hasher.update(block)
    return hasher.hexdigest()


def fetch(entry, destination):
    target = destination / entry["path"]
    if target.is_file() and digest(target) == entry["sha256"]:
        print(f"verified {target.name}")
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary_path = None
    try:
        with tempfile.NamedTemporaryFile(dir=target.parent, delete=False) as temporary:
            temporary_path = Path(temporary.name)
            with urllib.request.urlopen(entry["url"], timeout=120) as response:
                while block := response.read(1024 * 1024):
                    temporary.write(block)
            temporary.flush()
        if digest(temporary_path) != entry["sha256"]:
            raise ValueError(f"checksum mismatch for {entry['path']}")
        temporary_path.replace(target)
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)
    print(f"fetched {target.name}")


def download_bytes(url):
    with urllib.request.urlopen(url, timeout=120) as response:
        return response.read()


def parse_character_dict(model_name, document):
    lines = document.decode("utf-8").splitlines()
    try:
        start = lines.index("  character_dict:") + 1
    except ValueError as error:
        raise ValueError(f"{model_name} has no character dictionary") from error
    characters = []
    for line in lines[start:]:
        if not line.startswith("  - "):
            break
        value = line.removeprefix("  - ")
        if value.startswith("'") and value.endswith("'"):
            value = value[1:-1].replace("''", "'")
        elif value.startswith('"') and value.endswith('"'):
            value = ast.literal_eval(value)
        if not value or "\n" in value or "\r" in value:
            raise ValueError(f"{model_name} has an invalid character dictionary")
        characters.append(value)
    if not characters:
        raise ValueError(f"{model_name} has an empty character dictionary")
    # paddle-ocr-rs expects the CTC blank token at index 0 and appends no
    # delimiter itself when a dictionary file is supplied.
    return "\n".join(["#", *characters, " "]) + "\n"


def fetch_dictionary(entry, destination):
    target = destination / entry["dictionary_path"]
    document = download_bytes(entry["dictionary_url"])
    actual = hashlib.sha256(document).hexdigest()
    if actual != entry["dictionary_sha256"]:
        raise ValueError(f"dictionary source checksum mismatch for {entry['path']}")
    content = parse_character_dict(entry["path"], document)
    if target.is_file() and target.read_bytes() == content.encode("utf-8"):
        print(f"verified {target.name}")
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    temporary_path = None
    try:
        with tempfile.NamedTemporaryFile(
            dir=target.parent, mode="w", encoding="utf-8", newline="\n", delete=False
        ) as temporary:
            temporary_path = Path(temporary.name)
            temporary.write(content)
            temporary.flush()
        temporary_path.replace(target)
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)
    print(f"generated {target.name}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--destination", type=Path, default=Path("assets/models"))
    parser.add_argument("--sources", type=Path, default=Path("assets/model-sources.json"))
    args = parser.parse_args()
    source = json.loads(args.sources.read_text(encoding="utf-8"))
    if source.get("license") != "Apache-2.0" or not source.get("models"):
        raise ValueError("model source manifest is invalid")
    destination = args.destination.resolve()
    for entry in source["models"]:
        required = {"path", "url", "sha256"}
        if entry["path"] != "detector.onnx":
            required |= {"dictionary_path", "dictionary_url", "dictionary_sha256"}
        if set(entry) != required or len(entry["sha256"]) != 64:
            raise ValueError("model source entry is invalid")
        fetch(entry, destination)
        if "dictionary_path" in entry:
            if len(entry["dictionary_sha256"]) != 64:
                raise ValueError("dictionary source entry is invalid")
            fetch_dictionary(entry, destination)


if __name__ == "__main__":
    main()
