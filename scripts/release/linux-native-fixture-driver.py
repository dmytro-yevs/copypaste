#!/usr/bin/env python3
"""Exercise an exact Linux AppImage with real clipboard providers and windows.

This is deliberately a test driver, not a success-fixture reader.  It starts
the packaged daemon and Flutter executable, creates a separate GTK source
window, writes real X11/Wayland clipboard targets, and observes the packaged
CLI.  Product paths that have no observable native contract fail the scenario.
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile
import time
import socket
import stat
import hashlib
import importlib.util
import re
import signal
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import unquote, urlencode, urlparse


PROVIDER = Path(__file__).with_name("linux-clipboard-provider.py")
X11_INPUT_TARGET = Path(__file__).with_name("linux-x11-input-target.c")
MODULE_QUALIFICATION = Path(__file__).with_name("linux-module-qualification.py")
DESKTOP_ACCEPTANCE = Path(__file__).with_name("linux-desktop-acceptance.py")
WAYLAND_QUICK_PASTE = Path(__file__).with_name("linux-wayland-quick-paste.py")
NATIVE_MEDIA = Path(__file__).with_name("linux-native-media.py")
EXTERNAL_FILE_DRAG = Path(__file__).with_name("linux-native-file-drag.py")
SOURCE_APPLICATION_ID = "org.copypaste.QualificationSource"
SOURCE_APPLICATION_NAME = "CopyPaste Qualification Source"
CAPTURE_TIMEOUT_SECONDS = 8
UPGRADE_CANARY = "copypaste-upgrade-canary"
PROC_ROOT = Path("/proc")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
WAYLAND_INPUT_TARGET_TITLE = "CopyPaste Qualification Wayland Input Target"
MEDIA_PREFIX = "COPYPASTE_QUALIFICATION_MEDIA "
GNOME_CLIPBOARD_READ_OBSERVER = Path(__file__).with_name("linux-gnome-clipboard-read-observer.py")


@dataclass(frozen=True)
class OwnedDaemon:
    pid: int
    data_dir: Path


def module_qualification_helper():
    spec = importlib.util.spec_from_file_location("linux_module_qualification", MODULE_QUALIFICATION)
    if spec is None or spec.loader is None:
        raise RuntimeError("Linux module qualification helper is unavailable")
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    return helper


def run(argv, *, input_bytes=None, env=None, timeout=15):
    try:
        return subprocess.run(argv, input=input_bytes, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                              check=True, timeout=timeout, env=env)
    except subprocess.CalledProcessError as error:
        helpers = {PROVIDER, DESKTOP_ACCEPTANCE, WAYLAND_QUICK_PASTE, NATIVE_MEDIA, EXTERNAL_FILE_DRAG}
        helper = Path(argv[1]) if len(argv) > 1 else None
        if helper not in helpers:
            raise
        diagnostic = error.stderr or b""
        capability = (os.environ if env is None else env).get("COPYPASTE_QUALIFICATION_CAPABILITY")
        if capability:
            diagnostic = diagnostic.replace(capability.encode("utf-8"), b"[redacted]")
        message = diagnostic[:8 * 1024].decode("utf-8", "replace").strip()
        raise RuntimeError(f"{helper.name} exited {error.returncode}: {message}") from error


def emit_result(assertion, result):
    argv = result.args if isinstance(result.args, list) else [result.args]
    assertions = [assertion] if isinstance(assertion, str) else assertion
    print("COPYPASTE_QUALIFICATION_COMMAND " + json.dumps({
        "argv": [str(value) for value in argv], "returncode": result.returncode, "assertions": assertions,
    }, separators=(",", ":")))


def require_helper(path, description):
    try:
        details = path.lstat()
    except OSError as error:
        raise RuntimeError(f"{description} is unavailable") from error
    if path.is_symlink() or not stat.S_ISREG(details.st_mode) or details.st_size <= 0:
        raise RuntimeError(f"{description} must be a regular file")
    return path


def media_record(result, kind, runtime_format=None, emit=True):
    """Validate and relay one redacted media-helper record to the producer."""
    try:
        records = [
            json.loads(line.removeprefix(MEDIA_PREFIX))
            for line in result.stdout.decode("utf-8").splitlines()
            if line.startswith(MEDIA_PREFIX)
        ]
    except (UnicodeError, json.JSONDecodeError) as error:
        raise RuntimeError("native media helper emitted invalid evidence") from error
    if len(records) != 1 or not isinstance(records[0], dict) or records[0].get("kind") != kind:
        raise RuntimeError("native media helper emitted incomplete evidence")
    if runtime_format is not None:
        records[0]["runtime_format"] = runtime_format
    if emit:
        print(MEDIA_PREFIX + json.dumps(records[0], separators=(",", ":"), sort_keys=True))
    return records[0]


def run_media(*argv, kind, runtime_format=None, emit=True):
    require_helper(NATIVE_MEDIA, "native media helper")
    result = run([sys.executable, str(NATIVE_MEDIA), *argv])
    return result, media_record(result, kind, runtime_format=runtime_format, emit=emit)


def write_wayland_evidence(evidence_dir, name, receipt):
    target = evidence_dir / name
    if target.exists() or target.is_symlink():
        raise RuntimeError("Wayland qualification evidence path already exists")
    target.write_text(json.dumps(receipt, separators=(",", ":"), sort_keys=True) + "\n", encoding="utf-8")
    os.chmod(target, 0o600)


def runtime_evidence_namespace(runtime_format, *, seed_upgrade_canary):
    name = runtime_format.lower()
    return f"prior-{name}" if seed_upgrade_canary else name


def require_sha256(value, description):
    if not isinstance(value, str) or not SHA256.fullmatch(value):
        raise RuntimeError(f"{description} is invalid")


def require_wayland_text(value, description):
    if not isinstance(value, str) or not value or len(value) > 128 or any(ord(character) < 0x20 for character in value):
        raise RuntimeError(f"{description} is invalid")
    return value


def wayland_quick_paste_inputs(args):
    """Return the installed product control used to request its own consent.

    Portal dialogs are owned by the desktop session, not by a workflow input.
    The Wayland helper resolves their D-Bus/AT-SPI process identity immediately
    before each consent action.
    """
    return {
        "app_permission_target": require_wayland_text(
            args.wayland_app_permission_target, "app permission target",
        ),
    }


def desktop_acceptance_receipt(path, *, desktop, session, gui_pid):
    try:
        details = path.lstat()
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise RuntimeError("Linux desktop acceptance evidence is invalid") from error
    if stat.S_ISLNK(details.st_mode) or not stat.S_ISREG(details.st_mode):
        raise RuntimeError("Linux desktop acceptance evidence is invalid")
    required = {"schema", "desktop", "session", "gui_pid", "tray", "window", "notification", "history", "commands"}
    if not isinstance(value, dict) or set(value) != required:
        raise RuntimeError("Linux desktop acceptance evidence is invalid")
    if value["schema"] != 1 or value["desktop"] != desktop or value["session"] != session or session not in {"x11", "wayland"} or value["gui_pid"] != gui_pid:
        raise RuntimeError("Linux desktop acceptance evidence does not bind the GUI")
    window = value["window"]
    if session == "wayland":
        valid_window = window == {"observer": "compositor_dbus_signal", "hide_show": "dbusmenu_open"}
    else:
        valid_window = (isinstance(window, dict) and set(window) == {"id", "hide_show"}
                        and isinstance(window["id"], str) and re.fullmatch(r"[1-9][0-9]*", window["id"])
                        and window["hide_show"] == "dbusmenu_open")
    tray = value["tray"]
    window = value["window"]
    notification = value["notification"]
    history = value["history"]
    commands = value["commands"]
    if (not isinstance(tray, dict) or set(tray) != {"service", "menu_path", "open_item_id"}
            or not isinstance(tray["service"], str) or not tray["service"]
            or tray["menu_path"] != "/StatusNotifierItem/Menu"
            or type(tray["open_item_id"]) is not int or tray["open_item_id"] < 0
            or not valid_window
            or not isinstance(notification, dict) or type(notification.get("server_reply_id")) is not int or notification["server_reply_id"] <= 0
            or not isinstance(history, dict) or type(history.get("new_item_count")) is not int or history["new_item_count"] < 1
            or not isinstance(commands, list) or len(commands) != 2):
        raise RuntimeError("Linux desktop acceptance evidence is incomplete")
    require_sha256(notification.get("capture_command_sha256"), "Linux capture command digest")
    menu_command, capture_command = commands
    expected_menu_argv = [
        "busctl", "--user", "call", tray["service"], tray["menu_path"],
        "com.canonical.dbusmenu", "Event", "isvu", str(tray["open_item_id"]),
        "clicked", "s", "", "0",
    ]
    if (not isinstance(menu_command, dict) or set(menu_command) != {"argv", "returncode"}
            or menu_command.get("returncode") != 0 or menu_command["argv"] != expected_menu_argv):
        raise RuntimeError("Linux desktop acceptance command evidence is invalid")
    if (not isinstance(capture_command, dict) or set(capture_command) != {"argv_sha256", "returncode"}
            or capture_command.get("returncode") != 0):
        raise RuntimeError("Linux desktop acceptance command evidence is invalid")
    require_sha256(capture_command["argv_sha256"], "Linux desktop acceptance command digest")
    return value


def wayland_quick_paste_receipt(value, *, gui_pid, inputs, marker):
    required = {"schema", "kind", "actors", "target", "portal", "atspi"}
    if not isinstance(value, dict) or set(value) != required or value.get("schema") != 1 or value.get("kind") != "wayland_quick_paste":
        raise RuntimeError("Wayland Quick Paste receipt is invalid")
    actors = value["actors"]
    target = value["target"]
    portal = value["portal"]
    atspi = value["atspi"]
    if type(gui_pid) is not int or gui_pid <= 0:
        raise RuntimeError("Wayland Quick Paste GUI actor is invalid")
    if not isinstance(actors, dict) or set(actors) != {"gui_pid", "portal_dialog_pid", "quick_paste_pid", "input_target_pid"}:
        raise RuntimeError("Wayland Quick Paste receipt actor identity is invalid")
    if actors.get("gui_pid") != gui_pid or any(type(actors.get(name)) is not int or actors[name] <= 0 for name in (
        "portal_dialog_pid", "quick_paste_pid", "input_target_pid",
    )):
        raise RuntimeError("Wayland Quick Paste receipt actor identity does not match the discovered actors")
    if actors["input_target_pid"] in {gui_pid, actors["portal_dialog_pid"]} or actors["quick_paste_pid"] in {
        gui_pid, actors["portal_dialog_pid"], actors["input_target_pid"],
    }:
        raise RuntimeError("Wayland Quick Paste target actor is not separate")
    if (not isinstance(target, dict) or set(target) != {"pid", "title_sha256", "marker_sha256"}
            or target["pid"] != actors["input_target_pid"]):
        raise RuntimeError("Wayland Quick Paste target evidence is invalid")
    require_sha256(target["title_sha256"], "Wayland Quick Paste target title digest")
    if target["title_sha256"] != hashlib.sha256(WAYLAND_INPUT_TARGET_TITLE.encode("utf-8")).hexdigest():
        raise RuntimeError("Wayland Quick Paste target title digest does not match")
    if target["marker_sha256"] != hashlib.sha256(marker.encode("utf-8")).hexdigest():
        raise RuntimeError("Wayland Quick Paste target marker digest does not match")
    if (not isinstance(portal, dict) or set(portal) != {"input_calls", "session_sha256"}
            or type(portal["input_calls"]) is not int or portal["input_calls"] < 8):
        raise RuntimeError("Wayland Quick Paste portal evidence is invalid")
    require_sha256(portal["session_sha256"], "Wayland Quick Paste portal session digest")
    if not isinstance(atspi, dict) or set(atspi) != {"portal_click_sha256", "quick_paste_ready_sha256"}:
        raise RuntimeError("Wayland Quick Paste media evidence is invalid")
    for name, digest in atspi.items():
        require_sha256(digest, f"Wayland Quick Paste {name}")
    return value


def wayland_helper_receipt(output):
    records = []
    prefix = "COPYPASTE_QUALIFICATION_WAYLAND "
    for line in output.splitlines():
        if line.startswith(prefix):
            try:
                records.append(json.loads(line.removeprefix(prefix)))
            except json.JSONDecodeError as error:
                raise RuntimeError("Wayland Quick Paste helper emitted invalid JSON") from error
        elif line:
            raise RuntimeError("Wayland Quick Paste helper emitted unstructured output")
    if len(records) != 1:
        raise RuntimeError("Wayland Quick Paste helper did not emit exactly one receipt")
    return records[0]


def wayland_permission_receipt(output, *, gui_pid):
    prefix = "COPYPASTE_QUALIFICATION_WAYLAND_PERMISSION "
    records = [json.loads(line.removeprefix(prefix)) for line in output.splitlines() if line.startswith(prefix)]
    if len(records) != 1 or any(line and not line.startswith(prefix) for line in output.splitlines()):
        raise RuntimeError("Wayland onboarding permission helper emitted invalid output")
    record = records[0]
    if (not isinstance(record, dict) or set(record) != {"schema", "gui_pid", "dialog_pid", "click_sha256", "consent_sha256"}
            or record.get("schema") != 1 or record.get("gui_pid") != gui_pid
            or type(record.get("dialog_pid")) is not int or record["dialog_pid"] <= 0):
        raise RuntimeError("Wayland onboarding permission evidence is invalid")
    require_sha256(record["click_sha256"], "Wayland onboarding click digest")
    require_sha256(record["consent_sha256"], "Wayland onboarding consent digest")
    return record


def atspi_action(actor, command, gui_pid, target, environment):
    result = run(
        [sys.executable, str(actor), command, "--pid", str(gui_pid), "--target", target],
        env=environment,
    )
    return hashlib.sha256(result.stdout).hexdigest()


def atspi_present(actor, gui_pid, target, environment):
    result = run(
        [sys.executable, str(actor), "probe", "--pid", str(gui_pid), "--target", target],
        env=environment,
    )
    prefix = "COPYPASTE_QUALIFICATION_MEDIA "
    lines = result.stdout.decode("utf-8").splitlines()
    if len(lines) != 1 or not lines[0].startswith(prefix):
        raise RuntimeError("AT-SPI probe emitted invalid output")
    record = json.loads(lines[0].removeprefix(prefix))
    if (not isinstance(record, dict) or record.get("kind") != "atspi_probe" or record.get("gui_pid") != gui_pid
            or record.get("target_name_sha256") != hashlib.sha256(target.encode("utf-8")).hexdigest()
            or type(record.get("present")) is not bool):
        raise RuntimeError("AT-SPI probe evidence is invalid")
    return record["present"]


def require_quick_paste_ready(actor, quick_paste_pid, marker, timeout_seconds, environment):
    result = run(
        [
            sys.executable, str(actor), "quick-paste-ready", "--pid", str(quick_paste_pid),
            "--marker", marker, "--timeout-seconds", str(timeout_seconds),
        ],
        env=environment,
    )
    prefix = "COPYPASTE_QUALIFICATION_MEDIA "
    lines = result.stdout.decode("utf-8").splitlines()
    if len(lines) != 1 or not lines[0].startswith(prefix):
        raise RuntimeError("Quick Paste AT-SPI readiness helper emitted invalid output")
    record = json.loads(lines[0].removeprefix(prefix))
    if (not isinstance(record, dict) or record.get("kind") != "atspi_quick_paste_ready"
            or record.get("gui_pid") != quick_paste_pid
            or record.get("marker_sha256") != hashlib.sha256(marker.encode("utf-8")).hexdigest()):
        raise RuntimeError("Quick Paste AT-SPI readiness evidence is invalid")


def x11_window_pid(window, environment):
    result = run(["xdotool", "getwindowpid", window], env=environment)
    value = result.stdout.decode("ascii").strip()
    if not re.fullmatch(r"[1-9][0-9]*", value):
        raise RuntimeError("Quick Paste window owner is invalid")
    return int(value)


def require_linux_onboarding(args, *, gui_pid, environment):
    """Complete the installed Linux onboarding before feature qualification."""
    actor = require_helper(args.wayland_actor, "Linux onboarding AT-SPI actor")
    if atspi_present(actor, gui_pid, "History", environment):
        if args.session != "wayland":
            return
        atspi_action(actor, "click", gui_pid, "Settings", environment)
        atspi_action(actor, "observe", gui_pid, "Quick Paste", environment)
        atspi_action(actor, "click", gui_pid, "Quick Paste", environment)
        atspi_action(actor, "observe", gui_pid, "Open clipboard history from anywhere without switching windows.", environment)
        if atspi_present(actor, gui_pid, "Allow keyboard control", environment):
            helper = require_helper(args.wayland_quick_paste_helper, "Wayland Quick Paste helper")
            permission = run(
                [
                    sys.executable, str(helper), "--grant-product-permission", "--gui-pid", str(gui_pid),
                    "--actor", str(actor), "--app-permission-target", wayland_quick_paste_inputs(args)["app_permission_target"],
                ], env=environment,
            )
            # The Settings screen has no positive remote-desktop status copy.
            # Keep this action unreceipted until the later independent portal
            # session proves that the installed product actually accepts and
            # uses keyboard input against the external target.
            wayland_permission_receipt(permission.stdout.decode("utf-8"), gui_pid=gui_pid)
        atspi_action(actor, "click", gui_pid, "History", environment)
        atspi_action(actor, "observe", gui_pid, "History", environment)
        return
    atspi_action(actor, "observe", gui_pid, "Continue", environment)
    atspi_action(actor, "click", gui_pid, "Continue", environment)
    atspi_action(actor, "observe", gui_pid, "Set up your Linux desktop", environment)
    if args.session == "wayland":
        helper = require_helper(args.wayland_quick_paste_helper, "Wayland Quick Paste helper")
        permission = run(
            [
                sys.executable, str(helper), "--grant-product-permission", "--gui-pid", str(gui_pid),
                "--actor", str(actor), "--app-permission-target", wayland_quick_paste_inputs(args)["app_permission_target"],
                "--require-granted-text",
            ],
            env=environment,
        )
        receipt = wayland_permission_receipt(permission.stdout.decode("utf-8"), gui_pid=gui_pid)
        write_wayland_evidence(args.evidence_dir, "wayland-onboarding-permission.json", receipt)
        atspi_action(actor, "observe", gui_pid, "The authenticated Clipboard v2 bridge is ready.", environment)
        atspi_action(actor, "observe", gui_pid, "Keyboard control is allowed for Quick Paste.", environment)
    atspi_action(actor, "observe", gui_pid, "Quick Paste is ready.", environment)
    atspi_action(actor, "click", gui_pid, "Continue", environment)
    atspi_action(actor, "observe", gui_pid, "Get started", environment)
    atspi_action(actor, "click", gui_pid, "Get started", environment)
    atspi_action(actor, "observe", gui_pid, "History", environment)


def desktop_acceptance_capture_command(workspace, environment):
    """Build the real GTK source command invoked by the tray observer."""
    require_helper(PROVIDER, "native clipboard provider")
    helper = gtk_provider_helper(workspace, environment)
    directory = workspace / f"desktop-acceptance-{time.monotonic_ns()}"
    directory.mkdir(mode=0o700)
    payload = directory / "payload"
    payload.write_bytes(b"copypaste-desktop-notification-fixture")
    os.chmod(payload, 0o600)
    manifest = directory / "manifest.json"
    manifest.write_text(json.dumps({
        "application_id": SOURCE_APPLICATION_ID,
        "offers": [{"mime": "text/plain;charset=utf-8", "path": str(payload)}],
    }, separators=(",", ":")), encoding="utf-8")
    os.chmod(manifest, 0o600)
    return [
        sys.executable, str(PROVIDER), "--helper", str(helper), "--manifest", str(manifest),
        "--ready-file", str(directory / "ready"), "--activity-log", str(directory / "activity.jsonl"),
        "--hold-seconds", "8", "--require-focus",
    ]


def require_desktop_acceptance(args, *, gui_pid, cli, environment, workspace, socket_path):
    """Observe the owned GUI tray, close-to-tray, and accepted capture notification."""
    tray_helper = require_helper(args.wayland_desktop_acceptance_helper, "Linux desktop acceptance helper")
    capture_command = desktop_acceptance_capture_command(workspace, environment)
    tray_result = run(
        [
            sys.executable, str(tray_helper), "--gui-pid", str(gui_pid), "--desktop", args.desktop,
            "--session", args.session, "--cli", str(cli), "--socket", str(socket_path),
            "--capture-command", json.dumps(capture_command, separators=(",", ":")),
            "--evidence-dir", str(args.evidence_dir),
        ],
        env=environment,
        timeout=45,
    )
    desktop_acceptance_receipt(
        args.evidence_dir / "linux-desktop-acceptance.json", desktop=args.desktop, session=args.session, gui_pid=gui_pid,
    )
    emit_result("tray_window_notification", tray_result)


def require_wayland_qualification(args, *, gui_pid, cli, environment, workspace, socket_path):
    """Exercise actual tray, portal, and AT-SPI actors before emitting Wayland assertions."""
    inputs = wayland_quick_paste_inputs(args)
    require_desktop_acceptance(args, gui_pid=gui_pid, cli=cli, environment=environment,
                               workspace=workspace, socket_path=socket_path)
    quick_paste_helper = require_helper(args.wayland_quick_paste_helper, "Wayland Quick Paste helper")
    actor = require_helper(args.wayland_actor, "Wayland AT-SPI actor")
    marker = "copypaste-wayland-quick-paste-fixture"
    cli_environment = {**environment, "COPYPASTE_SOCKET": str(socket_path)}
    prepare_quick_paste_marker(cli, cli_environment, marker)
    quick_paste_result = run(
        [
            sys.executable, str(quick_paste_helper), "--gui-pid", str(gui_pid),
            "--workspace", str(workspace), "--marker", marker, "--actor", str(actor),
            "--app-permission-target", inputs["app_permission_target"],
        ],
        env=environment,
        timeout=75,
    )
    receipt = wayland_quick_paste_receipt(
        wayland_helper_receipt(quick_paste_result.stdout.decode("utf-8")),
        gui_pid=gui_pid, inputs=inputs, marker=marker,
    )
    write_wayland_evidence(args.evidence_dir, "wayland-quick-paste.json", receipt)
    emit_result(
        ["quick_paste_hotkey", "quick_paste_focus_restore", "portal_keyboard_grant"], quick_paste_result,
    )


def qualification_workspace(path):
    if path.is_symlink():
        raise ValueError("qualification root must not be a symlink")
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    resolved = path.resolve(strict=True)
    if resolved.is_symlink() or not resolved.is_dir() or stat.S_IMODE(resolved.stat().st_mode) != 0o700:
        raise ValueError("qualification root must be an owner-only directory")
    return resolved


def canary_receipt(workspace, executable):
    return workspace / "upgrade-canary.json"


def seed_upgrade_canary(cli, environment, workspace, executable):
    added = run([str(cli), "add", UPGRADE_CANARY], env=environment)
    items = list_items(cli, environment)
    item = next((item for item in items if item.get("content") == UPGRADE_CANARY), None)
    if not isinstance(item, dict) or not isinstance(item.get("id"), str):
        raise RuntimeError("GUI-owned daemon did not retain the upgrade canary")
    digest = hashlib.sha256(UPGRADE_CANARY.encode()).hexdigest()
    canary_receipt(workspace, executable).write_text(json.dumps({"id": item["id"], "content_sha256": digest, "executable_sha256": hashlib.sha256(executable.read_bytes()).hexdigest()}), encoding="utf-8")
    emit_result("package_upgrade", added)


def verify_upgrade_canary(cli, environment, workspace, executable):
    receipt = canary_receipt(workspace, executable)
    if not receipt.is_file() or receipt.is_symlink():
        return False
    value = json.loads(receipt.read_text(encoding="utf-8"))
    if value.get("content_sha256") != hashlib.sha256(UPGRADE_CANARY.encode()).hexdigest():
        raise RuntimeError("upgrade canary receipt is invalid")
    if not any(item.get("content") == UPGRADE_CANARY for item in list_items(cli, environment)):
        raise RuntimeError("GUI-owned daemon did not retain the prior runtime canary")
    return True


def cli_json(cli, environment, *args):
    output = run([str(cli), "--json", *args], env=environment).stdout.decode("utf-8")
    response = json.loads(output)
    if not isinstance(response, dict) or response.get("ok") is not True or not isinstance(response.get("data"), dict):
        raise RuntimeError("CLI did not return a successful typed response")
    return response


def response_variant(response, name):
    data = response["data"]
    if set(data) != {name}:
        raise RuntimeError(f"CLI response is not the expected {name} variant")
    return data[name]


def ipc(socket_path, method, params=None):
    request = {"id": 991, "protocol_version": 5, "method": method}
    if params is not None:
        request["params"] = params
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as channel:
        channel.settimeout(5)
        channel.connect(str(socket_path))
        channel.sendall(json.dumps(request, separators=(",", ":")).encode() + b"\n")
        response = channel.makefile("rb").readline()
    decoded = json.loads(response)
    if not isinstance(decoded, dict) or not decoded.get("ok"):
        code = decoded.get("error_code") if isinstance(decoded, dict) else None
        if isinstance(code, str) and 1 <= len(code) <= 64 and code.isascii() and all(character.islower() or character.isdigit() or character == "_" for character in code):
            raise RuntimeError(f"daemon rejected {method}: {code}")
        raise RuntimeError(f"daemon rejected {method}")
    return decoded


def status(cli, environment):
    value = response_variant(cli_json(cli, environment, "status"), "status")
    if not isinstance(value, dict):
        raise RuntimeError("status has an invalid shape")
    return value


def native_clipboard_transport_is_ready(value):
    """Return whether status proves the live Linux capture transport is usable."""
    return (
        value.get("capture_running") is True
        and value.get("clipboard_available") is True
        and isinstance(value.get("clipboard_backend"), str)
        and value["clipboard_backend"].startswith("linux-")
    )


def wait_for_startup_status(cli, environment, *, require_native_clipboard):
    """Wait within the startup deadline for IPC, and for native transport when required."""
    deadline = time.monotonic() + CAPTURE_TIMEOUT_SECONDS
    observed = None
    while time.monotonic() < deadline:
        try:
            observed = status(cli, environment)
        except (subprocess.CalledProcessError, json.JSONDecodeError, RuntimeError):
            time.sleep(0.15)
            continue
        if not require_native_clipboard or native_clipboard_transport_is_ready(observed):
            return observed
        time.sleep(0.15)
    if observed is None:
        raise RuntimeError("GUI-owned daemon IPC status did not become ready before the startup deadline")
    raise RuntimeError(
        "GUI-owned daemon native clipboard transport remained inactive before the startup deadline "
        f"(capture_running={observed.get('capture_running')!r}, "
        f"clipboard_available={observed.get('clipboard_available')!r}, "
        f"clipboard_backend={observed.get('clipboard_backend')!r})"
    )


def list_items(cli, environment):
    page = response_variant(cli_json(cli, environment, "list", "--limit", "100"), "page")
    if not isinstance(page, dict) or not isinstance(page.get("items"), list):
        raise RuntimeError("list response has no item page")
    if not all(isinstance(item, dict) for item in page["items"]):
        raise RuntimeError("item page has an invalid item")
    return page["items"]


def prepare_quick_paste_marker(cli, environment, marker):
    """Keep the real native selection consistent with the prepared newest clip."""
    run([str(cli), "add", marker], env=environment)
    prepared = [item for item in list_items(cli, environment) if item.get("content") == marker]
    if len(prepared) != 1 or not isinstance(prepared[0].get("id"), str) or not prepared[0]["id"]:
        raise RuntimeError("Quick Paste marker is not uniquely prepared")
    run([str(cli), "--json", "copy", prepared[0]["id"]], env=environment)


def require_clean_install_baseline(data_home, environment):
    """Prove the isolated profile has no existing files or dangling link before launch."""
    for predicate in ("-e", "-L"):
        result = run(["test", "!", predicate, str(data_home)], env=environment)
        emit_result("clean_install_baseline", result)


def write_source_desktop_entry(data_home):
    applications = data_home / "applications"
    applications.mkdir(parents=True, exist_ok=True)
    entry = applications / f"{SOURCE_APPLICATION_ID}.desktop"
    entry.write_text(
        "[Desktop Entry]\n"
        "Type=Application\n"
        f"Name={SOURCE_APPLICATION_NAME}\n"
        f"StartupWMClass={SOURCE_APPLICATION_ID[:1].upper()}{SOURCE_APPLICATION_ID[1:]}\n"
        "Exec=true\n",
        encoding="utf-8",
    )


def gtk_provider_helper(workspace, environment):
    helper = workspace / "gtk3-clipboard-provider"
    if helper.is_file() and not helper.is_symlink() and os.access(helper, os.X_OK):
        return helper
    run([sys.executable, str(PROVIDER), "--build-helper", str(helper)], env=environment, timeout=30)
    if not helper.is_file() or helper.is_symlink() or not os.access(helper, os.X_OK):
        raise RuntimeError("GTK3 clipboard provider build did not create an executable")
    return helper


def x11_input_target_helper(workspace, environment):
    """Build the external GTK input target used to observe XTEST paste."""
    if not X11_INPUT_TARGET.is_file() or X11_INPUT_TARGET.is_symlink():
        raise RuntimeError("X11 input target source is unavailable")
    helper = workspace / "x11-input-target"
    flags = run(["pkg-config", "--cflags", "--libs", "gtk+-3.0"], env=environment).stdout.decode("utf-8").split()
    run(["cc", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", str(X11_INPUT_TARGET), "-o", str(helper), *flags], env=environment, timeout=30)
    if not helper.is_file() or helper.is_symlink() or not os.access(helper, os.X_OK):
        raise RuntimeError("X11 input target build did not create an executable")
    return helper


@contextmanager
def x11_input_target(workspace, environment):
    helper = x11_input_target_helper(workspace, environment)
    directory = workspace / f"x11-input-{time.monotonic_ns()}"
    directory.mkdir(mode=0o700)
    ready = directory / "ready"
    result = directory / "result"
    target = subprocess.Popen(
        [str(helper), "--ready-file", str(ready), "--result-file", str(result)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=environment,
    )
    try:
        deadline = time.monotonic() + 5
        while not ready.is_file() and target.poll() is None and time.monotonic() < deadline:
            time.sleep(0.05)
        if target.poll() is not None or ready.read_text(encoding="utf-8") != "ready\n":
            raise RuntimeError("X11 input target did not become ready")
        target_window = wait_for_window("CopyPaste Qualification Input Target", environment, owner_pid=target.pid)
        run(["xdotool", "windowactivate", "--sync", target_window], env=environment)
        yield target_window, result
    finally:
        target.terminate()
        try:
            target.wait(timeout=5)
        except subprocess.TimeoutExpired:
            target.kill()
            target.wait(timeout=5)


def wait_for_gui_socket(runtime_dir, previous):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        candidates = [
            path for path in runtime_dir.glob("cp-*.sock")
            if path not in previous and path.exists() and stat.S_ISSOCK(path.stat().st_mode)
        ]
        if len(candidates) == 1:
            return candidates[0]
        if len(candidates) > 1:
            raise RuntimeError("packaged GUI created more than one runtime socket")
        time.sleep(0.1)
    raise RuntimeError("packaged GUI did not expose its app-owned runtime socket")


def start_gui_runtime(executable, environment, runtime_dir, previous_sockets):
    app = subprocess.Popen([str(executable)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=environment)
    try:
        socket_path = wait_for_gui_socket(runtime_dir, previous_sockets)
        if app.poll() is not None:
            raise RuntimeError("packaged GUI exited before its runtime became ready")
        return app, socket_path
    except BaseException:
        app.terminate()
        try:
            app.wait(timeout=5)
        except subprocess.TimeoutExpired:
            app.kill()
            app.wait(timeout=5)
        raise


@contextmanager
def clipboard_provider(workspace, evidence_dir, label, payloads, environment, request_audit_file=None):
    """Offer real MIME data from a visible GTK client until the assertion ends."""
    if not PROVIDER.is_file() or PROVIDER.is_symlink():
        raise RuntimeError("native clipboard provider is unavailable")
    helper = gtk_provider_helper(workspace, environment)
    directory = workspace / f"provider-{time.monotonic_ns()}"
    directory.mkdir(mode=0o700)
    offers = []
    for index, (mime, payload) in enumerate(payloads.items()):
        payload_path = directory / f"payload-{index}"
        payload_path.write_bytes(payload)
        os.chmod(payload_path, 0o600)
        offers.append({"mime": mime, "path": str(payload_path)})
    manifest = directory / "manifest.json"
    manifest.write_text(
        json.dumps({"application_id": SOURCE_APPLICATION_ID, "offers": offers}, separators=(",", ":")),
        encoding="utf-8",
    )
    os.chmod(manifest, 0o600)
    ready = directory / "ready.json"
    activity = directory / "activity.jsonl"
    command = [
        sys.executable, str(PROVIDER), "--helper", str(helper), "--manifest", str(manifest), "--ready-file", str(ready),
        "--activity-log", str(activity), "--hold-seconds", "20", "--require-focus",
    ]
    if request_audit_file is not None:
        command.extend(["--request-audit-file", str(request_audit_file)])
    provider = subprocess.Popen(
        command,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        env=environment,
    )
    try:
        deadline = time.monotonic() + 5
        while not ready.is_file() and provider.poll() is None and time.monotonic() < deadline:
            time.sleep(0.05)
        if provider.poll() is not None or not ready.is_file():
            raise RuntimeError("GTK clipboard provider did not become ready")
        if ready.read_text(encoding="utf-8") != "ready\n":
            raise RuntimeError("GTK clipboard provider readiness is invalid")
        yield activity
    finally:
        was_alive = provider.poll() is None
        if was_alive:
            provider.terminate()
        try:
            exit_code = provider.wait(timeout=5)
        except subprocess.TimeoutExpired:
            provider.kill()
            provider.wait(timeout=5)
            raise RuntimeError("GTK clipboard provider did not exit") from None
        stderr = provider.stderr.read().decode("utf-8", "replace") if provider.stderr is not None else ""
        if request_audit_file is not None and (not was_alive or exit_code != -signal.SIGTERM):
            message = "GTK clipboard provider failed during the privacy assertion"
            raise RuntimeError(f"{message}: {stderr.strip()}" if stderr.strip() else message)
        requests = []
        if activity.exists():
            for line in activity.read_text(encoding="utf-8").splitlines():
                entry = json.loads(line)
                if not isinstance(entry, dict) or set(entry) != {"mime"} or not isinstance(entry["mime"], str):
                    raise RuntimeError("GTK clipboard provider emitted an invalid activity record")
                requests.append(entry)
        # The evidence is intentionally a MIME-only transcript. It binds the
        # real provider interaction without retaining clipboard bytes, paths,
        # or a test-private marker in the uploaded qualification artifact.
        (evidence_dir / f"linux-clipboard-provider-{label}.json").write_text(
            json.dumps({"application_id": SOURCE_APPLICATION_ID, "requests": requests}, separators=(",", ":")) + "\n",
            encoding="utf-8",
        )


def appimage_prefix(artifact, workspace):
    run([str(artifact), "--appimage-extract"], env={**os.environ, "APPIMAGE_EXTRACT_AND_RUN": "1"}, timeout=30)
    root = workspace / "squashfs-root"
    prefix = root / "usr/lib/copypaste"
    for name in ("copypaste", "copypaste-daemon", "copypaste-cli"):
        if not os.access(prefix / name, os.X_OK):
            raise RuntimeError("AppImage is missing a required executable")
    return prefix


def runtime_prefix(prefix):
    prefix = prefix.resolve(strict=True)
    if prefix.is_symlink() or not prefix.is_dir():
        raise RuntimeError("qualified runtime prefix is unsafe")
    for name in ("copypaste", "copypaste-daemon", "copypaste-cli"):
        executable = prefix / name
        if not executable.is_file() or executable.is_symlink() or not os.access(executable, os.X_OK):
            raise RuntimeError("qualified runtime prefix is missing a required executable")
    return prefix


def runtime_gui_entrypoint(prefix, runtime_format):
    """Use the package's public launcher, including AppImage runtime setup."""
    binary = prefix / "copypaste"
    if runtime_format == "AppImage":
        launcher = prefix.parents[2] / "AppRun"
        if not launcher.is_file() or launcher.is_symlink() or not os.access(launcher, os.X_OK):
            raise RuntimeError("qualified AppImage is missing its executable AppRun")
        return launcher
    if runtime_format not in {"deb", "rpm"}:
        raise RuntimeError("qualified GUI package format is unsupported")
    launcher = prefix.parents[1] / "bin/copypaste"
    if not launcher.is_symlink() or launcher.resolve(strict=True) != binary.resolve(strict=True):
        raise RuntimeError("qualified native launcher does not resolve to the installed GUI")
    return launcher


