#!/usr/bin/env python3
"""Unit tests for the redacted Linux desktop acceptance observer."""

import importlib.util
import json
import os
import sys
import unittest
from unittest import mock
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "release" / "linux-desktop-acceptance.py"
SPEC = importlib.util.spec_from_file_location("linux_desktop_acceptance", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class FakeReply:
    def __init__(self, values):
        self.values = values

    def unpack(self):
        return self.values


class FakeConnection:
    owner = ':1.12'

    def __init__(self, gui_pid):
        self.gui_pid = gui_pid
        self.callback = None
        self.subscription = None
        self.calls = []
        self.events = []
        self.before_start_reply = None

    def call_sync(self, destination, _path, _interface, method, parameters, *_args):
        self.calls.append((destination, method, parameters))
        if method == 'StartQualification':
            if self.before_start_reply is not None:
                self.before_start_reply()
            return FakeReply(('transaction-1',))
        argument, = parameters.unpack()
        if method == 'GetNameOwner':
            self.assert_host(argument)
            return FakeReply((self.owner,))
        if method == 'GetConnectionUnixProcessID':
            if argument != self.owner:
                raise AssertionError(f'unexpected credential owner {argument}')
            return FakeReply((self.gui_pid,))
        if method == 'GetConnectionUnixUser':
            if argument != self.owner:
                raise AssertionError(f'unexpected credential owner {argument}')
            return FakeReply((os.getuid(),))
        raise AssertionError(f'unexpected D-Bus call {method}')

    def assert_host(self, argument):
        if argument != MODULE.HOST_BUS_NAME:
            raise AssertionError(f'unexpected D-Bus host {argument}')

    def signal_subscribe(self, sender, interface, signal, path, _arg0, _flags, callback):
        self.subscription = (sender, interface, signal, path)
        self.callback = callback
        return 9

    def signal_unsubscribe(self, subscription):
        self.events.append(('unsubscribe', subscription))

    def emit(self, sender, values):
        assert self.callback is not None
        self.events.append(('signal', sender, values))
        self.callback(
            self, sender, MODULE.HOST_OBJECT_PATH, MODULE.HOST_INTERFACE,
            'QualificationObservation', FakeReply(values),
        )


class FakeLoop:
    def __init__(self, glib):
        self.glib = glib
        self.stopped = False

    def run(self):
        self.glib.callbacks.pop(0)()

    def quit(self):
        self.stopped = True


class FakeGLib:
    class Variant:
        def __init__(self, _signature, values):
            self.values = values

        def unpack(self):
            return self.values

    class VariantType:
        @staticmethod
        def new(value):
            return value

    def __init__(self):
        self.callbacks = []
        self.removed = []

    def MainLoop(self):
        return FakeLoop(self)

    def timeout_add(self, _milliseconds, callback):
        self.timeout = callback
        return 41

    def source_remove(self, identifier):
        self.removed.append(identifier)


class FakeGio:
    class DBusSignalFlags:
        NONE = 0

    class DBusCallFlags:
        NONE = 0


class LinuxDesktopAcceptanceTest(unittest.TestCase):
    gui_pid = 4321

    def wayland_observer(self):
        connection = FakeConnection(self.gui_pid)
        glib = FakeGLib()
        owner = MODULE.require_wayland_gui_owner(connection, FakeGio, glib, self.gui_pid)
        observer = MODULE.WaylandQualificationObserver(connection, FakeGio, glib, owner, self.gui_pid)
        observer.subscribe()
        observer.start('close-main', 'a' * 64)
        return connection, glib, observer

    def test_finds_exact_gui_owned_status_notifier_item(self):
        service = MODULE.sni_service_for_pid(
            [
                "org.kde.StatusNotifierItem-4321-1",
                "org.kde.StatusNotifierItem-7654-1",
                "org.freedesktop.Notifications",
            ],
            4321,
        )
        self.assertEqual(service, "org.kde.StatusNotifierItem-4321-1")
        with self.assertRaisesRegex(MODULE.AcceptanceError, "exactly one"):
            MODULE.sni_service_for_pid(
                ["org.kde.StatusNotifierItem-4321-1", "org.kde.StatusNotifierItem-4321-2"], 4321
            )

    def test_finds_enabled_open_menu_item_in_busctl_layout(self):
        layout = [
            3,
            [
                0,
                {},
                [
                    [17, {"label": {"type": "s", "data": "Open"}, "enabled": {"type": "b", "data": True}}, []],
                    [18, {"label": {"type": "s", "data": "Quit"}, "enabled": {"type": "b", "data": True}}, []],
                ],
            ],
        ]
        self.assertEqual(MODULE.menu_item_id(layout, "Open"), 17)
        with self.assertRaisesRegex(MODULE.AcceptanceError, "no enabled 'Settings'"):
            MODULE.menu_item_id(layout, "Settings")

    def test_notification_transcript_keeps_only_server_reply_metadata(self):
        transcript = MODULE.NotificationTranscript(server_owner=':1.9')
        transcript.consume(
            "method call time=1.0 sender=:1.5 -> destination=org.freedesktop.Notifications serial=71 "
            "path=/org/freedesktop/Notifications; interface=org.freedesktop.Notifications; member=Notify"
        )
        transcript.consume('   string "CopyPaste"')
        transcript.consume('   string "sensitive clipboard body must never be retained"')
        transcript.consume(
            "method return time=1.1 sender=:1.9 -> destination=:1.5 serial=72 reply_serial=71"
        )
        transcript.consume("   uint32 24")
        self.assertEqual(transcript.notification_id, 24)
        evidence = json.dumps({"server_reply_id": transcript.notification_id})
        self.assertNotIn("sensitive clipboard body", evidence)

    def test_notification_body_uint32_cannot_be_a_server_reply(self):
        transcript = MODULE.NotificationTranscript(server_owner=':1.9')
        transcript.consume(
            "method call sender=:1.5 -> destination=org.freedesktop.Notifications serial=71 "
            "path=/org/freedesktop/Notifications; interface=org.freedesktop.Notifications; member=Notify"
        )
        transcript.consume(' string "CopyPaste"')
        transcript.consume(' uint32 999')
        self.assertIsNone(transcript.notification_id)
        transcript.consume('method return sender=:1.8 -> destination=:1.5 serial=72 reply_serial=71')
        transcript.consume(' uint32 24')
        self.assertIsNone(transcript.notification_id)
        transcript.consume('method return sender=:1.9 -> destination=:1.5 serial=72 reply_serial=71')
        transcript.consume(' uint32 24')
        self.assertEqual(transcript.notification_id, 24)

    def test_capture_command_requires_real_argv(self):
        provider = str(MODULE.CLIPBOARD_PROVIDER)
        command = ["python3", provider, "--helper", "/tmp/helper", "--manifest", "/tmp/manifest", "--ready-file", "/tmp/ready", "--activity-log", "/tmp/activity"]
        self.assertEqual(MODULE.parse_capture_command(json.dumps(command)), command)
        with self.assertRaisesRegex(MODULE.AcceptanceError, "JSON argv"):
            MODULE.parse_capture_command('sh -c true')
        with self.assertRaisesRegex(MODULE.AcceptanceError, "GTK clipboard provider"):
            MODULE.parse_capture_command('["true"]')

    def test_history_count_requires_a_successful_typed_cli_status(self):
        response = MODULE.CommandResult(
            ('copypaste', '--json', 'status'), 0,
            json.dumps({'ok': True, 'data': {'status': {'item_count': 4}}}), '',
        )
        with mock.patch.object(MODULE, 'run', return_value=response):
            self.assertEqual(MODULE.history_item_count(Path('/tmp/copypaste'), {}), 4)
        malformed = MODULE.CommandResult(
            ('copypaste', '--json', 'status'), 0,
            json.dumps({'ok': True, 'data': {'status': {'item_count': True}}}), '',
        )
        with mock.patch.object(MODULE, 'run', return_value=malformed):
            with self.assertRaisesRegex(MODULE.AcceptanceError, 'typed history count'):
                MODULE.history_item_count(Path('/tmp/copypaste'), {})

    def test_wayland_capability_requires_256_bits(self):
        prior = os.environ.get('COPYPASTE_QUALIFICATION_CAPABILITY')
        try:
            os.environ['COPYPASTE_QUALIFICATION_CAPABILITY'] = 'not-a-capability'
            with self.assertRaisesRegex(MODULE.AcceptanceError, 'capability'):
                MODULE.start_wayland_qualification('close-main', self.gui_pid)
        finally:
            if prior is None:
                os.environ.pop('COPYPASTE_QUALIFICATION_CAPABILITY', None)
            else:
                os.environ['COPYPASTE_QUALIFICATION_CAPABILITY'] = prior

    def test_wayland_observer_requires_ordered_authenticated_signal_callbacks(self):
        connection, glib, observer = self.wayland_observer()
        self.assertEqual(
            connection.subscription,
            (connection.owner, MODULE.HOST_INTERFACE, 'QualificationObservation', MODULE.HOST_OBJECT_PATH),
        )
        glib.callbacks.extend([
            lambda: connection.emit(connection.owner, ('transaction-1', self.gui_pid, 'main', False)),
            lambda: connection.emit(connection.owner, ('transaction-1', self.gui_pid, 'main', True)),
        ])

        self.assertEqual(observer.await_mapped(False), ('transaction-1', self.gui_pid, 'main', False))
        with mock.patch.object(
            MODULE, 'run',
            return_value=MODULE.CommandResult(('busctl',), 0, '', ''),
        ) as run:
            command = MODULE.invoke_menu_item({}, 'org.kde.StatusNotifierItem-4321-1', 17)
        self.assertEqual(command.returncode, 0)
        self.assertIn('Event', run.call_args.args[0])
        self.assertEqual(observer.await_mapped(True), ('transaction-1', self.gui_pid, 'main', True))
        self.assertEqual([event[0] for event in connection.events], ['signal', 'signal'])
        self.assertGreaterEqual(sum(method == 'GetNameOwner' for _, method, _ in connection.calls), 3)

    def test_wayland_observer_preserves_a_signal_arriving_during_start(self):
        connection = FakeConnection(self.gui_pid)
        glib = FakeGLib()
        owner = MODULE.require_wayland_gui_owner(connection, FakeGio, glib, self.gui_pid)
        observer = MODULE.WaylandQualificationObserver(connection, FakeGio, glib, owner, self.gui_pid)
        observer.subscribe()
        connection.before_start_reply = lambda: connection.emit(
            owner, ('transaction-1', self.gui_pid, 'main', False)
        )
        observer.start('close-main', 'a' * 64)
        self.assertEqual(observer.await_mapped(False), ('transaction-1', self.gui_pid, 'main', False))

    def test_wayland_observer_rejects_foreign_sender(self):
        connection, glib, observer = self.wayland_observer()
        glib.callbacks.append(
            lambda: connection.emit(':1.99', ('transaction-1', self.gui_pid, 'main', False))
        )
        with self.assertRaisesRegex(MODULE.AcceptanceError, 'unexpected GUI owner'):
            observer.await_mapped(False)

    def test_wayland_observer_rejects_stale_transaction(self):
        connection, glib, observer = self.wayland_observer()
        glib.callbacks.append(
            lambda: connection.emit(connection.owner, ('transaction-old', self.gui_pid, 'main', False))
        )
        with self.assertRaisesRegex(MODULE.AcceptanceError, 'stale transaction'):
            observer.await_mapped(False)

    def test_wayland_observer_rejects_wrong_gui_pid(self):
        connection, glib, observer = self.wayland_observer()
        glib.callbacks.append(
            lambda: connection.emit(connection.owner, ('transaction-1', self.gui_pid + 1, 'main', False))
        )
        with self.assertRaisesRegex(MODULE.AcceptanceError, 'main surface'):
            observer.await_mapped(False)


if __name__ == "__main__":
    unittest.main()
