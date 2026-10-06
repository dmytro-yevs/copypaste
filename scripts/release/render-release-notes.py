#!/usr/bin/env python3
"""Render the shared download table and reject links to missing artifacts."""

import argparse
from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[2]


def render(version: str, repository: str, artifacts: Path) -> str:
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("release version must be stable SemVer")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("repository must be owner/name")
    base = f"https://github.com/{repository}/releases/download/v{version}"
    notes = (ROOT / "packaging/release-template.md").read_text(encoding="utf-8")
    notes = notes.replace("{{VERSION}}", version).replace("{{DOWNLOAD_BASE}}", base)
    notes = notes.replace(
        "{{CHANGES}}",
        (ROOT / "packaging/release-notes.md").read_text(encoding="utf-8").strip(),
    )
    if "{{" in notes:
        raise ValueError("release template contains unresolved placeholders")
    for filename in re.findall(re.escape(base) + r"/([^\s)]+)", notes):
        if not (artifacts / filename).is_file():
            raise ValueError(f"download table artifact is missing: {filename}")
    return notes.rstrip() + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--artifacts", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    args.output.write_text(
        render(args.version, args.repository, args.artifacts), encoding="utf-8"
    )


if __name__ == "__main__":
    main()
