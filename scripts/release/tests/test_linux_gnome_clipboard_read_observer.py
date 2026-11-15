#!/usr/bin/env python3
"""Focused contracts for the private GNOME clipboard-read observer."""

import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import time
import unittest


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "linux_gnome_clipboard_read_observer",
    ROOT / "scripts/release/linux-gnome-clipboard-read-observer.py",
)
module = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = module
SPEC.loader.exec_module(module)

try:
    import gi  # type: ignore[import-not-found]
    gi.require_version("Gio", "2.0")
    from gi.repository import Gio as _NativeGio, GLib as _NativeGLib  # type: ignore[import-not-found]
    HAVE_PRIVATE_DBUS = True
except ImportError:
    HAVE_PRIVATE_DBUS = False


class Reply:
    def __init__(self, value):
        self.value = value

    def unpack(self):
        return (self.value,)


class AuthenticationConnection:
    def __init__(self, *, owner=":1.45", pid=52958, uid=None):
        self.owner = owner
        self.pid = pid
        self.uid = os.getuid() if uid is None else uid

    def call_sync(self, _name, _path, _interface, member, _parameters, *_args):
        return Reply({
            "GetNameOwner": self.owner,
            "GetConnectionUnixProcessID": self.pid,
            "GetConnectionUnixUser": self.uid,
        }[member])


class Gio:
    class DBusCallFlags:
        NONE = 0

    class DBusMessageType:
        SIGNAL = 1
        METHOD_CALL = 2


class GLib:
    @staticmethod
    def Variant(signature, values):
        return (signature, values)


class Body:
    def __init__(self, signature, values):
        self.signature = signature
        self.values = values

    def get_type_string(self):
        return self.signature

    def unpack(self):
        return self.values


class Message:
    def __init__(self, kind, *, path, interface, member, sender, body):
        self.kind = kind
        self.path = path
        self.interface = interface
        self.member = member
        self.sender = sender
        self.body = body

    def get_message_type(self):
        return self.kind

    def get_path(self):
        return self.path

    def get_interface(self):
        return self.interface

    def get_member(self):
        return self.member

    def get_sender(self):
        return self.sender

    def get_body(self):
        return self.body


class ObserverConnection:
    def __init__(self):
        self.filter = None
        self.connected = []
        self.calls = []

    def add_filter(self, callback):
        self.filter = callback

    def connect(self, signal_name, callback):
        self.connected.append((signal_name, callback))

    def call_sync(self, *args):
        self.calls.append(args)
        return object()


