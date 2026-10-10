#!/usr/bin/env python3
"""Drive a real Wayland Quick Paste session through the RemoteDesktop portal.

This is a qualification helper, not a fixture: it owns a separate keyboard
portal session, sends the configured global shortcut through that session, and
requires a GTK target to receive the expected marker after Return.  The caller
must arrange the application's own RemoteDesktop consent through its visible
settings UI before this helper is invoked.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


PORTAL_NAME = "org.freedesktop.portal.Desktop"
PORTAL_PATH = "/org/freedesktop/portal/desktop"
GUI_BUS_NAME = "app.copypaste.CopyPaste"
REQUEST_INTERFACE = "org.freedesktop.portal.Request"
SESSION_INTERFACE = "org.freedesktop.portal.Session"
REMOTE_DESKTOP = "org.freedesktop.portal.RemoteDesktop"
KEYBOARD_DEVICE = 1
EVDEV_CONTROL = 29
EVDEV_SHIFT = 42
EVDEV_C = 46
EVDEV_RETURN = 28
TARGET_TITLE = "CopyPaste Qualification Wayland Input Target"
TARGET_SOURCE = Path(__file__).with_name("linux-wayland-input-target.c")
MEDIA_ACTOR = Path(__file__).with_name("linux-native-media.py")
UNIQUE_NAME = re.compile(r"^:[0-9]+\.[0-9]+$")
PORTAL_IMPLEMENTATION_PREFIX = "org.freedesktop.impl.portal.desktop."
APP_PERMISSION_GRANTED = "Keyboard control is allowed for Quick Paste."
PORTAL_DIALOG_PRESENTERS = {
    "GNOME": ("org.gnome.Shell",),
    "KDE": ("org.kde.plasmashell", "org.kde.KWin"),
}


class QuickPasteQualificationError(RuntimeError):
    """A real Wayland portal or product observation was unavailable."""


def sha256(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def require_regular(path: Path, description: str) -> Path:
    try:
        details = path.lstat()
    except OSError as error:
        raise QuickPasteQualificationError(f"{description} is unavailable") from error
    if path.is_symlink() or not path.is_file() or details.st_size <= 0:
        raise QuickPasteQualificationError(f"{description} must be a regular file")
    return path


def marker_argument(value: str) -> str:
    if not 1 <= len(value) <= 512 or any(ord(character) < 0x20 or ord(character) > 0x7E for character in value):
        raise argparse.ArgumentTypeError("marker must be 1-512 printable ASCII characters")
    return value


def token(prefix: str) -> str:
    return f"copypaste_qualification_{prefix}_{os.getpid()}_{os.urandom(8).hex()}"


def portal_request_path(unique_owner: str, handle_token: str) -> str:
    """Derive a portal Request path before issuing its method call."""
    if (
        not isinstance(unique_owner, str) or not isinstance(handle_token, str) or
        not UNIQUE_NAME.fullmatch(unique_owner) or not re.fullmatch(r"[A-Za-z0-9_]+", handle_token)
    ):
        raise QuickPasteQualificationError("portal request identity is invalid")
    sender = re.sub(r"[^A-Za-z0-9]", "_", unique_owner[1:])
    return f"{PORTAL_PATH}/request/{sender}/{handle_token}"


def receipt(
    *, gui_pid: int, portal_dialog_pid: int, input_target_pid: int, marker: str,
    portal_input_calls: int, portal_session_digest: str, portal_click_digest: str,
) -> dict[str, Any]:
    """Return a redacted receipt bound to every UI actor process."""
    pids = (gui_pid, portal_dialog_pid, input_target_pid)
    digests = (portal_session_digest, portal_click_digest)
    if any(type(pid) is not int or pid <= 0 for pid in pids):
        raise QuickPasteQualificationError("Wayland receipt actor identity is invalid")
    if type(portal_input_calls) is not int or portal_input_calls < 8 or any(not re.fullmatch(r"[0-9a-f]{64}", value) for value in digests):
        raise QuickPasteQualificationError("Wayland receipt proof is invalid")
    return {
        "schema": 1,
        "kind": "wayland_quick_paste",
        "actors": {
            "gui_pid": gui_pid,
            "portal_dialog_pid": portal_dialog_pid,
            "input_target_pid": input_target_pid,
        },
        "target": {"pid": input_target_pid, "title_sha256": sha256(TARGET_TITLE), "marker_sha256": sha256(marker)},
        "portal": {"input_calls": portal_input_calls, "session_sha256": portal_session_digest},
        "atspi": {
            "portal_click_sha256": portal_click_digest,
        },
    }


def build_target(workspace: Path, environment: dict[str, str]) -> Path:
    require_regular(TARGET_SOURCE, "Wayland input target source")
    if shutil.which("pkg-config", path=environment.get("PATH")) is None or shutil.which("cc", path=environment.get("PATH")) is None:
        raise QuickPasteQualificationError("C compiler and GTK development metadata are required")
    flags = subprocess.run(
        ["pkg-config", "--cflags", "--libs", "gtk+-3.0"], env=environment, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    if flags.returncode:
        raise QuickPasteQualificationError("GTK 3 development metadata is unavailable")
    output = workspace / "copypaste-wayland-input-target"
    compiled = subprocess.run(
        ["cc", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", str(TARGET_SOURCE), "-o", str(output), *flags.stdout.split()],
        env=environment, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    if compiled.returncode or not output.is_file() or output.is_symlink() or not os.access(output, os.X_OK):
        raise QuickPasteQualificationError("Wayland input target did not compile")
    return output


def read_target_state(path: Path) -> dict[str, Any] | None:
    if not path.is_file() or path.is_symlink():
        return None
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError):
        return None
    if (
        not isinstance(value, dict) or set(value) != {"focus_events", "marker_received"} or
        type(value["focus_events"]) is not int or value["focus_events"] < 0 or
        type(value["marker_received"]) is not bool
    ):
        raise QuickPasteQualificationError("Wayland input target wrote an invalid observation")
    return value


def wait_for(path: Path, predicate, description: str, timeout: float = 12) -> Any:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate(path)
        if value is not None:
            return value
        time.sleep(0.05)
    raise QuickPasteQualificationError(description)


@dataclass
class Target:
    process: subprocess.Popen[bytes]
    ready: Path
    result: Path
    pid: int

    def close(self) -> None:
        self.process.terminate()
        try:
            self.process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=3)


def start_target(helper: Path, workspace: Path, marker: str, environment: dict[str, str]) -> Target:
    directory = Path(tempfile.mkdtemp(prefix="copypaste-wayland-input-", dir=workspace))
    os.chmod(directory, 0o700)
    ready = directory / "ready"
    result = directory / "result.json"
    process = subprocess.Popen(
        [str(helper), "--ready-file", str(ready), "--result-file", str(result), "--marker", marker],
        env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    try:
        text = wait_for(ready, lambda item: item.read_text(encoding="utf-8") if item.is_file() else None,
                        "Wayland input target did not gain focus")
        match = re.fullmatch(r"focused pid=([1-9][0-9]*)\n", text)
        if match is None or int(match.group(1)) != process.pid:
            raise QuickPasteQualificationError("Wayland input target identity is invalid")
        return Target(process, ready, result, process.pid)
    except Exception:
        process.terminate()
        process.wait(timeout=3)
        raise


class PortalKeyboard:
    """One independent, keyboard-only RemoteDesktop session with real replies."""

    def __init__(self, gio: Any, glib: Any):
        self.gio = gio
        self.glib = glib
        self.connection = gio.bus_get_sync(gio.BusType.SESSION, None)
        self.owner = self._owner()
        self.session = ""
        self.session_digest = ""
        self.calls: list[str] = []

    def _call(self, interface: str, method: str, parameters: Any, reply_type: str | None = None) -> Any:
        reply = self.connection.call_sync(
            PORTAL_NAME, PORTAL_PATH, interface, method, parameters,
            self.glib.VariantType.new(reply_type) if reply_type else None,
            self.gio.DBusCallFlags.NONE, 8000, None,
        )
        self.calls.append(method)
        return reply

    def _owner(self) -> str:
        reply = self.connection.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetNameOwner",
            self.glib.Variant("(s)", (PORTAL_NAME,)), self.glib.VariantType.new("(s)"),
            self.gio.DBusCallFlags.NONE, 8000, None,
        )
        owner, = reply.unpack()
        if not isinstance(owner, str) or not UNIQUE_NAME.fullmatch(owner):
            raise QuickPasteQualificationError("desktop portal has no unique owner")
        credentials = self.connection.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetConnectionUnixUser",
            self.glib.Variant("(s)", (owner,)), self.glib.VariantType.new("(u)"),
            self.gio.DBusCallFlags.NONE, 8000, None,
        )
        uid, = credentials.unpack()
        if type(uid) is not int or uid != os.getuid():
            raise QuickPasteQualificationError("desktop portal is not owned by the qualification user")
        return owner

    def _bus_owner(self, name: str) -> str:
        reply = self.connection.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetNameOwner",
            self.glib.Variant("(s)", (name,)), self.glib.VariantType.new("(s)"),
            self.gio.DBusCallFlags.NONE, 8000, None,
        )
        owner, = reply.unpack()
        if not isinstance(owner, str) or not UNIQUE_NAME.fullmatch(owner):
            raise QuickPasteQualificationError("portal implementation has no unique D-Bus owner")
        return owner

    def _owner_credentials(self, owner: str) -> tuple[int, int]:
        pid_reply = self.connection.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetConnectionUnixProcessID",
            self.glib.Variant("(s)", (owner,)), self.glib.VariantType.new("(u)"),
            self.gio.DBusCallFlags.NONE, 8000, None,
        )
        uid_reply = self.connection.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetConnectionUnixUser",
            self.glib.Variant("(s)", (owner,)), self.glib.VariantType.new("(u)"),
            self.gio.DBusCallFlags.NONE, 8000, None,
        )
        pid, = pid_reply.unpack()
        uid, = uid_reply.unpack()
        if type(pid) is not int or pid <= 0 or type(uid) is not int or uid != os.getuid():
            raise QuickPasteQualificationError("portal implementation credentials are invalid")
        return pid, uid

    def owned_portal_pids(self) -> set[int]:
        """Return same-user portal services and this desktop's live presenter."""
        reply = self.connection.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "ListNames",
            None, self.glib.VariantType.new("(as)"), self.gio.DBusCallFlags.NONE, 8000, None,
        )
        names, = reply.unpack()
        if not isinstance(names, (list, tuple)):
            raise QuickPasteQualificationError("session D-Bus portal inventory is invalid")
        portal_names = [
            name for name in names if name == PORTAL_NAME or (
                isinstance(name, str) and name.startswith(PORTAL_IMPLEMENTATION_PREFIX)
            )
        ]
        if PORTAL_NAME not in portal_names:
            raise QuickPasteQualificationError("desktop portal is absent from the session bus")
        desktop = os.environ.get("XDG_CURRENT_DESKTOP")
        presenters = PORTAL_DIALOG_PRESENTERS.get(desktop or "")
        if presenters is None:
            raise QuickPasteQualificationError("Wayland portal desktop identity is unsupported")
        portal_names.extend(name for name in presenters if name in names)
        return {self._owner_credentials(self._bus_owner(name))[0] for name in portal_names}

    def require_gui_owner(self, gui_pid: int) -> str:
        """Bind this run to the exact GUI process before driving any input."""
        owner_reply = self.connection.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetNameOwner",
            self.glib.Variant("(s)", (GUI_BUS_NAME,)), self.glib.VariantType.new("(s)"),
            self.gio.DBusCallFlags.NONE, 8000, None,
        )
        owner, = owner_reply.unpack()
        if not isinstance(owner, str) or not UNIQUE_NAME.fullmatch(owner):
            raise QuickPasteQualificationError("CopyPaste GUI has no unique D-Bus owner")
        pid_reply = self.connection.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetConnectionUnixProcessID",
            self.glib.Variant("(s)", (owner,)), self.glib.VariantType.new("(u)"),
            self.gio.DBusCallFlags.NONE, 8000, None,
        )
        uid_reply = self.connection.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetConnectionUnixUser",
            self.glib.Variant("(s)", (owner,)), self.glib.VariantType.new("(u)"),
            self.gio.DBusCallFlags.NONE, 8000, None,
        )
        pid, = pid_reply.unpack()
        uid, = uid_reply.unpack()
        if type(pid) is not int or type(uid) is not int or pid != gui_pid or uid != os.getuid():
            raise QuickPasteQualificationError("CopyPaste GUI D-Bus credentials do not match the requested process")
        return owner

    def _watch_response(self, request: str) -> dict[str, Any]:
        watch: dict[str, Any] = {"outcome": None, "failure": None, "loop": None, "subscription": 0, "settled": False}
        def received(_connection: Any, sender: str, path: str, _interface: str, _signal: str, parameters: Any) -> None:
            if watch["settled"]:
                return
            watch["settled"] = True
            if sender != self.owner or path != request:
                watch["failure"] = "desktop portal response identity changed"
            else:
                try:
                    code, values = parameters.unpack()
                    if type(code) is not int or code != 0 or not isinstance(values, dict):
                        watch["failure"] = "desktop portal did not grant keyboard control"
                    else:
                        watch["outcome"] = values
                except Exception:
                    watch["failure"] = "desktop portal response was not machine-readable"
            if watch["loop"] is not None:
                watch["loop"].quit()

        watch["subscription"] = self.connection.signal_subscribe(
            self.owner, REQUEST_INTERFACE, "Response", request, None,
            self.gio.DBusSignalFlags.NONE, received,
        )
        if not watch["subscription"]:
            raise QuickPasteQualificationError("could not subscribe to the desktop portal request")
        return watch

    def _unsubscribe_response(self, watch: dict[str, Any]) -> None:
        subscription = watch["subscription"]
        if subscription:
            self.connection.signal_unsubscribe(subscription)
            watch["subscription"] = 0

    def _wait_response(self, watch: dict[str, Any]) -> dict[str, Any]:
        if watch["outcome"] is not None or watch["failure"] is not None:
            self._unsubscribe_response(watch)
            if watch["failure"] is not None:
                raise QuickPasteQualificationError(watch["failure"])
            return watch["outcome"]
        loop = self.glib.MainLoop()
        watch["loop"] = loop
        timeout_failure: list[str] = []
        timeout = self.glib.timeout_add(15000, lambda: (timeout_failure.append("desktop portal consent timed out"), loop.quit(), False)[2])
        try:
            loop.run()
        finally:
            self._unsubscribe_response(watch)
            self.glib.source_remove(timeout)
        if timeout_failure or watch["failure"] is not None:
            raise QuickPasteQualificationError(timeout_failure[0] if timeout_failure else watch["failure"])
        if not isinstance(watch["outcome"], dict):
            raise QuickPasteQualificationError("desktop portal response was unavailable")
        return watch["outcome"]

    def _request(self, method: str, signature: str, values: tuple[Any, ...], on_requested=None) -> dict[str, Any]:
        options = next((value for value in values if isinstance(value, dict) and "handle_token" in value), None)
        if not isinstance(options, dict) or not isinstance(options.get("handle_token"), str):
            raise QuickPasteQualificationError("portal request has no safe handle token")
        predicted = portal_request_path(self.connection.get_unique_name(), options["handle_token"])
        watch = self._watch_response(predicted)
        try:
            reply = self._call(REMOTE_DESKTOP, method, self.glib.Variant(signature, values), "(o)")
            request, = reply.unpack()
            if not isinstance(request, str) or request != predicted:
                raise QuickPasteQualificationError("desktop portal did not return a request handle")
            if on_requested is not None:
                on_requested()
            return self._wait_response(watch)
        finally:
            self._unsubscribe_response(watch)

    def grant(self, on_start_requested) -> None:
        created = self._request("CreateSession", "(a{sv})", ({"handle_token": token("create"), "session_handle_token": token("session")},))
        session = created.get("session_handle")
        if not isinstance(session, str) or not session.startswith("/"):
            raise QuickPasteQualificationError("desktop portal did not create a keyboard session")
        self.session = session
        self.session_digest = sha256(session)
        self._request("SelectDevices", "(oa{sv})", (session, {"handle_token": token("devices"), "types": KEYBOARD_DEVICE}))
        started = self._request(
            "Start", "(osa{sv})", (session, "", {"handle_token": token("start")}), on_start_requested,
        )
        if type(started.get("devices")) is not int or not (started["devices"] & KEYBOARD_DEVICE):
            raise QuickPasteQualificationError("desktop portal did not grant keyboard input")

    def key(self, keycode: int, state: int) -> None:
        if not self.session:
            raise QuickPasteQualificationError("keyboard portal session is unavailable")
        self._call(REMOTE_DESKTOP, "NotifyKeyboardKeycode", self.glib.Variant("(oa{sv}iu)", (self.session, {}, keycode, state)))

    def chord(self, *keycodes: int) -> None:
        for keycode in keycodes:
            self.key(keycode, 1)
        for keycode in reversed(keycodes):
            self.key(keycode, 0)

    def close(self) -> None:
        if self.session:
            try:
                self._call(SESSION_INTERFACE, "Close", self.glib.Variant("(o)", (self.session,)))
            finally:
                self.session = ""


