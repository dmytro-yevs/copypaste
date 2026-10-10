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
from typing import Optional


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


def append_environment(path: Path, value: str) -> None:
    try:
        details = path.lstat()
    except OSError as error:
        raise RuntimeError("GitHub environment output is unavailable") from error
    if stat.S_ISLNK(details.st_mode) or not stat.S_ISREG(details.st_mode) or details.st_uid != os.getuid():
        raise RuntimeError("GitHub environment output is not a regular file owned by this job user")
    descriptor = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CLOEXEC)
    try:
        current = os.fstat(descriptor)
        if not stat.S_ISREG(current.st_mode) or current.st_uid != os.getuid():
            raise RuntimeError("GitHub environment output changed while opening it")
        payload = value.encode("utf-8")
        if os.write(descriptor, payload) != len(payload):
            raise RuntimeError("could not append qualification environment")
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


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


def write_state(path: Path, state: dict) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC, 0o600)
    created = os.fstat(descriptor)
    try:
        payload = (json.dumps(state, separators=(",", ":")) + "\n").encode("utf-8")
        if os.write(descriptor, payload) != len(payload):
            raise RuntimeError("could not persist V4L2 qualification state")
        os.fsync(descriptor)
    except Exception:
        os.close(descriptor)
        try:
            current = path.lstat()
            if current.st_dev == created.st_dev and current.st_ino == created.st_ino:
                path.unlink()
        finally:
            raise
    else:
        os.close(descriptor)


def state_base(uid: int, gid: int, phase: str) -> dict:
    return {"schema": STATE_SCHEMA, "module": MODULE, "device": str(DEVICE), "label": LABEL, "uid": uid, "gid": gid, "phase": phase}


def read_live_device() -> dict:
    details = DEVICE.stat()
    if not stat.S_ISCHR(details.st_mode):
        raise RuntimeError("owned V4L2 device mode or type differs")
    label = Path(f"/sys/class/video4linux/video{DEVICE_NUMBER}/name").read_text(encoding="utf-8").strip()
    exclusive = Path(f"/sys/module/{MODULE}/parameters/exclusive_caps").read_text(encoding="utf-8").strip()
    sysfs_dev = Path(f"/sys/class/video4linux/video{DEVICE_NUMBER}/dev").read_text(encoding="utf-8").strip()
    if label != LABEL or exclusive not in {"1", "Y", "y"} or sysfs_dev != f"{os.major(details.st_rdev)}:{os.minor(details.st_rdev)}":
        raise RuntimeError("owned V4L2 live device contract differs")
    return {"major": os.major(details.st_rdev), "minor": os.minor(details.st_rdev), "uid": details.st_uid, "gid": details.st_gid, "mode": f"{stat.S_IMODE(details.st_mode):04o}"}


def replace_state(path: Path, value: dict) -> None:
    temporary = path.with_name(path.name + ".next")
    if temporary.exists() or temporary.is_symlink():
        raise RuntimeError("V4L2 state transition path already exists")
    write_state(temporary, value)
    try:
        os.replace(temporary, path)
    except Exception:
        temporary.unlink(missing_ok=True)
        raise


def unwind_provision(state: Path, module_created: bool, expected_live: Optional[dict] = None) -> None:
    owned = read_state(state)
    if module_created:
        if not Path(f"/sys/module/{MODULE}").exists():
            raise RuntimeError("owned V4L2 module changed before it could be unwound")
        live = read_live_device()
        expected = expected_live or owned.get("live")
        if not isinstance(expected, dict) or live != expected:
            raise RuntimeError("owned V4L2 live device no longer matches this job state")
        command(["sudo", "modprobe", "-r", MODULE])
        if DEVICE.exists() or Path(f"/sys/module/{MODULE}").exists():
            raise RuntimeError("owned V4L2 loopback module did not unload")
    state.unlink()


def provision(state: Path, environment: Path) -> None:
    require_ci()
    regular_state(state)
    append_environment(environment, "")
    assert_unowned_target()
    kernel = platform.release()
    if not re.fullmatch(r"[A-Za-z0-9._+-]+", kernel):
        raise RuntimeError("kernel release is unsafe")
    command(["sudo", "apt-get", "update"])
    command(["sudo", "apt-get", "install", "--yes", "--no-install-recommends", f"linux-headers-{kernel}", "v4l2loopback-dkms", "v4l-utils"])
    uid = os.getuid(); gid = os.getgid()
    write_state(state, state_base(uid, gid, "prepared"))
    created = False
    live = None
    try:
        command(["sudo", "modprobe", MODULE, f"video_nr={DEVICE_NUMBER}", f"card_label={LABEL}", "exclusive_caps=1"])
        created = True
        if not device_is_character(DEVICE):
            raise RuntimeError("v4l2loopback did not create the requested character device")
        live = read_live_device()
        replace_state(state, {**state_base(uid, gid, "loaded"), "live": live})
        command(["sudo", "chown", f"{uid}:{gid}", str(DEVICE)])
        live = read_live_device()
        command(["sudo", "chmod", "600", str(DEVICE)])
        live = read_live_device()
        if live["uid"] != uid or live["gid"] != gid or live["mode"] != "0600":
            raise RuntimeError("owned V4L2 device permissions differ after job-user grant")
        replace_state(state, {**state_base(uid, gid, "active"), "live": live})
        append_environment(environment, f"COPYPASTE_QUALIFICATION_V4L2_DEVICE={DEVICE}\nCOPYPASTE_QUALIFICATION_V4L2_STATE={state}\n")
    except Exception:
        unwind_provision(state, created, live)
        raise


def read_state(path: Path) -> dict:
    if not path.is_file() or path.is_symlink():
        raise RuntimeError("owned V4L2 state is unavailable")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise RuntimeError("owned V4L2 state is invalid") from error
    base = state_base(os.getuid(), os.getgid(), value.get("phase")) if isinstance(value, dict) else {}
    if (not isinstance(value, dict) or value.get("phase") not in {"prepared", "loaded", "active"}
            or any(value.get(key) != expected for key, expected in base.items())
            or (value["phase"] in {"loaded", "active"} and (set(value) != set(base) | {"live"} or not isinstance(value.get("live"), dict)))
            or (value["phase"] == "prepared" and set(value) != set(base))):
        raise RuntimeError("owned V4L2 state does not describe this job's device")
    return value


def cleanup(state: Path, missing_ok: bool) -> None:
    if not state.exists() and missing_ok:
        return
    owned = read_state(state)
    if owned["phase"] != "active":
        raise RuntimeError("owned V4L2 provisioning did not reach an active state")
    unwind_provision(state, True)


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
