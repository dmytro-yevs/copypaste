import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "bundle_linux_gstreamer_camera",
    ROOT / "scripts/release/bundle-linux-gstreamer-camera.py",
)
bundler = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(bundler)


class BundleLinuxGstreamerCameraTest(unittest.TestCase):
    def test_required_elements_are_bound_to_actual_debian_runtime_packages(self):
        self.assertEqual(
            bundler.REQUIRED_ELEMENTS,
            {
                "videoconvert": "gstreamer1.0-plugins-base",
                "videoscale": "gstreamer1.0-plugins-base",
                "videorate": "gstreamer1.0-plugins-base",
                "appsink": "gstreamer1.0-plugins-base",
                "v4l2src": "gstreamer1.0-plugins-good",
                "jpegdec": "gstreamer1.0-plugins-good",
            },
        )

    def test_plugin_filename_requires_a_real_absolute_filename(self):
        self.assertEqual(
            bundler.plugin_filename("Plugin Details:\n  Filename                 /usr/lib/libgstapp.so\n"),
            Path("/usr/lib/libgstapp.so"),
        )
        with self.assertRaisesRegex(bundler.BundleError, "did not identify"):
            bundler.plugin_filename("Plugin Details:\n  Filename                 relative.so\n")

    def test_needed_reads_only_dynamic_sonames(self):
        completed = mock.Mock(stdout="""
 0x0000000000000001 (NEEDED)             Shared library: [libgstapp-1.0.so.0]
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]
""")
        with mock.patch.object(bundler, "run", return_value=completed):
            self.assertEqual(bundler.needed(Path("/runtime/plugin.so")), {"libgstapp-1.0.so.0", "libc.so.6"})

    def test_isolated_environment_disables_ambient_plugin_roots(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            environment = bundler.isolated_environment(appdir=root, registry=root / "cache/registry.bin")
        expected = str(root / "usr/lib/gstreamer-1.0")
        self.assertEqual(environment["GST_PLUGIN_PATH"], expected)
        self.assertEqual(environment["GST_PLUGIN_SYSTEM_PATH"], expected)
        self.assertEqual(environment["GST_PLUGIN_PATH_1_0"], expected)
        self.assertEqual(environment["GST_PLUGIN_SYSTEM_PATH_1_0"], expected)
        self.assertEqual(environment["GST_PLUGIN_SCANNER"], str(root / "usr/libexec/gstreamer-1.0/gst-plugin-scanner"))

    def test_copy_rejects_symlinked_or_conflicting_runtime_destinations(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source.so"
            source.write_bytes(b"runtime")
            destination = root / "destination"
            with mock.patch.object(bundler, "allowed_runtime_file", return_value=source.resolve()):
                copied = bundler.copy_with_soname(source, destination, "libsource.so")
                self.assertEqual(copied.read_bytes(), b"runtime")
                self.assertTrue((destination / "libsource.so").is_symlink())
                copied.write_bytes(b"different")
                with self.assertRaisesRegex(bundler.BundleError, "collide"):
                    bundler.copy_with_soname(source, destination, "libsource.so")

    def test_glibc_ceiling_is_fixed_to_the_linux_release_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            root.joinpath("usr").mkdir()
            manifest = root / "manifest.json"
            with self.assertRaisesRegex(bundler.BundleError, "glibc compatibility ceiling"):
                bundler.bundle(
                    appdir=root,
                    architecture="x86_64",
                    maximum_glibc="2.40",
                    manifest=manifest,
                    inspect="gst-inspect-1.0",
                )

    def test_manifest_cannot_escape_the_appimage(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            root.joinpath("usr").mkdir()
            with self.assertRaisesRegex(bundler.BundleError, "inside the AppImage"):
                bundler.bundle(
                    appdir=root,
                    architecture="x86_64",
                    maximum_glibc="2.39",
                    manifest=root.parent / "outside.json",
                    inspect="gst-inspect-1.0",
                )

    def test_camera_plugin_must_be_unique_regular_file_inside_appdir(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plugins = root / "usr/lib/copypaste/lib"
            plugins.mkdir(parents=True)
            plugin = plugins / "libcamera_desktop_plugin.so"
            plugin.write_bytes(b"camera")
            self.assertEqual(bundler.camera_plugin_path(root), plugin)
            plugin.unlink()
            with self.assertRaisesRegex(bundler.BundleError, "exactly one"):
                bundler.camera_plugin_path(root)


if __name__ == "__main__":
    unittest.main()
