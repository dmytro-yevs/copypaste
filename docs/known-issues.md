# Known Issues — v0.6

## Upgrade: one-time re-pair required

Upgrading to v0.6 requires a **one-time re-pair of all devices**. Two on-the-wire
contracts were bumped:

- P2P bootstrap protocol — `BOOTSTRAP_PROTO_VERSION` 2
  (`copypaste-p2p/src/bootstrap.rs`).
- Android UniFFI ABI — `UNIFFI_ABI_VERSION` 13
  (`copypaste-android/src/version.rs`).

Old pairings will not connect until re-paired. On each device, re-scan the
pairing QR, or re-run LAN discovery + the 6-digit SAS confirmation.

## Active limitations
- Windows daemon does not run yet — frozen as of 2026-05-23
  (see `docs/adr/ADR-012-windows-frozen-homebrew-only.md`); IPC stubs only.
- No code signing — macOS will Gatekeeper-warn on first launch (right-click → Open).

## Documented workarounds
- macOS: `xattr -d com.apple.quarantine CopyPaste.app` then double-click
- If daemon crashes: `tail -f ~/Library/Logs/copypaste/daemon.log`

## Cert rotation during in-flight handshake

- **S10**: Cert rotation during an in-flight TLS handshake may cause transient
  connection failures. When a device certificate is re-generated while a peer
  is mid-handshake, the new fingerprint may not yet be propagated to the peer's
  `PairedPeers` table, causing the handshake to be rejected. Mitigation: the
  retry logic already present in `PeerTransport::connect_with_retry` recovers
  from the transient failure automatically in most cases. Full fix (atomic
  cert-rotation with a grace-period dual-fingerprint acceptance window) remains
  deferred.

## Cloud/relay device revocation (C-P0-4)

**Cloud/relay device revocation = sync-key rotation.** Plain `revoke_peer` /
`revoke_all_peers` only cut off **P2P** (they evict the peer from the live mTLS
allowlist and record a `revoked_devices` audit row). They do **not** cut off
cloud or relay sync: a revoked device keeps the shared sync key, so it can still
decrypt cloud items and still addresses the same relay inbox.

The only honest cloud/relay revocation is **rotating the sync key**
(`rotate_sync_key`, or `revoke_and_rotate` to do both in one IPC call):

- The old key can no longer decrypt items encrypted under the new key
  (XChaCha20-Poly1305 auth-tag rejection — see
  `copypaste-core/src/crypto/sync_key.rs`).
- The relay inbox id is `HKDF(sync_key)`
  (`copypaste-core/src/relay.rs::derive_relay_inbox_id`), so rotation diverges
  the inbox: the revoked device's saved token now addresses a **dead** inbox.

Caveats and DEFERRED work:

- A revoked device **retains read access to PRE-rotation cloud data** until the
  server evicts it by TTL. Rotation does not retroactively re-encrypt or delete
  data already on the relay/Supabase. This is acceptable for the P0 scope (the
  device is cut off from all NEW data immediately).
- The Supabase account **bearer token is NOT auto-invalidated** by rotation.
  Rotating the Supabase account password is a separate **manual** step
  (DEFERRED — no automated bearer rotation yet).
- **Remaining devices must re-provision** after a rotation: re-scan the pairing
  QR or re-enter the new passphrase. The provisioning-apply path
  (`apply_peer_provisioning_to`) detects a rotation by constant-time comparing
  the incoming provisioned key against the existing one — an **identical** key
  is a routine-pairing no-op (never clobbers locally-encrypted blobs), a
  **differing** key is treated as a rotation re-provision and replaces the stale
  key. Automated multi-device re-key (push the new key to remaining devices
  without a manual re-scan) is DEFERRED.
