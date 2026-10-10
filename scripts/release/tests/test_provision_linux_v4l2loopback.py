import importlib.util
import os
from pathlib import Path
import stat
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
            tool.write_state(path, tool.state_base(os.getuid(), os.getgid(), "prepared"))
            self.assertEqual(tool.read_state(path)["device"], "/dev/video42")
            with self.assertRaisesRegex(RuntimeError, "already exists"):
                tool.regular_state(path)
            path.write_text("{}", encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "does not describe"):
                tool.read_state(path)

    def test_appends_to_precreated_environment_without_overwriting_exports(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "github-env"
            path.write_text("UNCHANGED=value\n", encoding="utf-8")
            tool.append_environment(path, "COPYPASTE_QUALIFICATION_V4L2_DEVICE=/dev/video42\n")
            self.assertEqual(path.read_text(encoding="utf-8"), "UNCHANGED=value\nCOPYPASTE_QUALIFICATION_V4L2_DEVICE=/dev/video42\n")

    def test_rejects_symlink_and_wrong_owner_environment_targets(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.write_text("other=value\n", encoding="utf-8")
            link = root / "github-env"
            link.symlink_to(target)
            with self.assertRaisesRegex(RuntimeError, "regular file"):
                tool.append_environment(link, "owned=value\n")
        details = mock.Mock(st_mode=stat.S_IFREG, st_uid=os.getuid() + 1)
        with mock.patch.object(Path, "lstat", return_value=details):
            with self.assertRaisesRegex(RuntimeError, "owned by this job user"):
                tool.append_environment(Path("/unused"), "owned=value\n")

    def test_unwinds_a_created_module_after_environment_write_failure(self):
        state = mock.Mock()
        owned = {**tool.state_base(os.getuid(), os.getgid(), "loaded"), "live": {"major": 81, "minor": 42, "uid": os.getuid(), "gid": os.getgid(), "mode": "0660"}}
        with mock.patch.object(tool, "read_state", return_value=owned), \
             mock.patch.object(tool, "read_live_device", return_value={"major": 81, "minor": 42, "uid": os.getuid(), "gid": os.getgid(), "mode": "0660"}), \
             mock.patch.object(tool.Path, "exists", side_effect=[True, False, False]), \
             mock.patch.object(tool, "command") as command:
            tool.unwind_provision(state, True, owned["live"])
        command.assert_called_once_with(["sudo", "modprobe", "-r", "v4l2loopback"])
        state.unlink.assert_called_once_with()

    def test_does_not_unwind_a_replaced_state_file(self):
        state = Path("/unsafe-replaced-state")
        with mock.patch.object(tool, "read_state", side_effect=RuntimeError("state does not describe this job")), \
             mock.patch.object(tool, "command") as command:
            with self.assertRaisesRegex(RuntimeError, "does not describe"):
                tool.unwind_provision(state, True)
        command.assert_not_called()

    def test_rejects_foreign_live_parameters_before_module_removal(self):
        state = mock.Mock()
        owned = {**tool.state_base(os.getuid(), os.getgid(), "active"), "live": {"major": 81, "minor": 42, "uid": os.getuid(), "gid": os.getgid(), "mode": "0600"}}
        with mock.patch.object(tool, "read_state", return_value=owned), \
             mock.patch.object(tool.Path, "exists", return_value=True), \
             mock.patch.object(tool, "read_live_device", return_value={**owned["live"], "minor": 43}), \
             mock.patch.object(tool, "command") as command:
            with self.assertRaisesRegex(RuntimeError, "no longer matches"):
                tool.unwind_provision(state, True)
        command.assert_not_called()

    def test_removes_only_its_partial_state_on_write_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "state"
            with mock.patch.object(tool.os, "write", side_effect=OSError("disk full")):
                with self.assertRaises(OSError):
                    tool.write_state(path, tool.state_base(os.getuid(), os.getgid(), "pending"))
            self.assertFalse(path.exists())

    def test_active_transition_write_failure_keeps_durable_loaded_state(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "state"
            loaded = {**tool.state_base(os.getuid(), os.getgid(), "loaded"), "live": {"major": 81, "minor": 42, "uid": 0, "gid": 44, "mode": "0660"}}
            tool.write_state(path, loaded)
            active = {**tool.state_base(os.getuid(), os.getgid(), "active"), "live": {"major": 81, "minor": 42, "uid": os.getuid(), "gid": os.getgid(), "mode": "0600"}}
            with mock.patch.object(tool.os, "replace", side_effect=OSError("rename failed")):
                with self.assertRaises(OSError):
                    tool.replace_state(path, active)
            self.assertEqual(tool.read_state(path), loaded)
            self.assertFalse(path.with_name("state.next").exists())

    def test_chown_then_chmod_failure_unwinds_with_refreshed_live_ownership(self):
        root_live = {"major": 81, "minor": 42, "uid": 0, "gid": 44, "mode": "0660"}
        job_live = {**root_live, "uid": os.getuid(), "gid": os.getgid()}
        commands = []

        def run(arguments):
            commands.append(arguments)
            if arguments[:3] == ["sudo", "chmod", "600"]:
                raise RuntimeError("chmod failed")

        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory) / "state"
            environment = Path(directory) / "github-env"
            environment.write_text("OTHER=value\n", encoding="utf-8")
            with mock.patch.object(tool, "require_ci"), \
                 mock.patch.object(tool, "assert_unowned_target"), \
                 mock.patch.object(tool, "device_is_character", return_value=True), \
                 mock.patch.object(tool.platform, "release", return_value="6.8.0"), \
                 mock.patch.object(tool, "command", side_effect=run), \
                 mock.patch.object(tool, "read_live_device", side_effect=[root_live, job_live]), \
                 mock.patch.object(tool, "unwind_provision") as unwind:
                with self.assertRaisesRegex(RuntimeError, "chmod failed"):
                    tool.provision(state, environment)
            self.assertEqual(commands[-2:], [["sudo", "chown", f"{os.getuid()}:{os.getgid()}", "/dev/video42"], ["sudo", "chmod", "600", "/dev/video42"]])
            self.assertEqual(unwind.call_args.args[2], job_live)
