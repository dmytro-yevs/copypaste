#!/usr/bin/env python3
"""Exercise CopyPaste's real Linux tray, window, and notification contracts.

The helper is intentionally a desktop observer.  It does not accept success
flags or manufacture a D-Bus reply: it finds the StatusNotifierItem owned by
the packaged GUI process, drives its exported DbusMenu, and records only
redacted notification metadata from the desktop notification service.
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import re
import select
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence


SNI_PATH = "/StatusNotifierItem"
SNI_INTERFACE = "org.kde.StatusNotifierItem"
MENU_PATH = "/StatusNotifierItem/Menu"
MENU_INTERFACE = "com.canonical.dbusmenu"
NOTIFICATIONS_NAME = "org.freedesktop.Notifications"
NOTIFICATIONS_PATH = "/org/freedesktop/Notifications"
HOST_BUS_NAME = "app.copypaste.CopyPaste"
HOST_OBJECT_PATH = "/app/copypaste/WaylandIntegration"
HOST_INTERFACE = "app.copypaste.WaylandIntegration"
DBUS_NAME = "org.freedesktop.DBus"
DBUS_PATH = "/org/freedesktop/DBus"
DBUS_INTERFACE = "org.freedesktop.DBus"
COPYPASTE_TITLE = "CopyPaste"
CLIPBOARD_PROVIDER = Path(__file__).with_name("linux-clipboard-provider.py").resolve()
SNI_NAME = re.compile(r"^org\.kde\.StatusNotifierItem-(?P<pid>[1-9][0-9]*)-(?P<index>[1-9][0-9]*)$")
SERIAL = re.compile(r"\bserial=(?P<serial>[0-9]+)\b")
REPLY_SERIAL = re.compile(r"\breply_serial=(?P<serial>[0-9]+)\b")
CALL_SENDER = re.compile(r"\bsender=(?P<sender>:[0-9]+\.[0-9]+)\b")
UINT32 = re.compile(r"^\s*uint32\s+(?P<value>[0-9]+)\s*$")
QUOTED_STRING = re.compile(r'^s\s+"(?P<value>(?:[^"\\]|\\.)*)"\s*$')
MONITOR_STRING = re.compile(r'^\s*string\s+"(?P<value>(?:[^"\\]|\\.)*)"\s*$')
UNIQUE_NAME = re.compile(r"^:[0-9]+\.[0-9]+$")


class AcceptanceError(RuntimeError):
    """A required native desktop contract was unavailable or incorrect."""


@dataclass(frozen=True)
class CommandResult:
    argv: tuple[str, ...]
    returncode: int
    stdout: str
    stderr: str


def run(argv: Sequence[str], *, environment: dict[str, str], timeout: float = 10) -> CommandResult:
    result = subprocess.run(
        list(argv),
        env=environment,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        timeout=timeout,
    )
    return CommandResult(tuple(argv), result.returncode, result.stdout, result.stderr)


def require_ok(result: CommandResult, purpose: str) -> CommandResult:
    if result.returncode:
        detail = result.stderr.strip() or result.stdout.strip() or f"exit {result.returncode}"
        raise AcceptanceError(f"{purpose} failed: {detail}")
    return result


def require_program(name: str, environment: dict[str, str]) -> None:
    if shutil.which(name, path=environment.get("PATH")) is None:
        raise AcceptanceError(f"required native protocol tool is unavailable: {name}")


def session_bus_names(environment: dict[str, str]) -> list[str]:
    result = require_ok(
        run(["busctl", "--user", "--no-pager", "--no-legend", "list"], environment=environment),
        "read session bus names",
    )
    names = []
    for line in result.stdout.splitlines():
        fields = line.split()
        if fields:
            names.append(fields[0])
    return names


def sni_service_for_pid(names: Iterable[str], gui_pid: int) -> str:
    matches = []
    for name in names:
        match = SNI_NAME.fullmatch(name)
        if match and int(match.group("pid")) == gui_pid:
            matches.append(name)
    if len(matches) != 1:
        raise AcceptanceError(
            f"expected exactly one StatusNotifierItem owned by GUI PID {gui_pid}; found {len(matches)}"
        )
    return matches[0]


def dbus_property(environment: dict[str, str], service: str, name: str) -> str:
    result = require_ok(
        run(
            [
                "busctl", "--user", "get-property", service, SNI_PATH,
                SNI_INTERFACE, name,
            ],
            environment=environment,
        ),
        f"read StatusNotifierItem {name}",
    )
    return result.stdout.strip()


def quoted_property(value: str, name: str) -> str:
    match = QUOTED_STRING.fullmatch(value)
    if not match:
        raise AcceptanceError(f"StatusNotifierItem {name} has an invalid D-Bus string value")
    return bytes(match.group("value"), "utf-8").decode("unicode_escape")


def busctl_value(environment: dict[str, str], method: str, argument: str) -> object:
    result = require_ok(
        run([
            "busctl", "--user", "--json=short", "call", "org.freedesktop.DBus",
            "/org/freedesktop/DBus", "org.freedesktop.DBus", method, "s", argument,
        ], environment=environment),
        f"read D-Bus credentials for {argument}",
    )
    try:
        payload = json.loads(result.stdout)
        value = payload["data"][0]
    except (json.JSONDecodeError, KeyError, IndexError, TypeError) as error:
        raise AcceptanceError("D-Bus credentials were not machine-readable") from error
    return value


def require_bus_credentials(environment: dict[str, str], service: str, gui_pid: int) -> None:
    pid = busctl_value(environment, "GetConnectionUnixProcessID", service)
    uid = busctl_value(environment, "GetConnectionUnixUser", service)
    if pid != gui_pid or uid != os.getuid():
        raise AcceptanceError("StatusNotifierItem D-Bus credentials do not belong to the GUI process")


def require_bus_uid(environment: dict[str, str], service: str) -> None:
    if busctl_value(environment, "GetConnectionUnixUser", service) != os.getuid():
        raise AcceptanceError("D-Bus service is not owned by the qualification user")


def expect_tray(environment: dict[str, str], gui_pid: int) -> str:
    service = sni_service_for_pid(session_bus_names(environment), gui_pid)
    require_bus_credentials(environment, service, gui_pid)
    if quoted_property(dbus_property(environment, service, "Title"), "Title") != COPYPASTE_TITLE:
        raise AcceptanceError("StatusNotifierItem title does not identify CopyPaste")
    if quoted_property(dbus_property(environment, service, "Status"), "Status") != "Active":
        raise AcceptanceError("StatusNotifierItem is not active in the desktop watcher")
    if dbus_property(environment, service, "ItemIsMenu") != "b true":
        raise AcceptanceError(
            "CopyPaste StatusNotifierItem does not export a clickable D-Bus menu; "
            "the native tray callback cannot be exercised"
        )
    if quoted_property(dbus_property(environment, service, "Menu"), "Menu") != MENU_PATH:
        raise AcceptanceError("StatusNotifierItem has no exported D-Bus menu path")
    return service


def dbus_menu_layout(environment: dict[str, str], service: str) -> object:
    result = require_ok(
        run(
            [
                "busctl", "--user", "--json=short", "call", service, MENU_PATH,
                MENU_INTERFACE, "GetLayout", "iias", "0", "-1", "0",
            ],
            environment=environment,
        ),
        "read StatusNotifierItem D-Bus menu",
    )
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise AcceptanceError("DbusMenu GetLayout did not return machine-readable data") from error
    if not isinstance(payload, dict) or not isinstance(payload.get("data"), list):
        raise AcceptanceError("DbusMenu GetLayout returned an invalid machine-readable shape")
    return payload["data"]


def variant_value(value: object) -> object:
    if isinstance(value, dict) and set(value) >= {"data"}:
        return value["data"]
    return value


def menu_item_id(layout: object, label: str) -> int:
    """Find a labelled, enabled DbusMenu item in busctl's JSON GetLayout data."""
    if isinstance(layout, list):
        if len(layout) == 3 and isinstance(layout[0], int) and isinstance(layout[1], dict):
            properties = layout[1]
            menu_label = variant_value(properties.get("label"))
            enabled = variant_value(properties.get("enabled"))
            if menu_label == label:
                if enabled is not True:
                    raise AcceptanceError(f"StatusNotifierItem menu item {label!r} is disabled")
                return layout[0]
        for child in layout:
            try:
                return menu_item_id(child, label)
            except AcceptanceError as error:
                if "is disabled" in str(error):
                    raise
    raise AcceptanceError(f"StatusNotifierItem D-Bus menu has no enabled {label!r} item")


