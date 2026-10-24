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
    def test_owned_top_level_bounds_ignore_duplicate_titles_inside_flutter(self):
        rectangle = mock.Mock(x=90, y=32, width=1100, height=768)
        application = mock.Mock(childCount=1)
        application.get_process_id.return_value = 42
        application.get_application.return_value = application
        frame = mock.Mock(name="frame", childCount=1)
        frame.name = "CopyPaste"
        frame.getRoleName.return_value = "frame"
        frame.queryComponent.return_value.getExtents.return_value = rectangle
        frame.getChildAtIndex.side_effect = AssertionError("a frame lookup must not traverse Flutter descendants")
        application.getChildAtIndex.return_value = frame
        frame.get_application.return_value = application
        desktop = mock.Mock(childCount=1)
        desktop.getChildAtIndex.return_value = application
        registry = mock.Mock()
        registry.getDesktop.return_value = desktop
        self.assertEqual(
            drag.atspi_bounds(registry, 42, "CopyPaste", drag.SCREEN_COORDINATES, top_level=True),
            drag.Bounds(90, 32, 1100, 768),
        )
        frame.queryComponent.return_value.getExtents.assert_called_once_with(drag.SCREEN_COORDINATES)

    def test_x11_pointer_uses_owned_accessibility_desktop_bounds_after_focus(self):
        source = mock.Mock(process=mock.Mock(pid=71))
        calls = []
        def command(argv, _environment):
            calls.append(argv)
            return mock.Mock(stdout=b"")
        def bounds(_registry, pid, name, coordinates, *, top_level=False):
            self.assertEqual(coordinates, drag.SCREEN_COORDINATES)
            self.assertIn(["xdotool", "windowactivate", "--sync", "11"], calls)
            if pid == 71:
                self.assertEqual(name, drag.SOURCE_CONTROL_NAME)
                return drag.Bounds(0, 69, 320, 96)
            self.assertEqual((pid, name), (72, drag.TARGET_TITLE))
            self.assertTrue(top_level)
            return drag.Bounds(400, 100, 600, 500)
        with mock.patch.dict(sys.modules, {"pyatspi": mock.Mock(Registry=mock.sentinel.registry)}), \
                mock.patch.object(drag, "x11_window", side_effect=["11", "12"]), \
                mock.patch.object(drag, "require_x11_window_pid"), \
                mock.patch.object(drag, "require_x11_focus"), \
                mock.patch.object(drag, "_atspi_bounds_or_none", side_effect=bounds), \
                mock.patch.object(drag, "x11_bounds", side_effect=AssertionError("raw X11 offsets must not position the pointer")), \
                mock.patch.object(drag, "run", side_effect=command):
            drag.drag_x11(source, 72, {"DISPLAY": ":99"})
        self.assertIn(["xdotool", "mousemove", "--sync", "160", "117"], calls)
        self.assertIn(["xdotool", "mousemove", "--sync", "832", "350"], calls)
        self.assertEqual(source.method_calls, [mock.call.begin(), mock.call.started(), mock.call.copied(), mock.call.completed()])

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

        source = drag.CompositorGeometry(drag.Bounds(100, 150, 320, 96), drag.Bounds(100, 150, 320, 96), 1)
        target = drag.CompositorGeometry(drag.Bounds(600, 120, 480, 600), drag.Bounds(600, 120, 480, 600), 2)
        self.assertEqual(
            drag.require_single_wayland_layout(Display(), source, target, source.buffer, target.buffer),
            drag.Bounds(0, 0, 1280, 720),
        )

        class MultiDisplay(Display):
            def get_n_monitors(self):
                return 2

        with self.assertRaisesRegex(drag.FileDragQualificationError, "one visible monitor"):
            drag.require_single_wayland_layout(MultiDisplay(), source, target, source.buffer, target.buffer)

    def test_wayland_geometry_composes_window_local_accessibility_bounds_from_buffer_origin(self):
        geometry = drag.CompositorGeometry(drag.Bounds(100, 120, 320, 96), drag.Bounds(92, 112, 336, 112), 3)
        self.assertEqual(
            drag.absolute_atspi_bounds(drag.Bounds(8, 8, 200, 72), geometry, "source"),
            drag.Bounds(100, 120, 200, 72),
        )
        with self.assertRaisesRegex(drag.FileDragQualificationError, "exceed"):
            drag.absolute_atspi_bounds(drag.Bounds(300, 8, 72, 72), geometry, "source")

    def test_wayland_compositor_geometry_uses_the_pid_and_title_bound_fixture_interface(self):
        calls = []

        class Reply:
            @staticmethod
            def unpack():
                return (10, 20, 300, 200, 4, 8, 312, 216, 9)

        class Connection:
            @staticmethod
            def call_sync(*args):
                calls.append(args)
                if args[3] == "GetNameOwner":
                    return type("Owner", (), {"unpack": staticmethod(lambda: (":1.1",))})()
                if args[3] == "GetConnectionUnixProcessID":
                    return type("Pid", (), {"unpack": staticmethod(lambda: (42,))})()
                return Reply()

        class Gio:
            class BusType:
                SESSION = "session"

            class DBusCallFlags:
                NONE = "none"

            @staticmethod
            def bus_get_sync(bus_type, _cancellable):
                self.assertEqual(bus_type, Gio.BusType.SESSION)
                return Connection()

        class GLib:
            @staticmethod
            def Variant(signature, values):
                return (signature, values)

            @staticmethod
            def VariantType(signature):
                return signature

        with mock.patch.dict(drag.os.environ, {drag.GEOMETRY_OWNER_PID_ENV: "42"}):
            geometry = drag.compositor_geometry(Gio, GLib, 77, "CopyPaste")
        self.assertEqual(geometry.buffer, drag.Bounds(4, 8, 312, 216))
        lookup = next(call for call in calls if call[3] == "Lookup")
        self.assertEqual(lookup[0:4], (drag.GEOMETRY_BUS_NAME, drag.GEOMETRY_OBJECT_PATH, drag.GEOMETRY_INTERFACE, "Lookup"))
        self.assertEqual(lookup[4], ("(us)", (77, "CopyPaste")))

    def test_wayland_injection_points_require_compositor_hit_testing(self):
        bounds = drag.Bounds(100, 100, 200, 100)
        with mock.patch.object(drag, "compositor_owns_point", side_effect=[False, True]):
            self.assertEqual(drag.owned_point(None, None, 7, "CopyPaste", bounds), (244, 150))
        with mock.patch.object(drag, "compositor_owns_point", return_value=False):
            with self.assertRaisesRegex(drag.FileDragQualificationError, "compositor-visible"):
                drag.owned_point(None, None, 7, "CopyPaste", bounds)

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

    def test_completed_reads_the_source_activity_file_before_checking_the_drop(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            activity = root / "activity.jsonl"
            activity.write_text(
                '{"event":"uri_list_requested","timestamp_us":1,"actions":2,"selected_action":2,"destination_window":true}\n'
                '{"event":"drag_end","timestamp_us":2,"actions":2,"selected_action":2,"destination_window":true}\n',
                encoding="utf-8",
            )
            source = drag.DragSource(mock.Mock(), root / "start", activity)
            source.completed()
            activity.write_text(
                '{"event":"drag_end","timestamp_us":3,"actions":0,"selected_action":0,"destination_window":false}\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(drag.FileDragQualificationError, "not accepted"):
                source.completed()

    def test_wayland_sibling_loader_registers_its_module_for_future_dataclasses(self):
        with tempfile.TemporaryDirectory() as directory:
            helper = Path(directory) / "wayland-helper.py"
            helper.write_text(
                "from __future__ import annotations\n"
                "from dataclasses import dataclass\n"
                "@dataclass\n"
                "class Receipt:\n"
                "    prior: Receipt | None = None\n",
                encoding="utf-8",
            )
            original = drag.WAYLAND_QUICK_PASTE
            previous = sys.modules.get(drag.WAYLAND_PORTAL_MODULE)
            drag.WAYLAND_QUICK_PASTE = helper
            try:
                module = drag.load_wayland_portal()
                self.assertEqual(module.Receipt.__module__, drag.WAYLAND_PORTAL_MODULE)
                self.assertIs(sys.modules[drag.WAYLAND_PORTAL_MODULE], module)
            finally:
                drag.WAYLAND_QUICK_PASTE = original
                if previous is None:
                    sys.modules.pop(drag.WAYLAND_PORTAL_MODULE, None)
                else:
                    sys.modules[drag.WAYLAND_PORTAL_MODULE] = previous

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

            def _call(self, interface, method, value, *, path='/org/freedesktop/portal/desktop'):
                calls.append((interface, method, value, path))

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
        self.assertEqual(calls[0][1][0]['handle_token'], ('s', 'create'))
        self.assertEqual(calls[0][1][0]['session_handle_token'], ('s', 'session'))
        self.assertEqual(calls[1][1][1]["types"], ('u', drag.POINTER_DEVICE))
        self.assertEqual(calls[2][1][2]['handle_token'], ('s', 'start'))
        self.assertEqual(accepted, [True])
        self.assertIn(("org.freedesktop.portal.RemoteDesktop", "NotifyPointerButton", ("(oa{sv}iu)", ("/session", {}, drag.LEFT_BUTTON, 1)), '/org/freedesktop/portal/desktop'), calls)
        self.assertEqual(calls[-1], ('org.freedesktop.portal.Session', 'Close', None, '/session'))

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
            navigation = []
            def action(_actor, command, pid, target, _environment):
                self.assertEqual(pid, 42)
                navigation.append((command, target))
            def drop(*_args, **_kwargs):
                self.assertEqual(navigation, [("click", "Done"), ("click", "History"), ("observe", "Import files")])
                return __import__("subprocess").CompletedProcess(["python3"], 0)
            try:
                with mock.patch.object(driver, "status", side_effect=[{"item_count": 1}, {"item_count": 2}]), \
                     mock.patch.object(driver, "list_items", side_effect=[before, imported]), \
                     mock.patch.object(driver, "atspi_present", return_value=True), \
                     mock.patch.object(driver, "atspi_action", side_effect=action), \
                     mock.patch.object(driver, "run", side_effect=drop) as run:
                    driver.require_external_file_drag(root / "copypaste-cli", {}, root, 42, "x11")
            finally:
                driver.EXTERNAL_FILE_DRAG = original
            command = run.call_args.args[0]
            self.assertEqual(command[command.index("--session") + 1], "x11")
            self.assertEqual(command[command.index("--gui-pid") + 1], "42")

    def test_driver_never_drags_into_an_unready_history_surface(self):
        with mock.patch.object(driver, "atspi_present", return_value=False), \
             mock.patch.object(driver, "atspi_action", side_effect=RuntimeError("History unavailable")), \
             mock.patch.object(driver, "run") as run, \
             mock.patch.object(driver, "status") as status, \
             self.assertRaisesRegex(RuntimeError, "History unavailable"):
            driver.require_external_file_drag(Path("cli"), {}, Path("workspace"), 42, "x11")
        run.assert_not_called()
        status.assert_not_called()


if __name__ == "__main__":
    unittest.main()
