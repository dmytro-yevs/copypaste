#!/usr/bin/env python3
"""Verify the exact native library architectures in a production APK."""

import argparse
from pathlib import Path
from zipfile import ZipFile


ARCHITECTURES = {
    "universal": {"armeabi-v7a", "arm64-v8a", "x86_64"},
    "arm64": {"arm64-v8a"},
    "armv7": {"armeabi-v7a"},
}
REQUIRED_LIBRARIES = (
    "libflutter.so",
    "libapp.so",
    "libcopypaste_flutter_bridge.so",
    "libcopypaste_pairing_host.so",
)


def verify(apk: Path, architecture: str) -> None:
    expected = ARCHITECTURES[architecture]
    with ZipFile(apk) as archive:
        names = set(archive.namelist())
    actual = {
        name.split("/")[1]
        for name in names
        if name.startswith("lib/") and name.endswith(".so")
    }
    if actual != expected:
        raise ValueError(f"APK architectures differ: expected {sorted(expected)}, got {sorted(actual)}")
    for abi in expected:
        for library in REQUIRED_LIBRARIES:
            if f"lib/{abi}/{library}" not in names:
                raise ValueError(f"APK is missing {abi}/{library}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("apk", type=Path)
    parser.add_argument("architecture", choices=ARCHITECTURES)
    args = parser.parse_args()
    verify(args.apk, args.architecture)


if __name__ == "__main__":
    main()
