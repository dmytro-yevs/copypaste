#!/usr/bin/env python3
"""Redacted, fixture-only KWin clipboard bridge observer.

The Gio filter deliberately never calls get_body for Read or Write.  Its JSON
records only bridge outcome metadata, so generated CI artifacts cannot contain
clipboard content, error text, source application IDs, titles, or paths.
"""
import argparse, json, subprocess, time
from pathlib import Path
import gi
gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib

BUS = "org.kde.KWin"; PATH = "/app/copypaste/Clipboard"; IFACE = "app.copypaste.Clipboard"
ERRORS = {"AccessDenied", "StaleSelection", "UnsupportedMime", "TooLarge", "Unavailable"}

def emit(kind, **fields):
    print("COPYPASTE_KWIN_DIAGNOSTIC " + json.dumps({"kind": kind, **fields}, sort_keys=True, separators=(",", ":")), flush=True)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=int, default=12)
    parser.add_argument("--x11-helper", type=Path)
    parser.add_argument("--ready-file", type=Path)
    args = parser.parse_args()
    if not 1 <= args.seconds <= 30: raise SystemExit(2)
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    owner = bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetNameOwner", GLib.Variant("(s)", (BUS,)), GLib.VariantType("(s)"), Gio.DBusCallFlags.NONE, 2000, None).unpack()[0]
    bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus.Monitoring", "BecomeMonitor",
                  GLib.Variant("(asu)", (["type='method_call',destination='org.kde.KWin',path='/app/copypaste/Clipboard',interface='app.copypaste.Clipboard'", "type='method_return'", "type='error'", "type='signal',sender='org.kde.KWin',path='/app/copypaste/Clipboard',interface='app.copypaste.Clipboard'"], 0)),
                  None, Gio.DBusCallFlags.NONE, 2000, None)
    if args.ready_file:
        args.ready_file.write_text("ready\n", encoding="ascii")
    reads = {}
    def observe(_connection, message, _incoming):
        member = message.get_member(); message_type = message.get_message_type()
        key = (message.get_destination(), message.get_reply_serial())
        if message_type == Gio.DBusMessageType.METHOD_CALL and member == "Read" and message.get_destination() == BUS and message.get_path() == PATH:
            reads[(message.get_sender(), message.get_serial())] = time.monotonic()
            emit("read_call", serial=message.get_serial(), signature=message.get_signature() or "")
        elif message_type in (Gio.DBusMessageType.METHOD_RETURN, Gio.DBusMessageType.ERROR) and message.get_sender() == owner and key in reads:
            started = reads.pop(key)
            name = (message.get_error_name() or "").rsplit(".", 1)[-1]
            emit("read_reply", reply_serial=message.get_reply_serial(), outcome="ok" if message_type == Gio.DBusMessageType.METHOD_RETURN else "error", error_name=name if name in ERRORS else "other", signature=message.get_signature() or "", byte_count=len(message.to_blob(Gio.DBusCapabilityFlags.NONE)), duration_ms=round((time.monotonic()-started)*1000))
        elif message_type == Gio.DBusMessageType.SIGNAL and message.get_sender() == owner and member == "OwnerChanged":
            body = message.get_body(); generation, _mimes, identity = body.unpack()
            status, pid, uid, _app_id = identity
            emit("owner_changed", generation=int(generation), status=status if status in {"verified","no-client","no-app-id","ambiguous"} else "other", pid=int(pid), uid=int(uid))
        return message
    bus.add_filter(observe)
    if args.x11_helper:
        def observe_x11():
            result = subprocess.run([str(args.x11_helper)], capture_output=True, text=True, check=False)
            emit("x11_owner", **(json.loads(result.stdout) if result.returncode == 0 else {"available": False}))
            return GLib.SOURCE_REMOVE
        GLib.timeout_add_seconds(2, observe_x11)
    loop = GLib.MainLoop(); GLib.timeout_add_seconds(args.seconds, loop.quit); loop.run()
    emit("complete", unresolved_reads=len(reads))
if __name__ == "__main__": main()