def sha256(path):
    import hashlib
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def emit_runtime(format_name, prefix):
    executables = {}
    for name in ("copypaste", "copypaste-daemon", "copypaste-cli"):
        path = prefix / name
        executables[name] = {"path": str(path), "sha256": sha256(path), "size_bytes": path.stat().st_size}
    print("COPYPASTE_QUALIFICATION_RUNTIME " + json.dumps({
        "format": format_name, "gui_owned_daemon": True, "executables": executables,
    }, separators=(",", ":")))


def cli_item_count(status):
    count = status.get("item_count")
    if isinstance(count, bool) or not isinstance(count, int) or count < 0:
        raise RuntimeError("status response has no valid item count")
    return count


def wait_for_count(cli, environment, minimum):
    deadline = time.monotonic() + CAPTURE_TIMEOUT_SECONDS
    observed = None
    while time.monotonic() < deadline:
        observed = cli_item_count(status(cli, environment))
        if observed >= minimum:
            return observed
        time.sleep(0.15)
    raise RuntimeError(f"clipboard history count did not reach at least {minimum}; saw {observed}")


def newest_item(items, before_ids, content_type):
    for item in items:
        if item.get("id") not in before_ids and item.get("content_type") == content_type:
            return item
    raise RuntimeError(f"native backend did not retain a new {content_type} item")


