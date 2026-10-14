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
import json
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
WAYLAND_PORTAL_MODULE = "copypaste_qualification_wayland_portal"
GEOMETRY_BUS_NAME = "app.copypaste.QualificationGeometry"
GEOMETRY_OBJECT_PATH = "/app/copypaste/QualificationGeometry"
GEOMETRY_INTERFACE = "app.copypaste.QualificationGeometry"
GEOMETRY_OWNER_PID_ENV = "COPYPASTE_QUALIFICATION_GEOMETRY_OWNER_PID"
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

    def contains(self, point: tuple[int, int]) -> bool:
        return self.x <= point[0] < self.x + self.width and self.y <= point[1] < self.y + self.height


@dataclass(frozen=True)
class CompositorGeometry:
    frame: Bounds
    buffer: Bounds
    stacking: int


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
    result = subprocess.run(["xdotool", "search", "--onlyvisible", "--name", f"^{re.escape(title)}$"], env=environment, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
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


def require_x11_focus(window: str, environment: dict[str, str]) -> None:
    focused = run(["xdotool", "getwindowfocus"], environment).stdout.decode("ascii").strip()
    if focused != window:
        raise FileDragQualificationError("X11 file-drag source did not receive input focus")


def source_activity(path: Path) -> list[dict[str, object]]:
    if not path.is_file() or path.is_symlink():
        return []
    try:
        events = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    except (OSError, UnicodeError, json.JSONDecodeError):
        raise FileDragQualificationError("GTK file-drag source activity is invalid") from None
    expected = {"event", "timestamp_us", "actions", "selected_action", "destination_window"}
    names = {"drag_armed", "drag_begin", "drag_failed", "drag_status_copy", "drag_end", "uri_list_requested"}
    if any(
        not isinstance(event, dict) or set(event) != expected or event["event"] not in names
        or not isinstance(event["timestamp_us"], int) or event["timestamp_us"] <= 0
        or not isinstance(event["actions"], int) or event["actions"] < 0
        or not isinstance(event["selected_action"], int) or event["selected_action"] < 0
        or not isinstance(event["destination_window"], bool)
        for event in events
    ):
        raise FileDragQualificationError("GTK file-drag source emitted an unknown activity")
    return events


def activity_has(events: list[dict[str, object]], name: str) -> bool:
    return any(event["event"] == name for event in events)


@dataclass
class DragSource:
    process: subprocess.Popen[bytes]
    start: Path
    activity: Path

    def begin(self) -> None:
        if self.start.exists() or self.start.is_symlink():
            raise FileDragQualificationError("GTK file-drag source start path already exists")
        self.start.write_text("start\n", encoding="ascii")
        wait_for(lambda: True if activity_has(source_activity(self.activity), "drag_armed") else None, "GTK file drag did not arm")

    def started(self) -> None:
        wait_for(lambda: True if activity_has(source_activity(self.activity), "drag_begin") else None, "GTK file drag did not begin from the pointer press")

    def copied(self) -> None:
        wait_for(lambda: True if activity_has(source_activity(self.activity), "drag_status_copy") else None, "GTK file drag target did not negotiate copy")

    def completed(self) -> None:
        events = wait_for(
            lambda: candidate if activity_has(candidate := source_activity(self.activity), "drag_end") else None,
            "GTK file drag did not report its outcome",
        )
        accepted = next(event for event in events if event["event"] == "drag_end")
        if accepted["selected_action"] != 2 or not activity_has(events, "uri_list_requested"):
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
    require_x11_focus(source_window, environment)
    run(["xdotool", "mousemove", "--sync", str(source_point[0]), str(source_point[1])], environment)
    source.begin()
    run(["xdotool", "mousedown", "1"], environment)
    source.started()
    run(["xdotool", "mousemove", "--sync", str(target_point[0]), str(target_point[1])], environment)
    source.copied()
    run(["xdotool", "mouseup", "1"], environment)
    source.completed()


def load_wayland_portal() -> Any:
    require_regular(WAYLAND_QUICK_PASTE, "Wayland portal helper")
    spec = importlib.util.spec_from_file_location(WAYLAND_PORTAL_MODULE, WAYLAND_QUICK_PASTE)
    if spec is None or spec.loader is None:
        raise FileDragQualificationError("Wayland portal helper is unavailable")
    module = importlib.util.module_from_spec(spec)
    previous = sys.modules.get(WAYLAND_PORTAL_MODULE)
    sys.modules[WAYLAND_PORTAL_MODULE] = module
    try:
        spec.loader.exec_module(module)
    except Exception:
        if previous is None:
            sys.modules.pop(WAYLAND_PORTAL_MODULE, None)
        else:
            sys.modules[WAYLAND_PORTAL_MODULE] = previous
        raise
    return module


def atspi_bounds(registry: Any, pid: int, name: str) -> Bounds:
    """Return one AT-SPI window-local rectangle for the PID-bound named node."""
    try:
        desktop = registry.getDesktop(0)
        def node_pid(node: Any) -> int | None:
            application = node.get_application()
            value = application.get_process_id()
            return value if isinstance(value, int) else None
        application = next(candidate for candidate in (desktop.getChildAtIndex(index) for index in range(desktop.childCount)) if node_pid(candidate) == pid)
        nodes = [application]
        matches = []
        while nodes:
            node = nodes.pop()
            if node.name == name:
                # ATK_XY_WINDOW is relative to the client toplevel. GTK3's
                # Wayland SCREEN coordinates are a per-client fake root.
                rect = node.queryComponent().getExtents(1)
                bounds = Bounds(rect.x, rect.y, rect.width, rect.height)
                if bounds.width >= 32 and bounds.height >= 32:
                    matches.append(bounds)
            nodes.extend(node.getChildAtIndex(index) for index in range(node.childCount))
        if len(matches) == 1:
            return matches[0]
    except Exception:
        pass
    raise FileDragQualificationError(f"accessible bounds for {name} are unavailable")


def compositor_geometry(Gio: Any, GLib: Any, pid: int, title: str) -> CompositorGeometry:
    if pid <= 0 or not title:
        raise FileDragQualificationError("compositor geometry identity is invalid")
    try:
        connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        owner = geometry_owner(connection, Gio, GLib)
        reply = connection.call_sync(
            GEOMETRY_BUS_NAME, GEOMETRY_OBJECT_PATH, GEOMETRY_INTERFACE, "Lookup",
            GLib.Variant("(us)", (pid, title)), GLib.VariantType("(iiiiiiiii)"),
            Gio.DBusCallFlags.NONE, 2000, None,
        )
        values = reply.unpack()
        if not isinstance(values, tuple) or len(values) != 9 or not all(isinstance(value, int) for value in values):
            raise ValueError("invalid reply")
        geometry = CompositorGeometry(Bounds(*values[:4]), Bounds(*values[4:8]), values[8])
        if geometry_owner(connection, Gio, GLib) != owner:
            raise ValueError("geometry owner changed")
    except Exception as error:
        raise FileDragQualificationError(f"compositor geometry for {title} is unavailable") from error
    for rect in (geometry.frame, geometry.buffer):
        if rect.width < 32 or rect.height < 32:
            raise FileDragQualificationError(f"compositor geometry for {title} is unusable")
    return geometry


def geometry_owner(connection: Any, Gio: Any, GLib: Any) -> str:
    try:
        expected_pid = int(os.environ[GEOMETRY_OWNER_PID_ENV])
        if expected_pid <= 1:
            raise ValueError("invalid expected owner")
        owner_reply = connection.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetNameOwner",
            GLib.Variant("(s)", (GEOMETRY_BUS_NAME,)), GLib.VariantType("(s)"), Gio.DBusCallFlags.NONE, 2000, None,
        )
        owner = owner_reply.unpack()[0]
        pid_reply = connection.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetConnectionUnixProcessID",
            GLib.Variant("(s)", (owner,)), GLib.VariantType("(u)"), Gio.DBusCallFlags.NONE, 2000, None,
        )
        if not isinstance(owner, str) or not owner or pid_reply.unpack() != (expected_pid,):
            raise ValueError("unexpected owner")
        return owner
    except Exception as error:
        raise FileDragQualificationError("compositor geometry owner is unavailable") from error


