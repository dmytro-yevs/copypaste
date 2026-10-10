#!/usr/bin/env python3
"""Place a checksum-verified ONNX Runtime library in a module native directory."""
import argparse
import hashlib
from pathlib import Path
import shutil


def sha256(path):
    hasher = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            hasher.update(block)
    return hasher.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", required=True, type=Path,
                        help="A locally downloaded ONNX Runtime CPU library.")
    parser.add_argument("--sha256", required=True,
                        help="The published SHA-256 for this exact library file.")
    parser.add_argument("--platform", required=True, choices=["macos", "windows", "linux", "android"])
    parser.add_argument("--architecture", required=True, choices=["x86", "x86_64", "arm", "aarch64"])
    parser.add_argument("--destination", type=Path, default=Path("native"))
    args = parser.parse_args()
    source = args.library.resolve(strict=True)
    expected = args.sha256.lower()
    if len(expected) != 64 or any(char not in "0123456789abcdef" for char in expected):
        raise ValueError("--sha256 must be a lowercase SHA-256 digest")
    if sha256(source) != expected:
        raise ValueError("ONNX Runtime checksum mismatch")
    names = {"macos": "libonnxruntime.dylib", "windows": "onnxruntime.dll", "linux": "libonnxruntime.so", "android": "libonnxruntime.so"}
    target = args.destination / args.platform / args.architecture / names[args.platform]
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, target)
    if sha256(target) != expected:
        target.unlink(missing_ok=True)
        raise ValueError("ONNX Runtime copy checksum mismatch")
    print(f"prepared {target}")


if __name__ == "__main__":
    main()
