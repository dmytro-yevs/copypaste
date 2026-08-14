#!/usr/bin/env python3
"""Prove every self-test in mutations.json still detects a broken production.

For each mutation the gate requires a non-zero exit whose output names the
assertion that was supposed to catch it. A self-test that stays green under a
mutation is a surviving mutation and fails this gate: it asserted nothing.

Four shipped self-tests reported green while asserting nothing — the worst of
them grepped its own source file, so it matched the literal in its own
assertion and passed with the production line deleted.
"""

import argparse
import json
import os
import pathlib
import platform
import shutil
import subprocess
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import tree  # noqa: E402

REPO_ROOT = HERE.parent.parent
MANIFEST = HERE / "mutations.json"


def current_platform():
    return {"Linux": "linux", "Darwin": "macos", "Windows": "windows"}.get(
        platform.system(), platform.system().lower()
    )


def load_manifest(path):
    document = json.loads(pathlib.Path(path).read_text(encoding="utf-8"))
    for entry in document.get("not_covered", []):
        if not entry.get("command") or not entry.get("why"):
            raise SystemExit(f"an uncovered self-test needs a command and a why: {entry}")
    seen = set()
    for entry in document["self_tests"]:
        for field in ("id", "verifies", "command", "platforms", "mutations"):
            if field not in entry:
                raise SystemExit(f"{entry.get('id', '?')} has no {field!r}")
        if entry["id"] in seen:
            raise SystemExit(f"duplicate self-test id {entry['id']!r}")
        seen.add(entry["id"])
        unknown = set(entry["platforms"]) - {"linux", "macos", "windows"}
        if unknown or not entry["platforms"]:
            raise SystemExit(f"{entry['id']} declares no usable platform: {entry['platforms']}")
        if not entry["mutations"]:
            raise SystemExit(f"{entry['id']} declares no mutations")
        names = set()
        for mutation in entry["mutations"]:
            for field in ("name", "file", "find", "replace", "expect"):
                if field not in mutation:
                    raise SystemExit(
                        f"{entry['id']}/{mutation.get('name', '?')} has no {field!r}"
                    )
            if mutation["find"] == mutation["replace"]:
                raise SystemExit(
                    f"{entry['id']}/{mutation['name']} replaces its find with itself"
                )
            if mutation["name"] in names:
                raise SystemExit(
                    f"{entry['id']} has two mutations named {mutation['name']!r}"
                )
            names.add(mutation["name"])
    return document


def missing_requirement(entry, root):
    for name in entry.get("requires", []):
        if shutil.which(name) is None:
            return f"{name} is not on PATH"
    for rel in entry.get("requires_paths", []):
        if not (pathlib.Path(root) / rel).exists():
            return f"{rel} is absent"
    return None


def run(command, cwd, timeout):
    environment = dict(os.environ)
    environment["COPYPASTE_MUTATION_GATE"] = "1"
    try:
        finished = subprocess.run(
            command,
            cwd=str(cwd),
            capture_output=True,
            text=True,
            errors="replace",
            timeout=timeout,
            env=environment,
        )
    except subprocess.TimeoutExpired as expired:
        return 124, f"timed out after {expired.timeout}s"
    return finished.returncode, (finished.stdout or "") + (finished.stderr or "")


def apply_mutation(scratch, mutation):
    text = scratch.read(mutation["file"])
    occurrences = text.count(mutation["find"])
    expected = mutation.get("occurrences", 1)
    if occurrences != expected:
        return (
            f"its find matched {occurrences} time(s) in {mutation['file']},"
            f" not {expected}; the mutation is stale"
        )
    scratch.write(mutation["file"], text.replace(mutation["find"], mutation["replace"]))
    return None


def check_entry(entry, scratch, timeout, report):
    command = entry["command"]
    code, output = run(command, scratch.root, timeout)
    if code != 0:
        report.baseline_failed(entry, code, output)
        return
    for mutation in entry["mutations"]:
        stale = apply_mutation(scratch, mutation)
        if stale is None:
            code, output = run(command, scratch.root, timeout)
            if code == 0:
                report.survived(entry, mutation, "the self-test still passed")
            elif mutation["expect"] not in output:
                report.survived(
                    entry,
                    mutation,
                    f"it failed but never named {mutation['expect']!r}",
                )
            else:
                report.killed(entry, mutation, code)
        else:
            report.survived(entry, mutation, stale)
        scratch.restore(mutation["file"])