def absolute_atspi_bounds(local: Bounds, geometry: CompositorGeometry, title: str) -> Bounds:
    if local.x < 0 or local.y < 0 or local.width < 32 or local.height < 32:
        raise FileDragQualificationError(f"accessible local bounds for {title} are invalid")
    if local.x + local.width > geometry.buffer.width or local.y + local.height > geometry.buffer.height:
        raise FileDragQualificationError(f"accessible local bounds for {title} exceed its compositor buffer")
    return Bounds(geometry.buffer.x + local.x, geometry.buffer.y + local.y, local.width, local.height)


def compositor_owns_point(Gio: Any, GLib: Any, pid: int, title: str, point: tuple[int, int]) -> bool:
    try:
        connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        owner = geometry_owner(connection, Gio, GLib)
        reply = connection.call_sync(
            GEOMETRY_BUS_NAME, GEOMETRY_OBJECT_PATH, GEOMETRY_INTERFACE, "PointBelongsTo",
            GLib.Variant("(usii)", (pid, title, *point)), GLib.VariantType("(b)"),
            Gio.DBusCallFlags.NONE, 2000, None,
        )
        values = reply.unpack()
        return geometry_owner(connection, Gio, GLib) == owner and isinstance(values, tuple) and len(values) == 1 and values[0] is True
    except Exception as error:
        raise FileDragQualificationError(f"compositor hit test for {title} is unavailable") from error


