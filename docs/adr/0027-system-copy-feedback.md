# ADR-0027 — Use platform acknowledgement sounds

**Status:** accepted · 2026-08-25

## Decision

Copy feedback uses a platform-native sound: the existing macOS `afplay` + stock
`Pop.aiff`, Windows `MessageBeep(MB_OK)` through the maintained `winsafe`
wrapper already in the tree, and Android `ToneGenerator` on
`STREAM_NOTIFICATION`, gated by ringer, mute and stream volume. Notification
permission is not part of this path.

This is dependency exemption 1. [`beep`](https://crates.io/crates/beep) targets
the PC speaker and [`actually_beep`](https://crates.io/crates/actually_beep)
pulls a general audio backend; neither queues system feedback on all targets.
Microsoft documents `MB_OK` as the user-configured Default Beep and asynchronous
[MessageBeep](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-messagebeep).
Android documents the public API-1 [`ToneGenerator`](https://developer.android.com/reference/android/media/ToneGenerator)
positive acknowledgement tone and explicit resource release.
