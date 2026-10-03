#!/usr/bin/env python3
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
ACTIVE = (ROOT / "README.md", ROOT / "SECURITY.md", ROOT / "docs" / "README.md", ROOT / "docs" / "development.md", ROOT / "docs" / "flutter-foundation.md", ROOT / "docs" / "release-qualification.md")
REMOVED = re.compile(r"(?:crates/copypaste-ui|src-tauri|e2e-android|e2e/|package-lock\.json|npm run|tauri-driver)")
LINK = re.compile(r"\[[^]]*\]\(([^)]+)\)")


def local_link_errors(path: pathlib.Path, root: pathlib.Path) -> list[str]:
    errors = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        for target in LINK.findall(line):
            target = target.split("#", 1)[0]
            if not target or "://" in target or target.startswith("mailto:"):
                continue
            if not (path.parent / target).resolve().is_file():
                errors.append(f"{path.relative_to(root)}:{number}: missing local link {target}")
    return errors

def main():
    failures = []
    for path in ACTIVE:
        if not path.is_file():
            failures.append(f"missing active document: {path.relative_to(ROOT)}")
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if REMOVED.search(line):
                failures.append(f"{path.relative_to(ROOT)}:{number}: removed implementation reference")
        failures.extend(local_link_errors(path, ROOT))
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("active documentation contains no removed implementation paths")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
