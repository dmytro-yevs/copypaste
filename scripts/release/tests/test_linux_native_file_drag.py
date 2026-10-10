import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "linux_native_file_drag", ROOT / "scripts/release/linux-native-file-drag.py",
)
drag = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = drag
SPEC.loader.exec_module(drag)

DRIVER_SPEC = importlib.util.spec_from_file_location(
    "linux_native_fixture_driver", ROOT / "scripts/release/linux-native-fixture-driver.py",
)
driver = importlib.util.module_from_spec(DRIVER_SPEC)
assert DRIVER_SPEC.loader is not None
sys.modules[DRIVER_SPEC.name] = driver
DRIVER_SPEC.loader.exec_module(driver)


class LinuxNativeFileDragTest(unittest.TestCase):
    def test_bounds_choose_a_main_content_target_point(self):
        self.assertEqual(drag.Bounds(10, 20, 100, 80).point(horizontal=0.72), (82, 60))

    def test_wayland_drag_requires_one_nonnegative_visible_monitor(self):
        class Monitor:
            def get_geometry(self):
                return drag.Bounds(0, 0, 1280, 720)

        class Display:
            def get_n_monitors(self):
                return 1

            def get_monitor(self, _index):
                return Monitor()

        source = drag.Bounds(100, 150, 320, 96)
        target = drag.Bounds(600, 120, 480, 600)
        self.assertEqual(drag.require_single_wayland_layout(Display(), source, target), drag.Bounds(0, 0, 1280, 720))

        class MultiDisplay(Display):
            def get_n_monitors(self):
                return 2

        with self.assertRaisesRegex(drag.FileDragQualificationError, "one visible monitor"):
            drag.require_single_wayland_layout(MultiDisplay(), source, target)

    def test_wayland_display_must_open_the_requested_socket(self):
        class Display:
            def get_name(self):
                return "wayland-0"

        class Gdk:
            class Display:
                @staticmethod
                def open(name):
                    return Display() if name == "wayland-0" else None

        self.assertIsInstance(drag.open_wayland_display(Gdk, "wayland-0"), Display)
        with self.assertRaisesRegex(drag.FileDragQualificationError, "requested Wayland display"):
            drag.open_wayland_display(Gdk, "wayland-1")

    def test_source_activity_rejects_unknown_or_malformed_events(self):
        with tempfile.TemporaryDirectory() as directory:
            activity = Path(directory) / "activity.jsonl"
            activity.write_text(
                '{"event":"unknown","timestamp_us":1,"actions":0,"selected_action":0,"destination_window":false}\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(drag.FileDragQualificationError, "unknown activity"):
                drag.source_activity(activity)
            activity.write_text("not json\n", encoding="utf-8")
            with self.assertRaisesRegex(drag.FileDragQualificationError, "activity is invalid"):
                drag.source_activity(activity)

    def test_source_activity_requires_copy_negotiation_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            activity = Path(directory) / "activity.jsonl"
            activity.write_text(
                '{"event":"drag_armed","timestamp_us":1,"actions":0,"selected_action":0,"destination_window":false}\n'
                '{"event":"drag_status_copy","timestamp_us":2,"actions":2,"selected_action":2,"destination_window":true}\n',
                encoding="utf-8",
            )
            events = drag.source_activity(activity)
            self.assertTrue(drag.activity_has(events, "drag_armed"))
            self.assertTrue(drag.activity_has(events, "drag_status_copy"))

    def test_x11_requires_unique_target_window(self):
        result = mock.Mock(stdout=b"1\n2\n")
        with mock.patch.object(drag.subprocess, "run", return_value=result):
            with self.assertRaisesRegex(drag.FileDragQualificationError, "not uniquely discoverable"):
                drag.x11_window("CopyPaste", {})

    def test_x11_target_must_belong_to_the_installed_gui(self):
        result = __import__("subprocess").CompletedProcess(["xdotool"], 0, stdout=b"51\n")
        with mock.patch.object(drag, "run", return_value=result):
            drag.require_x11_window_pid("100", 51, {})
        with mock.patch.object(drag, "run", return_value=result):
            with self.assertRaisesRegex(drag.FileDragQualificationError, "not owned"):
                drag.require_x11_window_pid("100", 52, {})

    def test_x11_drag_requires_source_focus(self):
        result = __import__("subprocess").CompletedProcess(["xdotool"], 0, stdout=b"100\n")
        with mock.patch.object(drag, "run", return_value=result):
            drag.require_x11_focus("100", {})
        with mock.patch.object(drag, "run", return_value=result):
            with self.assertRaisesRegex(drag.FileDragQualificationError, "input focus"):
                drag.require_x11_focus("101", {})

    def test_wayland_portal_pointer_requests_pointer_and_sends_evdev_button(self):
        calls = []

        class Helper:
            @staticmethod
            def token(value):
                return value

        class Portal:
            def _request(self, method, _signature, values, callback=None):
                calls.append((method, values))
                if callback is not None:
                    callback()
                if method == "CreateSession":
                    return {"session_handle": "/session"}
                if method == "Start":
                    return {"devices": drag.POINTER_DEVICE}
                return {}

            def _call(self, interface, method, value):
                calls.append((interface, method, value))

        class GLib:
            @staticmethod
            def Variant(signature, values):
                return (signature, values)

        pointer = drag.PortalPointer(Portal(), Helper(), GLib)
        accepted = []
        pointer.grant(lambda: accepted.append(True))
        pointer.motion(5, -3)
        pointer.button(1)
        pointer.close()
        self.assertEqual(calls[1][1][1]["types"], drag.POINTER_DEVICE)
        self.assertEqual(accepted, [True])
        self.assertIn(("org.freedesktop.portal.RemoteDesktop", "NotifyPointerButton", ("(oa{sv}iu)", ("/session", {}, drag.LEFT_BUTTON, 1))), calls)

    def test_driver_requires_a_new_cli_observed_external_file_attachment(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            helper = root / "linux-native-file-drag.py"
            helper.write_text("# helper\n", encoding="utf-8")
            original = driver.EXTERNAL_FILE_DRAG
            driver.EXTERNAL_FILE_DRAG = helper
            before = [{"id": "old", "content_type": "text"}]
            imported = [{
                "id": "new", "content_type": "file",
                "file_details": {"filename": "external-file-drag-fixture.txt", "size_bytes": 51},
            }]
            try:
                with mock.patch.object(driver, "status", side_effect=[{"item_count": 1}, {"item_count": 2}]), \
                     mock.patch.object(driver, "list_items", side_effect=[before, imported]), \
                     mock.patch.object(driver, "run", return_value=__import__("subprocess").CompletedProcess(["python3"], 0)) as run:
                    driver.require_external_file_drag(root / "copypaste-cli", {}, root, 42, "x11")
            finally:
                driver.EXTERNAL_FILE_DRAG = original
            command = run.call_args.args[0]
            self.assertEqual(command[command.index("--session") + 1], "x11")
            self.assertEqual(command[command.index("--gui-pid") + 1], "42")


if __name__ == "__main__":
    unittest.main()
