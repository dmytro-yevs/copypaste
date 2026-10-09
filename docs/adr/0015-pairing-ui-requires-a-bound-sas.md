# 0015 — Pairing UI requires a bound SAS ceremony

**Status:** Accepted. Current Flutter presentation and security documentation
aligned on 9 October 2026.

Pairing creates a memory-only invitation with a random 256-bit token and a
120-second monotonic deadline. Its QR is displayed immediately and contains the
shared `copypaste://pair/v1` URI. Rust owns URI validation, invitation lifetime,
handshake state, and peer persistence on macOS, Android, and Windows.

Displaying or scanning the QR does not establish trust. The authenticated Noise
handshake derives one common SAS. Both peers must explicitly accept that bound
SAS before persistence; the UI must not generate its own comparison code.
Cancel, close, and expiration release an uncommitted ceremony. A decision already
being committed cannot be undone by a concurrent close or cancellation.

## Screenshot policy

Settings exposes **Block screenshots**, off by default. The saved device-local
choice applies to the main application, Quick Paste, pairing QR/code/SAS, and
new application-owned windows or activities. Pairing lifecycle requests reconcile
this policy; they must not force it on or off. With protection off, QR and code
are capturable. This is the configured behavior, not a protection bypass.

Android uses FLAG_SECURE; Windows uses WDA_EXCLUDEFROMCAPTURE. macOS presents
Flutter surfaces through AVSampleBufferDisplayLayer.preventsCapture, converting
surfaces into supported GPU buffers. NSWindow.sharingType is not the screenshot
protection boundary. OS title bars, permission dialogs, and file pickers remain
system-owned surfaces.

The handshake, finite invitation lifetime, and bilateral SAS confirmation are
independent of screenshot protection. A custom URI scheme can be claimed by
another application; the mandatory bound-SAS decision remains the trust check.
UI hiding and Rust zeroization cannot guarantee removal of every Dart-engine or
OS-managed memory copy.

## Evidence contract

Capture verification uses synthetic fixtures in an authorized isolated target.
Never record actual user pairing tokens, QR credentials, SAS, clipboard content,
or their accessibility trees as evidence. Native screenshot receipts qualify
only the exercised API and platform. Recording APIs, transition frames, physical
displays, and other platforms require their own evidence.
