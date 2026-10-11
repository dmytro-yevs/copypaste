#!/usr/bin/env python3
"""Run one real GTK clipboard capture and emit metadata-only diagnostics."""
import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import re
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("fixture", ROOT / "scripts/release/linux-native-fixture-driver.py")
fixture = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(fixture)


def daemon_flags(gui_pid, helper_environment):
    for entry in Path("/proc").glob("[0-9]*"):
        try:
            if int((entry / "stat").read_text().split()[3]) != gui_pid:
                continue
            values = dict(value.split("=", 1) for value in (entry / "environ").read_bytes().decode(errors="ignore").split("\0") if "=" in value)
            data_home = values.get("XDG_DATA_HOME", "")
            entry_path = Path(data_home) / "applications" / f"{fixture.SOURCE_APPLICATION_ID}.desktop"
            return {"daemon_found": True, "xdg_matches_helper": bool(data_home) and data_home == helper_environment.get("XDG_DATA_HOME"), "home_present": bool(values.get("HOME")), "source_desktop_entry_accessible": entry_path.is_file() and not entry_path.is_symlink()}
        except (OSError, ValueError, IndexError):
            continue
    return {"daemon_found": False}


def capture_state(cli, environment, socket_path):
    status = fixture.status(cli, environment)
    private = fixture.ipc(socket_path, "get_private_mode", {}).get("data", {}).get("private_mode", {})
    return {"capture_running": status.get("capture_running") is True, "clipboard_backend": status.get("clipboard_backend") if isinstance(status.get("clipboard_backend"), str) else "invalid", "private_mode": private.get("private_mode") is True, "item_count": fixture.cli_item_count(status)}


def start_gui_diagnostic(executable, environment, runtime_dir, output):
    stderr = runtime_dir / "gui.startup.stderr"
    with stderr.open("wb") as sink:
        app = subprocess.Popen([str(executable)], stdout=subprocess.DEVNULL, stderr=sink, env=environment)
        try:
            socket_path = fixture.wait_for_gui_socket(runtime_dir, set())
            return app, socket_path, {"gui_exit_code": None, "startup_stderr_bytes": stderr.stat().st_size}
        except Exception:
            exit_code = app.poll()
            payload = stderr.read_bytes()[:65536]
            categories = {"missing_display": b"display" in payload.lower(), "missing_wayland": b"wayland" in payload.lower(), "library_load": b"shared librar" in payload.lower()}
            match = re.search(rb"error while loading shared libraries: ([A-Za-z0-9.+_-]+\.so(?:\.[0-9]+)*):", payload)
            record = {"kind": "first_clipboard_capture", "captured": False, "failed_before_capture": True, "gui_startup": {"gui_exit_code": exit_code, "startup_stderr_bytes": len(payload), "startup_stderr_sha256": hashlib.sha256(payload).hexdigest(), "startup_error_categories": categories, "missing_soname": match.group(1).decode("ascii") if match else None}}
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n", encoding="utf-8")
            raise RuntimeError("packaged GUI did not expose its app-owned runtime socket")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime-prefix", required=True, type=Path)
    parser.add_argument("--gui-launcher", required=True, type=Path)
    parser.add_argument("--session", required=True, choices=("x11", "wayland"))
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    prefix = fixture.runtime_prefix(args.runtime_prefix)
    workspace = Path(tempfile.mkdtemp(prefix="copypaste-first-clipboard-"))
    data_home, runtime = workspace / "data", workspace / "runtime"
    data_home.mkdir(mode=0o700); runtime.mkdir(mode=0o700)
    fixture.write_source_desktop_entry(data_home)
    environment = {**os.environ, "COPYPASTE_QUALIFICATION": "1", "XDG_DATA_HOME": str(data_home), "TMPDIR": str(runtime)}
    app, socket_path, gui_startup = start_gui_diagnostic(args.gui_launcher, environment, runtime, args.output)
    record = {"kind": "first_clipboard_capture", "captured": False, "gui_startup": gui_startup, "daemon_environment": daemon_flags(app.pid, environment), "before_provider": capture_state(cli, cli_environment, socket_path)}
    try:
        cli_environment = {**environment, "COPYPASTE_SOCKET": str(socket_path)}
        cli = prefix / "copypaste-cli"; deadline = time.monotonic() + 8; before = None
        while time.monotonic() < deadline:
            try:
                before = fixture.cli_item_count(fixture.status(cli, cli_environment)); break
            except Exception:
                time.sleep(.15)
        if before is None: raise RuntimeError("GUI-owned daemon did not become ready")
        observer = None
        observer_path = os.environ.get("COPYPASTE_DIAGNOSTIC_KWIN_OBSERVER")
        if observer_path:
            observer_ready = runtime / "kwin-observer.ready"
            observer = subprocess.Popen([sys.executable, observer_path, "--seconds", "30", "--x11-helper", os.environ["COPYPASTE_DIAGNOSTIC_X11_HELPER"], "--ready-file", str(observer_ready)], stdout=open(os.environ["COPYPASTE_DIAGNOSTIC_KWIN_OUTPUT"], "wb"), stderr=subprocess.DEVNULL, env=environment)
            deadline = time.monotonic() + 5
            while not observer_ready.is_file() and observer.poll() is None and time.monotonic() < deadline: time.sleep(.05)
            if observer.poll() is not None or not observer_ready.is_file(): raise RuntimeError("KWin observer did not become ready")
        with fixture.clipboard_provider(workspace, workspace, "first-capture", {"text/plain;charset=utf-8": b"copypaste-diagnostic"}, environment):
            record["during_provider"] = capture_state(cli, cli_environment, socket_path)
            fixture.wait_for_count(cli, cli_environment, before + 1)
            after = fixture.cli_item_count(fixture.status(cli, cli_environment))
            record.update({"before_item_count": before, "after_item_count": after, "captured": after > before, "daemon_environment": daemon_flags(app.pid, environment), "after_provider": capture_state(cli, cli_environment, socket_path)})
        if observer:
            observer.wait(timeout=35)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n", encoding="utf-8")
        print("COPYPASTE_FIRST_CLIPBOARD_DIAGNOSTIC " + json.dumps(record, sort_keys=True, separators=(",", ":")))
    except Exception as error:
        record["failed_before_capture"] = True
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n", encoding="utf-8")
        raise
    finally:
        app.terminate(); app.wait(timeout=5)
        shutil.rmtree(workspace, ignore_errors=True)


if __name__ == "__main__": main()