def invoke_menu_item(environment: dict[str, str], service: str, item_id: int) -> CommandResult:
    return require_ok(
        run(
            [
                "busctl", "--user", "call", service, MENU_PATH, MENU_INTERFACE,
                "Event", "isvu", str(item_id), "clicked", "s", "", "0",
            ],
            environment=environment,
        ),
        "invoke CopyPaste tray menu item",
    )


def x11_window_for_pid(environment: dict[str, str], gui_pid: int) -> str:
    result = require_ok(
        run(
            ["xdotool", "search", "--onlyvisible", "--pid", str(gui_pid), "--name", COPYPASTE_TITLE],
            environment=environment,
        ),
        "find CopyPaste X11 window",
    )
    windows = [line for line in result.stdout.splitlines() if line.isdigit()]
    if len(windows) != 1:
        raise AcceptanceError(f"expected exactly one visible CopyPaste X11 window; found {len(windows)}")
    window = windows[0]
    pid = require_ok(
        run(["xprop", "-id", window, "_NET_WM_PID"], environment=environment),
        "read CopyPaste X11 window owner",
    ).stdout
    if not re.search(rf"=\s*{gui_pid}\b", pid):
        raise AcceptanceError("CopyPaste X11 window is not owned by the GUI process")
    return window


def x11_map_state(environment: dict[str, str], window: str) -> str:
    result = require_ok(run(["xwininfo", "-id", window], environment=environment), "read CopyPaste X11 map state")
    for line in result.stdout.splitlines():
        if "Map State:" in line:
            return line.split("Map State:", 1)[1].strip()
    raise AcceptanceError("CopyPaste X11 window has no map state")


