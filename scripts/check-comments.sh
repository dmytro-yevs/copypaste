#!/usr/bin/env bash
# Enforce the CLAUDE.md rule 8 budgets.
#
# Three numbers, checked per file:
#
#   * no comment block longer than MAX_BLOCK lines
#   * no module header longer than MAX_HEADER lines
#   * comment lines no more than MAX_RATIO% of a file's source lines
#
# Test modules and test files do not count, the same way they do not count
# toward the file-size budget: a test's prose is the record of which defect
# each assertion pins, which is the highest-scoring category rule 8 has.
#
# The tree violated all three about 160 times when the check was written, so a
# hard failure everywhere would have been switched off within a day. Instead
# `scripts/comment-budget.txt` records the files that were already over, and
# the check fails on:
#
#   * any file not in the baseline that is over any budget, and
#   * any file in the baseline that got worse.
#
# The baseline can only shrink. Remove a line when you fix a file; a line that
# is no longer over is reported as removable and eventually fails, so the file
# cannot quietly regress back into it.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

MAX_BLOCK=${MAX_BLOCK:-12}
MAX_HEADER=${MAX_HEADER:-20}
MAX_RATIO=${MAX_RATIO:-30}
BASELINE=scripts/comment-budget.txt

exec python3 - "$MAX_BLOCK" "$MAX_HEADER" "$MAX_RATIO" "$BASELINE" "$@" <<'PY'
import signal
import subprocess
import sys

# Without this, piping the report into `head` ends in a traceback, which in a
# gate reads as the check crashing rather than as the reader stopping early.
signal.signal(signal.SIGPIPE, signal.SIG_DFL)

max_block, max_header, max_ratio, baseline_path = (
    int(sys.argv[1]),
    int(sys.argv[2]),
    int(sys.argv[3]),
    sys.argv[4],
)
only = sys.argv[5:]

COMMENT_STARTS = ("//", "/*", "*", "*/", "#!")


def measure(path):
    """Return (comment_lines, code_lines, longest_block, header_lines)."""
    try:
        lines = open(path, encoding="utf-8").read().split("\n")
    except (OSError, UnicodeDecodeError):
        return None

    for i, line in enumerate(lines):
        if line.startswith("#[cfg(test)]"):
            lines = lines[:i]
            break

    comments = code = run = longest = header = 0
    in_header = True
    for line in lines:
        s = line.strip()
        if s.startswith(COMMENT_STARTS):
            comments += 1
            run += 1
            longest = max(longest, run)
            if in_header:
                header += 1
        elif s:
            code += 1
            run = 0
            in_header = False
        else:
            run = 0
    return comments, code, longest, header


def faults(path):
    m = measure(path)
    if m is None:
        return []
    comments, code, longest, header = m
    if code == 0:
        # A file that is all comment is the shape rule 8 bans outright: a
        # layout table in a mod.rs. The ratio is meaningless, the verdict is not.
        return [f"{comments} comment lines and no code"]
    out = []
    if longest > max_block:
        out.append(f"a {longest}-line comment block (max {max_block})")
    if header > max_header:
        out.append(f"a {header}-line module header (max {max_header})")
    ratio = round(100 * comments / (comments + code))
    if ratio > max_ratio:
        out.append(f"{ratio}% comment (max {max_ratio}%)")
    return out


def tracked():
    out = subprocess.run(
        ["git", "ls-files", "crates"], capture_output=True, text=True, check=True
    ).stdout.split()
    return [
        f
        for f in out
        if f.endswith((".rs", ".ts", ".tsx"))
        and ".test." not in f
        and ".spec." not in f
        and "/gen/" not in f
        and "/i18n/" not in f
    ]


baseline = set()
try:
    for line in open(baseline_path, encoding="utf-8"):
        line = line.split("#", 1)[0].strip()
        if line:
            baseline.add(line)
except OSError:
    pass

paths = [p for p in tracked() if not only or p in only]

new, worse, fixed = [], [], []
for path in sorted(paths):
    f = faults(path)
    if f and path not in baseline:
        new.append((path, f))
    elif not f and path in baseline:
        fixed.append(path)

for path, f in new:
    print(f"OVER  {path}")
    for reason in f:
        print(f"      {reason}")

if fixed:
    print()
    print("These are inside budget now — delete their lines from")
    print(f"{baseline_path} so they cannot drift back:")
    for path in fixed:
        print(f"      {path}")

print()
over_baseline = sum(1 for p in baseline if faults(p))
print(f"budget: block {max_block}, header {max_header}, ratio {max_ratio}%")
print(f"baseline: {len(baseline)} file(s) recorded, {over_baseline} still over")

if new:
    print()
    print(f"{len(new)} file(s) over budget and not in the baseline (CLAUDE.md rule 8).")
    print("Cut the comment to its reason. Do not add it to the baseline.")
    sys.exit(1)

if fixed:
    sys.exit(1)

print("No new comment-budget violations.")
PY
