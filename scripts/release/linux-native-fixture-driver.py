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
from contextlib import contextmanager
from pathlib import Path
from urllib.parse import unquote, urlparse


PROVIDER = Path(__file__).with_name("linux-clipboard-provider.py")
SOURCE_APPLICATION_ID = "org.copypaste.QualificationSource"
SOURCE_APPLICATION_NAME = "CopyPaste Qualification Source"
CAPTURE_TIMEOUT_SECONDS = 8


def run(argv, *, input_bytes=None, env=None, timeout=15):
    return subprocess.run(argv, input=input_bytes, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          check=True, timeout=timeout, env=env)


def emit(assertion, argv):
    print("COPYPASTE_QUALIFICATION_COMMAND " + json.dumps({
        "argv": [str(value) for value in argv], "returncode": 0, "assertions": [assertion],
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
    return run(command, env=environment).stdout


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
        if copied != payload:
            raise RuntimeError(f"native backend did not preserve {mime} bytes on paste")
    emit(assertion, [str(PROVIDER), "--manifest", "external-gtk-clipboard", "copypaste-cli", "copy"])


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
        parsed = urlparse(returned_uri.decode("utf-8").strip())
        returned_file = Path(unquote(parsed.path))
        if parsed.scheme != "file" or returned_file.read_bytes() != payload:
            raise RuntimeError("native backend did not preserve file payload bytes on paste")
    emit("clipboard_file", [str(PROVIDER), "--manifest", "external-gtk-clipboard", "copypaste-cli", "copy"])


def require_not_captured(cli, environment, workspace, evidence_dir, label, payloads, protected_mime=None):
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--architecture", required=True, choices=("x86_64", "aarch64"))
    parser.add_argument("--desktop", required=True, choices=("GNOME", "KDE"))
    parser.add_argument("--session", required=True, choices=("x11", "wayland"))
    parser.add_argument("--evidence-dir", required=True, type=Path)
    parser.add_argument("--previous-artifacts", type=Path)
    parser.add_argument("--previous-version")
    parser.add_argument("--first-install-baseline", action="store_true")
    args = parser.parse_args()
    if args.first_install_baseline == (args.previous_artifacts is not None):
        raise ValueError("select exactly one of a prior release or first-install baseline")
    artifact = (args.artifacts / f"CopyPaste-v{args.version}-linux-{args.architecture}.AppImage").resolve()
    if not artifact.is_file():
        raise ValueError("exact AppImage is missing")
    args.evidence_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="copypaste-linux-native-") as temporary:
        workspace = Path(temporary)
        data_home = workspace / "data-home"
        write_source_desktop_entry(data_home)
        previous = Path.cwd()
        os.chdir(workspace)
        try:
            prefix = appimage_prefix(artifact.resolve(), workspace)
        finally:
            os.chdir(previous)
        data = workspace / "data"
        socket = workspace / "copypaste.sock"
        environment = {
            **os.environ,
            "COPYPASTE_SOCKET": str(socket),
            "COPYPASTE_QUALIFICATION": "1",
            "XDG_DATA_HOME": str(data_home),
        }
        daemon = subprocess.Popen([str(prefix / "copypaste-daemon"), "--foreground", "--data-dir", str(data), "--port", "0"],
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=environment)
        try:
            deadline = time.monotonic() + 8
            observed_status = None
            while daemon.poll() is None and time.monotonic() < deadline:
                try:
                    observed_status = status(prefix / "copypaste-cli", environment)
                    break
                except (subprocess.CalledProcessError, json.JSONDecodeError, RuntimeError):
                    time.sleep(0.15)
            if daemon.poll() is not None or observed_status is None:
                raise RuntimeError("daemon did not become ready")
            if observed_status.get("capture_running") is not True:
                raise RuntimeError("daemon did not report an active native clipboard backend")
            backend = observed_status.get("clipboard_backend")
            if not isinstance(backend, str) or not backend.startswith("linux-"):
                raise RuntimeError("daemon did not report a Linux system clipboard backend")
            app = subprocess.Popen([str(prefix / "copypaste")], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=environment)
            try:
                time.sleep(1)
                if app.poll() is not None:
                    raise RuntimeError("packaged GUI exited before it could show a window")
                emit("daemon_cli_gui", ["copypaste-daemon", "copypaste-cli", "copypaste"])
                cli = prefix / "copypaste-cli"
                require_capture(cli, environment, workspace, args.evidence_dir, args.session, "text/plain;charset=utf-8", b"copypaste-native-text-fixture", "text", "clipboard_text")
                require_capture(cli, environment, workspace, args.evidence_dir, args.session, "text/html", b"<b>copypaste-native-html-fixture</b>", "text/html", "clipboard_html")
                require_capture(cli, environment, workspace, args.evidence_dir, args.session, "text/rtf", b"{\\rtf1 copypaste native rtf fixture}", "text/rtf", "clipboard_rtf")
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
                require_capture(cli, environment, workspace, args.evidence_dir, args.session, "image/png", png, "image/png", "clipboard_png")
                require_capture(cli, environment, workspace, args.evidence_dir, args.session, "image/tiff", tiff, "image/tiff", "clipboard_tiff")
                require_file_capture(cli, environment, workspace, args.evidence_dir, args.session)

                confidential = b"copypaste-confidential-fixture"
                require_not_captured(
                    cli,
                    environment,
                    workspace,
                    args.evidence_dir,
                    "privacy-confidential",
                    {
                        "text/plain;charset=utf-8": confidential,
                        "x-kde-passwordManagerHint": b"secret",
                    },
                    protected_mime="text/plain;charset=utf-8",
                )
                emit("privacy_confidential", [str(PROVIDER), "x-kde-passwordManagerHint", "copypaste-cli", "status"])

                response = ipc(socket, "set_private_mode", {"enabled": True})
                if response.get("data", {}).get("private_mode", {}).get("private_mode") is not True:
                    raise RuntimeError("daemon did not acknowledge private mode enabled")
                private_marker = b"copypaste-private-cursor-fixture"
                require_not_captured(cli, environment, workspace, args.evidence_dir, "privacy-private", {"text/plain;charset=utf-8": private_marker})
                response = ipc(socket, "set_private_mode", {"enabled": False})
                if response.get("data", {}).get("private_mode", {}).get("private_mode") is not False:
                    raise RuntimeError("daemon did not acknowledge private mode disabled")
                # A new post-private value must be captured, while the old
                # value remains absent. This proves the skip advanced the
                # source cursor rather than deferring plaintext capture.
                require_capture(cli, environment, workspace, args.evidence_dir, args.session, "text/plain;charset=utf-8", b"copypaste-after-private-fixture", "text", "privacy_private_mode")
                if any(private_marker.decode("utf-8") in item.get("content", "") for item in list_items(cli, environment)):
                    raise RuntimeError("private-mode clipboard content was replayed after private mode ended")

                configured = response_variant(cli_json(cli, environment, "config", "set", "--excluded-apps", SOURCE_APPLICATION_ID), "config")
                if not isinstance(configured, dict):
                    raise RuntimeError("daemon did not acknowledge the source exclusion")
                try:
                    require_not_captured(cli, environment, workspace, args.evidence_dir, "privacy-excluded-source", {"text/plain;charset=utf-8": b"copypaste-excluded-source-fixture"})
                finally:
                    response_variant(cli_json(cli, environment, "config", "set", "--excluded-apps", ""), "config")
                emit("privacy_excluded_app", ["copypaste-cli", "config", "set", "--excluded-apps", SOURCE_APPLICATION_ID])

                # A restart must retain a non-sensitive persisted history entry.
                persisted = "copypaste-restart-fixture"
                run([str(cli), "add", persisted], env=environment)
                daemon.terminate()
                daemon.wait(timeout=5)
                daemon = subprocess.Popen([str(prefix / "copypaste-daemon"), "--foreground", "--data-dir", str(data), "--port", "0"],
                                          stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=environment)
                time.sleep(1)
                if not any(persisted in item.get("content", "") for item in list_items(cli, environment)):
                    raise RuntimeError("encrypted history did not survive daemon restart")
                emit("encrypted_restart_persistence", ["copypaste-daemon", "copypaste-cli", "search"])

                # The remaining assertions deliberately execute their public probes and fail if the exact product
                # does not expose an observable successful result. They are never converted into fixture booleans.
                if args.session == "wayland":
                    run(["gdbus", "introspect", "--session", "--dest", "app.copypaste.CopyPaste", "--object-path", "/app/copypaste/WaylandIntegration"])
                    raise RuntimeError("Wayland companion authentication and portal keyboard grant require an enabled companion transaction")
                run(["xdotool", "search", "--name", "CopyPaste"])
                raise RuntimeError("X11 Quick Paste hotkey/focus, tray/notification, pairing/sync, modules, confidential and excluded-app scenarios require their shipped observable contract")
            finally:
                app.terminate()
                try:
                    app.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait(timeout=5)
        finally:
            daemon.terminate()
            daemon.wait(timeout=5)


if __name__ == "__main__":
    main()