def wait_for_x11_map_state(environment: dict[str, str], window: str, expected: str) -> None:
    deadline = time.monotonic() + 8
    last = "unavailable"
    while time.monotonic() < deadline:
        last = x11_map_state(environment, window)
        if last == expected:
            return
        time.sleep(0.1)
    raise AcceptanceError(f"CopyPaste X11 window did not reach {expected}; last state was {last}")


@dataclass
class NotificationTranscript:
    """Extract only a Notify call's app identity, reply serial, and server id."""

    call_serial: int | None = None
    awaiting_app_name: bool = False
    awaiting_reply_id: bool = False
    server_owner: str = ""
    caller_owner: str = ""
    notification_id: int | None = None

    def consume(self, line: str) -> None:
        if "interface=org.freedesktop.Notifications; member=Notify" in line:
            match = SERIAL.search(line)
            self.call_serial = int(match.group("serial")) if match else None
            sender = CALL_SENDER.search(line)
            self.caller_owner = sender.group("sender") if sender else ""
            self.awaiting_app_name = self.call_serial is not None
            return
        if self.awaiting_app_name:
            match = MONITOR_STRING.fullmatch(line)
            self.awaiting_app_name = False
            if not match or bytes(match.group("value"), "utf-8").decode("unicode_escape") != COPYPASTE_TITLE:
                self.call_serial = None
            return
        if self.call_serial is not None and "method return" in line:
            reply = REPLY_SERIAL.search(line)
            if (reply and int(reply.group("serial")) == self.call_serial and
                    f"sender={self.server_owner}" in line):
                self.awaiting_reply_id = True
                return
        if self.awaiting_reply_id:
            value = UINT32.fullmatch(line)
            if value and int(value.group("value")) > 0:
                self.notification_id = int(value.group("value"))
                self.awaiting_reply_id = False


