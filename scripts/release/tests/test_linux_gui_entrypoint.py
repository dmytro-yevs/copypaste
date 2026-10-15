import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


SPEC = importlib.util.spec_from_file_location(
    "linux_gui_entrypoint_driver", Path(__file__).resolve().parents[1] / "linux-native-fixture-driver.py",
)
driver = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
import sys
sys.modules[SPEC.name] = driver
SPEC.loader.exec_module(driver)


class LinuxGuiEntrypointTest(unittest.TestCase):
    def test_capture_ignores_unrelated_startup_item_until_its_exact_fixture_arrives(self):
        unrelated = {"id": "startup", "content_type": "text", "content": "retained clipboard"}
        expected = {"id": "fixture", "content_type": "text", "content": "owned fixture"}
        with mock.patch.object(driver, "list_items", side_effect=[[unrelated], [expected, unrelated]]), \
             mock.patch.object(driver, "status", return_value={"item_count": 2}), \
             mock.patch.object(driver.time, "monotonic", side_effect=[0, 0, 1]), \
             mock.patch.object(driver.time, "sleep"):
            self.assertEqual(driver.wait_for_fixture_capture(None, {}, 0, set(), "text", b"owned fixture"), expected)

    def test_requested_fixture_with_missing_source_still_fails_identity_assertion(self):
        expected = {"id": "fixture", "content_type": "text", "content": "owned fixture"}
        with mock.patch.object(driver, "list_items", return_value=[expected]), \
             mock.patch.object(driver, "status", return_value={"item_count": 1}), \
             mock.patch.object(driver.time, "monotonic", side_effect=[0, 0]):
            item = driver.wait_for_fixture_capture(None, {}, 0, set(), "text", b"owned fixture")
        with self.assertRaisesRegex(RuntimeError, "GTK source application id"):
            driver.require_source_identity(item)

    def test_unrelated_items_do_not_extend_the_existing_capture_deadline(self):
        unrelated = {"id": "startup", "content_type": "text", "content": "retained clipboard"}
        with mock.patch.object(driver, "list_items", return_value=[unrelated]), \
             mock.patch.object(driver.time, "monotonic", side_effect=[0, 0, driver.CAPTURE_TIMEOUT_SECONDS]), \
             mock.patch.object(driver.time, "sleep"):
            with self.assertRaisesRegex(RuntimeError, "capture deadline"):
                driver.wait_for_fixture_capture(None, {}, 0, set(), "text", b"owned fixture")

    def prefix(self, root):
        prefix = root / "usr/lib/copypaste"
        prefix.mkdir(parents=True)
        (prefix / "copypaste").write_text("#!/bin/sh\nexit 0\n")
        (prefix / "copypaste").chmod(0o755)
        return prefix

    def test_appimage_runs_packaged_environment_setup_before_gui(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prefix = self.prefix(root)
            binary = prefix / "copypaste"
            binary.write_text("#!/bin/sh\nprintf '%s' \"$GST_PLUGIN_PATH\"\n")
            launcher = root / "AppRun"
            launcher.write_text("#!/bin/sh\nexport GST_PLUGIN_PATH=owned-packaged-plugins\nexec \"$(dirname \"$0\")/usr/lib/copypaste/copypaste\"\n")
            launcher.chmod(0o755)
            result = subprocess.run([str(driver.runtime_gui_entrypoint(prefix, "AppImage"))], check=True, capture_output=True, text=True)
            self.assertEqual(result.stdout, "owned-packaged-plugins")

    def test_appimage_rejects_missing_nonexecutable_or_symlinked_launcher(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prefix = self.prefix(root)
            launcher = root / "AppRun"
            for mode in ("missing", "nonexecutable", "symlink"):
                if mode == "nonexecutable":
                    launcher.write_bytes(b"script")
                elif mode == "symlink":
                    launcher.unlink()
                    launcher.symlink_to(prefix / "copypaste")
                with self.subTest(mode=mode), self.assertRaisesRegex(RuntimeError, "AppRun"):
                    driver.runtime_gui_entrypoint(prefix, "AppImage")

    def test_native_formats_use_only_the_installed_public_gui_link(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prefix = self.prefix(root)
            launcher = root / "usr/bin/copypaste"
            launcher.parent.mkdir(parents=True)
            launcher.symlink_to("../lib/copypaste/copypaste")
            for runtime_format in ("deb", "rpm"):
                self.assertEqual(driver.runtime_gui_entrypoint(prefix, runtime_format), launcher)
            launcher.unlink()
            launcher.symlink_to("/bin/sh")
            with self.assertRaisesRegex(RuntimeError, "installed GUI"):
                driver.runtime_gui_entrypoint(prefix, "deb")


if __name__ == "__main__":
    unittest.main()
