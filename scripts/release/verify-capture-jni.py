#!/usr/bin/env python3
"""Verify capture entry points in the optimized Android APK's disassembly."""

import argparse
from pathlib import Path


def verify(callback: str, runtime: str) -> None:
    methods = [line for line in callback.splitlines() if line.startswith(".method public ")]
    if not any(line.endswith(" run(J)V") for line in methods):
        raise ValueError("optimized APK is missing the JNI CaptureCallback.run(long) upcall")
    native = [line for line in runtime.splitlines() if line.startswith(".method public ") and " native " in line]
    for signature in (
        "ingestText(JLjava/lang/String;)Z",
        "ingestBinary(J[BLjava/lang/String;Ljava/lang/String;Ljava/lang/String;)Z",
    ):
        if not any(line.endswith(" " + signature) for line in native):
            raise ValueError(f"optimized APK is missing the JNI capture method {signature}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--callback", required=True, type=Path)
    parser.add_argument("--runtime", required=True, type=Path)
    args = parser.parse_args()
    verify(args.callback.read_text(), args.runtime.read_text())
    print("verified optimized Android capture JNI contract")


if __name__ == "__main__":
    main()