def accessible_children(node: Any):
    try:
        count = node.childCount
    except Exception as error:
        raise QuickPasteQualificationError("AT-SPI tree is unavailable") from error
    for index in range(count):
        try:
            yield node.getChildAtIndex(index)
        except Exception:
            continue


def accessible_descendants(node: Any):
    pending = [node]
    while pending:
        current = pending.pop()
        yield current
        pending.extend(reversed(tuple(accessible_children(current))))


def atspi_application(registry: Any, pid: int) -> Any | None:
    try:
        desktop = registry.getDesktop(0)
    except Exception as error:
        raise QuickPasteQualificationError("AT-SPI desktop is unavailable") from error
    for application in accessible_children(desktop):
        try:
            if application.get_process_id() == pid:
                return application
        except Exception:
            continue
    return None


def action_index(node: Any) -> int | None:
    try:
        actions = node.queryAction()
        matching = [index for index in range(actions.nActions) if actions.getName(index) in {"click", "press"}]
        if len(matching) == 1:
            return matching[0]
    except Exception:
        return None
    return None


def default_portal_action(application: Any, atspi: Any) -> tuple[Any, int] | None:
    """Find exactly one default action in a portal-owned modal dialog."""
    nodes = tuple(accessible_descendants(application))
    try:
        modal_roots = [node for node in nodes if node.getState().contains(atspi.STATE_MODAL)]
    except Exception:
        return None
    if len(modal_roots) != 1:
        return None
    candidates = []
    for node in accessible_descendants(modal_roots[0]):
        try:
            state = node.getState()
            if all(state.contains(value) for value in (atspi.STATE_DEFAULT, atspi.STATE_SHOWING, atspi.STATE_VISIBLE, atspi.STATE_ENABLED)):
                index = action_index(node)
                if index is not None:
                    candidates.append((node, index))
        except Exception:
            continue
    return candidates[0] if len(candidates) == 1 else None


