#!/usr/bin/env python3
"""Verify that a staged companion runtime cannot replace a vendor compositor."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from stage_runtime import ContractError, read_receipt, validate_payload, validate_receipt


FORBIDDEN_PATHS = (
    "usr/bin/kwin_wayland", "usr/bin/kwin_x11", "usr/bin/gnome-shell",
    "usr/lib/systemd", "etc/alternatives", "var/lib/dpkg/alternatives",
)


def verify(root: Path, runtime_id: str) -> None:
    root = root.resolve(strict=True)
    if any((root / path).exists() or (root / path).is_symlink() for path in FORBIDDEN_PATHS):
        raise ContractError("runtime package contains a forbidden vendor replacement path")
    receipt_path = root / "usr/share/copypaste/compositor-runtime" / f"{runtime_id}.receipt.json"
    receipt = read_receipt(receipt_path)
    validate_receipt(receipt)
    if receipt["runtime_id"] != runtime_id:
        raise ContractError("runtime receipt identity differs from package identity")
    runtime = root / "usr/lib/copypaste/compositor-runtime" / runtime_id
    validate_payload(runtime, receipt)
    launcher = root / "usr/lib/copypaste/compositor-runtime/bin" / f"copypaste-compositor-session-{runtime_id}"
    session = root / "usr/share/wayland-sessions" / f"copypaste-{runtime_id}.desktop"
    if not launcher.is_file() or launcher.is_symlink() or launcher.stat().st_mode & 0o111 == 0:
        raise ContractError("runtime session launcher is missing or not executable")
    if not session.is_file() or session.is_symlink():
        raise ContractError("runtime session descriptor is missing")
    text = session.read_text(encoding="utf-8")
    if f"Exec=/usr/lib/copypaste/compositor-runtime/bin/copypaste-compositor-session-{runtime_id}" not in text:
        raise ContractError("session descriptor does not start the owned launcher")
    launcher_text = launcher.read_text(encoding="utf-8")
    for forbidden in ("eval ", "exec /usr/bin/kwin_wayland", "systemctl", "restart"):
        if forbidden in launcher_text:
            raise ContractError("session launcher contains forbidden control flow")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--runtime-id", required=True)
    args = parser.parse_args()
    try:
        verify(args.root, args.runtime_id)
    except (ContractError, OSError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print("verified opt-in sidecar compositor runtime package")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