def owned_point(Gio: Any, GLib: Any, pid: int, title: str, bounds: Bounds) -> tuple[int, int]:
    for horizontal, vertical in ((0.5, 0.5), (0.72, 0.5), (0.28, 0.5), (0.5, 0.28), (0.5, 0.72)):
        point = bounds.point(horizontal, vertical)
        if compositor_owns_point(Gio, GLib, pid, title, point):
            return point
    raise FileDragQualificationError(f"no compositor-visible injection point is available for {title}")


class PortalPointer:
    """Small pointer-only sibling of the established keyboard portal helper."""

    def __init__(self, portal: Any, helper: Any, glib: Any):
        self.portal = portal
        self.helper = helper
        self.glib = glib
        self.session = ""

    def grant(self, on_start_requested) -> None:
        created = self.portal._request("CreateSession", "(a{sv})", ({
            "handle_token": self.glib.Variant("s", self.helper.token("create")),
            "session_handle_token": self.glib.Variant("s", self.helper.token("session")),
        },))
        session = created.get("session_handle")
        if not isinstance(session, str) or not session.startswith("/"):
            raise FileDragQualificationError("desktop portal did not create a pointer session")
        self.session = session
        self.portal._request("SelectDevices", "(oa{sv})", (session, {
            "handle_token": self.glib.Variant("s", self.helper.token("devices")),
            "types": self.glib.Variant("u", POINTER_DEVICE),
        }))
        started = self.portal._request("Start", "(osa{sv})", (session, "", {
            "handle_token": self.glib.Variant("s", self.helper.token("start")),
        }), on_start_requested)
        if type(started.get("devices")) is not int or not (started["devices"] & POINTER_DEVICE):
            raise FileDragQualificationError("desktop portal did not grant pointer input")

    def motion(self, x: int, y: int) -> None:
        self.portal._call("org.freedesktop.portal.RemoteDesktop", "NotifyPointerMotion", self.glib.Variant("(oa{sv}dd)", (self.session, {}, float(x), float(y))))

    def button(self, state: int) -> None:
        self.portal._call("org.freedesktop.portal.RemoteDesktop", "NotifyPointerButton", self.glib.Variant("(oa{sv}iu)", (self.session, {}, LEFT_BUTTON, state)))

    def close(self) -> None:
        if self.session:
            try:
                self.portal._call("org.freedesktop.portal.Session", "Close", None, path=self.session)
            finally:
                self.session = ""


