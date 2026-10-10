#!/usr/bin/env python3
"""Drive one external GTK file drag into the installed CopyPaste History UI.

The helper creates a separate GTK client that offers ``text/uri-list`` and
uses the session's native input path to move that drag into the installed
window.  It does not use CopyPaste IPC to import data, and it deliberately
leaves History verification to the scenario driver through the public CLI.
"""

from __future__ import annotations

import argparse
import importlib.util
import os
import re
import shutil
import stat
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


SOURCE = Path(__file__).with_name("linux-gtk-file-drop-source.c")
WAYLAND_QUICK_PASTE = Path(__file__).with_name("linux-wayland-quick-paste.py")
MAX_FIXTURE_BYTES = 8 * 1024
SOURCE_TITLE = "CopyPaste Qualification File Drop Source"
TARGET_TITLE = "CopyPaste"
LEFT_BUTTON = 272
POINTER_DEVICE = 2


class FileDragQualificationError(RuntimeError):
    """A real external drag could not reach the installed application."""


@dataclass(frozen=True)
class Bounds:
    x: int
    y: int
    width: int
    height: int

    def point(self, horizontal: float = 0.5, vertical: float = 0.5) -> tuple[int, int]:
        return (self.x + max(1, int(self.width * horizontal)), self.y + max(1, int(self.height * vertical)))


def require_regular(path: Path, description: str, maximum: int = 256 * 1024) -> Path:
    try:
        details = path.lstat()
    except OSError as error:
        raise FileDragQualificationError(f"{description} is unavailable") from error
    if stat.S_ISLNK(details.st_mode) or not stat.S_ISREG(details.st_mode) or not 0 < details.st_size <= maximum:
        raise FileDragQualificationError(f"{description} must be a bounded regular file")
    return path


def run(argv: list[str], environment: dict[str, str], timeout: int = 15) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(argv, env=environment, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)


def wait_for(predicate, description: str, timeout: float = 8) -> Any:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value is not None:
            return value
        time.sleep(0.05)
    raise FileDragQualificationError(description)


