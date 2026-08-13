#!/usr/bin/env python3
import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
MARKERS = re.compile(r"\b(?:TO" + r"DO|FIX" + r"ME|HA" + r"CK|X" + r"XX)\b", re.IGNORECASE)
LINK = re.compile(r"\[[^]]*\]\(([^)]+)\)")

IPC_LIB = ROOT / "crates" / "copypaste-ipc" / "src" / "lib.rs"
PROTOCOL_RE = re.compile(r"pub\s+const\s+PROTOCOL_VERSION\s*:\s*u32\s*=\s*(\d+)\s*;")

PROTOCOL_DOC_SITES = {
    "docs/rewrite/target-architecture.md": re.compile(
        r"`PROTOCOL_VERSION`\s+is\s+`(\d+)`"
    ),
    "docs/rewrite/port-manifest/06-ui-behaviour.md": re.compile(
        r"`CURRENT_PROTOCOL_VERSION\s*=\s*(\d+)`"
    ),
}


def ipc_protocol_version():
    text = IPC_LIB.read_text(encoding="utf-8")
    m = PROTOCOL_RE.search(text)
    if not m:
        raise SystemExit(f"Cannot parse PROTOCOL_VERSION from {IPC_LIB}")
    return int(m.group(1))


def check_protocol_docs(runtime_version, *, inject=None):
    """Verify docs cite the runtime protocol version.

    *inject* replaces file content for the self-test.
    """
    errs = []
    for rel_path, pattern in PROTOCOL_DOC_SITES.items():
        if inject and rel_path in inject:
            text = inject[rel_path]
        else:
            text = (ROOT / rel_path).read_text(encoding="utf-8-sig")
        m = pattern.search(text)
        if not m:
            errs.append(f"{rel_path}: protocol-version reference not found")
        elif int(m.group(1)) != runtime_version:
            errs.append(
                f"{rel_path}: protocol version {m.group(1)} does not match "
                f"runtime PROTOCOL_VERSION ({runtime_version})"
            )
    return errs


def tracked_files():
    roots = (ROOT / ".github" / "workflows", ROOT / "scripts", ROOT / "docs")
    suffixes = {".md", ".py", ".sh", ".yml", ".yaml"}
    for root in roots:
        for path in root.rglob("*"):
            if path.is_file() and path.suffix in suffixes:
                yield path


def main():
    errors = []

    pv = ipc_protocol_version()
    errors.extend(check_protocol_docs(pv))

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


if __name__ == "__main__":
    main()
