#!/usr/bin/env python3
"""Own a GTK clipboard selection for Linux native qualification.

The production daemon must observe a clipboard owned by another desktop
application.  Shell helpers such as ``xclip`` and ``wl-copy`` are useful for
manual inspection but cannot give the selection a stable desktop identity or
offer the KDE password-manager hint together with a real payload.  This
process is intentionally small: it owns one visible GTK window, serves only
the MIME payloads named in a private manifest, and records MIME requests
without recording clipboard bytes.
"""

import argparse
import json
import os
import sys
import time
from pathlib import Path


def load_manifest(path: Path) -> tuple[str, list[tuple[str, bytes]]]:
    """Read bounded, regular payload files without ever printing their bytes."""
    if path.is_symlink() or not path.is_file():
        raise ValueError("clipboard manifest must be a regular file")
    raw = path.read_bytes()
    if len(raw) > 64 * 1024:
        raise ValueError("clipboard manifest is too large")
    document = json.loads(raw)
    if not isinstance(document, dict) or set(document) != {"application_id", "offers"}:
        raise ValueError("clipboard manifest has an invalid shape")
    application_id = document["application_id"]
    offers = document["offers"]
    if not isinstance(application_id, str) or not application_id or len(application_id) > 255:
        raise ValueError("clipboard source application id is invalid")
    if not isinstance(offers, list) or not (1 <= len(offers) <= 16):
        raise ValueError("clipboard offer count is invalid")

    loaded = []
    seen = set()
    for offer in offers:
        if not isinstance(offer, dict) or set(offer) != {"mime", "path"}:
            raise ValueError("clipboard offer has an invalid shape")
        mime = offer["mime"]
        payload_path = Path(offer["path"])
        if (
            not isinstance(mime, str)
            or not mime
            or len(mime.encode("utf-8")) > 255
            or "\x00" in mime
            or mime in seen
            or payload_path.is_symlink()
            or not payload_path.is_file()
        ):
            raise ValueError("clipboard offer is invalid")
        payload = payload_path.read_bytes()
        if len(payload) > 32 * 1024 * 1024:
            raise ValueError("clipboard payload is too large")
        seen.add(mime)
        loaded.append((mime, payload))
    return application_id, loaded


def append_activity(path: Path, mime: str) -> None:
    """Record the requested MIME only; clipboard payloads must not leak."""
    with path.open("a", encoding="utf-8") as stream:
        stream.write(json.dumps({"mime": mime}, separators=(",", ":")) + "\n")
        stream.flush()
        os.fsync(stream.fileno())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--ready-file", required=True, type=Path)
    parser.add_argument("--activity-log", required=True, type=Path)
    parser.add_argument("--hold-seconds", type=int, default=20)
    args = parser.parse_args()
    if not 1 <= args.hold_seconds <= 60:
        raise ValueError("clipboard provider hold duration is invalid")

    application_id, offers = load_manifest(args.manifest)
    for path in (args.ready_file, args.activity_log):
        if path.exists() or path.is_symlink():
            raise ValueError("clipboard provider output already exists")
        path.parent.mkdir(parents=True, exist_ok=True)

    import gi

    gi.require_version("Gdk", "3.0")
    gi.require_version("Gtk", "3.0")
    from gi.repository import Gdk, Gio, GLib, Gtk

    application = Gtk.Application(
        application_id=application_id,
        flags=Gio.ApplicationFlags.FLAGS_NONE,
    )

    def activate(_application):
        window = Gtk.ApplicationWindow(application=application, title="CopyPaste qualification source")
        window.set_default_size(240, 80)
        # X11 attribution resolves the selection-owner WM_CLASS through this
        # desktop id.  GTK maps the application id to Wayland identity itself.
        window.set_wmclass(application_id, application_id)
        window.add(Gtk.Label(label="CopyPaste qualification source"))
        window.show_all()

        clipboard = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)
        targets = [Gtk.TargetEntry.new(mime, 0, index) for index, (mime, _) in enumerate(offers)]

        def get_data(_clipboard, selection_data, info, _user_data):
            index = int(info)
            mime, payload = offers[index]
            append_activity(args.activity_log, mime)
            selection_data.set(Gdk.Atom.intern(mime, False), 8, payload)

        def clear_data(*_unused):
            return None

        if not clipboard.set_with_data(targets, get_data, clear_data, None):
            raise RuntimeError("GTK refused clipboard ownership")
        # A regular file is the readiness handoff. It contains identity and
        # MIME names only, never a clipboard value.
        descriptor = {"application_id": application_id, "mimes": [mime for mime, _ in offers]}
        args.ready_file.write_text(json.dumps(descriptor, separators=(",", ":")) + "\n", encoding="utf-8")
        GLib.timeout_add_seconds(args.hold_seconds, application.quit)

    application.connect("activate", activate)
    return application.run([])


if __name__ == "__main__":
    raise SystemExit(main())