def build_source(workspace: Path, environment: dict[str, str]) -> Path:
    require_regular(SOURCE, "GTK file-drag source")
    if shutil.which("cc", path=environment.get("PATH")) is None or shutil.which("pkg-config", path=environment.get("PATH")) is None:
        raise FileDragQualificationError("C compiler and GTK development metadata are required")
    flags = run(["pkg-config", "--cflags", "--libs", "gtk+-3.0"], environment).stdout.decode("utf-8").split()
    output = workspace / "copypaste-gtk-file-drop-source"
    if output.exists() or output.is_symlink():
        raise FileDragQualificationError("GTK file-drag helper output already exists")
    run(["cc", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", str(SOURCE), "-o", str(output), *flags], environment, timeout=30)
    if not output.is_file() or output.is_symlink() or not os.access(output, os.X_OK):
        raise FileDragQualificationError("GTK file-drag source did not compile")
    return output


def x11_window(title: str, environment: dict[str, str]) -> str | None:
    result = subprocess.run(["xdotool", "search", "--name", f"^{re.escape(title)}$"], env=environment, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    windows = result.stdout.decode("ascii", errors="ignore").splitlines()
    if len(windows) > 1:
        raise FileDragQualificationError(f"{title} window is not uniquely discoverable")
    return windows[0] if len(windows) == 1 and windows[0].isdigit() else None


def x11_bounds(window: str, environment: dict[str, str]) -> Bounds:
    output = run(["xdotool", "getwindowgeometry", "--shell", window], environment).stdout.decode("ascii")
    values = dict(line.split("=", 1) for line in output.splitlines() if "=" in line)
    try:
        bounds = Bounds(*(int(values[key]) for key in ("X", "Y", "WIDTH", "HEIGHT")))
    except (KeyError, ValueError) as error:
        raise FileDragQualificationError("X11 window geometry is invalid") from error
    if bounds.width < 32 or bounds.height < 32:
        raise FileDragQualificationError("X11 window geometry is unusable")
    return bounds


def require_x11_window_pid(window: str, gui_pid: int, environment: dict[str, str]) -> None:
    result = run(["xdotool", "getwindowpid", window], environment)
    try:
        observed = int(result.stdout.decode("ascii").strip())
    except ValueError as error:
        raise FileDragQualificationError("X11 CopyPaste window owner is invalid") from error
    if observed != gui_pid:
        raise FileDragQualificationError("X11 CopyPaste window is not owned by the installed GUI")


def source_activity(path: Path) -> set[str]:
    if not path.is_file() or path.is_symlink():
        return set()
    try:
        events = {re.fullmatch(r'\{"event":"([a-z_]+)"\}', line).group(1) for line in path.read_text(encoding="utf-8").splitlines()}
    except (OSError, UnicodeError, AttributeError):
        raise FileDragQualificationError("GTK file-drag source activity is invalid") from None
    if not events <= {"drag_started", "uri_list_requested", "drag_completed", "drag_not_accepted", "drag_begin_failed"}:
        raise FileDragQualificationError("GTK file-drag source emitted an unknown activity")
    return events


@dataclass
class DragSource:
    process: subprocess.Popen[bytes]
    start: Path
    activity: Path

    def begin(self) -> None:
        if self.start.exists() or self.start.is_symlink():
            raise FileDragQualificationError("GTK file-drag source start path already exists")
        self.start.write_text("start\n", encoding="ascii")
        wait_for(lambda: "drag_started" if "drag_started" in source_activity(self.activity) else None, "GTK file drag did not start")

    def completed(self) -> None:
        events = wait_for(
            lambda: (candidate if {"drag_completed", "drag_not_accepted", "drag_begin_failed"} & candidate else None)
            if (candidate := source_activity(self.activity)) else None,
            "GTK file drag did not report its outcome",
        )
        if "drag_completed" not in events or "uri_list_requested" not in events:
            raise FileDragQualificationError("GTK file drag was not accepted by the target")

    def close(self) -> None:
        self.process.terminate()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=5)


def start_source(workspace: Path, environment: dict[str, str]) -> DragSource:
    helper = build_source(workspace, environment)
    directory = workspace / f"file-drag-{time.monotonic_ns()}"
    directory.mkdir(mode=0o700)
    payload = directory / "external-file-drag-fixture.txt"
    payload.write_bytes(b"CopyPaste external file drag qualification payload\n")
    os.chmod(payload, 0o600)
    uri_list = directory / "uri-list"
    uri_list.write_text(payload.as_uri() + "\r\n", encoding="utf-8")
    os.chmod(uri_list, 0o600)
    ready = directory / "ready"
    start = directory / "start"
    activity = directory / "activity.jsonl"
    process = subprocess.Popen(
        [str(helper), "--ready-file", str(ready), "--start-file", str(start), "--activity-log", str(activity), "--uri-list-file", str(uri_list)],
        env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    try:
        wait_for(lambda: "ready" if ready.is_file() and ready.read_text(encoding="ascii") == "ready\n" else None, "GTK file-drag source did not become ready")
        return DragSource(process, start, activity)
    except Exception:
        process.terminate()
        process.wait(timeout=5)
        raise


def drag_x11(source: DragSource, gui_pid: int, environment: dict[str, str]) -> None:
    if not environment.get("DISPLAY"):
        raise FileDragQualificationError("X11 display is unavailable")
    source_window = wait_for(lambda: x11_window(SOURCE_TITLE, environment), "GTK file-drag source window did not appear")
    target_window = wait_for(lambda: x11_window(TARGET_TITLE, environment), "installed CopyPaste window did not appear")
    require_x11_window_pid(target_window, gui_pid, environment)
    source_point = x11_bounds(source_window, environment).point()
    target_point = x11_bounds(target_window, environment).point(horizontal=0.72)
    run(["xdotool", "windowactivate", "--sync", source_window], environment)
    run(["xdotool", "mousemove", "--sync", str(source_point[0]), str(source_point[1])], environment)
    run(["xdotool", "mousedown", "1"], environment)
    source.begin()
    run(["xdotool", "mousemove", "--sync", str(target_point[0]), str(target_point[1])], environment)
    run(["xdotool", "mouseup", "1"], environment)
    source.completed()


def load_wayland_portal() -> Any:
    require_regular(WAYLAND_QUICK_PASTE, "Wayland portal helper")
    spec = importlib.util.spec_from_file_location("linux_wayland_quick_paste", WAYLAND_QUICK_PASTE)
    if spec is None or spec.loader is None:
        raise FileDragQualificationError("Wayland portal helper is unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def atspi_bounds(registry: Any, pid: int, name: str) -> Bounds:
    try:
        desktop = registry.getDesktop(0)
        def node_pid(node: Any) -> int | None:
            application = node.get_application()
            value = application.get_process_id()
            return value if isinstance(value, int) else None
        application = next(candidate for candidate in (desktop.getChildAtIndex(index) for index in range(desktop.childCount)) if node_pid(candidate) == pid)
        nodes = [application]
        while nodes:
            node = nodes.pop()
            if node.name == name:
                rect = node.queryComponent().getExtents(0)
                bounds = Bounds(rect.x, rect.y, rect.width, rect.height)
                if bounds.width >= 32 and bounds.height >= 32:
                    return bounds
            nodes.extend(node.getChildAtIndex(index) for index in range(node.childCount))
    except Exception:
        pass
    raise FileDragQualificationError(f"accessible bounds for {name} are unavailable")


class PortalPointer:
    """Small pointer-only sibling of the established keyboard portal helper."""

    def __init__(self, portal: Any, helper: Any, glib: Any):
        self.portal = portal
        self.helper = helper
        self.glib = glib
        self.session = ""

    def grant(self, on_start_requested) -> None:
        created = self.portal._request("CreateSession", "(a{sv})", ({"handle_token": self.helper.token("create"), "session_handle_token": self.helper.token("session")},))
        session = created.get("session_handle")
        if not isinstance(session, str) or not session.startswith("/"):
            raise FileDragQualificationError("desktop portal did not create a pointer session")
        self.session = session
        self.portal._request("SelectDevices", "(oa{sv})", (session, {"handle_token": self.helper.token("devices"), "types": POINTER_DEVICE}))
        started = self.portal._request("Start", "(osa{sv})", (session, "", {"handle_token": self.helper.token("start")}), on_start_requested)
        if type(started.get("devices")) is not int or not (started["devices"] & POINTER_DEVICE):
            raise FileDragQualificationError("desktop portal did not grant pointer input")

    def motion(self, x: int, y: int) -> None:
        self.portal._call("org.freedesktop.portal.RemoteDesktop", "NotifyPointerMotion", self.glib.Variant("(oa{sv}dd)", (self.session, {}, float(x), float(y))))

    def button(self, state: int) -> None:
        self.portal._call("org.freedesktop.portal.RemoteDesktop", "NotifyPointerButton", self.glib.Variant("(oa{sv}iu)", (self.session, {}, LEFT_BUTTON, state)))

    def close(self) -> None:
        if self.session:
            try:
                self.portal._call("org.freedesktop.portal.Session", "Close", self.glib.Variant("(o)", (self.session,)))
            finally:
                self.session = ""


def drag_wayland(source: DragSource, gui_pid: int, environment: dict[str, str]) -> None:
    if not environment.get("WAYLAND_DISPLAY"):
        raise FileDragQualificationError("Wayland display is unavailable")
    try:
        import pyatspi  # type: ignore[import-not-found]
        from gi.repository import Gdk, Gio, GLib  # type: ignore[import-not-found]
    except ImportError as error:
        raise FileDragQualificationError("PyGObject and AT-SPI are required for Wayland file-drag qualification") from error
    helper = load_wayland_portal()
    if not hasattr(helper, "PortalKeyboard") or not hasattr(helper, "click_owned_portal_consent") or not hasattr(helper, "token"):
        raise FileDragQualificationError("Wayland portal helper does not expose dynamic consent")
    source_bounds = wait_for(lambda: _atspi_bounds_or_none(pyatspi.Registry, source.process.pid, "Drag qualification file"), "GTK file-drag source is absent from the AT-SPI desktop")
    target_bounds = wait_for(lambda: _atspi_bounds_or_none(pyatspi.Registry, gui_pid, TARGET_TITLE), "installed CopyPaste window is absent from the AT-SPI desktop")
    require_single_wayland_layout(Gdk, source_bounds, target_bounds)
    portal = helper.PortalKeyboard(Gio, GLib)
    pointer = PortalPointer(portal, helper, GLib)
    try:
        gui_owner = portal.require_gui_owner(gui_pid)
        pointer.grant(lambda: helper.click_owned_portal_consent(portal))
        if portal.require_gui_owner(gui_pid) != gui_owner:
            raise FileDragQualificationError("CopyPaste GUI D-Bus owner changed before file drag")
        source_point = source_bounds.point()
        target_point = target_bounds.point(horizontal=0.72)
        # Relative portal motion has no public query for the current pointer.
        # First clamp it to the desktop origin, then move to the external GTK
        # source using the source's AT-SPI screen coordinates.
        pointer.motion(-10000, -10000)
        pointer.motion(*source_point)
        pointer.button(1)
        source.begin()
        pointer.motion(target_point[0] - source_point[0], target_point[1] - source_point[1])
        pointer.button(0)
        source.completed()
        if portal.require_gui_owner(gui_pid) != gui_owner:
            raise FileDragQualificationError("CopyPaste GUI D-Bus owner changed during file drag")
    finally:
        pointer.close()


def _atspi_bounds_or_none(registry: Any, pid: int, name: str) -> Bounds | None:
    try:
        return atspi_bounds(registry, pid, name)
    except FileDragQualificationError:
        return None


def require_single_wayland_layout(gdk: Any, source: Bounds, target: Bounds) -> Bounds:
    display = gdk.Display.get_default()
    if display is None or display.get_n_monitors() != 1:
        raise FileDragQualificationError("Wayland file drag requires one visible monitor")
    geometry = display.get_monitor(0).get_geometry()
    layout = Bounds(geometry.x, geometry.y, geometry.width, geometry.height)
    if layout.x != 0 or layout.y != 0 or layout.width <= 0 or layout.height <= 0:
        raise FileDragQualificationError("Wayland monitor layout does not have a nonnegative origin")
    for point in (source.point(), target.point(horizontal=0.72)):
        if not (layout.x <= point[0] < layout.x + layout.width and layout.y <= point[1] < layout.y + layout.height):
            raise FileDragQualificationError("Wayland file-drag target is outside the visible monitor")
    return layout


def qualify(session: str, workspace: Path, gui_pid: int, environment: dict[str, str]) -> None:
    if session not in {"x11", "wayland"} or gui_pid <= 0:
        raise FileDragQualificationError("file-drag qualification arguments are invalid")
    if workspace.exists() and workspace.is_symlink():
        raise FileDragQualificationError("file-drag workspace is unsafe")
    workspace.mkdir(mode=0o700, parents=True, exist_ok=True)
    source = start_source(workspace, environment)
    try:
        if session == "x11":
            drag_x11(source, gui_pid, environment)
        else:
            drag_wayland(source, gui_pid, environment)
    finally:
        source.close()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--session", required=True, choices=("x11", "wayland"))
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--gui-pid", required=True, type=int)
    args = parser.parse_args(argv)
    try:
        qualify(args.session, args.workspace, args.gui_pid, dict(os.environ))
        return 0
    except (FileDragQualificationError, OSError, subprocess.SubprocessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
