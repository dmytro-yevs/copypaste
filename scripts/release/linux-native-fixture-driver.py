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
import importlib.util
from contextlib import contextmanager
from pathlib import Path
from urllib.parse import unquote, urlparse


PROVIDER = Path(__file__).with_name("linux-clipboard-provider.py")
X11_INPUT_TARGET = Path(__file__).with_name("linux-x11-input-target.c")
MODULE_QUALIFICATION = Path(__file__).with_name("linux-module-qualification.py")
SOURCE_APPLICATION_ID = "org.copypaste.QualificationSource"
SOURCE_APPLICATION_NAME = "CopyPaste Qualification Source"
CAPTURE_TIMEOUT_SECONDS = 8
PROC_ROOT = Path("/proc")


def module_qualification_helper():
    spec = importlib.util.spec_from_file_location("linux_module_qualification", MODULE_QUALIFICATION)
    if spec is None or spec.loader is None:
        raise RuntimeError("Linux module qualification helper is unavailable")
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    return helper


def run(argv, *, input_bytes=None, env=None, timeout=15):
    return subprocess.run(argv, input=input_bytes, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          check=True, timeout=timeout, env=env)


def emit_result(assertion, result):
    argv = result.args if isinstance(result.args, list) else [result.args]
    print("COPYPASTE_QUALIFICATION_COMMAND " + json.dumps({
        "argv": [str(value) for value in argv], "returncode": result.returncode, "assertions": [assertion],
    }, separators=(",", ":")))


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


def ipc(socket_path, method, params):
    request = {"id": 991, "protocol_version": 5, "method": method, "params": params}
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as channel:
        channel.settimeout(5)
        channel.connect(str(socket_path))
        channel.sendall(json.dumps(request, separators=(",", ":")).encode() + b"\n")
        response = channel.makefile("rb").readline()
    decoded = json.loads(response)
    if not decoded.get("ok"):
        raise RuntimeError(f"daemon rejected {method}")
    return decoded


def status(cli, environment):
    value = response_variant(cli_json(cli, environment, "status"), "status")
    if not isinstance(value, dict):
        raise RuntimeError("status has an invalid shape")
    return value


def list_items(cli, environment):
    page = response_variant(cli_json(cli, environment, "list", "--limit", "100"), "page")
    if not isinstance(page, dict) or not isinstance(page.get("items"), list):
        raise RuntimeError("list response has no item page")
    if not all(isinstance(item, dict) for item in page["items"]):
        raise RuntimeError("item page has an invalid item")
    return page["items"]


