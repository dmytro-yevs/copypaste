#!/usr/bin/env python3
"""Time one build phase while preserving command output and failure status."""

import argparse
import json
import shutil
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=("dependencies", "compilation", "testing", "packaging", "qualification", "signing"))
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("a command is required")
    # Resolve PATHEXT on Windows so Flutter's .bat launcher works with argv calls.
    command[0] = shutil.which(command[0]) or command[0]
    started = time.monotonic()
    result = 1
    try:
        result = subprocess.run(command, check=False).returncode
        return result if result >= 0 else 128 - result
    finally:
        print("COPYPASTE_CI_TIMING " + json.dumps({"phase": args.phase, "duration_s": round(time.monotonic() - started, 3), "exit_code": result}), flush=True)


if __name__ == "__main__":
    raise SystemExit(main())