def wait_for_fixture_capture(cli, environment, before, before_ids, content_type, payload):
    """Bind the assertion to this offer rather than a competing startup capture."""
    deadline = time.monotonic() + CAPTURE_TIMEOUT_SECONDS
    expected_text = payload.decode("utf-8") if content_type == "text" else None
    while time.monotonic() < deadline:
        candidates = [
            item for item in list_items(cli, environment)
            if item.get("id") not in before_ids and item.get("content_type") == content_type
            and (expected_text is None or item.get("content") == expected_text)
        ]
        if candidates:
            if cli_item_count(status(cli, environment)) < before + 1:
                raise RuntimeError("fixture capture did not increase history count")
            return candidates[0]
        time.sleep(0.15)
    raise RuntimeError("native backend did not retain the requested fixture within its capture deadline")


def external_clipboard_bytes(session, mime, environment):
    if session == "x11":
        command = ["xclip", "-selection", "clipboard", "-t", mime, "-o"]
    else:
        command = ["wl-paste", "--no-newline", "--type", mime]
    return run(command, env=environment)


def require_source_identity(item):
    if item.get("source_app_bundle_id") != SOURCE_APPLICATION_ID:
        raise RuntimeError("captured clipboard item did not retain the GTK source application id")
    if item.get("source_app_name") != SOURCE_APPLICATION_NAME:
        raise RuntimeError("captured clipboard item did not retain the GTK source application name")


