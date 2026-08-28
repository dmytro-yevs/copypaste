# 0015 — Pairing UI requires a bound SAS ceremony

**Status:** Accepted — 2026-08-01; re-entry conditions met — 2026-08-09

The current native-safe command family creates a memory-only invite, joins the
authenticated handshake, exposes its bound SAS through native presentation,
records both peers' decisions before persistence, and supports idempotent
cancel. A locally generated six-digit code remains prohibited, as does copying
pairing credentials to the system clipboard.

The Devices screen continues to list, sync, unpair and revoke established
devices. Pairing may return only with a protocol/API that supplies all of:

- a common SAS derived from the authenticated handshake;
- a peer-visible confirm/reject state, before persistence;
- an idempotent abort for every close/cancel path;
- a QR rendered only after explicit reveal; desktop may also render the code
  and listen address as non-selectable text in the same protected native
  surface, with no clipboard copy path.

The native-safe pairing commands now meet those conditions. The WebView sees
only typed ceremony states, presentation availability, sanitized errors, and
confirmed device metadata. Invite data, QR payloads, desktop code/address
entry, SAS values, ceremony peer addresses and key material remain in the
native presentation layer.

## Consequences

Product controls call only the native-safe command family. Native presentation
owns QR generation/scanning and SAS comparison; the WebView owns progress,
cancellation, retry, and the confirmed-device refresh.

On Windows, every pairing window keeps `WDA_EXCLUDEFROMCAPTURE` (`0x11`)
regardless of the shell's Allow screenshots preference. Windows documents this
affinity as excluding a top-level window from capture; `WDA_NONE` imposes no
restriction. Release evidence therefore never screenshots pairing. It records
only a complete, allowlisted UI Automation tree, requires the code and address
edits to report `IsPassword=true`, and never reads `ValuePattern`.

Primary sources:

- [SetWindowDisplayAffinity](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity)
- [AutomationElement.IsPassword](https://learn.microsoft.com/dotnet/api/system.windows.automation.automationelement.automationelementinformation.ispassword)
