"""Prove the mutation gate itself reports a survivor rather than a pass.

Driven through the real check_entry against a fixture self-test whose verdict
this file controls — never by reading run.py's source, which is the failure the
gate exists to catch.
"""

import io
import pathlib
import shutil
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import run  # noqa: E402

FIXTURE = """import pathlib, sys
verdict = pathlib.Path("verdict.txt").read_text().strip()
if verdict == "green":
    print("fixture self-test passed")
    sys.exit(0)
print(verdict, file=sys.stderr)
sys.exit(1)
"""

PASS = 0
FAIL = 0


def ok(description):
    global PASS
    PASS += 1
    print(f"  ok    {description}")


def bad(description, detail=""):
    global FAIL
    FAIL += 1
    print(f"  FAIL  {description}")
    if detail:
        print(f"        {detail}")


class FixtureScratch:
    def __init__(self, root):
        self.root = pathlib.Path(root)
        self.pristine = {}

    def seed(self, rel, text):
        with (self.root / rel).open("w", encoding="utf-8", newline="") as handle:
            handle.write(text)
        self.pristine[rel] = text

    def read(self, rel):
        return (self.root / rel).read_text(encoding="utf-8")

    def write(self, rel, text):
        with (self.root / rel).open("w", encoding="utf-8", newline="") as handle:
            handle.write(text)

    def restore(self, rel):
        self.write(rel, self.pristine[rel])


def entry(name, find, replace, expect, occurrences=None):
    mutation = {
        "name": name,
        "file": "verdict.txt",
        "find": find,
        "replace": replace,
        "expect": expect,
    }
    if occurrences is not None:
        mutation["occurrences"] = occurrences
    return {
        "id": "fixture",
        "verifies": "a fixture whose verdict this test controls",
        "command": [sys.executable, "fixture.py"],
        "platforms": ["linux", "macos", "windows"],
        "mutations": [mutation],
    }


def verdicts(scratch, definition):
    report = run.Report(io.StringIO())
    run.check_entry(definition, scratch, 60, report)
    return report


def expect_survivor(description, report, needle):
    if report.killed_count:
        bad(description, "the gate counted a kill")
    elif not report.failures:
        bad(description, "the gate reported nothing")
    elif needle not in report.failures[0][2]:
        bad(description, f"reported {report.failures[0][2]!r}")
    else:
        ok(description)


def manifest_rejects(description, document):
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as handle:
        handle.write(document)
        path = handle.name
    try:
        run.load_manifest(path)
    except SystemExit:
        ok(description)
        return
    finally:
        pathlib.Path(path).unlink()
    bad(description)


def main():
    with tempfile.TemporaryDirectory(prefix="mutation-gate-self-test-") as workspace:
        scratch = FixtureScratch(workspace)
        (scratch.root / "fixture.py").write_text(FIXTURE, encoding="utf-8")
        scratch.seed("verdict.txt", "green\n")

        report = verdicts(scratch, entry("names-it", "green", "the guard did not hold", "guard did not hold"))
        if report.killed_count == 1 and not report.failures:
            ok("a mutation whose self-test fails and names the assertion is killed")
        else:
            bad("a mutation whose self-test fails and names the assertion is killed")

        expect_survivor(
            "a mutation that leaves the self-test green is reported as survived",
            verdicts(scratch, entry("stays-green", "green\n", "green\n\n", "guard did not hold")),
            "still passed",
        )
        expect_survivor(
            "a self-test that fails without naming the assertion is reported as survived",
            verdicts(scratch, entry("unnamed", "green", "some other failure", "guard did not hold")),
            "never named",
        )
        expect_survivor(
            "a mutation whose find no longer matches is reported as stale",
            verdicts(scratch, entry("stale", "no-such-text", "x", "guard did not hold")),
            "the mutation is stale",
        )
        expect_survivor(
            "a mutation matching more often than declared is reported as stale",
            verdicts(scratch, entry("ambiguous", "e", "x", "guard did not hold")),
            "the mutation is stale",
        )

        if scratch.read("verdict.txt") == "green\n":
            ok("every mutation is restored before the next one")
        else:
            bad("every mutation is restored before the next one", scratch.read("verdict.txt"))

        baseline_lines = [f"baseline diagnostic {number}" for number in range(12)]
        scratch.seed("verdict.txt", "\n".join(baseline_lines) + "\n")
        report = verdicts(scratch, entry("baseline", "broken", "x", "anything"))
        baseline_report = report.stream.getvalue()
        if (report.failures and report.failures[0][1] == "<baseline>" and
                baseline_lines[0] in baseline_report and baseline_lines[-1] in baseline_report):
            ok("a self-test that is already red is reported instead of mutated")
        else:
            bad("a self-test that is already red is reported instead of mutated")

    manifest_rejects(
        "a mutation that replaces its find with itself is rejected",
        '{"self_tests": [{"id": "a", "verifies": "v", "command": ["true"], "platforms": ["linux"],'
        ' "mutations": [{"name": "m", "file": "f", "find": "x", "replace": "x", "expect": "e"}]}]}',
    )
    manifest_rejects(
        "a self-test declaring no mutations is rejected",
        '{"self_tests": [{"id": "a", "verifies": "v", "command": ["true"], "platforms": ["linux"], "mutations": []}]}',
    )
    manifest_rejects(
        "a mutation with no expected assertion is rejected",
        '{"self_tests": [{"id": "a", "verifies": "v", "command": ["true"], "platforms": ["linux"],'
        ' "mutations": [{"name": "m", "file": "f", "find": "x", "replace": "y"}]}]}',
    )

    if shutil.which("git"):
        ok("git is available for the tracked-tree digest")
    else:
        bad("git is available for the tracked-tree digest")

    print(f"\nmutation gate self-test: {PASS} passed, {FAIL} failed")
    return 1 if FAIL else 0
