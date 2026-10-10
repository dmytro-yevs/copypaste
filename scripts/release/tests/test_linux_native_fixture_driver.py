import importlib.util
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "linux_native_fixture_driver", ROOT / "scripts/release/linux-native-fixture-driver.py",
)
driver = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(driver)


class LinuxNativeFixtureDriverTest(unittest.TestCase):
    def wayland_inputs(self, **overrides):
        values = {
            "wayland_app_permission_target": "Allow keyboard control",
        }
        values.update(overrides)
        return SimpleNamespace(**values)

    def wayland_receipt(self, **overrides):
        value = {
            "schema": 1,
            "kind": "wayland_quick_paste",
            "actors": {
                "gui_pid": 4321,
                "portal_dialog_pid": 4323,
                "input_target_pid": 4324,
            },
            "target": {
                "pid": 4324,
                "title_sha256": hashlib.sha256(driver.WAYLAND_INPUT_TARGET_TITLE.encode("utf-8")).hexdigest(),
                "marker_sha256": hashlib.sha256(b"copypaste-wayland-quick-paste-fixture").hexdigest(),
            },
            "portal": {"input_calls": 8, "session_sha256": "b" * 64},
            "atspi": {
                "portal_click_sha256": "e" * 64,
            },
        }
        value.update(overrides)
        return value

    def desktop_receipt(self):
        return {
            "schema": 1,
            "desktop": "GNOME",
            "session": "wayland",
            "gui_pid": 4321,
            "tray": {"service": "org.kde.StatusNotifierItem-4321-1", "menu_path": [0, 1], "open_item_id": 1},
            "window": {"observer": "compositor_dbus_signal", "hide_show": "dbusmenu_open"},
            "notification": {"server_reply_id": 1, "capture_command_sha256": "f" * 64},
            "history": {"new_item_count": 2},
            "commands": [
                {"argv": ["busctl", "call"], "returncode": 0},
                {"argv_sha256": "0" * 64, "returncode": 0},
            ],
        }

    def test_wayland_inputs_only_accept_the_installed_product_control(self):
        self.assertEqual(
            driver.wayland_quick_paste_inputs(self.wayland_inputs()),
            {"app_permission_target": "Allow keyboard control"},
        )
        with self.assertRaisesRegex(RuntimeError, "app permission target is invalid"):
            driver.wayland_quick_paste_inputs(self.wayland_inputs(wayland_app_permission_target=""))

    def test_wayland_quick_paste_receipt_binds_every_actor_and_media_proof(self):
        inputs = driver.wayland_quick_paste_inputs(self.wayland_inputs())
        receipt = self.wayland_receipt()
        self.assertEqual(
            driver.wayland_quick_paste_receipt(
                receipt, gui_pid=4321, inputs=inputs, marker="copypaste-wayland-quick-paste-fixture",
            ),
            receipt,
        )
        receipt["actors"]["gui_pid"] = 9876
        with self.assertRaisesRegex(RuntimeError, "does not match"):
            driver.wayland_quick_paste_receipt(
                receipt, gui_pid=4321, inputs=inputs, marker="copypaste-wayland-quick-paste-fixture",
            )
        receipt = self.wayland_receipt()
        receipt["atspi"].pop("portal_click_sha256")
        with self.assertRaisesRegex(RuntimeError, "media evidence"):
            driver.wayland_quick_paste_receipt(
                receipt, gui_pid=4321, inputs=inputs, marker="copypaste-wayland-quick-paste-fixture",
            )

    def test_wayland_helper_receipt_rejects_missing_or_unstructured_evidence(self):
        with self.assertRaisesRegex(RuntimeError, "exactly one receipt"):
            driver.wayland_helper_receipt("")
        with self.assertRaisesRegex(RuntimeError, "unstructured"):
            driver.wayland_helper_receipt("unexpected output\n")

    def test_wayland_desktop_receipt_rejects_missing_authenticated_notification_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "linux-desktop-acceptance.json"
            receipt = self.desktop_receipt()
            receipt["notification"].pop("capture_command_sha256")
            path.write_text(json.dumps(receipt), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "capture command digest"):
                driver.wayland_desktop_receipt(path, desktop="GNOME", gui_pid=4321)

    def test_wayland_wiring_resolves_portal_actors_at_runtime(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = root / "evidence"
            evidence.mkdir()
            (evidence / "linux-desktop-acceptance.json").write_text(json.dumps(self.desktop_receipt()), encoding="utf-8")
            tray = root / "tray.py"
            quick_paste = root / "quick-paste.py"
            actor = root / "actor.py"
            for helper in (tray, quick_paste, actor):
                helper.write_text("# helper\n", encoding="utf-8")
            args = self.wayland_inputs()
            args.desktop = "GNOME"
            args.evidence_dir = evidence
            args.wayland_desktop_acceptance_helper = tray
            args.wayland_quick_paste_helper = quick_paste
            args.wayland_actor = actor
            tray_result = subprocess.CompletedProcess(["python3", str(tray)], 0, stdout=b"")
            quick_result = subprocess.CompletedProcess(
                ["python3", str(quick_paste)], 0,
                stdout=("COPYPASTE_QUALIFICATION_WAYLAND " + json.dumps(self.wayland_receipt()) + "\n").encode("utf-8"),
            )
            with mock.patch.object(driver, "desktop_acceptance_capture_command", return_value=["python3", "provider"]), \
                 mock.patch.object(driver, "run", side_effect=[tray_result, quick_result]) as run:
                driver.require_wayland_qualification(
                    args, gui_pid=4321, cli=root / "copypaste-cli", environment={}, workspace=root, socket_path=root / "socket",
                )
            command = run.call_args_list[1].args[0]
            self.assertEqual(command[command.index("--app-permission-target") + 1], "Allow keyboard control")
            self.assertFalse(any(argument.startswith("--portal-dialog") or argument.startswith("--app-portal-dialog") for argument in command))
            written = json.loads((evidence / "wayland-quick-paste.json").read_text(encoding="utf-8"))
            self.assertEqual(written["actors"]["input_target_pid"], 4324)
            self.assertEqual(set(written["atspi"]), {"portal_click_sha256"})

    def test_linux_onboarding_drives_the_installed_x11_flow_without_seeding_preferences(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            actor = root / "actor.py"
            actor.write_text("# actor\n", encoding="utf-8")
            args = SimpleNamespace(session="x11", wayland_actor=actor)
            probe = subprocess.CompletedProcess(
                ["python3", str(actor)], 0,
                stdout=("COPYPASTE_QUALIFICATION_MEDIA " + json.dumps({
                    "kind": "atspi_probe", "gui_pid": 4321,
                    "target_name_sha256": hashlib.sha256(b"History").hexdigest(), "present": False,
                }) + "\n").encode(),
            )
            result = subprocess.CompletedProcess(["python3", str(actor)], 0, stdout=b"")
            with mock.patch.object(driver, "run", side_effect=[probe, *([result] * 7)]) as run:
                driver.require_linux_onboarding(args, gui_pid=4321, environment={})
            commands = [call.args[0] for call in run.call_args_list[1:]]
            self.assertEqual([command[2] for command in commands], ["click", "observe", "observe", "click", "observe", "click", "observe"])
            self.assertEqual(
                [command[command.index("--target") + 1] for command in commands],
                ["Continue", "Set up your Linux desktop", "Quick Paste is ready.", "Continue", "Get started", "Get started", "History"],
            )

    def test_workflow_producer_needs_no_external_wayland_consent_metadata(self):
        workflow = (ROOT / ".github/workflows/linux-native-qualification.yml").read_text(encoding="utf-8")
        producer = (ROOT / "scripts/release/produce-linux-native-qualification.py").read_text(encoding="utf-8")
        fixture = (ROOT / "scripts/release/linux-native-fixture-driver.py").read_text(encoding="utf-8")
        self.assertIn("produce-linux-native-qualification.py", workflow)
        self.assertIn('"--session", args.session', producer)
        self.assertNotIn("--wayland-app-portal-dialog-pid", workflow)
        self.assertNotIn("--wayland-portal-dialog-pid", producer)
        self.assertIn('default="Allow keyboard control"', fixture)

    def test_completed_profile_is_recognized_after_the_final_gui_restart(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            actor = root / "actor.py"
            actor.write_text("# actor\n", encoding="utf-8")
            args = SimpleNamespace(session="x11", wayland_actor=actor)
            probe = subprocess.CompletedProcess(
                ["python3", str(actor)], 0,
                stdout=("COPYPASTE_QUALIFICATION_MEDIA " + json.dumps({
                    "kind": "atspi_probe", "gui_pid": 4321,
                    "target_name_sha256": hashlib.sha256(b"History").hexdigest(), "present": True,
                }) + "\n").encode(),
            )
            with mock.patch.object(driver, "run", return_value=probe) as run:
                driver.require_linux_onboarding(args, gui_pid=4321, environment={})
            self.assertEqual(run.call_count, 1)
        fixture = (ROOT / "scripts/release/linux-native-fixture-driver.py").read_text(encoding="utf-8")
        final_onboarding = fixture.rindex("require_linux_onboarding(args, gui_pid=app.pid")
        self.assertLess(fixture.index("runtime_socket = require_modules("), final_onboarding)
        self.assertLess(final_onboarding, fixture.rindex("require_pairing_sync("))

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
