import ctypes
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock


SPEC = importlib.util.spec_from_file_location(
    "linux_camera_pipelines", Path(__file__).resolve().parents[1] / "verify-linux-camera-pipelines.py",
)
probe = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(probe)


class LinuxCameraPipelinesTest(unittest.TestCase):
    def native(self):
        return mock.Mock(), mock.Mock(), mock.Mock()

    def verify(self, libraries):
        with mock.patch.object(probe, "runtime_library", side_effect=lambda root, name: root / name), \
             mock.patch.object(probe.ctypes, "CDLL", side_effect=libraries):
            probe.verify_pipelines(Path("/packaged/runtime"))

    def test_partial_pipeline_with_native_error_is_rejected_and_freed(self):
        gst, gobject, glib = self.native()
        error = probe._GError(1, 1, b'no element "videoflip"')

        def parse(description, output):
            ctypes.cast(output, ctypes.POINTER(ctypes.POINTER(probe._GError)))[0] = ctypes.pointer(error)
            return 123

        gst.gst_parse_launch.side_effect = parse
        with self.assertRaisesRegex(probe.PipelineError, 'incomplete.*videoflip'):
            self.verify((gst, gobject, glib))
        gobject.g_object_unref.assert_called_once_with(123)
        glib.g_error_free.assert_called_once()

    def test_null_pipeline_without_native_error_is_rejected(self):
        gst, gobject, glib = self.native()
        gst.gst_parse_launch.return_value = None
        with self.assertRaisesRegex(probe.PipelineError, "unavailable"):
            self.verify((gst, gobject, glib))
        gobject.g_object_unref.assert_not_called()
        glib.g_error_free.assert_not_called()

    def test_raw_and_mjpeg_pipelines_are_constructed_and_released_without_capture(self):
        gst, gobject, glib = self.native()
        gst.gst_parse_launch.side_effect = [123, 456]
        self.verify((gst, gobject, glib))
        gobject.g_object_unref.assert_has_calls([mock.call(123), mock.call(456)])
        gst.gst_element_set_state.assert_not_called()
        glib.g_error_free.assert_not_called()

    def test_runtime_library_cannot_resolve_outside_packaged_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "runtime"
            runtime.mkdir()
            external = root / "external.so"
            external.write_bytes(b"library")
            (runtime / "libgstreamer-1.0.so.0").symlink_to(external)
            with self.assertRaisesRegex(probe.PipelineError, "outside"):
                probe.runtime_library(runtime, "libgstreamer-1.0.so.0")


if __name__ == "__main__":
    unittest.main()
