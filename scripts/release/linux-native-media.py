#!/usr/bin/env python3
"""Real Linux media inputs for exact-package desktop qualification.

This helper deliberately provides only two small capabilities:

* drive a named AT-SPI action on the installed application's accessible tree;
* render a pairing URI as a QR image and feed it to an owned virtual V4L2 node.

It never reports a product assertion.  The scenario driver must still observe
the History item or pairing state through the installed application's public
surface before it can produce qualification evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any, Iterable


MAX_PAYLOAD_BYTES = 8192
MAX_TIMEOUT_SECONDS = 20
MEDIA_SCHEMA = 1
VIDEO_DEVICE = re.compile(r"/dev/video[0-9]+$")


class MediaQualificationError(RuntimeError):
    """A required native media interaction was unavailable or unsuccessful."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def regular_file(path: Path, *, maximum_size: int = MAX_PAYLOAD_BYTES) -> os.stat_result:
    try:
        details = path.lstat()
    except OSError as error:
        raise MediaQualificationError("media input is unavailable") from error
    if stat.S_ISLNK(details.st_mode) or not stat.S_ISREG(details.st_mode):
        raise MediaQualificationError("media input must be a regular non-symlink file")
    if details.st_size <= 0 or details.st_size > maximum_size:
        raise MediaQualificationError("media input has an invalid size")
    return details


def read_pairing_uri(path: Path) -> tuple[str, dict[str, int | str]]:
    details = regular_file(path)
    try:
        raw = path.read_bytes()
        value = raw.decode("utf-8")
    except (OSError, UnicodeError) as error:
        raise MediaQualificationError("pairing URI must be UTF-8") from error
    if value.endswith("\n"):
        value = value[:-1]
    if not value or any(ord(character) < 0x20 or ord(character) == 0x7F for character in value):
        raise MediaQualificationError("pairing URI contains unsupported control characters")
    return value, {"sha256": hashlib.sha256(raw).hexdigest(), "size_bytes": details.st_size}


def redacted_record(kind: str, **values: Any) -> dict[str, Any]:
    """Build a structured record that contains no pairing URI, path, or bytes."""
    return {"schema": MEDIA_SCHEMA, "kind": kind, **values}


def emit(record: dict[str, Any]) -> None:
    print("COPYPASTE_QUALIFICATION_MEDIA " + json.dumps(record, separators=(",", ":"), sort_keys=True))


def write_exclusive(path: Path, value: str) -> None:
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC, 0o600)
    except OSError as error:
        raise MediaQualificationError("qualification runtime path already exists") from error
    try:
        encoded = value.encode("ascii")
        if os.write(descriptor, encoded) != len(encoded):
            raise MediaQualificationError("could not persist qualification runtime state")
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def accessible_children(node: Any) -> Iterable[Any]:
    try:
        count = node.childCount
    except Exception as error:  # pyatspi exposes several D-Bus exception classes.
        raise MediaQualificationError("AT-SPI tree is unavailable") from error
    for index in range(count):
        try:
            yield node.getChildAtIndex(index)
        except Exception:
            continue


def accessible_descendants(node: Any) -> Iterable[Any]:
    pending = [node]
    while pending:
        current = pending.pop()
        yield current
        pending.extend(reversed(tuple(accessible_children(current))))


def _node_pid(node: Any) -> int | None:
    try:
        application = node.get_application()
        pid = application.get_process_id()
    except Exception:
        return None
    return pid if isinstance(pid, int) and pid > 0 else None


def atspi_application(registry: Any, pid: int) -> Any:
    try:
        desktop = registry.getDesktop(0)
    except Exception as error:
        raise MediaQualificationError("AT-SPI desktop is unavailable") from error
    for application in accessible_children(desktop):
        if _node_pid(application) == pid:
            return application
    raise MediaQualificationError("installed GUI is absent from the AT-SPI desktop")


def matching_accessible(application: Any, name: str) -> Any | None:
    for node in accessible_descendants(application):
        try:
            if node.name == name:
                return node
        except Exception:
            continue
    return None


def action_index(node: Any, action_name: str) -> int | None:
    try:
        action = node.queryAction()
        count = action.nActions
    except Exception:
        return None
    for index in range(count):
        try:
            if action.getName(index) == action_name:
                return index
        except Exception:
            continue
    return None


