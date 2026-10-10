#!/usr/bin/env python3
"""Print bounded RPM closure provenance for a disposable Fedora smoke run."""

from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]


def closure_module():
    spec = importlib.util.spec_from_file_location(
        "copypaste_private_elf_closure",
        ROOT / "packaging/linux/compositor-runtime/private_elf_closure.py",
    )
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", required=True, type=Path)
    parser.add_argument("--manifest", type=Path)
    args = parser.parse_args()
    module = closure_module()
    if args.manifest is None:
        name, evr, source_rpm, license_expression = module.rpm_owner(args.library)
        print(json.dumps({
            "name": name,
            "evr": evr,
            "source_rpm": source_rpm,
            "license_length": len(license_expression),
        }, sort_keys=True))
        return 0
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    print(json.dumps({
        "libraries": len(manifest["libraries"]),
        "packages": len(manifest["packages"]),
        "license_files": len(manifest["licenses"]),
        "license_owners": sorted({item["license_package"] for item in manifest["licenses"]}),
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
