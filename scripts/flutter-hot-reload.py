#!/usr/bin/env python3
"""Hot reload a running macOS Flutter session when Dart source files change."""

import argparse
import os
from pathlib import Path
import signal
import time


def snapshot(directory):
    files = {}
    for path in directory.rglob("*.dart"):
        try:
            files[path] = path.stat().st_mtime_ns
        except FileNotFoundError:
            pass
    return files


def watch(app, pid_file):
    previous = snapshot(app / "lib")
    active_pid = None
    pending = set()
    last_change = 0.0
    print(f"Watching {app / 'lib'} for Dart changes. Ctrl+C stops the watcher.", flush=True)
    while True:
        time.sleep(0.5)
        try:
            pid = int(pid_file.read_text().strip())
        except (FileNotFoundError, ValueError):
            pid = None
        current = snapshot(app / "lib")
        if pid != active_pid:
            # A newly built process already contains the current source files.
            active_pid, previous = pid, current
            pending.clear()
            continue
        changed = {path for path in previous.keys() | current.keys()
                   if previous.get(path) != current.get(path)}
        previous = current
        if changed:
            pending.update(changed)
            last_change = time.monotonic()
        if not pid or not pending or time.monotonic() - last_change < 0.8:
            continue
        try:
            os.kill(pid, signal.SIGUSR1)
        except ProcessLookupError:
            continue
        print("Hot reload requested.", flush=True)
        pending.clear()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid-file", type=Path, required=True)
    parser.add_argument("--app", type=Path,
                        default=Path(__file__).resolve().parent.parent / "apps/copypaste_flutter")
    args = parser.parse_args()
    if not hasattr(signal, "SIGUSR1"):
        parser.error("This watcher requires Unix signals; use Flutter IDE hot reload on Windows.")
    if not (args.app / "lib").is_dir():
        parser.error("The Flutter application must contain a lib directory.")
    try:
        watch(args.app.resolve(), args.pid_file)
    except KeyboardInterrupt:
        print("\nWatcher stopped.")