class LinuxGnomeClipboardReadObserverTest(unittest.TestCase):
    def test_authentication_rejects_wrong_pid_uid_and_owner(self):
        for connection, expected in (
            (AuthenticationConnection(pid=7), "PID"),
            (AuthenticationConnection(uid=os.getuid() + 1), "UID"),
            (AuthenticationConnection(owner="app.copypaste.Daemon"), "unique owner"),
        ):
            with self.subTest(expected=expected), self.assertRaisesRegex(module.ObserverError, expected):
                module.require_daemon_owner(connection, Gio, GLib, 52958)

    def test_private_outputs_are_new_regular_files_with_mode_0600(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "audit.jsonl"
            descriptor = module.private_output(path)
            os.close(descriptor)
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
            with self.assertRaisesRegex(module.ObserverError, "already exists"):
                module.private_output(path)
            link = Path(directory) / "link"
            link.symlink_to(path)
            with self.assertRaisesRegex(module.ObserverError, "already exists"):
                module.private_output(link)

    def test_mime_requires_bounded_text_without_nul(self):
        self.assertTrue(module.valid_mime("x-kde-passwordManagerHint"))
        self.assertFalse(module.valid_mime("text/plain\0secret"))
        self.assertFalse(module.valid_mime("x" * 256))

    def test_malformed_authenticated_read_fails_and_never_writes_a_record(self):
        with tempfile.TemporaryDirectory() as directory:
            audit_path = Path(directory) / "audit.jsonl"
            audit = module.AuditFile(audit_path)
            observer = module.ClipboardReadObserver(ObserverConnection(), Gio, GLib, 52958, ":1.45", audit)
            observer.monitoring = True
            malformed = Message(
                Gio.DBusMessageType.METHOD_CALL,
                path=module.CLIPBOARD_PATH,
                interface=module.CLIPBOARD_INTERFACE,
                member=module.READ_MEMBER,
                sender=":1.45",
                body=Body("(ts)", (4, "text/plain")),
            )
            observer._message(None, malformed, True)
            audit.close()
            self.assertIsNotNone(observer.failure)
            self.assertEqual(audit_path.read_text(encoding="utf-8"), "")

    def test_audit_is_redacted_to_mime_and_authenticated_pid(self):
        with tempfile.TemporaryDirectory() as directory:
            audit_path = Path(directory) / "audit.jsonl"
            audit = module.AuditFile(audit_path)
            observer = module.ClipboardReadObserver(ObserverConnection(), Gio, GLib, 52958, ":1.45", audit)
            observer.monitoring = True
            read = Message(
                Gio.DBusMessageType.METHOD_CALL,
                path=module.CLIPBOARD_PATH,
                interface=module.CLIPBOARD_INTERFACE,
                member=module.READ_MEMBER,
                sender=":1.45",
                body=Body("(tsu)", (1, "x-kde-passwordManagerHint", 64)),
            )
            observer._message(None, read, True)
            audit.close()
            line = audit_path.read_text(encoding="utf-8")
            self.assertEqual(json.loads(line), {
                "target": "x-kde-passwordManagerHint", "requestor_pid": 52958, "error": None,
            })
            self.assertNotIn("64", line)
            self.assertNotIn("payload", line)

    def test_owner_replacement_and_connection_close_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            audit = module.AuditFile(Path(directory) / "audit.jsonl")
            observer = module.ClipboardReadObserver(ObserverConnection(), Gio, GLib, 52958, ":1.45", audit)
            observer.monitoring = True
            replacement = Message(
                Gio.DBusMessageType.SIGNAL,
                path=module.BUS_PATH,
                interface=module.BUS_INTERFACE,
                member="NameOwnerChanged",
                sender=module.BUS_NAME,
                body=Body("(sss)", (module.DAEMON_NAME, ":1.45", ":1.46")),
            )
            observer._message(None, replacement, True)
            self.assertIn("lost or replaced", str(observer.failure))
            observer.failure = None
            observer._closed()
            self.assertIn("connection closed", str(observer.failure))
            audit.close()

    def test_monitor_rules_are_installed_before_monitor_ack(self):
        connection = ObserverConnection()
        with tempfile.TemporaryDirectory() as directory:
            audit = module.AuditFile(Path(directory) / "audit.jsonl")
            observer = module.ClipboardReadObserver(connection, Gio, GLib, 52958, ":1.45", audit)
            observer.start()
            audit.close()
        self.assertIsNotNone(connection.filter)
        self.assertEqual(connection.connected[0][0], "closed")
        call = connection.calls[0]
        self.assertEqual(call[0:4], (module.BUS_NAME, module.BUS_PATH, module.MONITOR_INTERFACE, "BecomeMonitor"))
        self.assertEqual(call[4][0], "(asu)")
        self.assertIn("member='Read'", call[4][1][0][0])
        self.assertIn("NameOwnerChanged", call[4][1][0][1])

    @unittest.skipUnless(HAVE_PRIVATE_DBUS, "PyGObject is required for private D-Bus smoke coverage")
    def test_private_bus_monitor_audits_authenticated_read_and_stops_cleanly(self):
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(
                ["dbus-run-session", "--", sys.executable, "-c", """
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
from gi.repository import Gio, GLib

root = Path(sys.argv[1])
observer_script = root / "scripts/release/linux-gnome-clipboard-read-observer.py"
workspace = Path(sys.argv[2])
ready = workspace / "ready.json"
audit = workspace / "audit.jsonl"
trigger = workspace / "trigger"
daemon_code = r'''\
import os
import sys
import time
from pathlib import Path
from gi.repository import Gio, GLib
trigger = Path(sys.argv[1])
connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
connection.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName", GLib.Variant("(su)", ("app.copypaste.Daemon", 0)), GLib.VariantType.new("(u)"), Gio.DBusCallFlags.NONE, 2000, None)
print("ready", flush=True)
while not trigger.exists():
    time.sleep(0.01)
message = Gio.DBusMessage.new_method_call("app.copypaste.Daemon", "/app/copypaste/Clipboard", "app.copypaste.Clipboard", "Read")
message.set_body(GLib.Variant("(tsu)", (1, "x-kde-passwordManagerHint", 64)))
connection.send_message(message, Gio.DBusSendMessageFlags.NONE)
time.sleep(5)
'''
daemon = subprocess.Popen([sys.executable, "-c", daemon_code, str(trigger)], stdout=subprocess.PIPE, text=True)
try:
    if daemon.stdout.readline().strip() != "ready":
        raise RuntimeError("fixture daemon did not claim its D-Bus name")
    observer = None
    observer = subprocess.Popen([sys.executable, str(observer_script), "--daemon-pid", str(daemon.pid), "--ready-file", str(ready), "--audit-file", str(audit)], stderr=subprocess.PIPE, text=True)
    try:
        deadline = time.monotonic() + 5
        while not ready.exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        if not ready.exists():
            raise RuntimeError("observer did not become ready: " + observer.stderr.read())
        trigger.touch()
        deadline = time.monotonic() + 5
        while audit.stat().st_size == 0 and time.monotonic() < deadline:
            time.sleep(0.01)
        os.kill(observer.pid, signal.SIGTERM)
        if observer.wait(timeout=5) != 0:
            raise RuntimeError("observer did not stop cleanly: " + observer.stderr.read())
        readiness = json.loads(ready.read_text())
        if set(readiness) != {"schema", "daemon_pid", "bus_owner"} or readiness["schema"] != 1 or readiness["daemon_pid"] != daemon.pid or not isinstance(readiness["bus_owner"], str) or not readiness["bus_owner"].startswith(":"):
            raise RuntimeError("observer readiness is malformed")
        if json.loads(audit.read_text()) != {"target": "x-kde-passwordManagerHint", "requestor_pid": daemon.pid, "error": None}:
            raise RuntimeError("observer audit is malformed")
    finally:
        if observer is not None and observer.poll() is None:
            observer.terminate()
            observer.wait(timeout=5)
finally:
    if daemon.poll() is None:
        daemon.terminate()
        daemon.wait(timeout=5)
""", str(ROOT), directory],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                timeout=20,
            )
            self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
