#!/usr/bin/env python3
"""Audit authenticated CopyPaste clipboard read requests on the session bus."""

import argparse
import json
import os
import re
import signal
import stat
import sys
from pathlib import Path
from typing import Any, Optional


BUS_NAME = "org.freedesktop.DBus"
BUS_PATH = "/org/freedesktop/DBus"
BUS_INTERFACE = "org.freedesktop.DBus"
MONITOR_INTERFACE = "org.freedesktop.DBus.Monitoring"
DAEMON_NAME = "app.copypaste.Daemon"
CLIPBOARD_PATH = "/app/copypaste/Clipboard"
CLIPBOARD_INTERFACE = "app.copypaste.Clipboard"
READ_MEMBER = "Read"
MAX_MIME_BYTES = 255
UNIQUE_OWNER = re.compile(r"^:[0-9]+(?:\.[0-9]+)*$")


class ObserverError(RuntimeError):
    """The observer cannot provide trustworthy qualification evidence."""


def positive_pid(value: str) -> int:
    try:
        pid = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("daemon pid must be a positive integer") from error
    if pid <= 0:
        raise argparse.ArgumentTypeError("daemon pid must be a positive integer")
    return pid


def private_output(path: Path) -> int:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags, 0o600)
    except FileExistsError as error:
        raise ObserverError(f"output path already exists: {path}") from error
    except OSError as error:
        raise ObserverError(f"could not create private output: {path}") from error
    try:
        os.fchmod(descriptor, 0o600)
        details = os.fstat(descriptor)
        if not stat.S_ISREG(details.st_mode) or details.st_nlink != 1:
            raise ObserverError(f"output path is not a new private regular file: {path}")
        return descriptor
    except Exception:
        os.close(descriptor)
        raise


def variant_value(connection: Any, gio: Any, glib: Any, member: str, argument: str) -> Any:
    try:
        reply = connection.call_sync(
            BUS_NAME, BUS_PATH, BUS_INTERFACE, member,
            glib.Variant("(s)", (argument,)), None,
            gio.DBusCallFlags.NONE, 2000, None,
        )
        values = reply.unpack()
    except Exception as error:
        raise ObserverError(f"could not authenticate daemon D-Bus owner via {member}") from error
    if not isinstance(values, tuple) or len(values) != 1:
        raise ObserverError(f"daemon D-Bus owner returned an invalid {member} result")
    return values[0]


def require_daemon_owner(connection: Any, gio: Any, glib: Any, daemon_pid: int) -> str:
    owner = variant_value(connection, gio, glib, "GetNameOwner", DAEMON_NAME)
    if not isinstance(owner, str) or not UNIQUE_OWNER.fullmatch(owner):
        raise ObserverError("daemon D-Bus name does not have a valid unique owner")
    process_id = variant_value(connection, gio, glib, "GetConnectionUnixProcessID", owner)
    user_id = variant_value(connection, gio, glib, "GetConnectionUnixUser", owner)
    if isinstance(process_id, bool) or not isinstance(process_id, int) or process_id != daemon_pid:
        raise ObserverError("daemon D-Bus owner PID does not match the requested daemon PID")
    if isinstance(user_id, bool) or not isinstance(user_id, int) or user_id != os.getuid():
        raise ObserverError("daemon D-Bus owner UID does not match this user")
    return owner


def valid_mime(value: object) -> bool:
    return (
        isinstance(value, str)
        and "\0" not in value
        and 0 < len(value.encode("utf-8")) <= MAX_MIME_BYTES
    )


class AuditFile:
    def __init__(self, path: Path):
        self.path = path
        self.descriptor = private_output(path)

    def append(self, target: str, requestor_pid: int) -> None:
        record = json.dumps(
            {"target": target, "requestor_pid": requestor_pid, "error": None},
            separators=(",", ":"),
        ).encode("utf-8") + b"\n"
        try:
            write_all(self.descriptor, record)
            os.fsync(self.descriptor)
        except OSError as error:
            raise ObserverError("could not flush clipboard-read audit record") from error

    def close(self) -> None:
        if self.descriptor >= 0:
            os.close(self.descriptor)
            self.descriptor = -1


def write_ready(path: Path, daemon_pid: int, owner: str) -> None:
    descriptor = private_output(path)
    document = json.dumps(
        {"schema": 1, "daemon_pid": daemon_pid, "bus_owner": owner},
        separators=(",", ":"),
    ).encode("utf-8")
    try:
        write_all(descriptor, document)
        os.fsync(descriptor)
    except OSError as error:
        raise ObserverError("could not write observer readiness") from error
    finally:
        os.close(descriptor)


def write_all(descriptor: int, data: bytes) -> None:
    offset = 0
    while offset < len(data):
        written = os.write(descriptor, data[offset:])
        if written <= 0:
            raise OSError("could not write private observer output")
        offset += written


