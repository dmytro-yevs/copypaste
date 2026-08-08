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
