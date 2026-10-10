#!/usr/bin/env python3
"""Focused contracts for the real Wayland Quick Paste qualification helper."""

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "linux_wayland_quick_paste", ROOT / "scripts/release/linux-wayland-quick-paste.py",
)
module = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = module
SPEC.loader.exec_module(module)


class LinuxWaylandQuickPasteTest(unittest.TestCase):
    def test_portal_request_path_is_bound_to_owner_and_token(self):
        self.assertEqual(
            module.portal_request_path(':1.42', 'copypaste_create_7'),
            '/org/freedesktop/portal/desktop/request/1_42/copypaste_create_7',
        )
        with self.assertRaisesRegex(module.QuickPasteQualificationError, 'identity'):
            module.portal_request_path(':1.42', 'not/safe')

    def test_marker_rejects_control_data_and_bounds_length(self):
        self.assertEqual(module.marker_argument("marker"), "marker")
        with self.assertRaisesRegex(Exception, "printable ASCII"):
            module.marker_argument("bad\nmarker")
        with self.assertRaisesRegex(Exception, "printable ASCII"):
            module.marker_argument("a" * 513)

    def test_target_state_requires_only_redacted_focus_and_marker_fields(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "result"
            target.write_text(json.dumps({"focus_events": 2, "marker_received": True}), encoding="utf-8")
            self.assertEqual(module.read_target_state(target), {"focus_events": 2, "marker_received": True})
            target.write_text(json.dumps({"focus_events": 2, "marker_received": True, "text": "secret"}), encoding="utf-8")
            with self.assertRaisesRegex(module.QuickPasteQualificationError, "invalid observation"):
                module.read_target_state(target)

    def test_target_state_never_accepts_boolean_as_a_focus_count(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "result"
            target.write_text(json.dumps({"focus_events": True, "marker_received": True}), encoding="utf-8")
            with self.assertRaisesRegex(module.QuickPasteQualificationError, "invalid observation"):
                module.read_target_state(target)

    def test_target_source_and_actor_are_regular_files(self):
        self.assertEqual(module.require_regular(module.TARGET_SOURCE, "target"), module.TARGET_SOURCE)
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "file"
            source.write_text("x", encoding="utf-8")
            link = Path(directory) / "link"
            link.symlink_to(source)
            with self.assertRaisesRegex(module.QuickPasteQualificationError, "regular"):
                module.require_regular(link, "link")

    def test_evidence_hashes_do_not_retain_marker_or_target_title(self):
        marker = "copypaste-wayland-quick-paste-fixture"
        record = {
            "marker_sha256": module.sha256(marker),
            "title_sha256": module.sha256(module.TARGET_TITLE),
        }
        self.assertNotIn(marker, json.dumps(record))
        self.assertNotIn(module.TARGET_TITLE, json.dumps(record))

    def test_receipt_binds_every_actor_pid_and_rejects_weak_input_proof(self):
        digest = 'a' * 64
        record = module.receipt(
            gui_pid=11, app_portal_dialog_pid=12, portal_dialog_pid=13, input_target_pid=14,
            marker='fixture', portal_input_calls=8, portal_session_digest=digest,
            app_click_digest=digest, app_consent_digest=digest, portal_click_digest=digest,
        )
        self.assertEqual(record['actors'], {
            'gui_pid': 11,
            'app_portal_dialog_pid': 12,
            'portal_dialog_pid': 13,
            'input_target_pid': 14,
        })
        with self.assertRaisesRegex(module.QuickPasteQualificationError, 'proof'):
            module.receipt(
                gui_pid=11, app_portal_dialog_pid=12, portal_dialog_pid=13, input_target_pid=14,
                marker='fixture', portal_input_calls=7, portal_session_digest=digest,
                app_click_digest=digest, app_consent_digest=digest, portal_click_digest=digest,
            )

    def test_portal_response_received_during_call_is_not_lost(self):
        class Reply:
            def __init__(self, value):
                self.value = value

            def unpack(self):
                return self.value

        class Loop:
            def run(self):
                pass

            def quit(self):
                pass

        class Glib:
            class Variant:
                def __init__(self, _signature, value):
                    self.value = value

            class VariantType:
                @staticmethod
                def new(value):
                    return value

            def MainLoop(self):
                return Loop()

            def timeout_add(self, _milliseconds, _callback):
                return 1

            def source_remove(self, _identifier):
                pass

        class Gio:
            class DBusCallFlags:
                NONE = 0

            class DBusSignalFlags:
                NONE = 0

        class Connection:
            def __init__(self):
                self.callback = None
                self.unsubscribed = []

            def get_unique_name(self):
                return ':1.42'

            def signal_subscribe(self, _sender, _interface, _signal, path, _arg0, _flags, callback):
                self.path = path
                self.callback = callback
                return 12

            def signal_unsubscribe(self, identifier):
                self.unsubscribed.append(identifier)

            def call_sync(self, _destination, _path, _interface, method, _parameters, *_rest):
                self.callback(self, ':1.9', self.path, module.REQUEST_INTERFACE, 'Response', Reply((0, {'session_handle': '/session'})))
                self.callback(self, ':1.9', self.path, module.REQUEST_INTERFACE, 'Response', Reply((0, {'session_handle': '/different'})))
                return Reply((self.path,))

        portal = object.__new__(module.PortalKeyboard)
        portal.gio = Gio
        portal.glib = Glib()
        portal.connection = Connection()
        portal.owner = ':1.9'
        portal.calls = []
        outcome = portal._request('CreateSession', '(a{sv})', ({'handle_token': 'copypaste_create_7'},))
        self.assertEqual(outcome, {'session_handle': '/session'})
        self.assertEqual(portal.connection.unsubscribed, [12])

    def test_portal_call_error_unsubscribes_the_preinstalled_watch(self):
        class Glib:
            class Variant:
                def __init__(self, _signature, _value):
                    pass

            class VariantType:
                @staticmethod
                def new(value):
                    return value

        class Gio:
            class DBusCallFlags:
                NONE = 0

            class DBusSignalFlags:
                NONE = 0

        class Connection:
            def get_unique_name(self):
                return ':1.42'

            def signal_subscribe(self, *_args):
                return 25

            def signal_unsubscribe(self, identifier):
                self.unsubscribed.append(identifier)

            def call_sync(self, *_args):
                raise RuntimeError('D-Bus call failed')

            unsubscribed = []

        portal = object.__new__(module.PortalKeyboard)
        portal.gio = Gio
        portal.glib = Glib()
        portal.connection = Connection()
        portal.owner = ':1.9'
        portal.calls = []
        with self.assertRaisesRegex(RuntimeError, 'D-Bus call failed'):
            portal._request('CreateSession', '(a{sv})', ({'handle_token': 'copypaste_create_7'},))
        self.assertEqual(portal.connection.unsubscribed, [25])

    def test_malformed_portal_reply_unsubscribes_the_preinstalled_watch(self):
        class Reply:
            def unpack(self):
                return ('one', 'too-many')

        class Glib:
            class Variant:
                def __init__(self, _signature, _value):
                    pass

            class VariantType:
                @staticmethod
                def new(value):
                    return value

        class Gio:
            class DBusCallFlags:
                NONE = 0

            class DBusSignalFlags:
                NONE = 0

        class Connection:
            def __init__(self):
                self.unsubscribed = []

            def get_unique_name(self):
                return ':1.42'

            def signal_subscribe(self, *_args):
                return 26

            def signal_unsubscribe(self, identifier):
                self.unsubscribed.append(identifier)

            def call_sync(self, *_args):
                return Reply()

        portal = object.__new__(module.PortalKeyboard)
        portal.gio = Gio
        portal.glib = Glib()
        portal.connection = Connection()
        portal.owner = ':1.9'
        portal.calls = []
        with self.assertRaises(ValueError):
            portal._request('CreateSession', '(a{sv})', ({'handle_token': 'copypaste_create_7'},))
        self.assertEqual(portal.connection.unsubscribed, [26])


if __name__ == "__main__":
    unittest.main()