class Report:
    def __init__(self, stream):
        self.stream = stream
        self.killed_count = 0
        self.failures = []
        self.skipped = []
        self.elsewhere = []

    def line(self, text):
        print(text, file=self.stream, flush=True)

    def heading(self, entry):
        self.line(f"\n== {entry['id']}: {entry['verifies']}")

    def skip(self, entry, why):
        self.skipped.append((entry["id"], why))
        self.line(f"\n== {entry['id']}: SKIPPED ({why})")

    def runs_elsewhere(self, entry, target):
        self.elsewhere.append(entry["id"])
        self.line(f"\n== {entry['id']}: not declared for {target}")

    def baseline_failed(self, entry, code, output):
        tail = "\n".join(output.strip().splitlines()[-8:])
        self.failures.append(
            (entry["id"], "<baseline>", f"the unmutated self-test exited {code}")
        )
        self.line(f"  BASELINE FAILED  exit {code}\n{tail}")

    def killed(self, entry, mutation, code):
        self.killed_count += 1
        self.line(f"  killed    {mutation['name']}  (exit {code})")

    def survived(self, entry, mutation, why):
        self.failures.append((entry["id"], mutation["name"], why))
        self.line(f"  SURVIVED  {mutation['name']}  — {why}")

    def summarize(self, require_all):
        self.line("")
        self.line(f"mutation gate: {self.killed_count} killed, {len(self.failures)} survived")
        for identifier, name, why in self.failures:
            self.line(f"  SURVIVED  {identifier}/{name}: {why}")
        for identifier, why in self.skipped:
            self.line(f"  skipped   {identifier}: {why}")
        if self.elsewhere:
            self.line(f"  {len(self.elsewhere)} self-test(s) run on another platform: {', '.join(self.elsewhere)}")
        if self.skipped and require_all:
            self.line("  --require-all was set, so a skipped self-test is a failure")
        return bool(self.failures) or (require_all and bool(self.skipped))


def gate(entries, target, timeout, require_all, stream):
    report = Report(stream)
    before = tree.digest(REPO_ROOT)
    report.line(f"tracked-tree digest before: {before}")
    with tempfile.TemporaryDirectory(prefix="copypaste-mutation-gate-") as workspace:
        scratch = tree.Scratch(REPO_ROOT, pathlib.Path(workspace) / "tree")
        report.line(f"copied {scratch.populate()} tracked paths to a scratch tree")
        for entry in entries:
            if target not in entry["platforms"]:
                report.runs_elsewhere(entry, target)
                continue
            absent = missing_requirement(entry, scratch.root)
            if absent:
                report.skip(entry, absent)
                continue
            report.heading(entry)
            check_entry(entry, scratch, timeout, report)
    after = tree.digest(REPO_ROOT)
    report.line(f"\ntracked-tree digest after:  {after}")
    if before != after:
        report.line("THE WORKING TREE CHANGED during the run")
        return 1
    report.line("the working tree is unchanged")
    return 1 if report.summarize(require_all) else 0


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", default=current_platform())
    parser.add_argument("--only", default="", help="comma-separated self-test ids")
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--require-all", action="store_true")
    parser.add_argument("--list", action="store_true")
    parser.add_argument(
        "--platforms-covered",
        default="",
        help="fail if a self-test is declared for none of these comma-separated platforms",
    )
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--manifest", default=str(MANIFEST))
    arguments = parser.parse_args(argv)

    if arguments.self_test:
        import self_test

        return self_test.main()

    document = load_manifest(arguments.manifest)
    entries = document["self_tests"]
    if arguments.platforms_covered:
        covered = set(arguments.platforms_covered.split(","))
        stranded = [e["id"] for e in entries if not covered & set(e["platforms"])]
        if stranded:
            raise SystemExit(
                f"no CI platform runs: {', '.join(stranded)}; CI covers {sorted(covered)}"
            )
        print(f"every self-test in the manifest runs on one of {sorted(covered)}")
        return 0
    if arguments.only:
        wanted = set(arguments.only.split(","))
        unknown = wanted - {entry["id"] for entry in entries}
        if unknown:
            raise SystemExit(f"no such self-test: {', '.join(sorted(unknown))}")
        entries = [entry for entry in entries if entry["id"] in wanted]
    if arguments.list:
        for entry in entries:
            platforms = ",".join(entry["platforms"])
            print(f"{entry['id']:34} {len(entry['mutations']):2} mutations  [{platforms}]")
            print(f"{'':34} {entry['verifies']}")
        print("\nself-tests with no mutations yet:")
        for entry in document.get("not_covered", []):
            print(f"  {entry['command']}\n      {entry['why']}")
        return 0
    return gate(entries, arguments.platform, arguments.timeout, arguments.require_all, sys.stdout)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