def click_accessible(registry: Any, *, pid: int, target: str, expected: str, timeout_seconds: int) -> dict[str, Any]:
    application = atspi_application(registry, pid)
    button = matching_accessible(application, target)
    if button is None:
        raise MediaQualificationError("required installed UI control is absent")
    index = action_index(button, "click")
    if index is None:
        raise MediaQualificationError("required installed UI control is not actionable")
    try:
        performed = button.queryAction().doAction(index)
    except Exception as error:
        raise MediaQualificationError("AT-SPI click failed") from error
    if performed is not True:
        raise MediaQualificationError("AT-SPI click was not accepted")
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        if matching_accessible(application, expected) is not None:
            return redacted_record(
                "atspi_click",
                gui_pid=pid,
                target_name_sha256=hashlib.sha256(target.encode("utf-8")).hexdigest(),
                expected_name_sha256=hashlib.sha256(expected.encode("utf-8")).hexdigest(),
                action="click",
            )
        time.sleep(0.1)
    raise MediaQualificationError("installed UI did not reach the expected state")


def v4l2_device(path: Path) -> None:
    if not VIDEO_DEVICE.fullmatch(str(path)):
        raise MediaQualificationError("virtual camera path is invalid")
    try:
        details = path.stat()
    except OSError as error:
        raise MediaQualificationError("virtual camera device is unavailable") from error
    if not stat.S_ISCHR(details.st_mode):
        raise MediaQualificationError("virtual camera path is not a character device")


def camera_feed_command(*, image: Path, device: Path) -> list[str]:
    return [
        "ffmpeg", "-hide_banner", "-loglevel", "error", "-nostdin", "-stream_loop", "-1",
        "-framerate", "15", "-i", str(image), "-vf",
        "scale=640:480:force_original_aspect_ratio=decrease,pad=640:480:(ow-iw)/2:(oh-ih)/2:color=white,format=yuyv422",
        "-f", "v4l2",
        "-vcodec", "rawvideo", str(device),
    ]


def owned_ffmpeg_process(pid: int, device: str) -> bool:
    try:
        argv = Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0")
    except OSError:
        return False
    names = [Path(part.decode("utf-8", errors="replace")).name for part in argv if part]
    return bool(names) and names[0] == "ffmpeg" and device.encode("ascii") in argv and b"v4l2" in argv


def remove_owned_regular(path: Path, *, maximum_size: int) -> None:
    if not path.exists():
        return
    regular_file(path, maximum_size=maximum_size)
    path.unlink()


