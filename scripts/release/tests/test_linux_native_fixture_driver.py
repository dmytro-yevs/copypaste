import importlib.util
import hashlib
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import signal
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
    def test_clean_install_requires_a_new_profile_and_rejects_existing_or_linked_state(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            profile = root / "data-home"
            with mock.patch.object(driver, "emit_result") as emit:
                driver.require_clean_install_baseline(profile, dict(os.environ))
            self.assertEqual(emit.call_count, 2)
            self.assertTrue(all(call.args[0] == "clean_install_baseline" for call in emit.call_args_list))
            for kind in ("directory", "file", "dangling_link"):
                if kind == "directory":
                    profile.mkdir()
                elif kind == "file":
                    profile.write_bytes(b"existing state")
                else:
                    profile.symlink_to(root / "missing")
                with self.subTest(kind=kind), self.assertRaises(subprocess.CalledProcessError):
                    driver.require_clean_install_baseline(profile, dict(os.environ))
                if profile.is_dir() and not profile.is_symlink():
                    profile.rmdir()
                else:
                    profile.unlink()

    def test_x11_desktop_receipt_requires_its_session_and_live_window_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "desktop.json"
            receipt = self.desktop_receipt()
            receipt.update(session="x11", window={"id": "12345", "hide_show": "dbusmenu_open"})
            path.write_text(json.dumps(receipt))
            driver.desktop_acceptance_receipt(path, desktop="GNOME", session="x11", gui_pid=4321)
            for changed in ({"session": "wayland"}, {"gui_pid": 9999},
                            {"window": {"id": "unknown", "hide_show": "dbusmenu_open"}}):
                path.write_text(json.dumps({**receipt, **changed}))
                with self.subTest(changed=changed), self.assertRaises(RuntimeError):
                    driver.desktop_acceptance_receipt(path, desktop="GNOME", session="x11", gui_pid=4321)

    def test_x11_desktop_helper_must_complete_before_tray_assertion_is_emitted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            helper = root / "helper.py"
            helper.write_text("# owned helper\n")
            args = SimpleNamespace(session="x11", desktop="GNOME", evidence_dir=root,
                                   wayland_desktop_acceptance_helper=helper)
            with mock.patch.object(driver, "desktop_acceptance_capture_command", return_value=["provider"]), \
                 mock.patch.object(driver, "run", side_effect=RuntimeError("notification absent")), \
                 mock.patch.object(driver, "emit_result") as emit:
                with self.assertRaisesRegex(RuntimeError, "notification absent"):
                    driver.require_desktop_acceptance(args, gui_pid=4321, cli=Path("cli"), environment={},
                                                      workspace=root, socket_path=root / "socket")
            emit.assert_not_called()

    def test_native_helper_error_retains_stderr_without_stdout_or_capability(self):
        capability = "a" * 64
        command = ["python3", str(driver.DESKTOP_ACCEPTANCE)]
        error = subprocess.CalledProcessError(
            1, command, output=b"protected stdout", stderr=f"capture unavailable {capability}".encode(),
        )
        with mock.patch.object(driver.subprocess, "run", side_effect=error):
            with self.assertRaisesRegex(RuntimeError, "capture unavailable") as observed:
                driver.run(command, env={"COPYPASTE_QUALIFICATION_CAPABILITY": capability})
        self.assertNotIn(capability, str(observed.exception))
        self.assertNotIn("protected stdout", str(observed.exception))

    def test_native_helper_error_bounds_stderr_and_preserves_cli_errors(self):
        command = ["python3", str(driver.EXTERNAL_FILE_DRAG)]
        error = subprocess.CalledProcessError(1, command, stderr=b"x" * (16 * 1024))
        with mock.patch.object(driver.subprocess, "run", side_effect=error):
            with self.assertRaises(RuntimeError) as observed:
                driver.run(command)
        self.assertLess(len(str(observed.exception)), 9 * 1024)
        with mock.patch.object(driver.subprocess, "run", side_effect=error):
            with self.assertRaises(subprocess.CalledProcessError):
                driver.run(["copypaste-cli", "status"])

    def test_x11_target_discovery_binds_visible_exact_title_to_owner_pid(self):
        result = subprocess.CompletedProcess([], 0, stdout=b"42\n")
        with mock.patch.object(driver.subprocess, "run", return_value=result) as run:
            self.assertEqual(driver.wait_for_window("Input Target", {}, owner_pid=123), "42")
        self.assertEqual(run.call_args.args[0], [
            "xdotool", "search", "--onlyvisible", "--all", "--pid", "123", "--name", "^Input\\ Target$",
        ])

    def test_x11_target_discovery_still_rejects_ambiguous_owned_windows(self):
        result = subprocess.CompletedProcess([], 0, stdout=b"42\n43\n")
        with mock.patch.object(driver.subprocess, "run", return_value=result):
            with self.assertRaisesRegex(RuntimeError, "not uniquely discoverable"):
                driver.wait_for_window("Input Target", {}, owner_pid=123)

    def test_ipc_omits_params_only_for_the_unit_request(self):
        class Channel:
            def __init__(self):
                self.sent = b""

            def __enter__(self):
                return self

            def __exit__(self, *_args):
                pass

            def settimeout(self, _timeout):
                pass

            def connect(self, _path):
                pass

            def sendall(self, value):
                self.sent = value

            def makefile(self, _mode):
                return io.BytesIO(b'{"ok":true,"data":{}}\n')

        unit = Channel()
        nonunit = Channel()
        with mock.patch.object(driver.socket, "socket", side_effect=[unit, nonunit]):
            driver.ipc(Path("unit.sock"), "pair_create_invite")
            driver.ipc(Path("nonunit.sock"), "pair_join", {"code": "fixture"})
        self.assertNotIn("params", json.loads(unit.sent))
        self.assertEqual(json.loads(nonunit.sent)["params"], {"code": "fixture"})

    def test_ipc_redacts_error_message_but_preserves_a_safe_error_code(self):
        class Channel:
            def __enter__(self):
                return self

            def __exit__(self, *_args):
                pass

            def settimeout(self, _timeout):
                pass

            def connect(self, _path):
                pass

            def sendall(self, _value):
                pass

            def makefile(self, _mode):
                return io.BytesIO(b'{"ok":false,"error":"malformed request","error_code":"invalid_request"}\n')

        with mock.patch.object(driver.socket, "socket", return_value=Channel()):
            with self.assertRaisesRegex(RuntimeError, "invalid_request") as error:
                driver.ipc(Path("daemon.sock"), "pair_create_invite")
        self.assertNotIn("malformed request", str(error.exception))

    def test_ipc_redacts_malicious_or_unbounded_top_level_error_codes(self):
        class Channel:
            def __init__(self, response):
                self.response = response

            def __enter__(self):
                return self

            def __exit__(self, *_args):
                pass

            def settimeout(self, _timeout):
                pass

            def connect(self, _path):
                pass

            def sendall(self, _value):
                pass

            def makefile(self, _mode):
                return io.BytesIO(self.response)

        for code in ("UPPER", "a" * 65):
            response = json.dumps({"ok": False, "error": "sensitive server detail", "error_code": code}).encode() + b"\n"
            with mock.patch.object(driver.socket, "socket", return_value=Channel(response)):
                with self.assertRaisesRegex(RuntimeError, "daemon rejected pair_create_invite") as error:
                    driver.ipc(Path("daemon.sock"), "pair_create_invite")
            self.assertNotIn(code, str(error.exception))
            self.assertNotIn("sensitive server detail", str(error.exception))

    def test_audited_clipboard_provider_requires_its_expected_sigterm_exit(self):
        class Provider:
            def __init__(self, command, **_kwargs):
                ready = Path(command[command.index("--ready-file") + 1])
                ready.write_text("ready\n", encoding="utf-8")
                self.stderr = io.BytesIO()
                self.terminated = False

            def poll(self):
                return None

            def terminate(self):
                self.terminated = True

            def wait(self, timeout):
                assert timeout == 5
                return -signal.SIGTERM

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = root / "evidence"
            evidence.mkdir()
            audit = root / "audit.jsonl"
            with mock.patch.object(driver, "gtk_provider_helper", return_value=root / "helper"), \
                 mock.patch.object(driver.subprocess, "Popen", side_effect=Provider):
                with driver.clipboard_provider(root, evidence, "privacy", {"text/plain": b"fixture"}, {}, request_audit_file=audit):
                    pass

    def test_audited_clipboard_provider_rejects_an_early_writer_failure(self):
        class Provider:
            stderr = io.BytesIO(b"audit fsync failed")

            def poll(self):
                return 1

            def terminate(self):
                raise AssertionError("an exited provider must not be terminated")

            def wait(self, timeout):
                assert timeout == 5
                return 1

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = root / "evidence"
            evidence.mkdir()
            with mock.patch.object(driver, "gtk_provider_helper", return_value=root / "helper"), \
                 mock.patch.object(driver.subprocess, "Popen", return_value=Provider()):
                with self.assertRaisesRegex(RuntimeError, "audit fsync failed"):
                    with driver.clipboard_provider(
                        root, evidence, "privacy", {"text/plain": b"fixture"}, {}, request_audit_file=root / "audit.jsonl",
                    ):
                        pass
    def test_seed_and_current_runtime_evidence_namespaces_do_not_collide(self):
        self.assertEqual(
            driver.runtime_evidence_namespace("AppImage", seed_upgrade_canary=True),
            "prior-appimage",
        )
        self.assertEqual(
            driver.runtime_evidence_namespace("AppImage", seed_upgrade_canary=False),
            "appimage",
        )

    def native_status(self, **overrides):
        value = {
            "capture_running": True,
            "clipboard_available": True,
            "clipboard_backend": "linux-wayland",
        }
        value.update(overrides)
        return value

    def test_native_clipboard_transport_requires_exact_status_booleans(self):
        for availability in (None, False, 1, "true"):
            status = self.native_status()
            if availability is None:
                status.pop("clipboard_available")
            else:
                status["clipboard_available"] = availability
            self.assertFalse(driver.native_clipboard_transport_is_ready(status))
        self.assertFalse(driver.native_clipboard_transport_is_ready(self.native_status(capture_running=1)))

    def test_native_clipboard_transport_rejects_capture_task_without_available_transport(self):
        self.assertFalse(driver.native_clipboard_transport_is_ready(self.native_status(clipboard_available=False)))

    def test_source_desktop_entry_maps_gtk_wm_class_to_the_lowercase_source_id(self):
        with tempfile.TemporaryDirectory() as directory:
            data_home = Path(directory)
            driver.write_source_desktop_entry(data_home)

            entry = data_home / "applications" / f"{driver.SOURCE_APPLICATION_ID}.desktop"
            self.assertEqual(
                entry.read_text(encoding="utf-8"),
                "[Desktop Entry]\n"
                "Type=Application\n"
                f"Name={driver.SOURCE_APPLICATION_NAME}\n"
                "StartupWMClass=Org.copypaste.QualificationSource\n"
                "Exec=true\n",
            )

    def test_startup_waits_for_delayed_native_clipboard_transport(self):
        inactive = self.native_status(clipboard_available=False)
        ready = self.native_status()
        with mock.patch.object(driver, "status", side_effect=[inactive, ready]) as status, \
             mock.patch.object(driver.time, "sleep"):
            self.assertEqual(
                driver.wait_for_startup_status("cli", {}, require_native_clipboard=True),
                ready,
            )
        self.assertEqual(status.call_count, 2)

    def test_startup_timeout_distinguishes_missing_ipc_from_inactive_transport(self):
        with mock.patch.object(driver.time, "monotonic", side_effect=[0, 0, 9]), \
             mock.patch.object(driver, "status", side_effect=RuntimeError("socket unavailable")), \
             mock.patch.object(driver.time, "sleep"):
            with self.assertRaisesRegex(RuntimeError, "IPC status did not become ready"):
                driver.wait_for_startup_status("cli", {}, require_native_clipboard=True)
        with mock.patch.object(driver.time, "monotonic", side_effect=[0, 0, 9]), \
             mock.patch.object(driver, "status", return_value=self.native_status(clipboard_available=False)), \
             mock.patch.object(driver.time, "sleep"):
            with self.assertRaisesRegex(RuntimeError, "native clipboard transport remained inactive"):
                driver.wait_for_startup_status("cli", {}, require_native_clipboard=True)

    def test_prior_seed_startup_does_not_require_new_clipboard_availability_field(self):
        prior_status = {"capture_running": False, "clipboard_backend": "linux-x11"}
        with mock.patch.object(driver, "status", return_value=prior_status) as status:
            self.assertEqual(
                driver.wait_for_startup_status("cli", {}, require_native_clipboard=False),
                prior_status,
            )
        status.assert_called_once_with("cli", {})

    def test_locator_file_capture_saves_bytes_and_copies_the_original_locator_as_text(self):
        captured = {
            "id": "captured", "content_type": "file",
            "source_app_bundle_id": driver.SOURCE_APPLICATION_ID,
            "source_app_name": driver.SOURCE_APPLICATION_NAME,
        }
        @driver.contextmanager
        def provider(*_args, **_kwargs):
            yield None
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            saved = workspace / "captured-file-roundtrip.txt"
            copied = str(workspace / "fixture.txt").encode("utf-8")
            def ipc(socket_path, method, params):
                self.assertEqual(socket_path, workspace / "gui.sock")
                self.assertEqual(method, "save_file")
                self.assertEqual(params["id"], "captured")
                self.assertEqual(params["dest_path"], str(saved))
                saved.write_bytes(b"CopyPaste qualification file payload\n")
                return {"data": {"empty": {}}}
            with mock.patch.object(driver, "clipboard_provider", provider), \
                 mock.patch.object(driver, "status", side_effect=[{"item_count": 0}, {"item_count": 1}]), \
                 mock.patch.object(driver, "list_items", side_effect=[[], [captured]]), \
                 mock.patch.object(driver, "ipc", side_effect=ipc), \
                 mock.patch.object(driver, "run", side_effect=[
                     subprocess.CompletedProcess(["copypaste-cli", "copy"], 0),
                     subprocess.CompletedProcess(["wl-paste"], 0, stdout=copied),
                 ]):
                driver.require_locator_file_capture(
                    Path("copypaste-cli"), {}, workspace, workspace / "evidence", "wayland", workspace / "gui.sock",
                )

    def test_reference_free_import_copies_a_materialized_uri_with_original_bytes(self):
        imported = {
            "id": "imported", "content_type": "file",
            "file_details": {
                "filename": "reference-free-file.txt",
                "size_bytes": len(b"CopyPaste qualification reference-free file payload\n"),
            },
        }
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            materialized = workspace / "materialized.txt"
            materialized.write_bytes(b"CopyPaste qualification reference-free file payload\n")
            def ipc(socket_path, method, params):
                self.assertEqual(socket_path, workspace / "gui.sock")
                self.assertEqual(method, "import_file")
                self.assertEqual(params["source_reference"], None)
                self.assertEqual(params["path"], str(workspace / "reference-free-file.txt"))
                return {"data": {"empty": {}}}
            with mock.patch.object(driver, "list_items", side_effect=[[{"id": "old"}], [imported]]), \
                 mock.patch.object(driver, "ipc", side_effect=ipc), \
                 mock.patch.object(driver, "run", side_effect=[
                     subprocess.CompletedProcess(["copypaste-cli", "copy"], 0),
                     subprocess.CompletedProcess(["wl-paste"], 0, stdout=materialized.as_uri().encode("utf-8")),
                 ]):
                driver.require_reference_free_file_copy(
                    Path("copypaste-cli"), {}, workspace, "wayland", workspace / "gui.sock",
                )

    def test_reference_free_file_qualification_propagates_a_product_ipc_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            with mock.patch.object(driver, "list_items", return_value=[]), \
                 mock.patch.object(driver, "ipc", side_effect=RuntimeError("daemon rejected import_file")):
                with self.assertRaisesRegex(RuntimeError, "daemon rejected import_file"):
                    driver.require_reference_free_file_copy(
                        Path("copypaste-cli"), {}, workspace, "wayland", workspace / "gui.sock",
                    )

    def test_quick_paste_readiness_requires_a_matching_child_pid_and_marker_digest(self):
        marker = "copypaste-x11-quick-paste-fixture"
        record = {
            "schema": 1,
            "kind": "atspi_quick_paste_ready",
            "gui_pid": 4321,
            "marker_sha256": hashlib.sha256(marker.encode("utf-8")).hexdigest(),
        }
        result = subprocess.CompletedProcess(
            ["python3", "actor.py"], 0,
            stdout=("COPYPASTE_QUALIFICATION_MEDIA " + json.dumps(record) + "\n").encode("utf-8"),
        )
        with mock.patch.object(driver, "run", return_value=result):
            driver.require_quick_paste_ready(Path("actor.py"), 4321, marker, 3, {})
        record["gui_pid"] = 4322
        result = subprocess.CompletedProcess(
            ["python3", "actor.py"], 0,
            stdout=("COPYPASTE_QUALIFICATION_MEDIA " + json.dumps(record) + "\n").encode("utf-8"),
        )
        with mock.patch.object(driver, "run", return_value=result):
            with self.assertRaisesRegex(RuntimeError, "readiness evidence"):
                driver.require_quick_paste_ready(Path("actor.py"), 4321, marker, 3, {})

    def test_x11_quick_paste_window_pid_rejects_an_invalid_owner(self):
        with mock.patch.object(
            driver, "run", return_value=subprocess.CompletedProcess(["xdotool"], 0, stdout=b"not-a-pid\n"),
        ):
            with self.assertRaisesRegex(RuntimeError, "window owner is invalid"):
                driver.x11_window_pid("42", {})

    def test_x11_quick_paste_uses_observed_focus_for_a_normal_return_key(self):
        events = []
        @driver.contextmanager
        def input_target(_workspace, _environment):
            events.append("target_focus")
            with tempfile.TemporaryDirectory() as directory:
                result = Path(directory) / "result"
                result.write_text("copypaste-x11-quick-paste-fixture", encoding="utf-8")
                yield "100", result

        calls = []
        def run(argv, **_kwargs):
            calls.append(argv)
            if argv == ["xdotool", "mousemove", "--sync", "16", "16"]:
                events.append("pointer_park")
            if argv == ["xdotool", "key", "ctrl+shift+c"]:
                events.append("hotkey")
            if argv == ["xdotool", "getwindowfocus"]:
                value = b"100\n" if calls.count(argv) != 2 else b"200\n"
                return subprocess.CompletedProcess(argv, 0, stdout=value)
            return subprocess.CompletedProcess(argv, 0)

        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            with mock.patch.object(driver, "x11_input_target", input_target), \
                 mock.patch.object(driver, "wait_for_window", return_value="200"), \
                 mock.patch.object(driver, "x11_window_pid", return_value=4321), \
                 mock.patch.object(driver, "require_helper", return_value=Path("actor.py")), \
                 mock.patch.object(driver, "list_items", return_value=[{
                     "id": "marker-id", "content": "copypaste-x11-quick-paste-fixture",
                 }]), \
                 mock.patch.object(driver, "require_quick_paste_ready") as ready, \
                 mock.patch.object(driver, "run", side_effect=run):
                driver.require_x11_quick_paste(Path("copypaste-cli"), {}, workspace)
        ready.assert_called_once()
        self.assertIn(["xdotool", "key", "Return"], calls)
        self.assertLess(calls.index(["copypaste-cli", "--json", "copy", "marker-id"]),
                        calls.index(["xdotool", "key", "ctrl+shift+c"]))
        self.assertFalse(any(command[:3] == ["xdotool", "key", "--window"] for command in calls))
        self.assertLess(events.index("pointer_park"), events.index("target_focus"))
        self.assertLess(events.index("target_focus"), events.index("hotkey"))
        self.assertNotIn("pointer_park", events[events.index("hotkey") + 1:])

    def test_capture_requires_cli_copy_to_restore_the_original_after_a_real_overwrite(self):
        original = b"original HTML"
        marker = b"copypaste-native-overwrite-clipboard_html"
        captured = {
            "id": "original", "content_type": "text",
            "content": original.decode("utf-8"),
            "source_app_bundle_id": driver.SOURCE_APPLICATION_ID,
            "source_app_name": driver.SOURCE_APPLICATION_NAME,
        }
        overwritten = {
            "id": "marker", "content_type": "text", "content": marker.decode("utf-8"),
            "source_app_bundle_id": driver.SOURCE_APPLICATION_ID,
            "source_app_name": driver.SOURCE_APPLICATION_NAME,
        }
        labels = []
        @driver.contextmanager
        def provider(_workspace, _evidence, label, payloads, _environment):
            labels.append((label, payloads))
            yield None
        copied = False
        def run(argv, **_kwargs):
            nonlocal copied
            if argv[0] == "copypaste-cli":
                copied = True
                return subprocess.CompletedProcess(argv, 0)
            if argv[-1] == "text/plain;charset=utf-8":
                return subprocess.CompletedProcess(argv, 0, stdout=marker)
            self.assertEqual(argv[-1], "text/html")
            return subprocess.CompletedProcess(argv, 0, stdout=original if copied else marker)
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            with mock.patch.object(driver, "clipboard_provider", provider), \
                 mock.patch.object(driver, "status", side_effect=[
                     {"item_count": 0}, {"item_count": 1}, {"item_count": 1}, {"item_count": 2},
                 ]), \
                 mock.patch.object(driver, "list_items", side_effect=[[], [captured], [captured], [overwritten, captured]]), \
                 mock.patch.object(driver, "run", side_effect=run):
                driver.require_capture(
                    Path("copypaste-cli"), {}, workspace, workspace / "evidence", "wayland",
                    "text/html", original, "text", "clipboard_html",
                )
        self.assertEqual(
            labels,
            [
                ("clipboard_html", {"text/html": original}),
                ("clipboard_html-overwrite", {"text/plain;charset=utf-8": marker}),
            ],
        )

    def test_capture_rejects_a_no_op_copy_that_leaves_the_overwrite_marker_active(self):
        marker = b"copypaste-native-overwrite-clipboard_png"
        captured = {
            "id": "original", "content_type": "image/png",
            "source_app_bundle_id": driver.SOURCE_APPLICATION_ID,
            "source_app_name": driver.SOURCE_APPLICATION_NAME,
        }
        overwritten = {
            "id": "marker", "content_type": "text", "content": marker.decode("utf-8"),
            "source_app_bundle_id": driver.SOURCE_APPLICATION_ID,
            "source_app_name": driver.SOURCE_APPLICATION_NAME,
        }
        @driver.contextmanager
        def provider(*_args, **_kwargs):
            yield None
        def run(argv, **_kwargs):
            if argv[0] == "copypaste-cli":
                return subprocess.CompletedProcess(argv, 0)
            return subprocess.CompletedProcess(argv, 0, stdout=marker)
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            with mock.patch.object(driver, "clipboard_provider", provider), \
                 mock.patch.object(driver, "status", side_effect=[
                     {"item_count": 0}, {"item_count": 1}, {"item_count": 1}, {"item_count": 2},
                 ]), \
                 mock.patch.object(driver, "list_items", side_effect=[[], [captured], [captured], [overwritten, captured]]), \
                 mock.patch.object(driver, "run", side_effect=run):
                with self.assertRaisesRegex(RuntimeError, "did not preserve image/png bytes"):
                    driver.require_capture(
                        Path("copypaste-cli"), {}, workspace, workspace / "evidence", "wayland",
                        "image/png", b"expected PNG bytes", "image/png", "clipboard_png",
                    )

    def wayland_inputs(self, **overrides):
        values = {
            "wayland_app_permission_target": "Allow keyboard control",
            "session": "wayland",
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
                "quick_paste_pid": 4322,
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
                "quick_paste_ready_sha256": "d" * 64,
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
            "tray": {"service": "org.kde.StatusNotifierItem-4321-1", "menu_path": "/StatusNotifierItem/Menu", "open_item_id": 1},
            "window": {"observer": "compositor_dbus_signal", "hide_show": "dbusmenu_open"},
            "notification": {"server_reply_id": 1, "capture_command_sha256": "f" * 64},
            "history": {"new_item_count": 2},
            "commands": [
                {"argv": ["busctl", "--user", "call", "org.kde.StatusNotifierItem-4321-1",
                          "/StatusNotifierItem/Menu", "com.canonical.dbusmenu", "Event", "isvu",
                          "1", "clicked", "s", "", "0"], "returncode": 0},
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
        receipt["actors"]["quick_paste_pid"] = receipt["actors"]["input_target_pid"]
        with self.assertRaisesRegex(RuntimeError, "not separate"):
            driver.wayland_quick_paste_receipt(
                receipt, gui_pid=4321, inputs=inputs, marker="copypaste-wayland-quick-paste-fixture",
            )
        receipt = self.wayland_receipt()
        receipt["atspi"].pop("quick_paste_ready_sha256")
        with self.assertRaisesRegex(RuntimeError, "media evidence"):
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
                driver.desktop_acceptance_receipt(path, desktop="GNOME", session="wayland", gui_pid=4321)

    def test_wayland_desktop_receipt_requires_the_exported_menu_object_path(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "linux-desktop-acceptance.json"
            receipt = self.desktop_receipt()
            for menu_path in ([0, 1], "/", "/OtherMenu"):
                receipt["tray"]["menu_path"] = menu_path
                path.write_text(json.dumps(receipt), encoding="utf-8")
                with self.assertRaisesRegex(RuntimeError, "incomplete"):
                    driver.desktop_acceptance_receipt(path, desktop="GNOME", session="wayland", gui_pid=4321)

    def test_wayland_desktop_receipt_binds_the_real_menu_event_arguments(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "linux-desktop-acceptance.json"
            receipt = self.desktop_receipt()
            path.write_text(json.dumps(receipt), encoding="utf-8")
            driver.desktop_acceptance_receipt(path, desktop="GNOME", session="wayland", gui_pid=4321)
            receipt["commands"][0]["argv"][-2] = "unexpected-payload"
            path.write_text(json.dumps(receipt), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "command evidence"):
                driver.desktop_acceptance_receipt(path, desktop="GNOME", session="wayland", gui_pid=4321)

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
            socket = root / "socket"
            seed_result = subprocess.CompletedProcess([str(root / "copypaste-cli"), "add", "copypaste-wayland-quick-paste-fixture"], 0)
            copy_result = subprocess.CompletedProcess([str(root / "copypaste-cli"), "--json", "copy", "marker-id"], 0)
            with mock.patch.object(driver, "desktop_acceptance_capture_command", return_value=["python3", "provider"]), \
                 mock.patch.object(driver, "list_items", return_value=[{"id": "marker-id", "content": "copypaste-wayland-quick-paste-fixture"}]), \
                 mock.patch.object(driver, "run", side_effect=[tray_result, seed_result, copy_result, quick_result]) as run:
                driver.require_wayland_qualification(
                    args, gui_pid=4321, cli=root / "copypaste-cli", environment={"INHERITED": "value"}, workspace=root, socket_path=socket,
                )
            seed = run.call_args_list[1]
            self.assertEqual(seed.args[0], [str(root / "copypaste-cli"), "add", "copypaste-wayland-quick-paste-fixture"])
            self.assertEqual(seed.kwargs["env"], {"INHERITED": "value", "COPYPASTE_SOCKET": str(socket)})
            self.assertEqual(run.call_args_list[2].args[0], [str(root / "copypaste-cli"), "--json", "copy", "marker-id"])
            command = run.call_args_list[3].args[0]
            self.assertEqual(command[command.index("--app-permission-target") + 1], "Allow keyboard control")
            self.assertFalse(any(argument.startswith("--portal-dialog") or argument.startswith("--app-portal-dialog") for argument in command))
            written = json.loads((evidence / "wayland-quick-paste.json").read_text(encoding="utf-8"))
            self.assertEqual(written["actors"]["input_target_pid"], 4324)
            self.assertEqual(set(written["atspi"]), {"portal_click_sha256", "quick_paste_ready_sha256"})

    def test_wayland_seed_failure_prevents_the_quick_paste_helper_and_receipt(self):
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
            cli = root / "copypaste-cli"
            tray_result = subprocess.CompletedProcess(["python3", str(tray)], 0, stdout=b"")
            seed_failure = subprocess.CalledProcessError(1, [str(cli), "add", "copypaste-wayland-quick-paste-fixture"])
            with mock.patch.object(driver, "desktop_acceptance_capture_command", return_value=["python3", "provider"]), \
                 mock.patch.object(driver, "run", side_effect=[tray_result, seed_failure]) as run, \
                 mock.patch.object(driver, "wayland_quick_paste_receipt") as receipt:
                with self.assertRaises(subprocess.CalledProcessError):
                    driver.require_wayland_qualification(
                        args, gui_pid=4321, cli=cli, environment={}, workspace=root, socket_path=root / "socket",
                    )
            self.assertEqual(run.call_count, 2)
            receipt.assert_not_called()

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
            with mock.patch.object(driver, "run", side_effect=[probe, *([result] * 8)]) as run:
                driver.require_linux_onboarding(args, gui_pid=4321, environment={})
            commands = [call.args[0] for call in run.call_args_list[1:]]
            self.assertEqual([command[2] for command in commands], ["observe", "click", "observe", "observe", "click", "observe", "click", "observe"])
            self.assertEqual(
                [command[command.index("--target") + 1] for command in commands],
                ["Continue", "Continue", "Set up your Linux desktop", "Quick Paste is ready.", "Continue", "Get started", "Get started", "History"],
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

    def test_desktop_sessions_enable_the_atspi_status_property_after_bus_activation(self):
        for name in ("run-linux-native-desktop-session.sh", "run-fedora-plasma6-session.sh"):
            session = (ROOT / "scripts/release" / name).read_text(encoding="utf-8")
            self.assertIn("org.a11y.Bus.GetAddress", session)
            self.assertIn("set-property org.a11y.Bus /org/a11y/bus org.a11y.Status IsEnabled b true", session)
            self.assertIn("get-property org.a11y.Bus /org/a11y/bus org.a11y.Status IsEnabled", session)
            self.assertNotIn("--method org.a11y.Status.IsEnabled", session)

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

    def test_completed_wayland_profile_opens_the_quick_paste_category_before_permission_probe(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            actor = root / "actor.py"
            actor.write_text("# actor\n", encoding="utf-8")
            args = SimpleNamespace(session="wayland", wayland_actor=actor, wayland_quick_paste_helper=root / "quick-paste.py")
            def probe(target, present):
                return subprocess.CompletedProcess(
                    ["python3", str(actor)], 0,
                    stdout=("COPYPASTE_QUALIFICATION_MEDIA " + json.dumps({
                        "kind": "atspi_probe", "gui_pid": 4321,
                        "target_name_sha256": hashlib.sha256(target.encode()).hexdigest(), "present": present,
                    }) + "\n").encode(),
                )
            result = subprocess.CompletedProcess(["python3", str(actor)], 0, stdout=b"")
            with mock.patch.object(driver, "run", side_effect=[probe("History", True), result, result, result, result, probe("Allow keyboard control", False), result, result]) as run:
                driver.require_linux_onboarding(args, gui_pid=4321, environment={})
            commands = [call.args[0] for call in run.call_args_list[1:]]
            self.assertEqual(
                [command[command.index("--target") + 1] for command in commands],
                ["Settings", "Quick Paste", "Quick Paste", "Open clipboard history from anywhere without switching windows.", "Allow keyboard control", "History", "History"],
            )

    def test_discovers_only_the_gui_owned_explicit_data_directory_under_xdg_data(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            proc = root / "proc"
            app_pid = "100"
            daemon_pid = "101"
            data_home = root / "data-home"
            data = data_home / "copypaste" / "runtime"
            data.mkdir(parents=True)
            gui = proc / app_pid
            gui.mkdir(parents=True)
            gui.joinpath("status").write_text(f"Uid:\t{os.geteuid()}\t0\t0\t0\n", encoding="utf-8")
            main_children = gui / "task" / app_pid / "children"
            main_children.parent.mkdir(parents=True)
            main_children.write_text("", encoding="ascii")
            worker_children = gui / "task" / "102" / "children"
            worker_children.parent.mkdir(parents=True)
            worker_children.write_text(daemon_pid, encoding="ascii")
            daemon = proc / daemon_pid
            daemon.mkdir()
            daemon.joinpath("status").write_text(
                f"Name:\tcopypaste-daemon\nPPid:\t{app_pid}\nUid:\t{os.geteuid()}\t0\t0\t0\n", encoding="utf-8",
            )
            daemon.joinpath("cmdline").write_bytes(
                b"/bundle/copypaste-daemon\0--data-dir\0" + str(data).encode() + b"\0",
            )
            original = driver.PROC_ROOT
            driver.PROC_ROOT = proc
            try:
                daemon = driver.gui_owned_daemon(SimpleNamespace(pid=int(app_pid)), data_home)
                self.assertEqual(daemon.pid, int(daemon_pid))
                self.assertEqual(daemon.data_dir, data.resolve())
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
            gui = proc / "100"
            gui.mkdir(parents=True)
            gui.joinpath("status").write_text(f"Uid:\t{os.geteuid()}\t0\t0\t0\n", encoding="utf-8")
            children = gui / "task" / "100" / "children"
            children.parent.mkdir(parents=True)
            children.write_text("101", encoding="ascii")
            daemon = proc / "101"
            daemon.mkdir()
            daemon.joinpath("status").write_text(f"PPid:\t100\nUid:\t{os.geteuid()}\t0\t0\t0\n", encoding="utf-8")
            daemon.joinpath("cmdline").write_bytes(
                b"/bundle/copypaste-daemon\0--data-dir\0" + str(external).encode() + b"\0",
            )
            original = driver.PROC_ROOT
            driver.PROC_ROOT = proc
            try:
                with self.assertRaisesRegex(RuntimeError, "escapes isolated XDG data"):
                    driver.gui_owned_daemon(SimpleNamespace(pid=100), data_home)
            finally:
                driver.PROC_ROOT = original

    def test_rejects_adopted_foreign_uid_and_ambiguous_gui_daemons(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            proc = root / "proc"
            data_home = root / "data-home"
            data = data_home / "copypaste"
            data.mkdir(parents=True)
            gui = proc / "100"
            gui.mkdir(parents=True)
            gui.joinpath("status").write_text(f"Uid:\t{os.geteuid()}\t0\t0\t0\n", encoding="utf-8")
            children = gui / "task" / "101" / "children"
            children.parent.mkdir(parents=True)

            def daemon(pid, *, ppid, uid):
                process = proc / str(pid)
                process.mkdir()
                process.joinpath("status").write_text(f"PPid:\t{ppid}\nUid:\t{uid}\t0\t0\t0\n", encoding="utf-8")
                process.joinpath("cmdline").write_bytes(
                    b"/bundle/copypaste-daemon\0--data-dir\0" + str(data).encode() + b"\0",
                )

            daemon(102, ppid=999, uid=os.geteuid())
            daemon(103, ppid=100, uid=os.geteuid() + 1)
            children.write_text("102 103", encoding="ascii")
            original = driver.PROC_ROOT
            driver.PROC_ROOT = proc
            try:
                with self.assertRaisesRegex(RuntimeError, "exactly one daemon"):
                    driver.gui_owned_daemon(SimpleNamespace(pid=100), data_home)
                daemon(104, ppid=100, uid=os.geteuid())
                daemon(105, ppid=100, uid=os.geteuid())
                children.write_text("104 105", encoding="ascii")
                with self.assertRaisesRegex(RuntimeError, "exactly one daemon"):
                    driver.gui_owned_daemon(SimpleNamespace(pid=100), data_home)
            finally:
                driver.PROC_ROOT = original

    def test_confidential_reader_audit_binds_the_hint_to_the_owned_daemon_only(self):
        daemon_pid = 4321
        records = [
            {"target": "x-kde-passwordManagerHint", "requestor_pid": daemon_pid, "error": None},
            {"target": "text/plain;charset=utf-8", "requestor_pid": 9876, "error": None},
            {"target": "TARGETS", "requestor_pid": 9876, "error": None},
        ]
        driver.require_confidential_reader_audit(
            records, daemon_pid=daemon_pid, protected_mime="text/plain;charset=utf-8",
        )
        with self.assertRaisesRegex(RuntimeError, "did not inspect"):
            driver.require_confidential_reader_audit(
                records[1:], daemon_pid=daemon_pid, protected_mime="text/plain;charset=utf-8",
            )

    def test_confidential_reader_audit_rejects_daemon_plaintext_unknown_and_multiple(self):
        daemon_pid = 4321
        hint = {"target": "x-kde-passwordManagerHint", "requestor_pid": daemon_pid, "error": None}
        for record, message in (
            ({"target": "text/plain;charset=utf-8", "requestor_pid": daemon_pid, "error": None}, "read protected"),
            ({"target": "MULTIPLE", "requestor_pid": daemon_pid, "error": None}, "read protected"),
        ):
            with self.assertRaisesRegex(RuntimeError, message):
                driver.require_confidential_reader_audit(
                    [hint, record], daemon_pid=daemon_pid, protected_mime="text/plain;charset=utf-8",
                )
        with tempfile.TemporaryDirectory() as directory:
            audit = Path(directory) / "audit.jsonl"
            audit.write_text(json.dumps({"target": "unknown", "requestor_pid": None, "error": "x_target_query_failed"}) + "\n", encoding="utf-8")
            records = driver.read_request_audit(audit)
            driver.write_reader_audit(Path(directory), "privacy", records, daemon_pid=daemon_pid, protocol="x11-xres")
            written = json.loads((Path(directory) / "linux-clipboard-reader-privacy.json").read_text(encoding="utf-8"))
            self.assertEqual(written["expected_daemon_pid"], daemon_pid)
            self.assertEqual(written["protocol"], "x11-xres")
            with self.assertRaisesRegex(RuntimeError, "unknown reader"):
                driver.require_confidential_reader_audit(
                    records, daemon_pid=daemon_pid, protected_mime="text/plain;charset=utf-8",
                )

    def test_pairing_uri_uses_the_live_peer_invitation_without_retaining_it(self):
        uri = driver.pairing_uri_from_invite({
            "code": "ABCDEFGH", "listen_addr": "192.0.2.1:47654", "pairing_id": "pairing-id",
        })
        self.assertTrue(uri.startswith("copypaste://pair/v2?"))
        self.assertIn("address=192.0.2.1%3A47654", uri)

    def test_pairing_uri_rejects_a_peer_invitation_without_identity(self):
        with self.assertRaisesRegex(RuntimeError, "invalid"):
            driver.pairing_uri_from_invite({
                "code": "ABCDEFGH", "listen_addr": "192.0.2.1:47654", "pairing_id": "",
            })

    def test_pairing_uri_rejects_non_short_peer_code(self):
        with self.assertRaisesRegex(RuntimeError, "short-code"):
            driver.pairing_uri_from_invite({
                "code": "secret-token", "listen_addr": "192.0.2.1:47654", "pairing_id": "pairing-id",
            })

    def test_camera_pairing_uri_is_owner_only_and_removed_as_a_regular_file(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "pairing-uri"
            driver.write_private_pairing_uri(path, "copypaste://pair/v1?code=secret")
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
            driver.remove_private_regular(path)
            self.assertFalse(path.exists())


if __name__ == "__main__":
    unittest.main()
