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
