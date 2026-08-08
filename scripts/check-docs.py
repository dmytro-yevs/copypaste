#!/usr/bin/env python3
import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
MARKERS = re.compile(r"\b(?:TO" + r"DO|FIX" + r"ME|HA" + r"CK|X" + r"XX)\b", re.IGNORECASE)
LINK = re.compile(r"\[[^]]*\]\(([^)]+)\)")


def tracked_files():
    roots = (ROOT / ".github" / "workflows", ROOT / "scripts", ROOT / "docs")
    suffixes = {".md", ".py", ".sh", ".yml", ".yaml"}
    for root in roots:
        for path in root.rglob("*"):
            if path.is_file() and path.suffix in suffixes:
                yield path


errors = []
for path in tracked_files():
    relative = path.relative_to(ROOT).as_posix()
    text = path.read_text(encoding="utf-8-sig")
    historical = relative.startswith("docs/rewrite/port-manifest/")
    # The scripts that detect these markers have to spell them out.
    marker_example = relative in {
        "scripts/check-commit-msg.sh",
        "scripts/check-feature-ledger.py",
    }
    if not historical and not marker_example:
        for number, line in enumerate(text.splitlines(), 1):
            if MARKERS.search(line):
                errors.append(f"{relative}:{number}: unfinished-work marker: {line.strip()}")
    if path.suffix != ".md":
        continue
    for match in LINK.finditer(text):
        target = match.group(1).split("#", 1)[0]
        if not target or "://" in target or target.startswith("mailto:"):
            continue
        if not (path.parent / target).resolve().exists():
            number = text.count("\n", 0, match.start()) + 1
            errors.append(f"{relative}:{number}: missing local link target: {target}")

if errors:
    print("\n".join(errors))
    sys.exit(1)
print("Documentation links and unfinished-work markers are clean.")