def assert_secret_payload_was_not_requested(activity, protected_mime):
    if not activity.exists():
        return
    requests = [json.loads(line).get("mime") for line in activity.read_text(encoding="utf-8").splitlines()]
    if protected_mime in requests:
        raise RuntimeError("native backend read a confidential clipboard payload")


def require_capture(cli, environment, workspace, evidence_dir, session, mime, payload, content_type, assertion):
    before = cli_item_count(status(cli, environment))
    before_ids = {item.get("id") for item in list_items(cli, environment)}
    with clipboard_provider(workspace, evidence_dir, assertion, {mime: payload}, environment):
        item = wait_for_fixture_capture(cli, environment, before, before_ids, content_type, payload)
        require_source_identity(item)
    overwrite_marker = f"copypaste-native-overwrite-{assertion}".encode("utf-8")
    overwrite_before = cli_item_count(status(cli, environment))
    overwrite_ids = {item.get("id") for item in list_items(cli, environment)}
    with clipboard_provider(
        workspace,
        evidence_dir,
        f"{assertion}-overwrite",
        {"text/plain;charset=utf-8": overwrite_marker},
        environment,
    ):
        wait_for_count(cli, environment, overwrite_before + 1)
        overwrite_item = newest_item(list_items(cli, environment), overwrite_ids, "text")
        require_source_identity(overwrite_item)
        if overwrite_item.get("content") != overwrite_marker.decode("utf-8"):
            raise RuntimeError("native overwrite marker was not retained in clipboard history")
        overwritten = external_clipboard_bytes(session, "text/plain;charset=utf-8", environment)
        if overwritten.stdout != overwrite_marker:
            raise RuntimeError("native overwrite marker did not become the active clipboard value")
        run([str(cli), "--json", "copy", item["id"]], env=environment)
        copied = external_clipboard_bytes(session, mime, environment)
        if copied.stdout != payload:
            raise RuntimeError(f"native backend did not preserve {mime} bytes on paste")
    emit_result(assertion, copied)