def stop_qr_feed(pid_file: Path) -> dict[str, Any]:
    regular_file(pid_file, maximum_size=512)
    try:
        state = json.loads(pid_file.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise MediaQualificationError("camera runtime state is invalid") from error
    if not isinstance(state, dict) or set(state) != {"pid", "device", "ready_file"}:
        raise MediaQualificationError("camera runtime state is invalid")
    pid = state["pid"]
    device = state["device"]
    ready_name = state["ready_file"]
    if (not isinstance(pid, int) or pid <= 0 or not isinstance(device, str) or not VIDEO_DEVICE.fullmatch(device)
            or not isinstance(ready_name, str) or Path(ready_name).name != ready_name):
        raise MediaQualificationError("camera runtime state is invalid")
    if not owned_ffmpeg_process(pid, device):
        raise MediaQualificationError("camera runtime is not the owned virtual-camera feed")
    try:
        os.kill(pid, 15)
    except OSError as error:
        raise MediaQualificationError("could not stop the owned virtual-camera feed") from error
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline:
        if not Path(f"/proc/{pid}").exists():
            break
        time.sleep(0.05)
    else:
        raise MediaQualificationError("owned virtual-camera feed did not stop")
    image = pid_file.parent / ".copypaste-qualification-camera.png"
    remove_owned_regular(image, maximum_size=4 * 1024 * 1024)
    remove_owned_regular(pid_file.parent / ready_name, maximum_size=64)
    pid_file.unlink()
    return redacted_record("v4l2_qr_feed_stopped", device=Path(device).name)


def start_qr_feed(*, uri_file: Path, device: Path, ready_file: Path) -> tuple[subprocess.Popen[bytes], dict[str, Any]]:
    """Feed a QR code to an existing, qualification-owned V4L2 loopback node.

    The caller owns process shutdown.  A device must already exist because this
    helper must never load or unload a kernel module on a shared runner.
    """
    uri, payload = read_pairing_uri(uri_file)
    v4l2_device(device)
    if ready_file.exists() or ready_file.is_symlink():
        raise MediaQualificationError("camera readiness path already exists")
    ready_file.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    retained = ready_file.parent / ".copypaste-qualification-camera.png"
    if retained.exists() or retained.is_symlink():
        raise MediaQualificationError("camera runtime image path already exists")
    with tempfile.TemporaryDirectory(prefix="copypaste-qualification-qr-") as directory:
        image = Path(directory) / "pairing.png"
        try:
            subprocess.run(
                ["qrencode", "--8bit", "--output", str(image), "--", uri],
                check=True,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
                timeout=10,
            )
        except (OSError, subprocess.SubprocessError) as error:
            raise MediaQualificationError("could not render a virtual-camera QR frame") from error
        # The PNG must survive for ffmpeg's lifetime. Move it to a private
        # directory owned by the caller rather than retaining URI material in
        # the evidence directory.
        os.replace(image, retained)
    command = camera_feed_command(image=retained, device=device)
    try:
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    except OSError as error:
        retained.unlink(missing_ok=True)
        raise MediaQualificationError("could not start virtual-camera QR feed") from error
    time.sleep(0.2)
    if process.poll() is not None:
        process.wait(timeout=1)
        retained.unlink(missing_ok=True)
        raise MediaQualificationError("virtual-camera QR feed exited before it became ready")
    try:
        write_exclusive(ready_file, "ready\n")
    except MediaQualificationError:
        process.terminate()
        process.wait(timeout=2)
        retained.unlink(missing_ok=True)
        raise
    return process, redacted_record(
        "v4l2_qr_feed",
        device=device.name,
        frame_format="yuyv422",
        source="camera_desktop",
        decoder="CameraPluginPairingSource/ZxingQrFrameDecoder",
        payload=payload,
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    click = commands.add_parser("click", help="perform one named AT-SPI click in the installed GUI")
    click.add_argument("--pid", required=True, type=int)
    click.add_argument("--target", required=True)
    click.add_argument("--expect", required=True)
    click.add_argument("--timeout-seconds", type=int, default=8)
    feed = commands.add_parser("feed-qr", help="feed a QR URI to an existing virtual camera")
    feed.add_argument("--uri-file", required=True, type=Path)
    feed.add_argument("--device", required=True, type=Path)
    feed.add_argument("--ready-file", required=True, type=Path)
    feed.add_argument("--pid-file", required=True, type=Path)
    stop = commands.add_parser("stop-qr", help="stop and remove one owned virtual-camera QR feed")
    stop.add_argument("--pid-file", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        if args.command == "click":
            if args.pid <= 0 or not 1 <= args.timeout_seconds <= MAX_TIMEOUT_SECONDS:
                raise MediaQualificationError("AT-SPI arguments are invalid")
            try:
                import pyatspi  # type: ignore[import-not-found]
            except ImportError as error:
                raise MediaQualificationError("python3-pyatspi is required for native UI qualification") from error
            emit(click_accessible(pyatspi.Registry, pid=args.pid, target=args.target, expected=args.expect, timeout_seconds=args.timeout_seconds))
            return 0
        if args.command == "stop-qr":
            emit(stop_qr_feed(args.pid_file))
            return 0
        process, record = start_qr_feed(uri_file=args.uri_file, device=args.device, ready_file=args.ready_file)
        if args.pid_file.exists() or args.pid_file.is_symlink():
            process.terminate()
            process.wait(timeout=2)
            remove_owned_regular(args.ready_file, maximum_size=64)
            remove_owned_regular(args.ready_file.parent / ".copypaste-qualification-camera.png", maximum_size=4 * 1024 * 1024)
            raise MediaQualificationError("camera PID path already exists")
        try:
            write_exclusive(args.pid_file, json.dumps({
                "pid": process.pid,
                "device": str(args.device),
                "ready_file": args.ready_file.name,
            }, separators=(",", ":")))
        except MediaQualificationError:
            process.terminate()
            process.wait(timeout=2)
            remove_owned_regular(args.ready_file, maximum_size=64)
            remove_owned_regular(args.ready_file.parent / ".copypaste-qualification-camera.png", maximum_size=4 * 1024 * 1024)
            raise
        emit(record)
        return 0
    except MediaQualificationError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
