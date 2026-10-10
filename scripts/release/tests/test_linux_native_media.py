import importlib.util
from pathlib import Path
import stat
import tempfile
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

    def test_click_uses_named_atspi_action_and_records_only_hashes(self):
        class Action:
            nActions = 1

            def getName(self, index):
                return "click"

            def doAction(self, index):
                return True

        class Node:
            childCount = 0

            def __init__(self, name):
                self.name = name

            def getChildAtIndex(self, index):
                raise IndexError(index)

            def queryAction(self):
                return Action()

        application = Node("application")
        target = Node("Scan QR")
        expected = Node("Pair a device")
        with mock.patch.object(media, "atspi_application", return_value=application), \
             mock.patch.object(media, "matching_accessible", side_effect=[target, expected]):
            record = media.click_accessible(mock.Mock(), pid=123, target="Scan QR", expected="Pair a device", timeout_seconds=1)
        self.assertEqual(record["kind"], "atspi_click")
        self.assertEqual(record["gui_pid"], 123)
        self.assertNotIn("Scan QR", str(record))
        self.assertNotIn("Pair a device", str(record))

    def test_click_rejects_missing_actionable_control(self):
        with mock.patch.object(media, "atspi_application", return_value=mock.Mock()), \
             mock.patch.object(media, "matching_accessible", return_value=None):
            with self.assertRaisesRegex(media.MediaQualificationError, "control is absent"):
                media.click_accessible(mock.Mock(), pid=1, target="Scan QR", expected="Pair a device", timeout_seconds=1)


if __name__ == "__main__":
    unittest.main()
