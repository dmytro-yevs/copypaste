import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("provision_linux_v4l2", ROOT / "scripts/release/provision-linux-v4l2loopback.py")
tool = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(tool)


class ProvisionLinuxV4L2Test(unittest.TestCase):
    def test_requires_headless_ci(self):
        with mock.patch.dict(os.environ, {"CI": "", "COPYPASTE_QUALIFICATION_HEADLESS": ""}, clear=False):
            with self.assertRaisesRegex(RuntimeError, "headless CI"):
                tool.require_ci()

    def test_refuses_preexisting_device_or_module(self):
        with mock.patch.object(Path, "exists", return_value=True):
            with self.assertRaisesRegex(RuntimeError, "already in use"):
                tool.assert_unowned_target()
        with mock.patch.object(Path, "exists", side_effect=[False, False, True]):
            with self.assertRaisesRegex(RuntimeError, "already loaded"):
                tool.assert_unowned_target()

    def test_state_is_exclusive_and_exactly_job_owned(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "v4l2.json"
            tool.write_state(path, os.getuid(), os.getgid())
            self.assertEqual(tool.read_state(path)["device"], "/dev/video42")
            with self.assertRaisesRegex(RuntimeError, "already exists"):
                tool.regular_state(path)
            path.write_text("{}", encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "does not describe"):
                tool.read_state(path)
