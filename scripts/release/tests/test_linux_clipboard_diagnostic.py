#!/usr/bin/env python3
"""Focused contracts for the redacted first-capture diagnostic helpers."""

import importlib.util
import json
import sys
import tempfile
import types
import unittest
from contextlib import contextmanager
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "linux_first_clipboard_diagnostic", ROOT / "scripts/release/linux-first-clipboard-diagnostic.py",
)
diagnostic = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(diagnostic)


class LinuxClipboardDiagnosticTest(unittest.TestCase):
    def test_observer_subscribes_to_the_authenticated_unique_owner(self):
        filters = []

        class Reply:
            @staticmethod
            def unpack():
                return (":1.77",)

        class Connection:
            def call_sync(self, *args):
                if args[3] == "GetNameOwner":
                    return Reply()
                self.rules = args[4][1][0]
                return None

            def add_filter(self, callback):
                filters.append(callback)

        connection = Connection()

        class Gio:
            class BusType:
                SESSION = "session"

            class DBusCallFlags:
                NONE = "none"

            class DBusMessageType:
                METHOD_CALL = "call"
                METHOD_RETURN = "return"
                ERROR = "error"
                SIGNAL = "signal"

            class DBusCapabilityFlags:
                NONE = "none"

            @staticmethod
            def bus_get_sync(_bus_type, _cancellable):
                return connection

        class GLib:
            @staticmethod
            def Variant(signature, values):
                return (signature, values)

            @staticmethod
            def VariantType(signature):
                return signature

            @staticmethod
            def timeout_add_seconds(_seconds, callback):
                return callback

            @staticmethod
            def MainLoop():
                return types.SimpleNamespace(run=lambda: None, quit=lambda: None)

        gi = types.ModuleType("gi")
        gi.require_version = lambda *_args: None
        repository = types.ModuleType("gi.repository")
        repository.Gio = Gio
        repository.GLib = GLib
        observer_spec = importlib.util.spec_from_file_location(
            "linux_kwin_clipboard_observer", ROOT / "scripts/release/linux-kwin-clipboard-observer.py",
        )
        observer = importlib.util.module_from_spec(observer_spec)
        assert observer_spec.loader is not None
        with mock.patch.dict(sys.modules, {"gi": gi, "gi.repository": repository}), \
             mock.patch.object(sys, "argv", ["observer", "--seconds", "1"]), \
             mock.patch("sys.stdout"):
            observer_spec.loader.exec_module(observer)
            observer.main()

        self.assertTrue(filters)
        self.assertTrue(any("destination='org.kde.KWin'" in rule for rule in connection.rules))
        self.assertTrue(any("destination=':1.77'" in rule for rule in connection.rules))

    def test_capture_state_records_only_capture_gate_booleans(self):
        with mock.patch.object(diagnostic.fixture, "status", return_value={
            "capture_running": True, "clipboard_backend": "linux-kde", "item_count": 4,
        }), mock.patch.object(diagnostic.fixture, "cli_json", return_value={
            "data": {"config": {"private_mode": False, "excluded_app_bundle_ids": []}},
        }), mock.patch.object(diagnostic.fixture, "response_variant", return_value={
            "config": {"private_mode": False, "excluded_app_bundle_ids": []},
        }), mock.patch.object(diagnostic.fixture, "ipc") as ipc:
            state = diagnostic.capture_state(Path("cli"), {}, Path("socket"))

        self.assertEqual(state, {
            "capture_running": True,
            "clipboard_backend": "linux-kde",
            "private_mode": False,
            "settings_capture_enabled": True,
            "source_allowed_by_exclusion_settings": True,
            "item_count": 4,
        })
        self.assertNotIn(diagnostic.fixture.SOURCE_APPLICATION_ID, json.dumps(state))
        ipc.assert_not_called()

    def test_capture_state_reads_the_effective_config_from_config_applied(self):
        with mock.patch.object(diagnostic.fixture, "status", return_value={
            "capture_running": True, "clipboard_backend": "linux-x11", "item_count": 0,
        }), mock.patch.object(diagnostic.fixture, "cli_json", return_value={}), \
             mock.patch.object(diagnostic.fixture, "response_variant", return_value={
                 "config": {"private_mode": False, "excluded_app_bundle_ids": []}, "restart_required": [],
             }):
            state = diagnostic.capture_state(Path("cli"), {}, Path("socket"))

        self.assertTrue(state["settings_capture_enabled"])
        self.assertTrue(state["source_allowed_by_exclusion_settings"])

    def test_first_capture_records_metadata_after_cli_is_assigned(self):
        class App:
            pid = 41

            def terminate(self):
                pass

            def wait(self, timeout):
                pass

        phase = {"captured": False}

        def status(_cli, _environment):
            return {
                "capture_running": True,
                "clipboard_backend": "linux-kde",
                "item_count": int(phase["captured"]),
            }

        @contextmanager
        def provider(*_args):
            yield

        def wait_for_count(*_args):
            phase["captured"] = True

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prefix = root / "prefix"
            prefix.mkdir()
            output = root / "evidence.json"
            arguments = [
                "diagnostic", "--runtime-prefix", str(prefix), "--gui-launcher", str(root / "gui"),
                "--session", "wayland", "--output", str(output),
            ]
            with mock.patch.object(diagnostic.sys, "argv", arguments), \
                 mock.patch.object(diagnostic.fixture, "runtime_prefix", return_value=prefix), \
                 mock.patch.object(diagnostic, "start_gui_diagnostic", return_value=(App(), root / "socket", {"gui_exit_code": None})), \
                 mock.patch.object(diagnostic, "start_observer", return_value=None), \
                 mock.patch.object(diagnostic.fixture, "status", side_effect=status), \
                 mock.patch.object(diagnostic.fixture, "cli_json", return_value={"data": {"config": {"private_mode": False, "excluded_app_bundle_ids": []}}}), \
                 mock.patch.object(diagnostic.fixture, "response_variant", return_value={"config": {"private_mode": False, "excluded_app_bundle_ids": []}}), \
                 mock.patch.object(diagnostic.fixture, "list_items", side_effect=[[], [{"id": "new", "content_type": "text", "source_app_bundle_id": diagnostic.fixture.SOURCE_APPLICATION_ID, "source_app_name": diagnostic.fixture.SOURCE_APPLICATION_NAME}]]), \
                 mock.patch.object(diagnostic.fixture, "clipboard_provider", side_effect=provider), \
                 mock.patch.object(diagnostic.fixture, "wait_for_count", side_effect=wait_for_count), \
                 mock.patch.object(diagnostic, "daemon_flags", return_value={"daemon_found": True}), \
                 mock.patch.object(diagnostic.shutil, "rmtree"):
                diagnostic.main()

            record = json.loads(output.read_text(encoding="utf-8"))
        self.assertTrue(record["captured"])
        self.assertTrue(record["source_fixture_match"])
        self.assertEqual(record["before_provider"]["item_count"], 0)
        self.assertEqual(record["during_provider"]["item_count"], 0)
        self.assertEqual(record["after_provider"]["item_count"], 1)

    def test_observer_uses_the_current_unique_owner_and_starts_before_gui(self):
        observer = (ROOT / "scripts/release/linux-kwin-clipboard-observer.py").read_text(encoding="utf-8")
        self.assertIn("destinations = {BUS, owner}", observer)
        self.assertIn("message.get_destination() in destinations", observer)
        self.assertIn('member in {"Version", "Snapshot", "Read"}', observer)
        self.assertNotIn("message.get_body()", observer.split('member in {"Version", "Snapshot", "Read"}')[0])
        helper = (ROOT / "scripts/release/linux-first-clipboard-diagnostic.py").read_text(encoding="utf-8")
        self.assertLess(
            helper.index("observer = start_observer(environment, runtime)"),
            helper.index("app, socket_path, gui_startup = start_gui_diagnostic"),
        )
        self.assertIn('record["x11_owner_during_provider"] = x11_state', helper)


if __name__ == "__main__":
    unittest.main()
