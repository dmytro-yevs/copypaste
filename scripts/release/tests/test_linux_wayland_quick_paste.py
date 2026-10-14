#!/usr/bin/env python3
"""Focused contracts for the real Wayland Quick Paste qualification helper."""

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from types import ModuleType, SimpleNamespace
from unittest import mock
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
            gui_pid=11, portal_dialog_pid=13, quick_paste_pid=14, input_target_pid=15,
            marker='fixture', portal_input_calls=8, portal_session_digest=digest,
            portal_click_digest=digest, quick_paste_ready_digest=digest,
        )
        self.assertEqual(record['actors'], {
            'gui_pid': 11,
            'portal_dialog_pid': 13,
            'quick_paste_pid': 14,
            'input_target_pid': 15,
        })
        with self.assertRaisesRegex(module.QuickPasteQualificationError, 'proof'):
            module.receipt(
                gui_pid=11, portal_dialog_pid=13, quick_paste_pid=14, input_target_pid=15,
                marker='fixture', portal_input_calls=7, portal_session_digest=digest,
                portal_click_digest=digest, quick_paste_ready_digest=digest,
            )

    def test_quick_paste_child_excludes_foreign_children_and_rejects_ambiguity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            proc = root / 'proc'
            proc.mkdir()
            executable = root / 'copypaste'
            executable.write_text('binary', encoding='utf-8')
            foreign_executable = root / 'foreign'
            foreign_executable.write_text('binary', encoding='utf-8')

            def process(pid, *, ppid, executable_path, arguments):
                node = proc / str(pid)
                node.mkdir()
                node.joinpath('status').write_text(
                    f'PPid:\t{ppid}\nUid:\t{os.getuid()}\t0\t0\t0\n', encoding='utf-8',
                )
                node.joinpath('exe').symlink_to(executable_path)
                node.joinpath('cmdline').write_bytes(b'\0'.join(arguments) + b'\0')
                return node

            gui = process(100, ppid=1, executable_path=executable, arguments=(b'/app/copypaste',))
            process(101, ppid=100, executable_path=executable, arguments=(b'/app/copypaste', b'--copypaste-quick-paste'))
            process(102, ppid=999, executable_path=executable, arguments=(b'/app/copypaste', b'--copypaste-quick-paste'))
            process(103, ppid=100, executable_path=foreign_executable, arguments=(b'/app/copypaste', b'--copypaste-quick-paste'))
            process(104, ppid=100, executable_path=executable, arguments=(b'/app/copypaste', b'--copypaste-quick-pastes'))
            original = module.PROC_ROOT
            module.PROC_ROOT = proc
            try:
                self.assertEqual(module.status_number({'Uid': '0 0 0 0'}, 'Uid'), 0)
                self.assertIsNone(module.status_number({}, 'Uid'))
                self.assertEqual(module.quick_paste_child(100), 101)
                process(105, ppid=100, executable_path=executable, arguments=(b'/app/copypaste', b'--copypaste-quick-paste'))
                with self.assertRaisesRegex(module.QuickPasteQualificationError, 'ambiguous'):
                    module.quick_paste_child(100)
            finally:
                module.PROC_ROOT = original

    def test_malformed_quick_paste_readiness_evidence_blocks_completion(self):
        malformed = subprocess.CompletedProcess(['python3', 'actor'], 0, stdout='unexpected\n', stderr='')
        with mock.patch.object(module, 'quick_paste_child', return_value=101), \
             mock.patch.object(module.subprocess, 'run', return_value=malformed) as run:
            with self.assertRaisesRegex(module.QuickPasteQualificationError, 'invalid output'):
                module.wait_for_quick_paste_ready(module.MEDIA_ACTOR, 100, 'fixture', {})
        self.assertEqual(run.call_args.args[0][2], 'quick-paste-ready')

    def test_quick_paste_readiness_rejects_boolean_schema_and_keeps_actor_failure_details(self):
        marker = 'fixture'
        output = 'COPYPASTE_QUALIFICATION_MEDIA ' + json.dumps({
            'schema': True,
            'kind': 'atspi_quick_paste_ready',
            'gui_pid': 101,
            'marker_sha256': module.sha256(marker),
        }) + '\n'
        with self.assertRaisesRegex(module.QuickPasteQualificationError, 'evidence'):
            module.quick_paste_ready_record(output, pid=101, marker=marker)
        failed = subprocess.CompletedProcess(['python3', 'actor'], 7, stdout='', stderr='causal actor failure')
        with mock.patch.object(module, 'quick_paste_child', return_value=101), \
             mock.patch.object(module.subprocess, 'run', return_value=failed):
            with self.assertRaisesRegex(module.QuickPasteQualificationError, 'exit 7: causal actor failure'):
                module.wait_for_quick_paste_ready(module.MEDIA_ACTOR, 100, marker, {})

    def test_quick_paste_readiness_rejects_an_exhausted_deadline(self):
        with mock.patch.object(module, 'quick_paste_child', return_value=None), \
             mock.patch.object(module.time, 'monotonic', side_effect=[0, 8]):
            with self.assertRaisesRegex(module.QuickPasteQualificationError, 'deadline'):
                module.wait_for_quick_paste_ready(module.MEDIA_ACTOR, 100, 'fixture', {})

    def test_readiness_failure_prevents_return_input(self):
        class Portal:
            def __init__(self, *_args):
                self.calls = []
                self.session_digest = 'a' * 64
                self.chords = []

            def require_gui_owner(self, _pid):
                return ':1.2'

            def grant(self, callback):
                callback()

            def chord(self, *keycodes):
                self.chords.append(keycodes)

            def close(self):
                pass

        portal = Portal()
        target = SimpleNamespace(pid=103, close=mock.Mock())
        repository = ModuleType('gi.repository')
        repository.Gio = object()
        repository.GLib = object()
        gi = ModuleType('gi')
        gi.repository = repository
        args = SimpleNamespace(gui_pid=100, workspace=Path('/workspace'), marker='fixture', actor=module.MEDIA_ACTOR)
        with mock.patch.dict(sys.modules, {'gi': gi, 'gi.repository': repository}), \
             mock.patch.dict(os.environ, {'WAYLAND_DISPLAY': 'wayland-0'}), \
             mock.patch.object(module, 'build_target', return_value=Path('/target')), \
             mock.patch.object(module, 'PortalKeyboard', return_value=portal), \
             mock.patch.object(module, 'start_target', return_value=target), \
             mock.patch.object(module, 'click_owned_portal_consent', return_value=(101, 'b' * 64)), \
             mock.patch.object(module, 'wait_for_quick_paste_ready', side_effect=module.QuickPasteQualificationError('malformed readiness')):
            with self.assertRaisesRegex(module.QuickPasteQualificationError, 'malformed readiness'):
                module.qualify(args)
        self.assertEqual(portal.chords, [(module.EVDEV_CONTROL, module.EVDEV_SHIFT, module.EVDEV_C)])
        target.close.assert_called_once()

    def test_gnome_portal_consent_checks_remote_interaction_before_sharing(self):
        class State:
            def __init__(self, *values):
                self.values = set(values)

            def contains(self, value):
                return value in self.values

        class Action:
            def __init__(self, names):
                self.names = names
                self.nActions = len(names)

            def getName(self, index):
                return self.names[index]

            def doAction(self, index):
                self.node.clicked.append(self.names[index])
                self.node.events.append(f'{self.node.name}:{self.names[index]}')
                if self.node.name == 'Allow Remote Interaction':
                    self.node._state.values.add('checked')
                return True

        class Node:
            def __init__(self, name='', *states, children=(), actions=('click',), events=None):
                self.name = name
                self._state = State(*states)
                self._children = list(children)
                self.childCount = len(self._children)
                self.actions = actions
                self.clicked = []
                self.events = events if events is not None else []

            def getState(self):
                return self._state

            def getChildAtIndex(self, index):
                return self._children[index]

            def queryAction(self):
                action = Action(self.actions)
                action.node = self
                return action

        atspi = SimpleNamespace(STATE_CHECKABLE='checkable', STATE_CHECKED='checked', STATE_SHOWING='showing', STATE_VISIBLE='visible', STATE_SENSITIVE='sensitive')
        events = []
        interaction = Node('Allow Remote Interaction', 'checkable', 'showing', 'visible', 'sensitive', actions=('toggle',), events=events)
        share = Node('Share', 'showing', 'visible', 'sensitive', events=events)
        application = Node(children=(interaction, share))
        self.assertTrue(module.gnome_portal_consent(application, atspi))
        self.assertEqual(interaction.clicked, ['toggle'])
        self.assertEqual(share.clicked, ['click'])
        self.assertEqual(events, ['Allow Remote Interaction:toggle', 'Share:click'])

    def test_portal_control_rejects_insensitive_ambiguous_or_wrong_action_controls(self):
        class State:
            def __init__(self, *values):
                self.values = set(values)

            def contains(self, value):
                return value in self.values

        class Action:
            def __init__(self, names):
                self.names = names
                self.nActions = len(names)

            def getName(self, index):
                return self.names[index]

        class Node:
            def __init__(self, name='', *states, children=(), actions=('click',)):
                self.name = name
                self.state = State(*states)
                self.children = list(children)
                self.childCount = len(self.children)
                self.actions = actions

            def getState(self):
                return self.state

            def getChildAtIndex(self, index):
                return self.children[index]

            def queryAction(self):
                return Action(self.actions)

        atspi = SimpleNamespace(STATE_SHOWING='showing', STATE_VISIBLE='visible', STATE_SENSITIVE='sensitive')
        insensitive = Node('Share', 'showing', 'visible', 'enabled')
        self.assertIsNone(module.portal_control(insensitive, atspi, 'Share', {'click'}))
        first = Node('Share', 'showing', 'visible', 'sensitive')
        second = Node('Share', 'showing', 'visible', 'sensitive')
        self.assertIsNone(module.portal_control(Node(children=(first, second)), atspi, 'Share', {'click'}))
        self.assertIsNone(module.portal_control(Node('Share', 'showing', 'visible', 'sensitive', actions=('toggle',)), atspi, 'Share', {'click'}))

    def test_kde_portal_consent_uses_the_unique_qt_share_press_button(self):
        class State:
            def contains(self, _value):
                return True

        class Action:
            def __init__(self, node):
                self.node = node
                self.nActions = 1

            def getName(self, _index):
                return self.node.action

            def doAction(self, _index):
                self.node.clicked = True
                return True

        class Node:
            def __init__(self, name='', action='Press', children=()):
                self.name = name
                self.action = action
                self.children = list(children)
                self.childCount = len(self.children)
                self.clicked = False

            def getState(self):
                return State()

            def getChildAtIndex(self, index):
                return self.children[index]

            def queryAction(self):
                return Action(self)

        atspi = SimpleNamespace(STATE_SHOWING='showing', STATE_VISIBLE='visible', STATE_SENSITIVE='sensitive')
        share = Node('Share', 'pReSs')
        self.assertTrue(module.kde_portal_consent(share, atspi))
        self.assertTrue(share.clicked)
        self.assertFalse(module.kde_portal_consent(Node('Allow', 'Press'), atspi))
        self.assertFalse(module.kde_portal_consent(Node(children=(Node('Share'), Node('Share'))), atspi))

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
                def __init__(self, signature, value):
                    self.signature = signature
                    self.value = value

                def get_type_string(self):
                    return self.signature

                def unpack(self):
                    return self.value

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
        outcome = portal._request('CreateSession', '(a{sv})', ({'handle_token': Glib.Variant('s', 'copypaste_create_7')},))
        self.assertEqual(outcome, {'session_handle': '/session'})
        self.assertEqual(portal.connection.unsubscribed, [12])

    def test_portal_call_error_unsubscribes_the_preinstalled_watch(self):
        class Glib:
            class Variant:
                def __init__(self, signature, value):
                    self.signature = signature
                    self.value = value

                def get_type_string(self):
                    return self.signature

                def unpack(self):
                    return self.value

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
            portal._request('CreateSession', '(a{sv})', ({'handle_token': Glib.Variant('s', 'copypaste_create_7')},))
        self.assertEqual(portal.connection.unsubscribed, [25])

    def test_malformed_portal_reply_unsubscribes_the_preinstalled_watch(self):
        class Reply:
            def unpack(self):
                return ('one', 'too-many')

        class Glib:
            class Variant:
                def __init__(self, signature, value):
                    self.signature = signature
                    self.value = value

                def get_type_string(self):
                    return self.signature

                def unpack(self):
                    return self.value

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
            portal._request('CreateSession', '(a{sv})', ({'handle_token': Glib.Variant('s', 'copypaste_create_7')},))
        self.assertEqual(portal.connection.unsubscribed, [26])

    def test_portal_request_rejects_an_untyped_handle_token_before_subscribing(self):
        class Glib:
            class Variant:
                def __init__(self, signature, value):
                    self.signature = signature
                    self.value = value

                def get_type_string(self):
                    return self.signature

                def unpack(self):
                    return self.value

        class Connection:
            def signal_subscribe(self, *_args):
                raise AssertionError('untyped handle token must not install a response watch')

        portal = object.__new__(module.PortalKeyboard)
        portal.glib = Glib()
        portal.connection = Connection()
        with self.assertRaisesRegex(module.QuickPasteQualificationError, 'safe handle token'):
            portal._request('CreateSession', '(a{sv})', ({'handle_token': 'copypaste_create_7'},))

    def test_grant_uses_typed_variants_for_all_portal_options(self):
        class Variant:
            def __init__(self, signature, value):
                self.signature = signature
                self.value = value

            def get_type_string(self):
                return self.signature

            def unpack(self):
                return self.value

        portal = object.__new__(module.PortalKeyboard)
        portal.glib = SimpleNamespace(Variant=Variant)
        portal.session = ''
        portal.session_digest = ''
        calls = []

        def request(method, signature, values, on_requested=None):
            calls.append((method, signature, values, on_requested))
            if method == 'CreateSession':
                return {'session_handle': '/org/freedesktop/portal/desktop/session/test'}
            if method == 'SelectDevices':
                return {}
            self.assertEqual(method, 'Start')
            return {'devices': module.KEYBOARD_DEVICE}

        portal._request = request
        callback = lambda: None
        portal.grant(callback)

        self.assertEqual([call[0] for call in calls], ['CreateSession', 'SelectDevices', 'Start'])
        create_options = calls[0][2][0]
        select_options = calls[1][2][1]
        start_options = calls[2][2][2]
        self.assertEqual(create_options['handle_token'].get_type_string(), 's')
        self.assertEqual(create_options['session_handle_token'].get_type_string(), 's')
        self.assertEqual(select_options['handle_token'].get_type_string(), 's')
        self.assertEqual(select_options['types'].get_type_string(), 'u')
        self.assertEqual(select_options['types'].unpack(), module.KEYBOARD_DEVICE)
        self.assertEqual(start_options['handle_token'].get_type_string(), 's')
        self.assertIs(calls[2][3], callback)

    def test_close_targets_the_session_object_without_a_body_and_clears_it(self):
        class Glib:
            class VariantType:
                @staticmethod
                def new(value):
                    return value

        class Gio:
            class DBusCallFlags:
                NONE = 0

        class Connection:
            def __init__(self):
                self.calls = []

            def call_sync(self, *args):
                self.calls.append(args)
                return object()

        portal = object.__new__(module.PortalKeyboard)
        portal.glib = Glib()
        portal.gio = Gio
        portal.connection = Connection()
        portal.calls = []
        portal.session = '/org/freedesktop/portal/desktop/session/test'
        portal.close()

        self.assertEqual(portal.connection.calls[0][0:5], (
            module.PORTAL_NAME, '/org/freedesktop/portal/desktop/session/test', module.SESSION_INTERFACE, 'Close', None,
        ))
        self.assertEqual(portal.session, '')


if __name__ == "__main__":
    unittest.main()