def require_empty_ipc_response(response, operation):
    if not isinstance(response, dict) or response.get("data") != {"empty": {}}:
        raise RuntimeError(f"daemon returned an invalid {operation} response")


def require_locator_file_capture(cli, environment, workspace, evidence_dir, session, socket_path):
    original = workspace / "fixture.txt"
    payload = b"CopyPaste qualification file payload\n"
    original.write_bytes(payload)
    before = cli_item_count(status(cli, environment))
    before_ids = {item.get("id") for item in list_items(cli, environment)}
    with clipboard_provider(workspace, evidence_dir, "clipboard-file", {"text/uri-list": original.as_uri().encode("utf-8") + b"\r\n"}, environment):
        wait_for_count(cli, environment, before + 1)
        item = newest_item(list_items(cli, environment), before_ids, "file")
        require_source_identity(item)
        saved = workspace / "captured-file-roundtrip.txt"
        require_empty_ipc_response(
            ipc(socket_path, "save_file", {"id": item["id"], "dest_path": str(saved)}),
            "save_file",
        )
        if saved.read_bytes() != payload:
            raise RuntimeError("captured file bytes did not survive the product save_file path")
        run([str(cli), "--json", "copy", item["id"]], env=environment)
        locator = external_clipboard_bytes(session, "text/plain;charset=utf-8", environment)
        if locator.stdout != str(original).encode("utf-8"):
            raise RuntimeError("native backend did not preserve the file locator on copy")
    return locator


def require_reference_free_file_copy(cli, environment, workspace, session, socket_path):
    original = workspace / "reference-free-file.txt"
    payload = b"CopyPaste qualification reference-free file payload\n"
    original.write_bytes(payload)
    before_ids = {item.get("id") for item in list_items(cli, environment)}
    require_empty_ipc_response(
        ipc(
            socket_path,
            "import_file",
            {
                "path": str(original),
                "filename": original.name,
                "mime_type": "text/plain",
                "source_reference": None,
            },
        ),
        "import_file",
    )
    item = newest_item(list_items(cli, environment), before_ids, "file")
    details = item.get("file_details")
    if not isinstance(details, dict) or details.get("filename") != original.name or details.get("size_bytes") != len(payload):
        raise RuntimeError("reference-free file import did not retain the expected History attachment")
    run([str(cli), "--json", "copy", item["id"]], env=environment)
    returned_uri = external_clipboard_bytes(session, "text/uri-list", environment)
    parsed = urlparse(returned_uri.stdout.decode("utf-8").strip())
    returned_file = Path(unquote(parsed.path))
    if parsed.scheme != "file" or returned_file.read_bytes() != payload:
        raise RuntimeError("native backend did not preserve reference-free file bytes on paste")
    return returned_uri


def require_file_capture(cli, environment, workspace, evidence_dir, session, socket_path):
    require_locator_file_capture(cli, environment, workspace, evidence_dir, session, socket_path)
    returned_uri = require_reference_free_file_copy(cli, environment, workspace, session, socket_path)
    emit_result("clipboard_file", returned_uri)


def require_external_file_drag(cli, environment, workspace, gui_pid, session):
    """Drop a URI-list from a separate GTK client and observe its History item through the CLI."""
    require_helper(EXTERNAL_FILE_DRAG, "external GTK file-drag helper")
    before = cli_item_count(status(cli, environment))
    before_ids = {item.get("id") for item in list_items(cli, environment)}
    result = run(
        [
            sys.executable, str(EXTERNAL_FILE_DRAG), "--session", session,
            "--workspace", str(workspace), "--gui-pid", str(gui_pid),
        ],
        env=environment,
        timeout=45,
    )
    wait_for_count(cli, environment, before + 1)
    item = newest_item(list_items(cli, environment), before_ids, "file")
    details = item.get("file_details")
    if not isinstance(details, dict) or details.get("filename") != "external-file-drag-fixture.txt" or details.get("size_bytes") != 51:
        raise RuntimeError("external GTK file drag did not retain the expected History attachment")
    emit_result("external_file_drag", result)


def wait_for_window(name, environment, deadline=None, *, owner_pid=None):
    deadline = time.monotonic() + 8 if deadline is None else deadline
    command = ["xdotool", "search", "--onlyvisible", "--all"]
    if owner_pid is not None:
        if type(owner_pid) is not int or owner_pid <= 0:
            raise RuntimeError("X11 window owner PID is invalid")
        command += ["--pid", str(owner_pid)]
    command += ["--name", f"^{re.escape(name)}$"]
    while time.monotonic() < deadline:
        result = subprocess.run(command, stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, env=environment)
        windows = result.stdout.decode("utf-8").splitlines()
        if len(windows) == 1 and windows[0].isdigit():
            return windows[0]
        if len(windows) > 1:
            raise RuntimeError(f"{name} window is not uniquely discoverable")
        time.sleep(0.1)
    raise RuntimeError(f"{name} window did not appear")


def require_x11_quick_paste(cli, environment, workspace):
    """Drive the packaged global shortcut through an external GTK input field."""
    marker = "copypaste-x11-quick-paste-fixture"
    prepare_quick_paste_marker(cli, environment, marker)
    run(["xdotool", "mousemove", "--sync", "16", "16"], env=environment)
    with x11_input_target(workspace, environment) as (target_window, result_file):
        # The target owns focus before the real global shortcut is sent.
        before_focus = run(["xdotool", "getwindowfocus"], env=environment)
        if before_focus.stdout.decode("utf-8").strip() != target_window:
            raise RuntimeError("X11 input target did not receive focus")
        invoked = run(["xdotool", "key", "ctrl+shift+c"], env=environment)
        readiness_deadline = time.monotonic() + CAPTURE_TIMEOUT_SECONDS
        quick_paste = wait_for_window("CopyPaste Quick Paste", environment, deadline=readiness_deadline)
        if quick_paste == target_window:
            raise RuntimeError("global shortcut did not open a separate Quick Paste window")
        quick_paste_pid = x11_window_pid(quick_paste, environment)
        remaining = int(readiness_deadline - time.monotonic())
        if remaining < 1:
            raise RuntimeError("Quick Paste did not become ready before the X11 readiness deadline")
        actor = require_helper(NATIVE_MEDIA, "Quick Paste AT-SPI actor")
        require_quick_paste_ready(actor, quick_paste_pid, marker, remaining, environment)
        quick_paste_focus = run(["xdotool", "getwindowfocus"], env=environment)
        if quick_paste_focus.stdout.decode("utf-8").strip() != quick_paste:
            raise RuntimeError("Quick Paste window did not receive X11 input focus")
        emit_result("quick_paste_hotkey", invoked)
        run(["xdotool", "key", "Return"], env=environment)
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if result_file.is_file() and result_file.read_text(encoding="utf-8") == marker:
                break
            time.sleep(0.05)
        else:
            raise RuntimeError("Quick Paste did not restore focus and insert the selected item")
        restored = run(["xdotool", "getwindowfocus"], env=environment)
        if restored.stdout.decode("utf-8").strip() != target_window:
            raise RuntimeError("Quick Paste did not restore focus to the invoking window")
        emit_result("quick_paste_focus_restore", restored)
        emit_result("native_x11_keyboard_input", restored)