def click_owned_portal_consent(portal: PortalKeyboard) -> tuple[int, str]:
    """Accept one visible consent dialog owned by this user's portal services."""
    try:
        import pyatspi  # type: ignore[import-not-found]
    except ImportError as error:
        raise QuickPasteQualificationError("python3-pyatspi is required for Wayland portal qualification") from error
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline:
        for pid in sorted(portal.owned_portal_pids()):
            application = atspi_application(pyatspi.Registry, pid)
            if application is None:
                continue
            candidate = default_portal_action(application, pyatspi)
            if candidate is None:
                continue
            node, index = candidate
            try:
                if node.queryAction().doAction(index) is True:
                    return pid, sha256(f"portal-default-action:{pid}")
            except Exception:
                continue
        time.sleep(0.2)
    raise QuickPasteQualificationError("owned portal consent dialog did not expose one default action")


def actor_command(actor: Path, command: str, pid: int, target: str, environment: dict[str, str]) -> str:
    require_regular(actor, "AT-SPI actor")
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline:
        result = subprocess.run(
            [sys.executable, str(actor), command, "--pid", str(pid), "--target", target],
            env=environment, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False, timeout=20,
        )
        if not result.returncode:
            return hashlib.sha256(result.stdout.encode("utf-8")).hexdigest()
        time.sleep(0.2)
    raise QuickPasteQualificationError(f"AT-SPI {command} action failed")