def drag_wayland(source: DragSource, gui_pid: int, environment: dict[str, str]) -> None:
    if not environment.get("WAYLAND_DISPLAY"):
        raise FileDragQualificationError("Wayland display is unavailable")
    try:
        import gi  # type: ignore[import-not-found]
        import pyatspi  # type: ignore[import-not-found]
        gi.require_version("Gdk", "3.0")
        from gi.repository import Gdk, Gio, GLib  # type: ignore[import-not-found]
    except ImportError as error:
        raise FileDragQualificationError("PyGObject and AT-SPI are required for Wayland file-drag qualification") from error
    helper = load_wayland_portal()
    if not hasattr(helper, "PortalKeyboard") or not hasattr(helper, "click_owned_portal_consent") or not hasattr(helper, "token"):
        raise FileDragQualificationError("Wayland portal helper does not expose dynamic consent")
    source_local = wait_for(lambda: _atspi_bounds_or_none(pyatspi.Registry, source.process.pid, "Drag qualification file"), "GTK file-drag source is absent from the AT-SPI desktop")
    target_local = wait_for(lambda: _atspi_bounds_or_none(pyatspi.Registry, gui_pid, TARGET_TITLE), "installed CopyPaste window is absent from the AT-SPI desktop")
    source_geometry = wait_for(lambda: _compositor_geometry_or_none(Gio, GLib, source.process.pid, SOURCE_TITLE), "GTK file-drag source is absent from compositor geometry")
    target_geometry = wait_for(lambda: _compositor_geometry_or_none(Gio, GLib, gui_pid, TARGET_TITLE), "installed CopyPaste window is absent from compositor geometry")
    source_bounds = absolute_atspi_bounds(source_local, source_geometry, SOURCE_TITLE)
    target_bounds = absolute_atspi_bounds(target_local, target_geometry, TARGET_TITLE)
    display = open_wayland_display(Gdk, environment["WAYLAND_DISPLAY"])
    require_single_wayland_layout(display, source_geometry, target_geometry, source_bounds, target_bounds)
    portal = helper.PortalKeyboard(Gio, GLib)
    pointer = PortalPointer(portal, helper, GLib)
    try:
        gui_owner = portal.require_gui_owner(gui_pid)
        pointer.grant(lambda: helper.click_owned_portal_consent(portal))
        if portal.require_gui_owner(gui_pid) != gui_owner:
            raise FileDragQualificationError("CopyPaste GUI D-Bus owner changed before file drag")
        source_geometry = compositor_geometry(Gio, GLib, source.process.pid, SOURCE_TITLE)
        target_geometry = compositor_geometry(Gio, GLib, gui_pid, TARGET_TITLE)
        source_bounds = absolute_atspi_bounds(source_local, source_geometry, SOURCE_TITLE)
        target_bounds = absolute_atspi_bounds(target_local, target_geometry, TARGET_TITLE)
        layout = require_single_wayland_layout(display, source_geometry, target_geometry, source_bounds, target_bounds)
        source_point = owned_point(Gio, GLib, source.process.pid, SOURCE_TITLE, source_bounds)
        target_point = owned_point(Gio, GLib, gui_pid, TARGET_TITLE, target_bounds)
        approach_point = (target_point[0] - 1, target_point[1])
        if not target_bounds.contains(approach_point) or not compositor_owns_point(Gio, GLib, gui_pid, TARGET_TITLE, approach_point):
            raise FileDragQualificationError("file-drag approach is not owned by the installed CopyPaste window")
        # Relative portal motion has no public query for the current pointer.
        # Clamp to the bottom-left, avoiding GNOME's top-left hot corner,
        # then move to the authenticated external GTK source coordinates.
        pointer.motion(-10000, 10000)
        pointer.motion(source_point[0], source_point[1] - (layout.height - 1))
        source.begin()
        pointer.button(1)
        source.started()
        pointer.motion(approach_point[0] - source_point[0], approach_point[1] - source_point[1])
        # Wayland enter announces the offer; subsequent motion lets GTK
        # negotiate the destination before the real pointer release.
        pointer.motion(1, 0)
        source.copied()
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


def _compositor_geometry_or_none(Gio: Any, GLib: Any, pid: int, title: str) -> CompositorGeometry | None:
    try:
        return compositor_geometry(Gio, GLib, pid, title)
    except FileDragQualificationError:
        return None


def open_wayland_display(gdk: Any, name: str) -> Any:
    try:
        display = gdk.Display.open(name)
    except Exception as error:
        raise FileDragQualificationError("could not open the requested Wayland display") from error
    if display is None or display.get_name() != name:
        raise FileDragQualificationError("GDK did not open the requested Wayland display")
    return display


def require_single_wayland_layout(display: Any, source: CompositorGeometry, target: CompositorGeometry,
                                  source_bounds: Bounds, target_bounds: Bounds) -> Bounds:
    if display.get_n_monitors() != 1:
        raise FileDragQualificationError("Wayland file drag requires one visible monitor")
    geometry = display.get_monitor(0).get_geometry()
    layout = Bounds(geometry.x, geometry.y, geometry.width, geometry.height)
    if layout.x != 0 or layout.y != 0 or layout.width <= 0 or layout.height <= 0:
        raise FileDragQualificationError("Wayland monitor layout does not have a nonnegative origin")
    def inside(rect: Bounds) -> bool:
        return (layout.x <= rect.x and layout.y <= rect.y and rect.x + rect.width <= layout.x + layout.width
                and rect.y + rect.height <= layout.y + layout.height)
    for rect in (source.frame, target.frame, source_bounds, target_bounds):
        if not inside(rect):
            raise FileDragQualificationError("Wayland file-drag geometry is outside the visible monitor")
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
