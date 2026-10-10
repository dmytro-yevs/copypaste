#!/usr/bin/env python3
"""Provision one owned V4L2 loopback device for CI-native media qualification."""

import argparse
import json
import os
from pathlib import Path
import platform
import re
import stat
import subprocess


DEVICE_NUMBER = 42
DEVICE = Path(f"/dev/video{DEVICE_NUMBER}")
MODULE = "v4l2loopback"
LABEL = "CopyPasteQualification"
STATE_SCHEMA = 1


def command(arguments: list[str]) -> None:
    subprocess.run(arguments, check=True)


def require_ci() -> None:
    if platform.system() != "Linux" or os.environ.get("CI") != "true" or os.environ.get("COPYPASTE_QUALIFICATION_HEADLESS") != "1":
        raise RuntimeError("V4L2 loopback provisioning is limited to headless CI qualification")


def regular_state(path: Path) -> None:
    if path.exists() or path.is_symlink():
        raise RuntimeError("V4L2 qualification state path already exists")
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)


def device_is_character(path: Path) -> bool:
    try:
        return stat.S_ISCHR(path.stat().st_mode)
    except OSError:
        return False


def assert_unowned_target() -> None:
    if DEVICE.exists() or DEVICE.is_symlink() or Path(f"/sys/class/video4linux/video{DEVICE_NUMBER}").exists():
        raise RuntimeError("qualification V4L2 device number is already in use")
    if Path(f"/sys/module/{MODULE}").exists():
        raise RuntimeError("v4l2loopback was already loaded; refusing to alter a pre-existing module")


def write_state(path: Path, uid: int, gid: int) -> None:
    state = {"schema": STATE_SCHEMA, "module": MODULE, "device": str(DEVICE), "label": LABEL, "uid": uid, "gid": gid}
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as output:
        json.dump(state, output, separators=(",", ":"))
        output.write("\n")
        output.flush()
        os.fsync(output.fileno())


def provision(state: Path, environment: Path) -> None:
    require_ci()
    regular_state(state)
    if environment.exists() or environment.is_symlink():
        raise RuntimeError("qualification environment output already exists")
    assert_unowned_target()
    kernel = platform.release()
    if not re.fullmatch(r"[A-Za-z0-9._+-]+", kernel):
        raise RuntimeError("kernel release is unsafe")
    command(["sudo", "apt-get", "update"])
    command(["sudo", "apt-get", "install", "--yes", "--no-install-recommends", f"linux-headers-{kernel}", "v4l2loopback-dkms", "v4l-utils"])
    created = False
    try:
        command(["sudo", "modprobe", MODULE, f"video_nr={DEVICE_NUMBER}", f"card_label={LABEL}", "exclusive_caps=1"])
        created = True
        if not device_is_character(DEVICE):
            raise RuntimeError("v4l2loopback did not create the requested character device")
        uid = os.getuid()
        gid = os.getgid()
        command(["sudo", "chown", f"{uid}:{gid}", str(DEVICE)])
        command(["sudo", "chmod", "600", str(DEVICE)])
        write_state(state, uid, gid)
        environment.write_text(
            f"COPYPASTE_QUALIFICATION_V4L2_DEVICE={DEVICE}\nCOPYPASTE_QUALIFICATION_V4L2_STATE={state}\n",
            encoding="utf-8",
        )
    except Exception:
        if created and not state.exists():
            command(["sudo", "modprobe", "-r", MODULE])
        raise


def read_state(path: Path) -> dict:
    if not path.is_file() or path.is_symlink():
        raise RuntimeError("owned V4L2 state is unavailable")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise RuntimeError("owned V4L2 state is invalid") from error
    expected = {"schema": STATE_SCHEMA, "module": MODULE, "device": str(DEVICE), "label": LABEL, "uid": os.getuid(), "gid": os.getgid()}
    if value != expected:
        raise RuntimeError("owned V4L2 state does not describe this job's device")
    return value


def cleanup(state: Path, missing_ok: bool) -> None:
    if not state.exists() and missing_ok:
        return
    read_state(state)
    if not device_is_character(DEVICE) or not Path(f"/sys/module/{MODULE}").exists():
        raise RuntimeError("owned V4L2 device or module is no longer present")
    command(["sudo", "modprobe", "-r", MODULE])
    if DEVICE.exists() or Path(f"/sys/module/{MODULE}").exists():
        raise RuntimeError("owned V4L2 loopback module did not unload")
    state.unlink()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="action", required=True)
    provision_parser = actions.add_parser("provision")
    provision_parser.add_argument("--state", required=True, type=Path)
    provision_parser.add_argument("--environment", required=True, type=Path)
    cleanup_parser = actions.add_parser("cleanup")
    cleanup_parser.add_argument("--state", required=True, type=Path)
    cleanup_parser.add_argument("--missing-ok", action="store_true")
    args = parser.parse_args()
    try:
        if args.action == "provision":
            provision(args.state, args.environment)
        else:
            cleanup(args.state, args.missing_ok)
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=os.sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