def grant_product_permission(args: argparse.Namespace) -> dict[str, Any]:
    """Grant the installed application's own portal request from onboarding."""
    if args.gui_pid <= 0:
        raise QuickPasteQualificationError("GUI PID is invalid")
    environment = dict(os.environ)
    if not environment.get("WAYLAND_DISPLAY"):
        raise QuickPasteQualificationError("Wayland display is unavailable")
    try:
        from gi.repository import Gio, GLib  # type: ignore[import-not-found]
    except ImportError as error:
        raise QuickPasteQualificationError("PyGObject is required for Wayland portal qualification") from error
    portal = PortalKeyboard(Gio, GLib)
    try:
        gui_owner = portal.require_gui_owner(args.gui_pid)
        app_click = actor_command(args.actor, "click", args.gui_pid, args.app_permission_target, environment)
        dialog_pid, consent = click_owned_portal_consent(portal)
        granted = (
            actor_command(args.actor, "observe", args.gui_pid, APP_PERMISSION_GRANTED, environment)
            if args.require_granted_text else consent
        )
        if portal.require_gui_owner(args.gui_pid) != gui_owner:
            raise QuickPasteQualificationError("CopyPaste GUI D-Bus owner changed during its portal consent")
        return {"schema": 1, "gui_pid": args.gui_pid, "dialog_pid": dialog_pid,
                "click_sha256": app_click, "consent_sha256": sha256(f"{consent}:{granted}")}
    finally:
        portal.close()


