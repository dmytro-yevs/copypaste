import importlib.util
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "linux_native_fixture_driver", ROOT / "scripts/release/linux-native-fixture-driver.py",
)
driver = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(driver)


class LinuxNativeFixtureDriverTest(unittest.TestCase):
    def test_discovers_only_the_gui_owned_explicit_data_directory_under_xdg_data(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            proc = root / "proc"
            app_pid = "100"
            daemon_pid = "101"
            data_home = root / "data-home"
            data = data_home / "copypaste" / "runtime"
            data.mkdir(parents=True)
            children = proc / app_pid / "task" / app_pid / "children"
            children.parent.mkdir(parents=True)
            children.write_text(daemon_pid, encoding="ascii")
            daemon = proc / daemon_pid
            daemon.mkdir()
            daemon.joinpath("status").write_text(f"Name:\tcopypaste-daemon\nUid:\t{os.geteuid()}\t0\t0\t0\n", encoding="utf-8")
            daemon.joinpath("cmdline").write_bytes(
                b"/bundle/copypaste-daemon\0--data-dir\0" + str(data).encode() + b"\0",
            )
            original = driver.PROC_ROOT
            driver.PROC_ROOT = proc
            try:
                self.assertEqual(
                    driver.gui_daemon_data_dir(SimpleNamespace(pid=int(app_pid)), data_home),
                    data.resolve(),
                )
            finally:
                driver.PROC_ROOT = original

    def test_rejects_a_gui_child_data_directory_outside_isolated_xdg_data(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            proc = root / "proc"
            data_home = root / "data-home"
            data_home.mkdir()
            external = root / "external"
            external.mkdir()
            children = proc / "100" / "task" / "100" / "children"
            children.parent.mkdir(parents=True)
            children.write_text("101", encoding="ascii")
            daemon = proc / "101"
            daemon.mkdir()
            daemon.joinpath("status").write_text(f"Uid:\t{os.geteuid()}\t0\t0\t0\n", encoding="utf-8")
            daemon.joinpath("cmdline").write_bytes(
                b"/bundle/copypaste-daemon\0--data-dir\0" + str(external).encode() + b"\0",
            )
            original = driver.PROC_ROOT
            driver.PROC_ROOT = proc
            try:
                with self.assertRaisesRegex(RuntimeError, "escapes isolated XDG data"):
                    driver.gui_daemon_data_dir(SimpleNamespace(pid=100), data_home)
            finally:
                driver.PROC_ROOT = original


if __name__ == "__main__":
    unittest.main()
