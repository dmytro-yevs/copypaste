# 0015 — Pairing UI requires a bound SAS ceremony

**Status:** Accepted — 2026-08-01

The current v2 `PairCreate` / `PairAccept` protocol uses a long-lived pairing
credential and persists the pairing as part of accepting it. It has no daemon
pairing state machine, no peer-visible SAS confirmation, and no abort operation
that can cancel a pending ceremony without also unpairing a confirmed device.

Therefore the desktop and Android UI must not expose Pair or Add-device
controls. A blurred QR or a locally generated six-digit code would only look
like a confirmation ceremony; it could not bind both peers' decision to the
stored pairing. Copying the long-lived credential to the system clipboard is
also prohibited.

The Devices screen continues to list, sync, unpair and revoke established
devices. Pairing may return only with a protocol/API that supplies all of:

- a common SAS derived from the authenticated handshake;
- a peer-visible confirm/reject state, before persistence;
- an idempotent abort for every close/cancel path;
- a QR rendered only after explicit reveal, with no credential text or
  clipboard copy path.

This is a product-security boundary, not a temporary UI styling limitation.

## Consequences

A Pair/Join dialog reached the app regardless, and was removed rather than
kept: it copied the 52-character credential and a `copypaste://pair?code=…`
URI to the system clipboard, and the "security code" it asked both users to
compare was the first six characters of the pairing id — which is derived from
the credential and travels in the same QR, so it verified nothing while looking
exactly like a ceremony that does. Comparing it would have taught the gesture
without ever buying the property.

The controls are absent, not disabled, and `pair_create`/`pair_accept` are
absent from the `Backend` trait, the Tauri command list and the dev web bridge
— a verb the WebView can name is a verb a compromised renderer can call. The
`copypaste://pair` deep link no longer opens anything. `PairCreate`/`PairAccept`
stay on the daemon's IPC surface for the CLI, which is the scripting surface and
not a product one (`CLAUDE.md` rule 6).

Re-entry needs all four bullets above, and the SAS in the first one has to come
from the Noise transcript — `snow` exposes it as `HandshakeState::get_handshake_hash`,
which must be captured before `into_transport_mode`, since `TransportState` does
not carry it. Nothing short of that is a partial step towards this decision.