def require_modules(socket_path, artifacts, fixtures, application_data, architecture, app_version, evidence_dir, restart):
    """Exercise every signed module through the GUI-owned daemon lifecycle."""
    def emit(record):
        print("COPYPASTE_QUALIFICATION_IPC " + json.dumps(record, separators=(",", ":")))

    return module_qualification_helper().qualify_modules(
        socket_path, artifacts, fixtures, application_data, architecture, app_version, evidence_dir, restart, emit,
    )


def gui_owned_daemon(gui, data_home):
    """Return the one direct GUI-owned daemon and its explicit data directory."""
    try:
        gui_status = (PROC_ROOT / str(gui.pid) / "status").read_text(encoding="utf-8")
        task_directory = PROC_ROOT / str(gui.pid) / "task"
        tasks = tuple(task_directory.iterdir())
    except OSError as error:
        raise RuntimeError("packaged GUI child process inventory is unavailable") from error
    gui_uid = next((line.split()[1] for line in gui_status.splitlines() if line.startswith("Uid:") and len(line.split()) >= 2), None)
    if gui_uid != str(os.geteuid()):
        raise RuntimeError("packaged GUI process identity is invalid")
    pids = set()
    for task in tasks:
        if not re.fullmatch(r"[1-9][0-9]*", task.name):
            continue
        try:
            pids.update(pid for pid in task.joinpath("children").read_text(encoding="ascii").split()
                        if re.fullmatch(r"[1-9][0-9]*", pid))
        except OSError:
            continue
    candidates: list[OwnedDaemon] = []
    for pid in pids:
        process = PROC_ROOT / pid
        try:
            status = process.joinpath("status").read_text(encoding="utf-8")
            cmdline = process.joinpath("cmdline").read_bytes().split(b"\0")
        except OSError:
            continue
        uid = next((line.split()[1] for line in status.splitlines() if line.startswith("Uid:") and len(line.split()) >= 2), None)
        ppid = next((line.split()[1] for line in status.splitlines() if line.startswith("PPid:") and len(line.split()) >= 2), None)
        if uid != gui_uid or ppid != str(gui.pid):
            continue
        argv = [value.decode("utf-8") for value in cmdline if value]
        if not argv or Path(argv[0]).name != "copypaste-daemon":
            continue
        if argv.count("--data-dir") != 1:
            raise RuntimeError("GUI-owned daemon did not receive exactly one explicit data directory")
        index = argv.index("--data-dir")
        if index + 1 >= len(argv):
            raise RuntimeError("GUI-owned daemon has no data directory argument")
        path = Path(argv[index + 1])
        try:
            resolved = path.resolve(strict=True)
            resolved.relative_to(data_home.resolve(strict=True))
        except (OSError, ValueError) as error:
            raise RuntimeError("GUI-owned daemon data directory escapes isolated XDG data") from error
        if path.is_symlink() or not resolved.is_dir():
            raise RuntimeError("GUI-owned daemon data directory is unsafe")
        candidates.append(OwnedDaemon(pid=int(pid), data_dir=resolved))
    if len(candidates) != 1:
        raise RuntimeError("packaged GUI did not own exactly one daemon with an explicit data directory")
    return candidates[0]


def require_pairing_sync(prefix, cli, environment, workspace, primary_socket):
    """Pair two exact bundled daemons and verify one real history transfer."""
    peer_socket = workspace / "peer.sock"
    peer_data = workspace / "peer-data"
    peer_environment = {**environment, "COPYPASTE_SOCKET": str(peer_socket)}
    peer = subprocess.Popen(
        [str(prefix / "copypaste-daemon"), "--foreground", "--data-dir", str(peer_data), "--port", "0"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=peer_environment,
    )
    try:
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            try:
                status(cli, peer_environment)
                break
            except (subprocess.CalledProcessError, json.JSONDecodeError, RuntimeError):
                time.sleep(0.1)
        else:
            raise RuntimeError("paired exact daemon did not become ready")
        invite = ipc(primary_socket, "pair_create_invite").get("data", {}).get("pairing_invite")
        if not isinstance(invite, dict) or not isinstance(invite.get("code"), str) or not isinstance(invite.get("listen_addr"), str):
            raise RuntimeError("GUI-owned daemon did not create a typed pairing invitation")
        joined = ipc(peer_socket, "pair_join", {"code": invite["code"], "addr": invite["listen_addr"]})
        if not isinstance(joined.get("data", {}).get("pairing_progress"), dict):
            raise RuntimeError("paired exact daemon did not report pairing progress")
        for socket_path in (primary_socket, peer_socket):
            response = ipc(socket_path, "pair_confirm", {"accept": True})
            if not isinstance(response.get("data", {}).get("pairing_progress"), dict):
                raise RuntimeError("pairing confirmation did not return typed progress")
        marker = "copypaste-pairing-sync-fixture"
        primary_environment = {**environment, "COPYPASTE_SOCKET": str(primary_socket)}
        run([str(cli), "add", marker], env=primary_environment)
        synced = run([str(cli), "--json", "sync"], env=peer_environment, timeout=30)
        sync_data = response_variant(json.loads(synced.stdout), "sync")
        if not isinstance(sync_data, list) or not sync_data:
            raise RuntimeError("paired exact daemon reported no sync result")
        peer_items = list_items(cli, peer_environment)
        if not any(item.get("content") == marker for item in peer_items):
            raise RuntimeError("paired exact daemon did not receive the synced item")
        emit_result("pairing_sync", synced)
    finally:
        peer.terminate()
        try:
            peer.wait(timeout=5)
        except subprocess.TimeoutExpired:
            peer.kill()
            peer.wait(timeout=5)


def pairing_uri_from_invite(invite):
    """Serialize the versioned URI from the live peer invitation without logging it."""
    if not isinstance(invite, dict):
        raise RuntimeError("camera peer did not create a typed pairing invitation")
    code = invite.get("code")
    address = invite.get("listen_addr")
    pairing_id = invite.get("pairing_id")
    if (not isinstance(code, str) or not code or len(code) > 512
            or not isinstance(address, str) or not address or len(address) > 256
            or not isinstance(pairing_id, str) or not pairing_id):
        raise RuntimeError("camera peer pairing invitation is invalid")
    # `PairingManager::create_invite` always exposes a short PairingCode; the
    # Rust PairingLink contract serializes that exact code as a v2 URI.
    if not re.fullmatch(r"[A-Z0-9]{8}", code):
        raise RuntimeError("camera peer pairing invitation does not use the current short-code contract")
    return "copypaste://pair/v2?" + urlencode({"code": code, "address": address})


def write_private_pairing_uri(path, uri):
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC, 0o600)
    except OSError as error:
        raise RuntimeError("camera pairing URI path already exists") from error
    try:
        value = (uri + "\n").encode("utf-8")
        if os.write(descriptor, value) != len(value):
            raise RuntimeError("could not persist camera pairing URI")
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def remove_private_regular(path):
    if not path.exists():
        return
    details = path.lstat()
    if path.is_symlink() or not stat.S_ISREG(details.st_mode):
        raise RuntimeError("camera qualification path is unsafe")
    path.unlink()


