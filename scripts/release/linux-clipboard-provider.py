#!/usr/bin/env python3
"""Validate one private clipboard manifest and execute the GTK3 provider."""

import argparse
import json
import os
import re
import shlex
import subprocess
from pathlib import Path


MAX_MANIFEST_BYTES = 64 * 1024
MAX_PAYLOAD_BYTES = 32 * 1024 * 1024
MAX_OFFERS = 16
MIME = re.compile(r"^[A-Za-z0-9!#$&^_.+\-/;=]{1,255}$")
APPLICATION_ID = re.compile(r"^[A-Za-z0-9_.-]{1,255}$")


def regular(path: Path, maximum: int) -> None:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > maximum:
        raise ValueError("clipboard provider input is not a bounded regular file")


def load_manifest(path: Path) -> tuple[str, list[tuple[str, Path]]]:
    regular(path, MAX_MANIFEST_BYTES)
    document = json.loads(path.read_bytes())
    if not isinstance(document, dict) or set(document) != {"application_id", "offers"}:
        raise ValueError("clipboard manifest has an invalid shape")
    application_id = document["application_id"]
    offers = document["offers"]
    if not isinstance(application_id, str) or not APPLICATION_ID.fullmatch(application_id):
        raise ValueError("clipboard source application id is invalid")
    if not isinstance(offers, list) or not (1 <= len(offers) <= MAX_OFFERS):
        raise ValueError("clipboard offer count is invalid")

    result = []
    seen = set()
    for offer in offers:
        if not isinstance(offer, dict) or set(offer) != {"mime", "path"}:
            raise ValueError("clipboard offer has an invalid shape")
        mime = offer["mime"]
        payload = Path(offer["path"])
        if not isinstance(mime, str) or not MIME.fullmatch(mime) or mime in seen:
            raise ValueError("clipboard MIME is invalid")
        regular(payload, MAX_PAYLOAD_BYTES)
        seen.add(mime)
        result.append((mime, payload))
    return application_id, result


def build(output: Path) -> None:
    source = Path(__file__).with_suffix(".c")
    regular(source, 128 * 1024)
    if output.exists() or output.is_symlink():
        raise ValueError("GTK clipboard provider output already exists")
    output.parent.mkdir(parents=True, exist_ok=True)
    flags = subprocess.run(
        ["pkg-config", "--cflags", "--libs", "gtk+-3.0"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if flags.returncode:
        raise RuntimeError("GTK3 development files are unavailable")
    temporary = output.with_name(f".{output.name}.tmp")
    result = subprocess.run(
        ["cc", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", str(source), "-o", str(temporary), *shlex.split(flags.stdout)],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode:
        temporary.unlink(missing_ok=True)
        raise RuntimeError("could not compile the GTK3 clipboard provider")
    os.chmod(temporary, 0o700)
    os.replace(temporary, output)


def run(helper: Path, manifest: Path, ready: Path, activity: Path, hold_seconds: int) -> None:
    if not helper.is_file() or helper.is_symlink() or not os.access(helper, os.X_OK):
        raise ValueError("GTK clipboard provider helper is unavailable")
    application_id, offers = load_manifest(manifest)
    for path in (ready, activity):
        if path.exists() or path.is_symlink():
            raise ValueError("clipboard provider output already exists")
        path.parent.mkdir(parents=True, exist_ok=True)
    arguments = [
        str(helper), "--application-id", application_id, "--ready-file", str(ready),
        "--activity-log", str(activity), "--hold-seconds", str(hold_seconds),
    ]
    for mime, payload in offers:
        arguments.extend(("--offer", mime, str(payload)))
    os.execv(str(helper), arguments)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-helper", type=Path)
    parser.add_argument("--helper", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--ready-file", type=Path)
    parser.add_argument("--activity-log", type=Path)
    parser.add_argument("--hold-seconds", type=int, default=20)
    args = parser.parse_args()
    if args.build_helper is not None:
        if any(value is not None for value in (args.helper, args.manifest, args.ready_file, args.activity_log)):
            raise ValueError("GTK clipboard provider build mode has no runtime arguments")
        build(args.build_helper)
        return 0
    if args.helper is None or args.manifest is None or args.ready_file is None or args.activity_log is None:
        raise ValueError("GTK clipboard provider runtime arguments are incomplete")
    if not 1 <= args.hold_seconds <= 60:
        raise ValueError("clipboard provider hold duration is invalid")
    run(args.helper, args.manifest, args.ready_file, args.activity_log, args.hold_seconds)
    raise AssertionError("execv returned")


if __name__ == "__main__":
    raise SystemExit(main())
