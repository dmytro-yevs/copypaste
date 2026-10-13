import importlib.util
import json
from pathlib import Path
import stat
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "linux_native_media", ROOT / "scripts/release/linux-native-media.py",
)
media = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(media)


class LinuxNativeMediaTest(unittest.TestCase):
    def test_pairing_uri_metadata_is_redacted_and_hash_bound(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "uri"
            source.write_text("copypaste://pair/secret\n", encoding="utf-8")
            value, metadata = media.read_pairing_uri(source)
        self.assertEqual(value, "copypaste://pair/secret")
        self.assertEqual(metadata["size_bytes"], len(b"copypaste://pair/secret\n"))
        self.assertEqual(len(metadata["sha256"]), 64)
        self.assertNotIn("secret", str(metadata))

    def test_pairing_uri_rejects_symlink_and_control_data(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.write_bytes(b"copypaste://pair\x00")
            with self.assertRaisesRegex(media.MediaQualificationError, "control"):
                media.read_pairing_uri(target)
            link = root / "link"
            link.symlink_to(target)
            with self.assertRaisesRegex(media.MediaQualificationError, "non-symlink"):
                media.read_pairing_uri(link)

    def test_camera_command_never_puts_pairing_uri_in_argv(self):
        command = media.camera_feed_command(image=Path("/private/frame.png"), device=Path("/dev/video42"))
        self.assertEqual(command[-1], "/dev/video42")
        self.assertIn("format=yuyv422", command[-6])
        self.assertIn("scale=640:480", command[-6])
        self.assertNotIn("copypaste://pair/secret", command)

    def test_v4l2_device_requires_a_video_character_device(self):
        character = mock.Mock()
        character.st_mode = stat.S_IFCHR
        with mock.patch.object(Path, "stat", return_value=character):
            media.v4l2_device(Path("/dev/video77"))
        with self.assertRaisesRegex(media.MediaQualificationError, "invalid"):
            media.v4l2_device(Path("/tmp/video77"))

    def test_owned_ffmpeg_check_requires_the_exact_v4l2_device(self):
        with mock.patch.object(Path, "read_bytes", return_value=b"ffmpeg\0-f\0v4l2\0/dev/video42\0"):
            self.assertTrue(media.owned_ffmpeg_process(123, "/dev/video42"))
            self.assertFalse(media.owned_ffmpeg_process(123, "/dev/video43"))

    def test_stop_rejects_state_with_a_path_escape(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory) / "camera.json"
            state.write_text('{"pid":1,"device":"/dev/video42","ready_file":"../other"}', encoding="utf-8")
            with self.assertRaisesRegex(media.MediaQualificationError, "invalid"):
                media.stop_qr_feed(state)

    def test_click_uses_the_supported_gtk_or_flutter_atspi_action_and_records_only_hashes(self):
        class Action:
            def __init__(self, name, accepted=True):
                self.name = name
                self.accepted = accepted
                self.invoked = []
                self.nActions = 1

            def getName(self, index):
                return self.name

            def doAction(self, index):
                self.invoked.append(index)
                return self.accepted

        class Node:
            def __init__(self, name, action=None, children=()):
                self.name = name
                self.action = action
                self.children = list(children)
                self.childCount = len(self.children)

            def getChildAtIndex(self, index):
                return self.children[index]

            def queryAction(self):
                if self.action is None:
                    raise RuntimeError("not actionable")
                return self.action

        expected = Node("Pair a device")
        for action_name in ("click", "Tap"):
            with self.subTest(action_name=action_name):
                target = Node("Scan QR")
                target.action = Action(action_name)
                application = Node("application", children=(target,))
                with mock.patch.object(media, "atspi_application", return_value=application), \
                     mock.patch.object(media, "matching_accessible", return_value=expected):
                    record = media.click_accessible(mock.Mock(), pid=123, target="Scan QR", expected="Pair a device", timeout_seconds=1)
                self.assertEqual(target.action.invoked, [0])
                self.assertEqual(record["kind"], "atspi_click")
                self.assertEqual(record["gui_pid"], 123)
                self.assertNotIn("Scan QR", str(record))
                self.assertNotIn("Pair a device", str(record))

    def test_click_rejects_missing_actionable_control(self):
        class Application:
            childCount = 0

            def getChildAtIndex(self, index):
                raise IndexError(index)

        with mock.patch.object(media, "atspi_application", return_value=Application()):
            with self.assertRaisesRegex(media.MediaQualificationError, "control is absent"):
                media.click_accessible(mock.Mock(), pid=1, target="Scan QR", expected="Pair a device", timeout_seconds=1)

    def test_click_rejects_unsupported_or_rejected_atspi_actions(self):
        class Action:
            nActions = 1

            def __init__(self, name, accepted=True):
                self.name = name
                self.accepted = accepted

            def getName(self, _index):
                return self.name

            def doAction(self, _index):
                return self.accepted

        class Node:
            def __init__(self, name, action=None, children=()):
                self.name = name
                self.action = action
                self.children = list(children)
                self.childCount = len(self.children)

            def getChildAtIndex(self, index):
                return self.children[index]

            def queryAction(self):
                if self.action is None:
                    raise RuntimeError("not actionable")
                return self.action

        application = Node("application", children=(Node("Continue", Action("Focus")),))
        with mock.patch.object(media, "atspi_application", return_value=application):
            with self.assertRaisesRegex(media.MediaQualificationError, "not actionable"):
                media.invoke_accessible(mock.Mock(), pid=1, target="Continue")
        application = Node("application", children=(Node("Continue", Action("Tap", accepted=False)),))
        with mock.patch.object(media, "atspi_application", return_value=application):
            with self.assertRaisesRegex(media.MediaQualificationError, "not accepted"):
                media.invoke_accessible(mock.Mock(), pid=1, target="Continue")

    def test_click_ignores_a_same_name_non_actionable_heading_before_the_button(self):
        class Action:
            nActions = 1

            def __init__(self):
                self.invoked = []

            def getName(self, _index):
                return "Tap"

            def doAction(self, index):
                self.invoked.append(index)
                return True

        class Node:
            def __init__(self, name, action=None, children=()):
                self.name = name
                self.action = action
                self.children = list(children)
                self.childCount = len(self.children)

            def getChildAtIndex(self, index):
                return self.children[index]

            def queryAction(self):
                if self.action is None:
                    raise RuntimeError("not actionable")
                return self.action

        action = Action()
        heading = Node("Continue")
        button = Node("Continue", action)
        application = Node("application", children=(heading, button))
        with mock.patch.object(media, "atspi_application", return_value=application):
            media.invoke_accessible(mock.Mock(), pid=1, target="Continue")
        self.assertEqual(action.invoked, [0])

    def test_click_rejects_two_same_name_actionable_controls(self):
        class Action:
            nActions = 1

            def getName(self, _index):
                return "click"

            def doAction(self, _index):
                return True

        class Node:
            def __init__(self, name, action=None, children=()):
                self.name = name
                self.action = action
                self.children = list(children)
                self.childCount = len(self.children)

            def getChildAtIndex(self, index):
                return self.children[index]

            def queryAction(self):
                if self.action is None:
                    raise RuntimeError("not actionable")
                return self.action

        application = Node("application", children=(Node("Continue", Action()), Node("Continue", Action())))
        with mock.patch.object(media, "atspi_application", return_value=application):
            with self.assertRaisesRegex(media.MediaQualificationError, "ambiguous"):
                media.invoke_accessible(mock.Mock(), pid=1, target="Continue")

    def test_quick_paste_readiness_requires_visible_marker_and_one_focused_editable_text(self):
        class State:
            def __init__(self, *values):
                self.values = set(values)

            def contains(self, value):
                return value in self.values

        class Node:
            def __init__(self, name, state, *, text=False, children=()):
                self.name = name
                self._state = state
                self._text = text
                self._children = list(children)
                self.childCount = len(self._children)

            def getChildAtIndex(self, index):
                return self._children[index]

            def getState(self):
                return self._state

            def queryText(self):
                if not self._text:
                    raise RuntimeError("not text")
                return object()

        atspi = SimpleNamespace(
            STATE_SHOWING="showing", STATE_VISIBLE="visible", STATE_FOCUSED="focused", STATE_EDITABLE="editable",
        )
        marker = Node("requested clip\nCtrl\n1", State("showing", "visible"))
        input_node = Node("search", State("showing", "visible", "focused", "editable"), text=True)
        application = Node("application", State(), children=(marker, input_node))
        with mock.patch.object(media, "atspi_application", return_value=application):
            record = media.quick_paste_ready(mock.Mock(), atspi, pid=123, marker="requested clip", timeout_seconds=1)
        self.assertEqual(record["kind"], "atspi_quick_paste_ready")
        self.assertEqual(record["gui_pid"], 123)

    def test_quick_paste_marker_matches_only_the_exact_first_content_line(self):
        class Node:
            def __init__(self, name, children=()):
                self.name = name
                self.children = list(children)
                self.childCount = len(self.children)

            def getChildAtIndex(self, index):
                return self.children[index]

        marker = "copypaste-x11-quick-paste-fixture"
        bare = Node(marker)
        aggregated = Node(f"{marker}\nCtrl\n1")
        self.assertIs(media.quick_paste_marker_accessible(Node("application", (bare,)), marker), bare)
        self.assertIs(media.quick_paste_marker_accessible(Node("application", (aggregated,)), marker), aggregated)
        for name in (
            f"prefix {marker}",
            f"{marker} suffix",
            f"Ctrl\n{marker}",
        ):
            with self.subTest(name=name):
                self.assertIsNone(media.quick_paste_marker_accessible(Node("application", (Node(name),)), marker))

    def test_quick_paste_readiness_waits_for_its_accessibility_application(self):
        class State:
            def contains(self, _value):
                return True

        class Node:
            def __init__(self, name, *, text=False, children=()):
                self.name = name
                self.text = text
                self.children = list(children)
                self.childCount = len(self.children)

            def getChildAtIndex(self, index):
                return self.children[index]

            def getState(self):
                return State()

            def queryText(self):
                if not self.text:
                    raise RuntimeError("not text")
                return object()

        atspi = SimpleNamespace(
            STATE_SHOWING="showing", STATE_VISIBLE="visible", STATE_FOCUSED="focused", STATE_EDITABLE="editable",
        )
        application = Node("application", children=(Node("marker"), Node("search", text=True)))
        with mock.patch.object(
            media,
            "atspi_application",
            side_effect=[
                media.MediaQualificationError("installed GUI is absent from the AT-SPI desktop"), application,
            ],
        ) as observed, mock.patch.object(media.time, "sleep"):
            media.quick_paste_ready(mock.Mock(), atspi, pid=123, marker="marker", timeout_seconds=1)
        self.assertEqual(observed.call_count, 2)

    def test_quick_paste_readiness_rejects_missing_marker_focus_or_pid(self):
        class State:
            def __init__(self, *values):
                self.values = set(values)

            def contains(self, value):
                return value in self.values

        class Node:
            childCount = 0

            def __init__(self, name, state, text=False):
                self.name = name
                self.state = state
                self.text = text

            def getChildAtIndex(self, index):
                raise IndexError(index)

            def getState(self):
                return self.state

            def queryText(self):
                if not self.text:
                    raise RuntimeError("not text")
                return object()

        atspi = SimpleNamespace(
            STATE_SHOWING="showing", STATE_VISIBLE="visible", STATE_FOCUSED="focused", STATE_EDITABLE="editable",
        )
        application = Node("application", State())
        with mock.patch.object(media, "atspi_application", return_value=application), \
             mock.patch.object(media.time, "monotonic", side_effect=[0, 0, 2]), \
             mock.patch.object(media.time, "sleep"):
            with self.assertRaisesRegex(media.MediaQualificationError, "requested clip"):
                media.quick_paste_ready(mock.Mock(), atspi, pid=123, marker="missing", timeout_seconds=1)
        with self.assertRaisesRegex(media.MediaQualificationError, "arguments"):
            media.quick_paste_ready(mock.Mock(), atspi, pid=0, marker="marker", timeout_seconds=1)
        marker = Node("marker", State("showing", "visible"))
        with mock.patch.object(media, "atspi_application", return_value=application), \
             mock.patch.object(media, "matching_accessible", return_value=marker), \
             mock.patch.object(media, "focused_editable_text", return_value=False), \
             mock.patch.object(media.time, "monotonic", side_effect=[0, 0, 2]), \
             mock.patch.object(media.time, "sleep"):
            with self.assertRaisesRegex(media.MediaQualificationError, "focused editable"):
                media.quick_paste_ready(mock.Mock(), atspi, pid=123, marker="marker", timeout_seconds=1)

    def test_quick_paste_readiness_rejects_a_pid_absent_for_the_entire_deadline(self):
        atspi = SimpleNamespace(
            STATE_SHOWING="showing", STATE_VISIBLE="visible", STATE_FOCUSED="focused", STATE_EDITABLE="editable",
        )
        absent = media.MediaQualificationError("installed GUI is absent from the AT-SPI desktop")
        with mock.patch.object(media, "atspi_application", side_effect=absent), \
             mock.patch.object(media.time, "monotonic", side_effect=[0, 0, 2]), \
             mock.patch.object(media.time, "sleep"):
            with self.assertRaisesRegex(media.MediaQualificationError, "requested clip"):
                media.quick_paste_ready(mock.Mock(), atspi, pid=123, marker="marker", timeout_seconds=1)

    def test_camera_observation_binds_the_gui_pairing_state_and_gstreamer_mapping(self):
        application = mock.Mock()
        with mock.patch.object(media, "atspi_application", return_value=application), \
             mock.patch.object(media, "matching_accessible", return_value=mock.Mock()), \
             mock.patch.object(media, "packaged_camera_runtime", return_value=({
                 "camera_plugin": Path("/camera"), "gstreamer_library": Path("/gstreamer"), "v4l2_plugin": Path("/v4l2"),
             }, {"origin": "appimage_manifest", "gstreamer_library": {"sha256": "b" * 64}, "v4l2_plugin": {"sha256": "b" * 64}})), \
             mock.patch.object(media, "gstreamer_mapping_sha256", return_value="a" * 64), \
             mock.patch.object(media, "sha256", return_value="b" * 64), \
             mock.patch.object(Path, "read_text", return_value="mapped runtime"):
            record = media.observe_camera_scan(
                mock.Mock(), pid=123, expected="Confirm on both devices", runtime_prefix=Path("/runtime"), runtime_format="AppImage",
            )
        self.assertEqual(record["kind"], "camera_qr_scan_observed")
        self.assertEqual(record["gui_pid"], 123)
        self.assertEqual(record["gstreamer_mapping_sha256"], "a" * 64)
        self.assertNotIn("Confirm on both devices", str(record))

    def test_camera_observation_rejects_absent_decoded_pairing_state(self):
        with mock.patch.object(media, "atspi_application", return_value=mock.Mock()), \
             mock.patch.object(media, "matching_accessible", return_value=None):
            with self.assertRaisesRegex(media.MediaQualificationError, "decoded pairing state"):
                media.observe_camera_scan(mock.Mock(), pid=123, expected="Confirm on both devices", runtime_prefix=Path("/runtime"), runtime_format="AppImage")

    def test_appimage_runtime_uses_manifest_versioned_gstreamer_library(self):
        with tempfile.TemporaryDirectory() as directory:
            appdir = Path(directory)
            prefix = appdir / "usr/lib/copypaste"
            camera = prefix / "lib/libcamera_desktop_plugin.so"
            library = appdir / "usr/lib/gstreamer-runtime/libgstreamer-1.0.so.0"
            v4l2 = appdir / "usr/lib/gstreamer-1.0/libgstvideo4linux2.so.0"
            for path in (camera, library, v4l2):
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(path.name.encode("utf-8"))
            (library.parent / "libgstreamer-1.0.so").symlink_to(library.name)
            manifest = {
                "schema": 1,
                "camera_plugin": {"path": str(camera.relative_to(appdir)), "sha256": media.sha256(camera)},
                "runtime_libraries": [{"path": str(library.relative_to(appdir)), "sha256": media.sha256(library)}],
                "elements": [{"element": "v4l2src", "path": str(v4l2.relative_to(appdir)), "sha256": media.sha256(v4l2)}],
            }
            target = appdir / "usr/share/copypaste/gstreamer-camera-runtime.json"
            target.parent.mkdir(parents=True)
            target.write_text(json.dumps(manifest), encoding="utf-8")
            runtime, dependency = media.packaged_camera_runtime(prefix, "AppImage", "")
            self.assertEqual(runtime["gstreamer_library"], library.resolve())
            self.assertEqual(dependency["origin"], "appimage_manifest")

    def test_native_runtime_records_distro_owned_media_dependencies(self):
        with tempfile.TemporaryDirectory() as directory:
            prefix = Path(directory) / "usr/lib/copypaste"
            camera = prefix / "lib/libcamera_desktop_plugin.so"
            camera.parent.mkdir(parents=True)
            camera.write_bytes(b"camera")
            gstreamer = Path(directory) / "libgstreamer-1.0.so.0"
            v4l2 = Path(directory) / "libgstvideo4linux2.so"
            gstreamer.write_bytes(b"gstreamer")
            v4l2.write_bytes(b"v4l2")
            with mock.patch.object(media, "mapped_file", side_effect=[gstreamer, v4l2]), \
                 mock.patch.object(media, "package_metadata", return_value={"package": "gstreamer1", "version": "1.0", "sha256": "a" * 64}):
                _, dependency = media.packaged_camera_runtime(prefix, "deb", "maps")
            self.assertEqual(dependency["origin"], "distro_package")

    def test_mapped_file_accepts_multiple_elf_map_segments_for_one_library(self):
        with tempfile.TemporaryDirectory() as directory:
            library = Path(directory) / "libgstreamer-1.0.so.0"
            library.write_bytes(b"gstreamer")
            maps = "\n".join(f"0000-0001 r--p 00000000 00:00 0 {library}" for _ in range(5))
            self.assertEqual(media.mapped_file(maps, "libgstreamer-1.0.so"), library.resolve())

    def test_debian_multiarch_owner_flows_into_native_dependency_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            library = Path(directory) / "libgstreamer-1.0.so.0"
            library.write_bytes(b"gstreamer")
            completed = [mock.Mock(stdout="libgstreamer1.0-0:amd64: /usr/lib/libgstreamer-1.0.so.0\n"), mock.Mock(stdout="1.26.0-1\n")]
            with mock.patch.object(media.subprocess, "run", side_effect=completed):
                metadata = media.package_metadata(library, "deb")
            self.assertEqual(metadata["package"], "libgstreamer1.0-0:amd64")
            self.assertEqual(metadata["version"], "1.26.0-1")


if __name__ == "__main__":
    unittest.main()
