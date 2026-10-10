#!/usr/bin/env python3
"""Unit tests for the redacted Linux desktop acceptance observer."""

import importlib.util
import json
import os
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "release" / "linux-desktop-acceptance.py"
SPEC = importlib.util.spec_from_file_location("linux_desktop_acceptance", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class LinuxDesktopAcceptanceTest(unittest.TestCase):
    def test_finds_exact_gui_owned_status_notifier_item(self):
        service = MODULE.sni_service_for_pid(
            [
                "org.kde.StatusNotifierItem-4321-1",
                "org.kde.StatusNotifierItem-7654-1",
                "org.freedesktop.Notifications",
            ],
            4321,
        )
        self.assertEqual(service, "org.kde.StatusNotifierItem-4321-1")
        with self.assertRaisesRegex(MODULE.AcceptanceError, "exactly one"):
            MODULE.sni_service_for_pid(
                ["org.kde.StatusNotifierItem-4321-1", "org.kde.StatusNotifierItem-4321-2"], 4321
            )

    def test_finds_enabled_open_menu_item_in_busctl_layout(self):
        layout = [
            3,
            [
                0,
                {},
                [
                    [17, {"label": {"type": "s", "data": "Open"}, "enabled": {"type": "b", "data": True}}, []],
                    [18, {"label": {"type": "s", "data": "Quit"}, "enabled": {"type": "b", "data": True}}, []],
                ],
            ],
        ]
        self.assertEqual(MODULE.menu_item_id(layout, "Open"), 17)
        with self.assertRaisesRegex(MODULE.AcceptanceError, "no enabled 'Settings'"):
            MODULE.menu_item_id(layout, "Settings")

    def test_notification_transcript_keeps_only_server_reply_metadata(self):
        transcript = MODULE.NotificationTranscript(server_owner=':1.9')
        transcript.consume(
            "method call time=1.0 sender=:1.5 -> destination=org.freedesktop.Notifications serial=71 "
            "path=/org/freedesktop/Notifications; interface=org.freedesktop.Notifications; member=Notify"
        )
        transcript.consume('   string "CopyPaste"')
        transcript.consume('   string "sensitive clipboard body must never be retained"')
        transcript.consume(
            "method return time=1.1 sender=:1.9 -> destination=:1.5 serial=72 reply_serial=71"
        )
        transcript.consume("   uint32 24")
        self.assertEqual(transcript.notification_id, 24)
        evidence = json.dumps({"server_reply_id": transcript.notification_id})
        self.assertNotIn("sensitive clipboard body", evidence)

    def test_notification_body_uint32_cannot_be_a_server_reply(self):
        transcript = MODULE.NotificationTranscript(server_owner=':1.9')
        transcript.consume(
            "method call sender=:1.5 -> destination=org.freedesktop.Notifications serial=71 "
            "path=/org/freedesktop/Notifications; interface=org.freedesktop.Notifications; member=Notify"
        )
        transcript.consume(' string "CopyPaste"')
        transcript.consume(' uint32 999')
        self.assertIsNone(transcript.notification_id)
        transcript.consume('method return sender=:1.8 -> destination=:1.5 serial=72 reply_serial=71')
        transcript.consume(' uint32 24')
        self.assertIsNone(transcript.notification_id)
        transcript.consume('method return sender=:1.9 -> destination=:1.5 serial=72 reply_serial=71')
        transcript.consume(' uint32 24')
        self.assertEqual(transcript.notification_id, 24)

    def test_capture_command_requires_real_argv(self):
        provider = str(MODULE.CLIPBOARD_PROVIDER)
        command = ["python3", provider, "--helper", "/tmp/helper", "--manifest", "/tmp/manifest", "--ready-file", "/tmp/ready", "--activity-log", "/tmp/activity"]
        self.assertEqual(MODULE.parse_capture_command(json.dumps(command)), command)
        with self.assertRaisesRegex(MODULE.AcceptanceError, "JSON argv"):
            MODULE.parse_capture_command('sh -c true')
        with self.assertRaisesRegex(MODULE.AcceptanceError, "GTK clipboard provider"):
            MODULE.parse_capture_command('["true"]')

    def test_wayland_capability_requires_256_bits(self):
        prior = os.environ.get('COPYPASTE_QUALIFICATION_CAPABILITY')
        try:
            os.environ['COPYPASTE_QUALIFICATION_CAPABILITY'] = 'not-a-capability'
            with self.assertRaisesRegex(MODULE.AcceptanceError, 'capability'):
                MODULE.start_wayland_qualification('close-main')
        finally:
            if prior is None:
                os.environ.pop('COPYPASTE_QUALIFICATION_CAPABILITY', None)
            else:
                os.environ['COPYPASTE_QUALIFICATION_CAPABILITY'] = prior


if __name__ == "__main__":
    unittest.main()
