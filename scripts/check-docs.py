#!/usr/bin/env python3
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
ACTIVE = ("README.md", "SECURITY.md")
REMOVED = re.compile(r"(?:crates/copypaste-ui|src-tauri|e2e-android|e2e/|package-lock\.json|npm run|tauri-driver)")
LINK = re.compile(r"\[[^]]*\]\(([^)]+)\)")


def local_link_errors(path: pathlib.Path, root: pathlib.Path, tracked: set[pathlib.Path]) -> list[str]:
    errors = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        for target in LINK.findall(line):
            target = target.split("#", 1)[0]
            if not target or "://" in target or target.startswith("mailto:"):
                continue
            destination = (path.parent / target).resolve()
            if not destination.is_file():
                errors.append(f"{path.relative_to(root)}:{number}: missing local link {target}")
            elif destination not in tracked:
                errors.append(f"{path.relative_to(root)}:{number}: local link is not tracked: {target}")
    return errors

def check(root: pathlib.Path, tracked: set[pathlib.Path]) -> list[str]:
    failures = []
    for name in ACTIVE:
        path = root / name
        if not path.is_file() or path not in tracked:
            failures.append(f"missing tracked document: {name}")
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if REMOVED.search(line):
                failures.append(f"{path.relative_to(root)}:{number}: removed implementation reference")
        failures.extend(local_link_errors(path, root, tracked))
    return failures

def main():
    tracked = {
        (ROOT / name).resolve()
        for name in subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0")
        if name
    }
    failures = check(ROOT, tracked)
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("active documentation contains no removed implementation paths")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