class ClipboardReadObserver:
    def __init__(self, connection: Any, gio: Any, glib: Any, daemon_pid: int, owner: str, audit: AuditFile):
        self.connection = connection
        self.gio = gio
        self.glib = glib
        self.daemon_pid = daemon_pid
        self.owner = owner
        self.audit = audit
        self.failure: Optional[ObserverError] = None
        self.loop: Any = None
        self.monitoring = False
        self.stopping = False

    def fail(self, message: str) -> None:
        if self.failure is None:
            self.failure = ObserverError(message)
        if self.loop is not None:
            self.loop.quit()

    def _message(self, _connection: Any, message: Any, incoming: bool, *_unused: Any) -> Any:
        if not incoming:
            return message
        if not self.monitoring:
            return message
        if self.stopping:
            return None
        try:
            message_type = message.get_message_type()
            if message_type == self.gio.DBusMessageType.SIGNAL:
                self._name_owner_changed(message)
            elif message_type == self.gio.DBusMessageType.METHOD_CALL:
                self._read(message)
        except ObserverError as error:
            self.fail(str(error))
        except Exception:
            self.fail("clipboard-read monitor received an unreadable D-Bus message")
        return None

    def _name_owner_changed(self, message: Any) -> None:
        if (
            message.get_path() != BUS_PATH
            or message.get_interface() != BUS_INTERFACE
            or message.get_member() != "NameOwnerChanged"
            or message.get_sender() != BUS_NAME
        ):
            return
        body = message.get_body()
        if body is None or body.get_type_string() != "(sss)":
            raise ObserverError("daemon ownership monitor received a malformed NameOwnerChanged signal")
        values = body.unpack()
        if not isinstance(values, tuple) or len(values) != 3 or values[0] != DAEMON_NAME:
            return
        if values[1] != self.owner or values[2] != self.owner:
            raise ObserverError("daemon D-Bus owner was lost or replaced during clipboard-read qualification")

    def _read(self, message: Any) -> None:
        if (
            message.get_path() != CLIPBOARD_PATH
            or message.get_interface() != CLIPBOARD_INTERFACE
            or message.get_member() != READ_MEMBER
            or message.get_sender() != self.owner
        ):
            return
        body = message.get_body()
        if body is None or body.get_type_string() != "(tsu)":
            raise ObserverError("authenticated clipboard Read call has an invalid body signature")
        values = body.unpack()
        if (
            not isinstance(values, tuple)
            or len(values) != 3
            or isinstance(values[0], bool)
            or not isinstance(values[0], int)
            or not valid_mime(values[1])
            or isinstance(values[2], bool)
            or not isinstance(values[2], int)
        ):
            raise ObserverError("authenticated clipboard Read call has an invalid body")
        self.audit.append(values[1], self.daemon_pid)

    def _closed(self, *_unused: Any) -> None:
        if not self.stopping:
            self.fail("clipboard-read monitor D-Bus connection closed")

    def start(self) -> None:
        self.connection.add_filter(self._message)
        self.connection.connect("closed", self._closed)
        try:
            self.connection.call_sync(
                BUS_NAME, BUS_PATH, MONITOR_INTERFACE, "BecomeMonitor",
                self.glib.Variant(
                    "(asu)",
                    ([
                        "type='method_call',path='/app/copypaste/Clipboard',interface='app.copypaste.Clipboard',member='Read'",
                        "type='signal',path='/org/freedesktop/DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='app.copypaste.Daemon'",
                    ], 0),
                ),
                None, self.gio.DBusCallFlags.NONE, 2000, None,
            )
            self.monitoring = True
        except Exception as error:
            raise ObserverError("could not become a private D-Bus monitor") from error

    def run(self) -> None:
        if self.failure is not None:
            raise self.failure
        if self.stopping:
            return
        self.loop = self.glib.MainLoop()
        try:
            self.loop.run()
        finally:
            self.loop = None
        if self.failure is not None:
            raise self.failure

    def stop(self) -> None:
        self.stopping = True
        if self.loop is not None:
            self.loop.quit()


def parse_args(argv: Optional[list[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--daemon-pid", required=True, type=positive_pid)
    parser.add_argument("--ready-file", required=True, type=Path)
    parser.add_argument("--audit-file", required=True, type=Path)
    return parser.parse_args(argv)


def main(argv: Optional[list[str]] = None) -> int:
    args = parse_args(argv)
    audit: Optional[AuditFile] = None
    try:
        if args.ready_file.exists() or args.ready_file.is_symlink() or args.audit_file.exists() or args.audit_file.is_symlink():
            raise ObserverError("observer output path already exists")
        try:
            from gi.repository import Gio, GLib  # type: ignore[import-not-found]
        except ImportError as error:
            raise ObserverError("PyGObject is required for clipboard-read qualification") from error
        connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        owner = require_daemon_owner(connection, Gio, GLib, args.daemon_pid)
        audit = AuditFile(args.audit_file)
        observer = ClipboardReadObserver(connection, Gio, GLib, args.daemon_pid, owner, audit)
        observer.start()
        signal.signal(signal.SIGTERM, lambda *_unused: observer.stop())
        signal.signal(signal.SIGINT, lambda *_unused: observer.stop())
        if observer.stopping:
            raise ObserverError("clipboard-read monitor stopped before readiness")
        write_ready(args.ready_file, args.daemon_pid, owner)
        observer.run()
        return 0
    except ObserverError as error:
        print(f"clipboard-read observer: {error}", file=sys.stderr)
        return 1
    finally:
        if audit is not None:
            audit.close()


if __name__ == "__main__":
    raise SystemExit(main())