def require_camera_qr_scan(prefix, cli, environment, workspace, gui_pid, runtime_format):
    """Scan a live peer's pairing URI through the installed GUI's V4L2 camera."""
    device_value = os.environ.get("COPYPASTE_QUALIFICATION_V4L2_DEVICE")
    if not isinstance(device_value, str) or not device_value:
        raise RuntimeError("owned qualification V4L2 device is unavailable")
    device = Path(device_value)
    camera = workspace / "camera-qr"
    camera.mkdir(mode=0o700)
    if camera.is_symlink() or stat.S_IMODE(camera.stat().st_mode) != 0o700:
        raise RuntimeError("camera qualification directory must be owner-only")
    uri_file = camera / "pairing-uri"
    ready_file = camera / "ready"
    pid_file = camera / "feed.json"
    peer_socket = workspace / "camera-peer.sock"
    peer_data = workspace / "camera-peer-data"
    peer_environment = {**environment, "COPYPASTE_SOCKET": str(peer_socket)}
    peer = subprocess.Popen(
        [str(prefix / "copypaste-daemon"), "--foreground", "--data-dir", str(peer_data), "--port", "0"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=peer_environment,
    )
    try:
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            try:
                status(cli, peer_environment)
                break
            except (subprocess.CalledProcessError, json.JSONDecodeError, RuntimeError):
                time.sleep(0.1)
        else:
            raise RuntimeError("camera peer daemon did not become ready")
        response = ipc(peer_socket, "pair_create_invite")
        uri = pairing_uri_from_invite(response.get("data", {}).get("pairing_invite"))
        write_private_pairing_uri(uri_file, uri)
        run_media(
            "click", "--pid", str(gui_pid), "--target", "Devices", "--expect", "Nearby devices",
            "--timeout-seconds", "8", kind="atspi_click", emit=False,
        )
        run_media(
            "click", "--pid", str(gui_pid), "--target", "Scan QR", "--expect", "Scan QR code",
            "--timeout-seconds", "8", kind="atspi_click", runtime_format=runtime_format,
        )
        run_media(
            "feed-qr", "--uri-file", str(uri_file), "--device", str(device),
            "--ready-file", str(ready_file), "--pid-file", str(pid_file), kind="v4l2_qr_feed", runtime_format=runtime_format,
        )
        run_media(
            "click", "--pid", str(gui_pid), "--target", "Start scanner", "--expect", "Confirm on both devices",
            "--timeout-seconds", "8", kind="atspi_click", runtime_format=runtime_format,
        )
        observed, _ = run_media(
            "observe-camera", "--pid", str(gui_pid), "--expect", "Confirm on both devices",
            "--runtime-prefix", str(prefix), "--runtime-format", runtime_format,
            kind="camera_qr_scan_observed", runtime_format=runtime_format,
        )
        emit_result("camera_qr_scan", observed)
    finally:
        try:
            if pid_file.exists():
                run_media("stop-qr", "--pid-file", str(pid_file), kind="v4l2_qr_feed_stopped", runtime_format=runtime_format)
        finally:
            remove_private_regular(uri_file)
            peer.terminate()
            try:
                peer.wait(timeout=5)
            except subprocess.TimeoutExpired:
                peer.kill()
                peer.wait(timeout=5)


def read_request_audit(path):
    records = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise RuntimeError("clipboard reader audit is unavailable") from error
    for line in lines:
        try:
            record = json.loads(line)
        except json.JSONDecodeError as error:
            raise RuntimeError("clipboard reader audit is invalid") from error
        if (
            not isinstance(record, dict) or set(record) != {"target", "requestor_pid", "error"}
            or not isinstance(record.get("target"), str)
            or record.get("requestor_pid") is not None and (type(record["requestor_pid"]) is not int or record["requestor_pid"] <= 0)
            or record.get("error") is not None and not isinstance(record["error"], str)
        ):
            raise RuntimeError("clipboard reader audit is invalid")
        records.append(record)
    return records


def require_confidential_reader_audit(records, *, daemon_pid, protected_mime):
    hint = "x-kde-passwordManagerHint"
    if any(record["error"] is not None or type(record["requestor_pid"]) is not int or record["requestor_pid"] <= 0 for record in records):
        raise RuntimeError("clipboard reader audit has an unknown reader")
    if not any(record["target"] == hint and record["requestor_pid"] == daemon_pid and record["error"] is None for record in records):
        raise RuntimeError("GUI-owned daemon did not inspect the confidential clipboard hint")
    for record in records:
        if record["target"] not in {protected_mime, "MULTIPLE"}:
            continue
        if record["error"] is not None or record["requestor_pid"] is None:
            raise RuntimeError("clipboard reader for protected content is unknown")
        if record["requestor_pid"] == daemon_pid:
            raise RuntimeError("GUI-owned daemon read protected clipboard content")


def write_reader_audit(evidence_dir, label, records, *, daemon_pid, protocol):
    (evidence_dir / f"linux-clipboard-reader-{label}.json").write_text(
        json.dumps({"expected_daemon_pid": daemon_pid, "protocol": protocol, "records": records}, separators=(",", ":")) + "\n", encoding="utf-8",
    )


@contextmanager
def gnome_clipboard_read_observer(workspace, daemon_pid, environment):
    if not GNOME_CLIPBOARD_READ_OBSERVER.is_file() or GNOME_CLIPBOARD_READ_OBSERVER.is_symlink():
        raise RuntimeError("GNOME clipboard read observer is unavailable")
    directory = workspace / f"gnome-clipboard-reader-{time.monotonic_ns()}"
    directory.mkdir(mode=0o700)
    ready = directory / "ready.json"
    audit = directory / "audit.jsonl"
    observer = subprocess.Popen(
        [
            sys.executable, str(GNOME_CLIPBOARD_READ_OBSERVER), "--daemon-pid", str(daemon_pid),
            "--ready-file", str(ready), "--audit-file", str(audit),
        ],
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, env=environment,
    )
    try:
        deadline = time.monotonic() + 5
        while not ready.is_file() and observer.poll() is None and time.monotonic() < deadline:
            time.sleep(0.05)
        if observer.poll() is not None or not ready.is_file():
            raise RuntimeError("GNOME clipboard read observer did not become ready")
        try:
            record = json.loads(ready.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise RuntimeError("GNOME clipboard read observer readiness is invalid") from error
        if (
            not isinstance(record, dict) or set(record) != {"schema", "daemon_pid", "bus_owner"}
            or type(record.get("schema")) is not int or record["schema"] != 1
            or type(record.get("daemon_pid")) is not int or record["daemon_pid"] != daemon_pid
            or not isinstance(record.get("bus_owner"), str) or not re.fullmatch(r":[0-9]+\.[0-9]+", record["bus_owner"])
        ):
            raise RuntimeError("GNOME clipboard read observer readiness is invalid")
        yield audit
        if observer.poll() is not None:
            raise RuntimeError("GNOME clipboard read observer exited during the privacy assertion")
    finally:
        if observer.poll() is None:
            observer.terminate()
        try:
            exit_code = observer.wait(timeout=5)
        except subprocess.TimeoutExpired as error:
            observer.kill()
            observer.wait(timeout=5)
            raise RuntimeError("GNOME clipboard read observer did not exit") from error
        if exit_code != 0:
            stderr = observer.stderr.read().decode("utf-8", "replace") if observer.stderr is not None else ""
            message = "GNOME clipboard read observer failed"
            raise RuntimeError(f"{message}: {stderr.strip()}" if stderr.strip() else message)


def require_not_captured(cli, environment, workspace, evidence_dir, label, payloads, assertion=None, protected_mime=None,
                         session=None, desktop=None, owned_daemon=None):
    before = cli_item_count(status(cli, environment))

    def privacy_window():
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            if cli_item_count(status(cli, environment)) != before:
                raise RuntimeError("privacy-protected clipboard content was captured")
            time.sleep(0.15)
        return run([str(cli), "--json", "status"], env=environment)

    reader_audit = None
    if protected_mime is not None and session == "x11":
        reader_audit = workspace / f"clipboard-reader-{label}.jsonl"
        with clipboard_provider(workspace, evidence_dir, label, payloads, environment, request_audit_file=reader_audit):
            final_status = privacy_window()
    elif protected_mime is not None and session == "wayland" and desktop == "GNOME":
        if not isinstance(owned_daemon, OwnedDaemon):
            raise RuntimeError("GUI-owned daemon identity is unavailable for the privacy assertion")
        with gnome_clipboard_read_observer(workspace, owned_daemon.pid, environment) as reader_audit:
            with clipboard_provider(workspace, evidence_dir, label, payloads, environment):
                final_status = privacy_window()
    else:
        with clipboard_provider(workspace, evidence_dir, label, payloads, environment) as activity:
            final_status = privacy_window()
            if protected_mime is not None:
                requests = [] if not activity.exists() else [
                    json.loads(line).get("mime") for line in activity.read_text(encoding="utf-8").splitlines()
                ]
                if "x-kde-passwordManagerHint" not in requests:
                    raise RuntimeError("native backend did not inspect the confidential clipboard hint")
                assert_secret_payload_was_not_requested(activity, protected_mime)
    if reader_audit is not None:
        records = read_request_audit(reader_audit)
        daemon_pid = owned_daemon.pid if isinstance(owned_daemon, OwnedDaemon) else None
        if daemon_pid is None:
            raise RuntimeError("GUI-owned daemon identity is unavailable for the privacy assertion")
        protocol = "x11-xres" if session == "x11" else "gnome-wayland-dbus"
        write_reader_audit(evidence_dir, label, records, daemon_pid=daemon_pid, protocol=protocol)
        require_confidential_reader_audit(records, daemon_pid=daemon_pid, protected_mime=protected_mime)
    if cli_item_count(response_variant(json.loads(final_status.stdout), "status")) != before:
        raise RuntimeError("privacy-protected clipboard content was captured")
    if assertion is not None:
        emit_result(assertion, final_status)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--architecture", required=True, choices=("x86_64", "aarch64"))
    parser.add_argument("--desktop", required=True, choices=("GNOME", "KDE"))
    parser.add_argument("--session", required=True, choices=("x11", "wayland"))
    parser.add_argument("--evidence-dir", required=True, type=Path)
    parser.add_argument("--module-artifacts", required=True, type=Path)
    parser.add_argument("--module-fixtures", required=True, type=Path)
    parser.add_argument("--runtime-format", required=True, choices=("AppImage", "deb", "rpm"))
    parser.add_argument("--runtime-prefix", required=True, type=Path)
    parser.add_argument("--qualification-root", type=Path)
    parser.add_argument("--seed-upgrade-canary", action="store_true")
    parser.add_argument("--previous-artifacts", type=Path)
    parser.add_argument("--previous-version")
    parser.add_argument("--first-install-baseline", action="store_true")
    parser.add_argument("--wayland-desktop-acceptance-helper", type=Path, default=DESKTOP_ACCEPTANCE)
    parser.add_argument("--wayland-quick-paste-helper", type=Path, default=WAYLAND_QUICK_PASTE)
    parser.add_argument("--wayland-actor", type=Path, default=NATIVE_MEDIA)
    parser.add_argument("--wayland-app-permission-target", default="Allow keyboard control")
    args = parser.parse_args()
    if args.first_install_baseline == (args.previous_artifacts is not None):
        raise ValueError("select exactly one of a prior release or first-install baseline")
    args.evidence_dir.mkdir(parents=True, exist_ok=True)
    context = tempfile.TemporaryDirectory(prefix="copypaste-linux-native-") if args.qualification_root is None else None
    with context if context is not None else contextmanager(lambda: (yield args.qualification_root))() as temporary:
        workspace = Path(temporary) if args.qualification_root is None else qualification_workspace(temporary)
        data_home = workspace / "data-home"
        if args.first_install_baseline:
            require_clean_install_baseline(data_home, dict(os.environ))
        write_source_desktop_entry(data_home)
        prefix = runtime_prefix(args.runtime_prefix)
        gui_entrypoint = runtime_gui_entrypoint(prefix, args.runtime_format)
        args.evidence_dir = args.evidence_dir / runtime_evidence_namespace(
            args.runtime_format, seed_upgrade_canary=args.seed_upgrade_canary,
        )
        args.evidence_dir.mkdir(mode=0o700)
        runtime_dir = workspace / "runtime"
        runtime_dir.mkdir(mode=0o700, exist_ok=True)
        if runtime_dir.is_symlink() or stat.S_IMODE(runtime_dir.stat().st_mode) != 0o700:
            raise RuntimeError("qualification runtime directory must be owner-only")
        environment = {
            **os.environ,
            "COPYPASTE_QUALIFICATION": "1",
            "COPYPASTE_QUALIFICATION_CAPABILITY": os.urandom(32).hex(),
            "XDG_DATA_HOME": str(data_home),
            "TMPDIR": str(runtime_dir),
        }
        app, runtime_socket = start_gui_runtime(
            gui_entrypoint, environment, runtime_dir, set(runtime_dir.glob("cp-*.sock")),
        )
        try:
            cli_environment = {**environment, "COPYPASTE_SOCKET": str(runtime_socket)}
            cli = prefix / "copypaste-cli"
            wait_for_startup_status(
                cli, cli_environment, require_native_clipboard=not args.seed_upgrade_canary,
            )
            if args.seed_upgrade_canary:
                seed_upgrade_canary(cli, cli_environment, workspace, prefix / "copypaste")
                return
            upgrading = verify_upgrade_canary(cli, cli_environment, workspace, prefix / "copypaste") if args.qualification_root else False
            gui_status = run([str(cli), "--json", "status"], env=cli_environment)
            try:
                if not isinstance(response_variant(json.loads(gui_status.stdout), "status"), dict):
                    raise RuntimeError("packaged daemon status has an invalid shape")
                emit_result("daemon_cli_gui", gui_status)
                require_capture(cli, cli_environment, workspace, args.evidence_dir, args.session, "text/plain;charset=utf-8", b"copypaste-native-text-fixture", "text", "clipboard_text")
                require_capture(cli, cli_environment, workspace, args.evidence_dir, args.session, "text/html", b"<b>copypaste-native-html-fixture</b>", "text/html", "clipboard_html")
                require_capture(cli, cli_environment, workspace, args.evidence_dir, args.session, "text/rtf", b"{\\rtf1 copypaste native rtf fixture}", "text/rtf", "clipboard_rtf")
                # These are valid, minimal image payloads. The copy-back probe
                # verifies the exact bytes after daemon storage, not merely a
                # row count or image label.
                png = bytes.fromhex(
                    "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c489"
                    "0000000d49444154789c63103209fb0f000294019c1d5b465f0000000049454e44ae426082"
                )
                tiff = bytes.fromhex(
                    "49492a00080000000b00000104000100000001000000010104000100000001000000"
                    "02010300040000009200000003010300010000000100000006010300010000000200"
                    "000011010400010000009a0000001501030001000000040000001601040001000000"
                    "010000001701040001000000040000001c0103000100000001000000520103000100"
                    "000002000000000000000800080008000800123456ff"
                )
                require_capture(cli, cli_environment, workspace, args.evidence_dir, args.session, "image/png", png, "image/png", "clipboard_png")
                require_capture(cli, cli_environment, workspace, args.evidence_dir, args.session, "image/tiff", tiff, "image/tiff", "clipboard_tiff")
                require_file_capture(cli, cli_environment, workspace, args.evidence_dir, args.session, runtime_socket)

                confidential = b"copypaste-confidential-fixture"
                owned_daemon = gui_owned_daemon(app, data_home)
                require_not_captured(
                    cli,
                    cli_environment,
                    workspace,
                    args.evidence_dir,
                    "privacy-confidential",
                    {
                        "text/plain;charset=utf-8": confidential,
                        "x-kde-passwordManagerHint": b"secret",
                    },
                    "privacy_confidential",
                    protected_mime="text/plain;charset=utf-8",
                    session=args.session,
                    desktop=args.desktop,
                    owned_daemon=owned_daemon,
                )

                response = ipc(runtime_socket, "set_private_mode", {"enabled": True})
                if response.get("data", {}).get("private_mode", {}).get("private_mode") is not True:
                    raise RuntimeError("daemon did not acknowledge private mode enabled")
                private_marker = b"copypaste-private-cursor-fixture"
                require_not_captured(cli, cli_environment, workspace, args.evidence_dir, "privacy-private", {"text/plain;charset=utf-8": private_marker})
                response = ipc(runtime_socket, "set_private_mode", {"enabled": False})
                if response.get("data", {}).get("private_mode", {}).get("private_mode") is not False:
                    raise RuntimeError("daemon did not acknowledge private mode disabled")
                # A new post-private value must be captured, while the old
                # value remains absent. This proves the skip advanced the
                # source cursor rather than deferring plaintext capture.
                require_capture(cli, cli_environment, workspace, args.evidence_dir, args.session, "text/plain;charset=utf-8", b"copypaste-after-private-fixture", "text", "privacy_private_mode")
                if any(private_marker.decode("utf-8") in item.get("content", "") for item in list_items(cli, cli_environment)):
                    raise RuntimeError("private-mode clipboard content was replayed after private mode ended")

                configured = response_variant(cli_json(cli, cli_environment, "config", "set", "--excluded-apps", SOURCE_APPLICATION_ID), "config")
                if not isinstance(configured, dict):
                    raise RuntimeError("daemon did not acknowledge the source exclusion")
                try:
                    require_not_captured(cli, cli_environment, workspace, args.evidence_dir, "privacy-excluded-source", {"text/plain;charset=utf-8": b"copypaste-excluded-source-fixture"}, "privacy_excluded_app")
                finally:
                    response_variant(cli_json(cli, cli_environment, "config", "set", "--excluded-apps", ""), "config")

                # A restart must retain a non-sensitive persisted history entry.
                # Configure feedback before this restart so the GUI loads the
                # same notification preferences as the GUI-owned daemon.
                run([
                    str(cli), "--json", "config", "set", "--notify-on-copy", "true",
                    "--notification-preview", "false",
                ], env=cli_environment)
                persisted = "copypaste-restart-fixture"
                run([str(cli), "add", persisted], env=cli_environment)
                prior_sockets = set(runtime_dir.glob("cp-*.sock"))
                app.terminate()
                app.wait(timeout=5)
                app, runtime_socket = start_gui_runtime(
                    gui_entrypoint, environment, runtime_dir, prior_sockets,
                )
                cli_environment = {**environment, "COPYPASTE_SOCKET": str(runtime_socket)}
                restart_items = run([str(cli), "--json", "list", "--limit", "100"], env=cli_environment)
                restart_page = response_variant(json.loads(restart_items.stdout), "page")
                if not isinstance(restart_page, dict) or not any(persisted in item.get("content", "") for item in restart_page.get("items", [])):
                    raise RuntimeError("encrypted history did not survive daemon restart")
                emit_result("encrypted_restart_persistence", restart_items)
                restarted_daemon = gui_owned_daemon(app, data_home)
                if restarted_daemon.data_dir != owned_daemon.data_dir:
                    raise RuntimeError("GUI-owned daemon changed its data directory during restart")
                owned_daemon = restarted_daemon
                if upgrading:
                    verify_upgrade_canary(cli, cli_environment, workspace, prefix / "copypaste")
                    emit_result("package_upgrade", restart_items)
                daemon_data = owned_daemon.data_dir
                def restart_gui_for_modules():
                    nonlocal app, runtime_socket, cli_environment, daemon_data
                    prior_sockets = set(runtime_dir.glob("cp-*.sock"))
                    app.terminate()
                    app.wait(timeout=5)
                    app, runtime_socket = start_gui_runtime(
                        gui_entrypoint, environment, runtime_dir, prior_sockets,
                    )
                    cli_environment = {**environment, "COPYPASTE_SOCKET": str(runtime_socket)}
                    wait_for_startup_status(cli, cli_environment, require_native_clipboard=True)
                    restarted_data = gui_owned_daemon(app, data_home).data_dir
                    if restarted_data != daemon_data:
                        raise RuntimeError("GUI-owned daemon changed its data directory during module qualification")
                    return runtime_socket

                runtime_socket = require_modules(
                    runtime_socket, args.module_artifacts, args.module_fixtures,
                    daemon_data, args.architecture, args.version, args.evidence_dir, restart_gui_for_modules,
                )
                require_linux_onboarding(args, gui_pid=app.pid, environment=environment)
                require_pairing_sync(prefix, cli, environment, workspace, runtime_socket)

                # The remaining assertions deliberately execute their public probes and fail if the exact product
                # does not expose an observable successful result. They are never converted into fixture booleans.
                if args.session == "x11":
                    require_desktop_acceptance(args, gui_pid=app.pid, cli=cli, environment=environment,
                                               workspace=workspace, socket_path=runtime_socket)
                    require_x11_quick_paste(cli, cli_environment, workspace)
                else:
                    require_wayland_qualification(
                        args, gui_pid=app.pid, cli=cli, environment=environment,
                        workspace=workspace, socket_path=runtime_socket,
                    )
                require_external_file_drag(cli, cli_environment, workspace, app.pid, args.session)
                require_camera_qr_scan(prefix, cli, environment, workspace, app.pid, args.runtime_format)
                emit_runtime(args.runtime_format, prefix)
            finally:
                app.terminate()
                try:
                    app.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait(timeout=5)
        finally:
            pass


if __name__ == "__main__":
    main()
