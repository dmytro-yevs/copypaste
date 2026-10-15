#!/usr/bin/env python3
"""Validate current GitHub queue syntax and retain actionlint/ShellCheck coverage."""

import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile

from policy import check


# actionlint 1.7.12 (and upstream main at 011a6d15) lacks GitHub's supported
# queue field. policy.py validates that field before this exact obsolete-schema
# diagnostic is filtered. No other diagnostic is suppressed.
QUEUE_SCHEMA_DIAGNOSTIC = r'^unexpected key "queue" for "concurrency" section\. expected one of "cancel-in-progress", "group"$'
FIXTURE = """name: Queue compatibility fixture
on: workflow_dispatch
permissions:
  contents: read
concurrency:
  group: queue-fixture
  cancel-in-progress: false
  queue: max
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 5
    steps:
      - run: COMMAND
"""


def verify_coverage(command):
    # Prove that queue compatibility does not disable expression or shell checks.
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "queue-fixture.yml"
        for script, expected in [("echo checked", None),
                                 ("echo '${{ invalid_context.value }}'", "invalid_context"),
                                 ("echo $unquoted_value", "SC2086")]:
            path.write_text(FIXTURE.replace("COMMAND", script), encoding="utf-8")
            result = subprocess.run([*command, str(path)], capture_output=True, text=True)
            diagnostic = result.stdout + result.stderr
            if expected is None and result.returncode:
                raise ValueError("valid queue syntax failed the combined validators: " + diagnostic)
            if expected is not None and (result.returncode == 0 or expected not in diagnostic):
                raise ValueError("actionlint compatibility filtering lost required lint coverage: " + diagnostic)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--actionlint", default="actionlint")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    errors = check(root)
    if errors:
        raise SystemExit("\n".join(errors))
    if not shutil.which("shellcheck"):
        raise SystemExit("ShellCheck is required for workflow shell validation")
    command = [args.actionlint, "-color=false", "-ignore", QUEUE_SCHEMA_DIAGNOSTIC]
    verify_coverage(command)
    print("GitHub queue syntax passed repository policy; all other actionlint and ShellCheck diagnostics remain enforced.", flush=True)
    raise SystemExit(subprocess.run(command, cwd=root).returncode)


if __name__ == "__main__":
    main()