def notification_monitor(environment: dict[str, str]) -> tuple[subprocess.Popen[bytes], str]:
    require_ok(
        run(["busctl", "--user", "status", NOTIFICATIONS_NAME], environment=environment),
        "contact desktop notification server",
    )
    owner = busctl_value(environment, "GetNameOwner", NOTIFICATIONS_NAME)
    if not isinstance(owner, str) or not UNIQUE_NAME.fullmatch(owner):
        raise AcceptanceError("desktop notification service has no unique owner")
    require_bus_uid(environment, owner)
    monitor = subprocess.Popen(
        [
            "dbus-monitor", "--session",
            "type='method_call',interface='org.freedesktop.Notifications',member='Notify'",
            "type='method_return',sender='org.freedesktop.Notifications'",
        ],
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if monitor.stdout is None:
        raise AcceptanceError("cannot read D-Bus notification observer output")
    flags = fcntl.fcntl(monitor.stdout.fileno(), fcntl.F_GETFL)
    fcntl.fcntl(monitor.stdout.fileno(), fcntl.F_SETFL, flags | os.O_NONBLOCK)
    return monitor, owner


def parse_capture_command(raw: str) -> list[str]:
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as error:
        raise AcceptanceError("capture command must be a JSON argv array") from error
    if not isinstance(value, list) or not value or not all(isinstance(item, str) and item for item in value):
        raise AcceptanceError("capture command must be a non-empty JSON argv array of strings")
    if len(value) < 2 or Path(value[1]).resolve() != CLIPBOARD_PROVIDER:
        raise AcceptanceError("capture command must invoke the repository GTK clipboard provider")
    required = {"--helper", "--manifest", "--ready-file", "--activity-log"}
    if not required <= set(value):
        raise AcceptanceError("GTK clipboard provider command is incomplete")
    return value


def history_item_count(cli: Path, environment: dict[str, str]) -> int:
    """Read the daemon's redacted history count through the packaged CLI."""
    response = require_ok(
        run([str(cli), "--json", "status"], environment=environment),
        "read CopyPaste history count",
    )
    try:
        payload = json.loads(response.stdout)
        status = payload["data"]["status"]
        count = status["item_count"]
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        raise AcceptanceError("CopyPaste CLI did not return a typed history count") from error
    if (
        not isinstance(payload, dict) or not isinstance(status, dict) or
        payload.get("ok") is not True or type(count) is not int or count < 0
    ):
        raise AcceptanceError("CopyPaste CLI did not return a typed history count")
    return count


def wait_for_history_capture(cli: Path, environment: dict[str, str], previous_count: int) -> int:
    deadline = time.monotonic() + 8
    observed = previous_count
    while time.monotonic() < deadline:
        observed = history_item_count(cli, environment)
        if observed > previous_count:
            return observed
        time.sleep(0.15)
    raise AcceptanceError(f"controlled clipboard provider did not create a new history item; saw {observed}")


def run_notification_capture(
    environment: dict[str, str],
    cli: Path,
    socket_path: Path,
    capture_command: list[str],
    gui_pid: int,
) -> tuple[NotificationTranscript, CommandResult, int]:
    cli_environment = {**environment, "COPYPASTE_SOCKET": str(socket_path)}
    config = require_ok(
        run(
            [
                str(cli), "--json", "config", "set", "--notify-on-copy", "true",
                "--notification-preview", "false",
            ],
            environment=cli_environment,
        ),
        "enable capture notifications without previews",
    )
    try:
        decoded = json.loads(config.stdout)
    except json.JSONDecodeError as error:
        raise AcceptanceError("CopyPaste CLI did not acknowledge notification settings as JSON") from error
    if decoded.get("ok") is not True:
        raise AcceptanceError("CopyPaste CLI did not enable capture notifications")
    before_count = history_item_count(cli, cli_environment)

    monitor, owner = notification_monitor(environment)
    try:
        if monitor.stdout is None:
            raise AcceptanceError("cannot read D-Bus notification observer output")
        # Give dbus-monitor time to install both match rules before the real
        # GTK provider writes its controlled clipboard item.
        time.sleep(0.15)
        capture = require_ok(run(capture_command, environment=environment, timeout=20), "trigger controlled clipboard capture")
        after_count = wait_for_history_capture(cli, cli_environment, before_count)
        transcript = NotificationTranscript(server_owner=owner)
        buffered = b""
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            readable, _, _ = select.select([monitor.stdout], [], [], min(0.1, deadline - time.monotonic()))
            if not readable:
                if monitor.poll() is not None:
                    raise AcceptanceError("D-Bus notification observer exited before a server reply")
                continue
            try:
                buffered += os.read(monitor.stdout.fileno(), 4096)
            except BlockingIOError:
                continue
            while b"\n" in buffered:
                raw, buffered = buffered.split(b"\n", 1)
                transcript.consume(raw.decode("utf-8", errors="replace"))
                if transcript.call_serial is not None and not transcript.awaiting_app_name and transcript.caller_owner:
                    require_bus_credentials(environment, transcript.caller_owner, gui_pid)
                if transcript.notification_id is not None:
                    return transcript, capture, after_count
        raise AcceptanceError("desktop notification server did not accept CopyPaste's captured-clip notification")
    finally:
        monitor.terminate()
        try:
            monitor.wait(timeout=3)
        except subprocess.TimeoutExpired:
            monitor.kill()
            monitor.wait(timeout=3)


def command_digest(argv: Sequence[str]) -> str:
    encoded = json.dumps(list(argv), separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def gio_bus_value(connection: object, gio: object, glib: object, method: str, name: str) -> object:
    """Read one D-Bus daemon credential through the same session connection."""
    reply = connection.call_sync(
        DBUS_NAME, DBUS_PATH, DBUS_INTERFACE, method, glib.Variant("(s)", (name,)),
        None, gio.DBusCallFlags.NONE, 2000, None,
    )
    values = reply.unpack()
    if not isinstance(values, tuple) or len(values) != 1:
        raise AcceptanceError("D-Bus credentials were not machine-readable")
    return values[0]


def require_wayland_gui_owner(connection: object, gio: object, glib: object, gui_pid: int) -> str:
    """Capture and verify the GUI's current unique session-bus owner."""
    owner = gio_bus_value(connection, gio, glib, "GetNameOwner", HOST_BUS_NAME)
    if not isinstance(owner, str) or not UNIQUE_NAME.fullmatch(owner):
        raise AcceptanceError("CopyPaste GUI has no unique session-bus owner")
    pid = gio_bus_value(connection, gio, glib, "GetConnectionUnixProcessID", owner)
    uid = gio_bus_value(connection, gio, glib, "GetConnectionUnixUser", owner)
    if type(pid) is not int or type(uid) is not int or pid != gui_pid or uid != os.getuid():
        raise AcceptanceError("CopyPaste GUI D-Bus credentials do not belong to the GUI process")
    return owner


class WaylandQualificationObserver:
    """Bind an ordered close-to-tray observation to one GUI D-Bus owner."""

    def __init__(self, connection: object, gio: object, glib: object, gui_owner: str, gui_pid: int):
        self.connection = connection
        self.gio = gio
        self.glib = glib
        self.gui_owner = gui_owner
        self.gui_pid = gui_pid
        self.transaction = ""
        self.expected_mapped: bool | None = None
        self.observation: tuple[str, int, str, bool] | None = None
        self.failure: AcceptanceError | None = None
        self.loop: object | None = None
        self.subscription = 0
        self.pending_observations: list[tuple[str, tuple[str, int, str, bool]]] = []

    def subscribe(self) -> None:
        self.subscription = self.connection.signal_subscribe(
            self.gui_owner, HOST_INTERFACE, "QualificationObservation", HOST_OBJECT_PATH,
            None, self.gio.DBusSignalFlags.NONE, self._on_observation,
        )
        if not self.subscription:
            raise AcceptanceError("could not subscribe to CopyPaste Wayland qualification observations")

    def close(self) -> None:
        if self.subscription:
            self.connection.signal_unsubscribe(self.subscription)
            self.subscription = 0

    def start(self, action: str, capability: str) -> str:
        # A compositor can answer the request before the synchronous method
        # returns. Arm the first required observation before making that call.
        self.expected_mapped = False
        reply = self.connection.call_sync(
            HOST_BUS_NAME, HOST_OBJECT_PATH, HOST_INTERFACE, "StartQualification",
            self.glib.Variant("(ss)", (action, capability)), self.glib.VariantType.new("(s)"),
            self.gio.DBusCallFlags.NONE, 2000, None,
        )
        values = reply.unpack()
        if not isinstance(values, tuple) or len(values) != 1 or not isinstance(values[0], str) or not values[0]:
            raise AcceptanceError("Wayland qualification did not return a transaction")
        self.transaction = values[0]
        for sender, observation in self.pending_observations:
            self._accept_observation(sender, observation)
        self.pending_observations.clear()
        return self.transaction

    def _fail(self, message: str) -> None:
        self.failure = AcceptanceError(message)
        if self.loop is not None:
            self.loop.quit()

    def _accept_observation(self, sender: str, values: tuple[str, int, str, bool]) -> None:
        if sender != self.gui_owner:
            self._fail("Wayland qualification observation came from an unexpected GUI owner")
            return
        if (
            not isinstance(values, tuple) or len(values) != 4 or type(values[0]) is not str or
            type(values[1]) is not int or type(values[2]) is not str or type(values[3]) is not bool
        ):
            self._fail("Wayland qualification observation was not machine-readable")
            return
        transaction, pid, role, mapped = values
        if transaction != self.transaction:
            self._fail("Wayland qualification observation belongs to a stale transaction")
            return
        if pid != self.gui_pid or role != "main":
            self._fail("Wayland qualification observation does not identify the GUI main surface")
            return
        if self.expected_mapped is None or mapped != self.expected_mapped:
            self._fail("Wayland qualification observations arrived out of order")
            return
        self.observation = (transaction, pid, role, mapped)
        if self.loop is not None:
            self.loop.quit()

    def _on_observation(
        self, _connection: object, sender: str, _path: str, _interface: str, _signal: str,
        parameters: object,
    ) -> None:
        if sender != self.gui_owner:
            self._fail("Wayland qualification observation came from an unexpected GUI owner")
            return
        try:
            # Recheck both name ownership and process credentials for every signal.
            if require_wayland_gui_owner(self.connection, self.gio, self.glib, self.gui_pid) != self.gui_owner:
                self._fail("CopyPaste GUI D-Bus owner changed during Wayland qualification")
                return
            values = parameters.unpack()
        except AcceptanceError as error:
            self._fail(str(error))
            return
        except Exception:
            self._fail("Wayland qualification observation was not machine-readable")
            return
        if not self.transaction:
            if self.expected_mapped is None:
                self._fail("Wayland qualification observation arrived before its expected phase")
                return
            self.pending_observations.append((sender, values))
            return
        self._accept_observation(sender, values)

    def await_mapped(self, expected_mapped: bool) -> tuple[str, int, str, bool]:
        self.expected_mapped = expected_mapped
        if self.failure is not None:
            raise self.failure
        if self.observation is not None and self.observation[3] == expected_mapped:
            return self.observation
        self.observation = None
        loop = self.glib.MainLoop()
        self.loop = loop

        def timed_out() -> bool:
            self._fail("timed out waiting for authenticated Wayland qualification observation")
            return False

        timeout = self.glib.timeout_add(8000, timed_out)
        try:
            loop.run()
        finally:
            self.loop = None
            self.glib.source_remove(timeout)
        if self.failure is not None:
            raise self.failure
        if self.observation is None:
            raise AcceptanceError("Wayland qualification observer stopped without an observation")
        return self.observation


def start_wayland_qualification(action: str, gui_pid: int) -> WaylandQualificationObserver:
    """Subscribe before starting a guarded transaction; capability stays in D-Bus body only."""
    if action not in {"close-main", "quick-paste"}:
        raise AcceptanceError("unsupported Wayland qualification action")
    capability = os.environ.get("COPYPASTE_QUALIFICATION_CAPABILITY")
    if not capability or not re.fullmatch(r"[0-9a-fA-F]{64}", capability):
        raise AcceptanceError("Wayland qualification capability is unavailable")
    try:
        from gi.repository import Gio, GLib  # type: ignore[import-not-found]
    except ImportError as error:
        raise AcceptanceError("PyGObject is required for capability-safe Wayland qualification") from error
    connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    owner = require_wayland_gui_owner(connection, Gio, GLib, gui_pid)
    observer = WaylandQualificationObserver(connection, Gio, GLib, owner, gui_pid)
    observer.subscribe()
    try:
        observer.start(action, capability)
    except Exception:
        observer.close()
        raise
    return observer


def verify_wayland_close_to_tray(environment: dict[str, str], service: str, open_item: int, gui_pid: int) -> CommandResult:
    """Observe compositor remove/create around the real tray Open menu event."""
    observer = start_wayland_qualification("close-main", gui_pid)
    try:
        observer.await_mapped(False)
        menu_command = invoke_menu_item(environment, service, open_item)
        observer.await_mapped(True)
        return menu_command
    finally:
        observer.close()


def write_evidence(path: Path, evidence: dict) -> None:
    path.mkdir(parents=True, exist_ok=True)
    target = path / "linux-desktop-acceptance.json"
    target.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"COPYPASTE_QUALIFICATION_EVIDENCE {target.name}")


def qualify(args: argparse.Namespace) -> None:
    environment = dict(os.environ)
    if args.desktop not in {"GNOME", "KDE"} or args.session not in {"x11", "wayland"}:
        raise AcceptanceError("desktop and session must name a supported native qualification coordinate")
    for program in ("busctl", "dbus-monitor"):
        require_program(program, environment)
    service = expect_tray(environment, args.gui_pid)
    layout = dbus_menu_layout(environment, service)
    open_item = menu_item_id(layout, "Open")

    if args.session == "x11":
        for program in ("xdotool", "xprop", "xwininfo"):
            require_program(program, environment)
        window = x11_window_for_pid(environment, args.gui_pid)
        if x11_map_state(environment, window) != "IsViewable":
            raise AcceptanceError("CopyPaste X11 window is not visible before its close-to-tray action")
        require_ok(run(["xdotool", "windowclose", window], environment=environment), "request CopyPaste window close")
        wait_for_x11_map_state(environment, window, "IsUnMapped")
        menu_command = invoke_menu_item(environment, service, open_item)
        wait_for_x11_map_state(environment, window, "IsViewable")
        window_evidence = {"id": window, "hide_show": "dbusmenu_open"}
    else:
        menu_command = verify_wayland_close_to_tray(environment, service, open_item, args.gui_pid)
        window_evidence = {"observer": "compositor_dbus_signal", "hide_show": "dbusmenu_open"}

    notification, capture, history_count = run_notification_capture(
        environment, args.cli, args.socket, parse_capture_command(args.capture_command), args.gui_pid,
    )
    if notification.notification_id is None:
        raise AcceptanceError("desktop notification server did not return a notification id")
    write_evidence(
        args.evidence_dir,
        {
            "schema": 1,
            "desktop": args.desktop,
            "session": args.session,
            "gui_pid": args.gui_pid,
            "tray": {
                "service": service,
                "menu_path": MENU_PATH,
                "open_item_id": open_item,
            },
            "window": window_evidence,
            "notification": {
                "server_reply_id": notification.notification_id,
                "capture_command_sha256": command_digest(capture.argv),
            },
            "history": {"new_item_count": history_count},
            "commands": [
                {"argv": list(menu_command.argv), "returncode": menu_command.returncode},
                {"argv_sha256": command_digest(capture.argv), "returncode": capture.returncode},
            ],
        },
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gui-pid", type=int, required=True)
    parser.add_argument("--desktop", choices=("GNOME", "KDE"), required=True)
    parser.add_argument("--session", choices=("x11", "wayland"), required=True)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--socket", type=Path, required=True)
    parser.add_argument("--capture-command", required=True,
                        help="JSON argv for the real controlled clipboard-capture trigger")
    parser.add_argument("--evidence-dir", type=Path, required=True)
    args = parser.parse_args()
    try:
        qualify(args)
    except (AcceptanceError, OSError, subprocess.TimeoutExpired) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
