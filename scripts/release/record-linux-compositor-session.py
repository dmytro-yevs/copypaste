#!/usr/bin/env python3
"""Record receipt-bound proof of the private compositor process used by Wayland CI."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path


SAFE = re.compile(r"^[A-Za-z0-9._/+@-]{1,255}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def read_json(path: Path) -> dict:
    if not path.is_file() or path.is_symlink():
        raise ValueError("required compositor evidence input is unsafe")
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("required compositor evidence input is invalid")
    return value


def mapped_files(pid: int, runtime: Path, payload: dict[str, dict]) -> list[dict]:
    result = []
    seen = set()
    for line in (Path("/proc") / str(pid) / "maps").read_text(encoding="utf-8").splitlines():
        fields = line.split(maxsplit=5)
        if len(fields) != 6 or not fields[5].startswith("/"):
            continue
        path = Path(fields[5].removesuffix(" (deleted)"))
        try:
            relative = path.resolve(strict=True).relative_to(runtime.resolve(strict=True)).as_posix()
        except (FileNotFoundError, ValueError):
            continue
        if relative in seen or not SAFE.fullmatch(relative) or relative not in payload:
            continue
        expected = payload[relative]
        if expected.get("type") != "file" or expected.get("sha256") != digest(path):
            raise ValueError("mapped compositor library differs from its runtime receipt")
        seen.add(relative)
        result.append({"path": relative, "sha256": expected["sha256"]})
    return sorted(result, key=lambda item: item["path"])


def record(binding_path: Path, root: Path, pid: int, session: str, output: Path) -> None:
    if session != "wayland" or pid <= 1 or output.exists() or output.is_symlink():
        raise ValueError("invalid Wayland compositor session record request")
    binding = read_json(binding_path)
    required = {"schema", "producer_run_id", "commit", "runtime_id", "desktop", "architecture", "distribution", "format", "package", "runtime_receipt"}
    if set(binding) != required or binding.get("schema") != 1 or binding.get("desktop") not in {"GNOME", "KDE"}:
        raise ValueError("compositor binding is invalid")
    runtime = root / "usr/lib/copypaste/compositor-runtime" / binding["runtime_id"]
    receipt_path = root / "usr/share/copypaste/compositor-runtime" / f"{binding['runtime_id']}.receipt.json"
    receipt = read_json(receipt_path)
    if receipt.get("runtime_id") != binding["runtime_id"] or receipt.get("desktop") != binding["desktop"]:
        raise ValueError("installed runtime receipt does not match binding")
    qualification = receipt.get("qualification")
    if not isinstance(qualification, dict) or qualification.get("kind") != "headless" or not isinstance(qualification.get("entrypoint"), str):
        raise ValueError("runtime receipt has no headless qualification entrypoint")
    payload = {item.get("path"): item for item in receipt.get("payload", []) if isinstance(item, dict) and isinstance(item.get("path"), str)}
    if qualification["entrypoint"] not in payload:
        raise ValueError("qualification entrypoint is not receipt-listed")
    executable = (Path("/proc") / str(pid) / "exe").resolve(strict=True)
    try:
        executable_relative = executable.relative_to(runtime.resolve(strict=True)).as_posix()
    except ValueError as error:
        raise ValueError("Wayland compositor executable is outside the private runtime") from error
    if executable_relative not in payload or payload[executable_relative].get("sha256") != digest(executable):
        raise ValueError("Wayland compositor executable differs from private runtime receipt")
    mapped = mapped_files(pid, runtime, payload)
    library_marker = "libmutter" if binding["desktop"] == "GNOME" else "libkwin"
    if not any(library_marker in item["path"].lower() for item in mapped):
        raise ValueError("private compositor library was not mapped by the Wayland process")
    output.write_text(json.dumps({
        "schema": 1, "binding_sha256": digest(binding_path), "session": session,
        "runtime_id": binding["runtime_id"], "desktop": binding["desktop"], "pid": pid,
        "executable": {"path": executable_relative, "sha256": payload[executable_relative]["sha256"]},
        "mapped_private_libraries": mapped,
    }, sort_keys=True) + "\n", encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binding", required=True, type=Path)
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--pid", required=True, type=int)
    parser.add_argument("--session", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    record(args.binding, args.root, args.pid, args.session, args.output)


if __name__ == "__main__":
    main()