def write_source_desktop_entry(data_home):
    applications = data_home / "applications"
    applications.mkdir(parents=True, exist_ok=True)
    entry = applications / f"{SOURCE_APPLICATION_ID}.desktop"
    entry.write_text(
        "[Desktop Entry]\n"
        "Type=Application\n"
        f"Name={SOURCE_APPLICATION_NAME}\n"
        f"StartupWMClass={SOURCE_APPLICATION_ID}\n"
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
        target_window = wait_for_window("CopyPaste Qualification Input Target", environment)
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
def clipboard_provider(workspace, evidence_dir, label, payloads, environment):
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
    provider = subprocess.Popen(
        [sys.executable, str(PROVIDER), "--helper", str(helper), "--manifest", str(manifest), "--ready-file", str(ready),
         "--activity-log", str(activity), "--hold-seconds", "20"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
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
        provider.terminate()
        try:
            provider.wait(timeout=5)
        except subprocess.TimeoutExpired:
            provider.kill()
            provider.wait(timeout=5)
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
        wait_for_count(cli, environment, before + 1)
        item = newest_item(list_items(cli, environment), before_ids, content_type)
        require_source_identity(item)
        run([str(cli), "--json", "copy", item["id"]], env=environment)
        copied = external_clipboard_bytes(session, mime, environment)
        if copied.stdout != payload:
            raise RuntimeError(f"native backend did not preserve {mime} bytes on paste")
    emit_result(assertion, copied)


def require_file_capture(cli, environment, workspace, evidence_dir, session):
    original = workspace / "fixture.txt"
    payload = b"CopyPaste qualification file payload\n"
    original.write_bytes(payload)
    before = cli_item_count(status(cli, environment))
    before_ids = {item.get("id") for item in list_items(cli, environment)}
    with clipboard_provider(workspace, evidence_dir, "clipboard-file", {"text/uri-list": original.as_uri().encode("utf-8") + b"\r\n"}, environment):
        wait_for_count(cli, environment, before + 1)
        item = newest_item(list_items(cli, environment), before_ids, "file")
        require_source_identity(item)
        run([str(cli), "--json", "copy", item["id"]], env=environment)
        returned_uri = external_clipboard_bytes(session, "text/uri-list", environment)
        parsed = urlparse(returned_uri.stdout.decode("utf-8").strip())
        returned_file = Path(unquote(parsed.path))
        if parsed.scheme != "file" or returned_file.read_bytes() != payload:
            raise RuntimeError("native backend did not preserve file payload bytes on paste")
    emit_result("clipboard_file", returned_uri)


def wait_for_window(name, environment):
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        result = subprocess.run(["xdotool", "search", "--name", name], stdout=subprocess.PIPE,
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
    run([str(cli), "add", marker], env=environment)
    with x11_input_target(workspace, environment) as (target_window, result_file):
        # The target owns focus before the real global shortcut is sent.
        before_focus = run(["xdotool", "getwindowfocus"], env=environment)
        if before_focus.stdout.decode("utf-8").strip() != target_window:
            raise RuntimeError("X11 input target did not receive focus")
        invoked = run(["xdotool", "key", "ctrl+shift+c"], env=environment)
        quick_paste = wait_for_window("CopyPaste Quick Paste", environment)
        if quick_paste == target_window:
            raise RuntimeError("global shortcut did not open a separate Quick Paste window")
        emit_result("quick_paste_hotkey", invoked)
        # The search field has autofocus, and Enter activates the first item.
        run(["xdotool", "key", "--window", quick_paste, "Return"], env=environment)
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


def gui_daemon_data_dir(gui, data_home):
    """Read the controlled GUI child's explicit daemon data directory."""
    children = PROC_ROOT / str(gui.pid) / "task" / str(gui.pid) / "children"
    try:
        pids = children.read_text(encoding="ascii").split()
    except OSError as error:
        raise RuntimeError("packaged GUI child process inventory is unavailable") from error
    candidates = []
    for pid in pids:
        process = PROC_ROOT / pid
        try:
            status = process.joinpath("status").read_text(encoding="utf-8")
            cmdline = process.joinpath("cmdline").read_bytes().split(b"\0")
        except OSError:
            continue
        uid = next((line.split()[1] for line in status.splitlines() if line.startswith("Uid:") and len(line.split()) >= 2), None)
        if uid != str(os.geteuid()):
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
        candidates.append(resolved)
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
        invite = ipc(primary_socket, "pair_create_invite", {}).get("data", {}).get("pairing_invite")
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


def require_not_captured(cli, environment, workspace, evidence_dir, label, payloads, assertion=None, protected_mime=None):
    before = cli_item_count(status(cli, environment))
    with clipboard_provider(workspace, evidence_dir, label, payloads, environment) as activity:
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            if cli_item_count(status(cli, environment)) != before:
                raise RuntimeError("privacy-protected clipboard content was captured")
            time.sleep(0.15)
        if protected_mime is not None:
            requests = [] if not activity.exists() else [
                json.loads(line).get("mime") for line in activity.read_text(encoding="utf-8").splitlines()
            ]
            if "x-kde-passwordManagerHint" not in requests:
                raise RuntimeError("native backend did not inspect the confidential clipboard hint")
            assert_secret_payload_was_not_requested(activity, protected_mime)
        final_status = run([str(cli), "--json", "status"], env=environment)
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
    parser.add_argument("--previous-artifacts", type=Path)
    parser.add_argument("--previous-version")
    parser.add_argument("--first-install-baseline", action="store_true")
    args = parser.parse_args()
    if args.first_install_baseline == (args.previous_artifacts is not None):
        raise ValueError("select exactly one of a prior release or first-install baseline")
    args.evidence_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="copypaste-linux-native-") as temporary:
        workspace = Path(temporary)
        data_home = workspace / "data-home"
        write_source_desktop_entry(data_home)
        prefix = runtime_prefix(args.runtime_prefix)
        args.evidence_dir = args.evidence_dir / args.runtime_format.lower()
        args.evidence_dir.mkdir(mode=0o700)
        runtime_dir = workspace / "runtime"
        runtime_dir.mkdir(mode=0o700)
        environment = {
            **os.environ,
            "COPYPASTE_QUALIFICATION": "1",
            "XDG_DATA_HOME": str(data_home),
            "TMPDIR": str(runtime_dir),
        }
        app, runtime_socket = start_gui_runtime(prefix / "copypaste", environment, runtime_dir, set())
        try:
            cli_environment = {**environment, "COPYPASTE_SOCKET": str(runtime_socket)}
            cli = prefix / "copypaste-cli"
            deadline = time.monotonic() + 8
            observed_status = None
            while time.monotonic() < deadline:
                try:
                    observed_status = status(cli, cli_environment)
                    break
                except (subprocess.CalledProcessError, json.JSONDecodeError, RuntimeError):
                    time.sleep(0.15)
            if observed_status is None:
                raise RuntimeError("GUI-owned daemon did not become ready")
            if observed_status.get("capture_running") is not True:
                raise RuntimeError("GUI-owned daemon did not report an active native clipboard backend")
            backend = observed_status.get("clipboard_backend")
            if not isinstance(backend, str) or not backend.startswith("linux-"):
                raise RuntimeError("GUI-owned daemon did not report a Linux system clipboard backend")
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
                require_file_capture(cli, cli_environment, workspace, args.evidence_dir, args.session)

                confidential = b"copypaste-confidential-fixture"
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
                persisted = "copypaste-restart-fixture"
                run([str(cli), "add", persisted], env=cli_environment)
                prior_sockets = set(runtime_dir.glob("cp-*.sock"))
                app.terminate()
                app.wait(timeout=5)
                app, runtime_socket = start_gui_runtime(
                    prefix / "copypaste", environment, runtime_dir, prior_sockets,
                )
                cli_environment = {**environment, "COPYPASTE_SOCKET": str(runtime_socket)}
                restart_items = run([str(cli), "--json", "list", "--limit", "100"], env=cli_environment)
                restart_page = response_variant(json.loads(restart_items.stdout), "page")
                if not isinstance(restart_page, dict) or not any(persisted in item.get("content", "") for item in restart_page.get("items", [])):
                    raise RuntimeError("encrypted history did not survive daemon restart")
                emit_result("encrypted_restart_persistence", restart_items)
                daemon_data = gui_daemon_data_dir(app, data_home)
                def restart_gui_for_modules():
                    nonlocal app, runtime_socket, cli_environment, daemon_data
                    prior_sockets = set(runtime_dir.glob("cp-*.sock"))
                    app.terminate()
                    app.wait(timeout=5)
                    app, runtime_socket = start_gui_runtime(
                        prefix / "copypaste", environment, runtime_dir, prior_sockets,
                    )
                    cli_environment = {**environment, "COPYPASTE_SOCKET": str(runtime_socket)}
                    if not isinstance(status(cli, cli_environment), dict):
                        raise RuntimeError("GUI-owned daemon did not become ready after module lifecycle restart")
                    restarted_data = gui_daemon_data_dir(app, data_home)
                    if restarted_data != daemon_data:
                        raise RuntimeError("GUI-owned daemon changed its data directory during module qualification")
                    return runtime_socket

                runtime_socket = require_modules(
                    runtime_socket, args.module_artifacts, args.module_fixtures,
                    daemon_data, args.architecture, args.version, args.evidence_dir, restart_gui_for_modules,
                )
                require_pairing_sync(prefix, cli, environment, workspace, runtime_socket)

                # The remaining assertions deliberately execute their public probes and fail if the exact product
                # does not expose an observable successful result. They are never converted into fixture booleans.
                if args.session == "x11":
                    require_x11_quick_paste(cli, cli_environment, workspace)
                else:
                    run(["gdbus", "introspect", "--session", "--dest", "app.copypaste.CopyPaste", "--object-path", "/app/copypaste/WaylandIntegration"])
                    raise RuntimeError("Wayland companion authentication and portal keyboard grant require an enabled companion transaction")
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
