#!/usr/bin/env python3
"""Redacted, fixture-only KWin clipboard bridge observer.

The Gio filter deliberately never calls get_body for Read or Write.  Its JSON
records only bridge outcome metadata, so generated CI artifacts cannot contain
clipboard content, error text, source application IDs, titles, or paths.
"""
import argparse, json, time
from xml.etree import ElementTree
from pathlib import Path
import gi
gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib

BUS = "org.kde.KWin"; PATH = "/app/copypaste/Clipboard"; IFACE = "app.copypaste.Clipboard"
ERRORS = {
    "app.copypaste.Clipboard.Error.AccessDenied", "app.copypaste.Clipboard.Error.StaleSelection",
    "app.copypaste.Clipboard.Error.UnsupportedMime", "app.copypaste.Clipboard.Error.TooLarge",
    "app.copypaste.Clipboard.Error.Unavailable", "org.freedesktop.DBus.Error.Failed",
    "org.freedesktop.DBus.Error.NoReply", "org.freedesktop.DBus.Error.InvalidArgs",
    "org.freedesktop.DBus.Error.UnknownMethod", "org.freedesktop.DBus.Error.NotSupported",
    "org.freedesktop.DBus.Error.AccessDenied", "org.freedesktop.DBus.Error.Disconnected",
    "org.freedesktop.DBus.Error.Timeout", "org.freedesktop.DBus.Error.TimedOut",
}

def emit(kind, **fields):
    print("COPYPASTE_KWIN_DIAGNOSTIC " + json.dumps({"kind": kind, **fields}, sort_keys=True, separators=(",", ":")), flush=True)

def interface_metadata(bus, owner):
    try:
        xml = bus.call_sync(owner, PATH, "org.freedesktop.DBus.Introspectable", "Introspect", GLib.Variant("()", ()), GLib.VariantType("(s)"), Gio.DBusCallFlags.NONE, 2000, None).unpack()[0]
        root = ElementTree.fromstring(xml)
        interface = next(item for item in root.findall("interface") if item.get("name") == IFACE)
        snapshot = next(item for item in interface.findall("method") if item.get("name") == "Snapshot")
        out_types = [item.get("type") for item in snapshot.findall("arg") if item.get("direction") == "out"]
        emit("interface", snapshot_exported=True, snapshot_out_signature="".join(out_types), snapshot_out_count=len(out_types))
    except (ElementTree.ParseError, StopIteration, TypeError, ValueError):
        emit("interface", snapshot_exported=False)

def reported_error_name(message):
    name = message.get_error_name() or ""
    return name if name in ERRORS else "other"

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=int, default=12)
    parser.add_argument("--ready-file", type=Path)
    args = parser.parse_args()
    if not 1 <= args.seconds <= 30: raise SystemExit(2)
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    owner = bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetNameOwner", GLib.Variant("(s)", (BUS,)), GLib.VariantType("(s)"), Gio.DBusCallFlags.NONE, 2000, None).unpack()[0]
    interface_metadata(bus, owner)
    destinations = {BUS, owner}
    bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus.Monitoring", "BecomeMonitor",
                  GLib.Variant("(asu)", ([
                      *(f"type='method_call',destination='{destination}',path='{PATH}',interface='{IFACE}'" for destination in destinations),
                      "type='method_return'", "type='error'",
                      f"type='signal',sender='{owner}',path='{PATH}',interface='{IFACE}'",
                  ], 0)),
                  None, Gio.DBusCallFlags.NONE, 2000, None)
    if args.ready_file:
        args.ready_file.write_text("ready\n", encoding="ascii")
    calls = {}
    def observe(_connection, message, _incoming):
        member = message.get_member(); message_type = message.get_message_type()
        key = (message.get_destination(), message.get_reply_serial())
        if (message_type == Gio.DBusMessageType.METHOD_CALL and member in {"Version", "Snapshot", "Read"}
                and message.get_destination() in destinations and message.get_path() == PATH and message.get_interface() == IFACE):
            calls[(message.get_sender(), message.get_serial())] = (member.lower(), time.monotonic())
            emit(f"{member.lower()}_call", serial=message.get_serial(), signature=message.get_signature() or "")
        elif message_type in (Gio.DBusMessageType.METHOD_RETURN, Gio.DBusMessageType.ERROR) and message.get_sender() == owner and key in calls:
            kind, started = calls.pop(key)
            fields = {"reply_serial": message.get_reply_serial(), "outcome": "ok" if message_type == Gio.DBusMessageType.METHOD_RETURN else "error", "error_name": reported_error_name(message), "signature": message.get_signature() or "", "duration_ms": round((time.monotonic()-started)*1000)}
            if kind == "read": fields["byte_count"] = len(message.to_blob(Gio.DBusCapabilityFlags.NONE))
            emit(f"{kind}_reply", **fields)
        elif message_type == Gio.DBusMessageType.SIGNAL and message.get_sender() == owner and member == "OwnerChanged":
            body = message.get_body(); generation, _mimes, identity = body.unpack()
            status, pid, uid, _app_id = identity
            emit("owner_changed", generation=int(generation), status=status if status in {"verified","no-client","no-app-id","ambiguous"} else "other", pid=int(pid), uid=int(uid))
        # A monitor observes incoming traffic; forwarding it would make GDBus
        # dispatch a second, invalid reply to the peer's method call.
        return None if _incoming else message
    bus.add_filter(observe)
    loop = GLib.MainLoop(); GLib.timeout_add_seconds(args.seconds, loop.quit); loop.run()
    emit("complete", unresolved_calls=len(calls))
if __name__ == "__main__": main()
