# Known Issues — v0.1.0-alpha.1

## Active limitations
- P2P pairing UI is preview only (`get_own_fingerprint` returns real cert FP, but `pair_peer` / `unpair_peer` are stubs)
- Windows daemon does not run yet — IPC stubs in place for future work
- No code signing — macOS will Gatekeeper-warn on first launch (right-click → Open)
- Android app shell exists; no signed APK
- Relay loses state on restart

## Documented workarounds
- macOS: `xattr -d com.apple.quarantine CopyPaste.app` then double-click
- If daemon crashes: `tail -f ~/Library/Logs/copypaste/daemon.log`

## Known Issues (v0.3)

- **S10**: Cert rotation during an in-flight TLS handshake may cause transient
  connection failures. When a device certificate is re-generated while a peer
  is mid-handshake, the new fingerprint may not yet be propagated to the peer's
  `PairedPeers` table, causing the handshake to be rejected. Mitigation: the
  retry logic already present in `PeerTransport::connect_with_retry` recovers
  from the transient failure automatically in most cases. Full fix (atomic
  cert-rotation with a grace-period dual-fingerprint acceptance window) is
  deferred to v0.4.

- **Cnew (RESOLVED in v0.4)**: Image clipboard items captured before upgrading
  to v0.3 previously retained their original encryption key derivation (v1 HKDF
  family) because the v4 migration sweep was scoped to text items only. This is
  now fixed: the v4 sweep (`migrate_v1_to_v2_keys`) rotates image rows too, via
  `migrate_v1_image_chunks_to_v2`. Each image row's chunk blob is decrypted with
  the v1 key, re-encrypted with the v2 key (fresh per-chunk nonces), and the
  row's `key_version` is bumped to 2. The per-chunk AAD `file_id` (read from the
  row's `blob_ref` JSON) is preserved across the rotation, and undecryptable
  rows are left at `key_version = 1` without aborting the sweep.

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