def qualify(args: argparse.Namespace) -> dict[str, Any]:
    if args.gui_pid <= 0:
        raise QuickPasteQualificationError("GUI PID is invalid")
    environment = dict(os.environ)
    if not environment.get("WAYLAND_DISPLAY"):
        raise QuickPasteQualificationError("Wayland display is unavailable")
    helper = build_target(args.workspace, environment)
    try:
        from gi.repository import Gio, GLib  # type: ignore[import-not-found]
    except ImportError as error:
        raise QuickPasteQualificationError("PyGObject is required for Wayland portal qualification") from error
    portal = PortalKeyboard(Gio, GLib)
    try:
        gui_owner = portal.require_gui_owner(args.gui_pid)
        target = start_target(helper, args.workspace, args.marker, environment)
        try:
            consent_click = ""
            portal_dialog_pid = 0

            def accept_helper_consent() -> None:
                nonlocal consent_click, portal_dialog_pid
                portal_dialog_pid, consent_click = click_owned_portal_consent(portal)

            portal.grant(accept_helper_consent)
            if portal.require_gui_owner(args.gui_pid) != gui_owner:
                raise QuickPasteQualificationError("CopyPaste GUI D-Bus owner changed before global shortcut input")
            portal.chord(EVDEV_CONTROL, EVDEV_SHIFT, EVDEV_C)
            # A real target result after Return requires the product's actual
            # shortcut activation and focus restoration; no qualification D-Bus
            # transaction is used as a substitute for that proof.
            portal.chord(EVDEV_RETURN)
            state = wait_for(
                target.result,
                lambda path: (candidate if (candidate := read_target_state(path)) and candidate["focus_events"] >= 2 and candidate["marker_received"] else None),
                "Quick Paste did not restore target focus and insert its marker",
            )
            if portal.require_gui_owner(args.gui_pid) != gui_owner:
                raise QuickPasteQualificationError("CopyPaste GUI D-Bus owner changed during Quick Paste input")
        finally:
            target.close()
        return receipt(
            gui_pid=args.gui_pid, portal_dialog_pid=portal_dialog_pid, input_target_pid=target.pid, marker=args.marker,
            portal_input_calls=portal.calls.count("NotifyKeyboardKeycode"), portal_session_digest=portal.session_digest,
            portal_click_digest=consent_click,
        )
    finally:
        portal.close()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gui-pid", required=True, type=int)
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--marker", type=marker_argument)
    parser.add_argument("--actor", type=Path, default=MEDIA_ACTOR)
    parser.add_argument("--app-permission-target", default="Allow keyboard control")
    parser.add_argument("--grant-product-permission", action="store_true")
    parser.add_argument("--require-granted-text", action="store_true")
    args = parser.parse_args()
    try:
        if args.grant_product_permission:
            print("COPYPASTE_QUALIFICATION_WAYLAND_PERMISSION " + json.dumps(grant_product_permission(args), separators=(",", ":"), sort_keys=True))
        else:
            if args.workspace is None or args.marker is None:
                raise QuickPasteQualificationError("workspace and marker are required for Quick Paste qualification")
            print("COPYPASTE_QUALIFICATION_WAYLAND " + json.dumps(qualify(args), separators=(",", ":"), sort_keys=True))
        return 0
    except (QuickPasteQualificationError, OSError, subprocess.TimeoutExpired) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
