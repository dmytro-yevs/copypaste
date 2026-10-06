# ADR-0027 — Use platform acknowledgement sounds

**Status:** accepted · 2026-08-25

## Decision

Copy feedback uses a platform-native sound: the existing macOS `afplay` + stock
`Pop.aiff`, Windows `MessageBeep(MB_OK)` through the maintained `winsafe`
wrapper already in the tree, and Android `ToneGenerator` on
`STREAM_NOTIFICATION`, gated by ringer, mute and stream volume. Notification
permission is not part of this path.

Capture sound is enabled by default; stored user choices remain unchanged.
Notifications remain opt-in, with an independent content-preview switch that is
enabled by default. Notifications use the captured item ID to retrieve its text,
file name, or bounded image thumbnail through the ordinary item APIs. Watch
events contain no clipboard content. Text previews are limited to 1,000 Unicode
code points, following the content-based feedback in Maccy.

Native notifications are silent on macOS, Windows, and Android so the independent
capture sound plays only once. Turning off content previews keeps the generic
capture message and omits image attachments.

This is dependency exemption 1. [`beep`](https://crates.io/crates/beep) targets
the PC speaker and [`actually_beep`](https://crates.io/crates/actually_beep)
pulls a general audio backend; neither queues system feedback on all targets.
Microsoft documents `MB_OK` as the user-configured Default Beep and asynchronous
[MessageBeep](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-messagebeep).
Android documents the public API-1 [`ToneGenerator`](https://developer.android.com/reference/android/media/ToneGenerator)
positive acknowledgement tone and explicit resource release.
