# 0015 — Pairing UI requires a bound SAS ceremony

**Status:** Accepted — 2026-08-01; re-entry conditions met — 2026-08-09

The current v2 `PairCreate` / `PairAccept` protocol uses a long-lived pairing
credential and persists the pairing as part of accepting it. It has no daemon
pairing state machine, no peer-visible SAS confirmation, and no abort operation
that can cancel a pending ceremony without also unpairing a confirmed device.

Therefore a product UI may expose Pair or Add-device controls only through a
ceremony that binds both peers' decisions before persistence. A locally
generated six-digit code remains prohibited, as does copying the long-lived
credential to the system clipboard.

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

The old WebView Pair/Join dialog remains prohibited: it copied a long-lived
credential and displayed a value derived from the pairing id as if it were a
bound SAS. Product controls call only the native-safe command family. Native
presentation owns QR generation/scanning and SAS comparison; the WebView owns
progress, cancellation, retry, and the confirmed-device refresh.
