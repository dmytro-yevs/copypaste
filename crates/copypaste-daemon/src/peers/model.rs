//! The on-disk `PairedDevice` record.
//!
//! Split out of the former flat `peers.rs` (ADR-017, CopyPaste-vp63.4) —
//! moved verbatim, no behavior change.

/// A device that has been paired with this instance.
///
/// `name` / `added_at` are `#[serde(default)]` so this type can also parse the
/// leaner records written by the IPC PAKE pairing handlers
/// (`{"fingerprint":…, "added_at":…}`, sometimes `"password_file_b64"`), which
/// omit a display name. Unknown fields (e.g. `password_file_b64`) are ignored.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct PairedDevice {
    /// SHA-256 fingerprint of the peer's TLS certificate, lowercase hex.
    pub fingerprint: String,
    /// Human-readable display name of the device.
    #[serde(default)]
    pub name: String,
    /// Unix timestamp (seconds) when this device was paired.
    #[serde(default)]
    pub added_at: i64,
    /// The peer's P2P sync-listener address (`host:port`), learned in-band
    /// during PAKE pairing. Used by the Phase 3 outbound connector to dial an
    /// already-paired peer directly (loopback mDNS filters 127.0.0.1 and is
    /// unreliable, so the connector relies on this persisted address rather than
    /// mDNS). `#[serde(default)]` keeps backward compatibility with older
    /// `peers.json` records that predate this field (they deserialise to `None`).
    #[serde(default)]
    pub address: Option<String>,
    /// Base64 (standard) of the 32-byte shared content sync key for this peer,
    /// derived deterministically from the PAKE `SessionKey` at pairing time
    /// (P2P Phase 3, cross-device readability).
    ///
    /// Both sides converge on the same `SessionKey` after a successful PAKE
    /// handshake, so each derives — and persists — the IDENTICAL key here. The
    /// sync orchestrator uses it to re-encrypt outgoing item plaintext (so a
    /// paired peer can decrypt it) and to decrypt incoming items before
    /// re-encrypting them under this device's own local-storage key.
    ///
    /// `#[serde(default)]` keeps backward compatibility with records that
    /// predate this field. chmod 0600 on the file (see [`super::store::save_peers`])
    /// keeps the key off world-readable storage; it never leaves this host as
    /// plaintext.
    #[serde(default)]
    pub sync_key_b64: Option<String>,

    /// Friendly hardware model of the peer (e.g. `"MacBook Air"`), learned
    /// in-band over the bootstrap channel during pairing. `#[serde(default)]`
    /// for backward compatibility with records that predate this field.
    #[serde(default)]
    pub model: Option<String>,
    /// Peer's OS name + version (e.g. `"macOS 15.5"`), learned in-band.
    #[serde(default)]
    pub os_version: Option<String>,
    /// Peer's app/daemon version string, learned in-band.
    #[serde(default)]
    pub app_version: Option<String>,
    /// Peer's best LAN-routable display IP, learned in-band. Preferred over
    /// parsing the `host:port` `address` field for UI display.
    #[serde(default)]
    pub local_ip: Option<String>,
    /// Peer's stable mDNS device UUID (`PeerMeta::device_id`), learned in-band
    /// over the bootstrap channel at pairing time (CopyPaste-8ebg.27).
    ///
    /// This is the SAME random per-install UUID advertised as the mDNS TXT
    /// `did` field — NOT the TLS cert fingerprint this record is keyed by (see
    /// `resolve_addr_from_discovery`'s doc comment for why the two must not be
    /// conflated). Persisting it lets
    /// `p2p::connector::discovery_resolve::refresh_peer_meta_from_discovery`
    /// re-correlate a paired peer against the live mDNS snapshot by a stable
    /// identifier instead of the persisted (and potentially stale, e.g. after
    /// DHCP renewal or network roaming) IP address.
    ///
    /// `#[serde(default)]` keeps backward compatibility with `peers.json`
    /// records written before this field existed (they deserialise to
    /// `None`); those legacy records fall back to IP-based correlation.
    #[serde(default)]
    pub device_id: Option<String>,
    /// Peer's STUN-discovered public / global IP (e.g. `"203.0.113.42"`), learned
    /// in-band over the bootstrap channel during pairing (B1: full device info).
    /// Surfaced verbatim in the `list_peers` IPC response so the Devices UI can
    /// show the remote peer's global IP. `None` when the peer opted out of
    /// public-IP collection, STUN had not resolved, or the record predates this
    /// field. Informational only — never used for auth/trust. `#[serde(default)]`
    /// keeps backward compatibility with older `peers.json` records.
    #[serde(default)]
    pub public_ip: Option<String>,
    /// Unix timestamp (seconds) of the FIRST successful sync connection with
    /// this peer. Set once and never overwritten. `None` until the first sync.
    #[serde(default)]
    pub first_sync_at: Option<i64>,
    /// Unix timestamp (seconds) of the MOST RECENT successful sync connection
    /// with this peer. Updated on every established (throttled) connection.
    #[serde(default)]
    pub last_sync_at: Option<i64>,
    /// Peer's Supabase account identity (CopyPaste-yw2k).
    ///
    /// Derived from the peer's Supabase project URL + GoTrue user UUID via
    /// `copypaste_supabase::supabase_account_id`. Learned in-band over the
    /// bootstrap channel at pairing time (exchanged in `PeerMeta`).
    ///
    /// Two paired devices MUST share the same value for Supabase RLS to
    /// let them see each other's rows. A mismatch means they are on
    /// different Supabase projects or different GoTrue accounts.
    ///
    /// This is a **non-secret** stable identifier (not a token or key).
    /// `#[serde(default, skip_serializing_if)]` keeps backward compatibility
    /// with older records that predate this field (they deserialise to `None`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supabase_account_id: Option<String>,

    /// **DEPRECATED (plaintext at-rest)** — Base64 of the raw `PasswordFile`
    /// blob. Kept for **migration reads only**: if this field is present in an
    /// existing `peers.json` and `password_file_enc` is absent the daemon
    /// treats the value as a legacy plaintext entry and re-encrypts it into
    /// `password_file_enc` on the next save. New writes always use
    /// `password_file_enc`; this field is never written by current code.
    /// `#[serde(default, skip_serializing_if)]` means it is silently ignored
    /// on load when absent and NEVER written to disk by `save_peers`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_file_b64: Option<String>,

    /// Encrypted-at-rest `PasswordFile` blob (CopyPaste-5lm).
    ///
    /// Encoding: base64-standard of `nonce[24] || ciphertext` where the
    /// ciphertext is `XChaCha20-Poly1305(plaintext=PasswordFile::serialized,
    /// key=local_key, aad=b"pake_password_file|{canonical_fingerprint}")`.
    ///
    /// Written by `pair_accept_finish` (responder side). `None` on the
    /// INITIATOR side (which uses `sync_key_b64`), and `None` for legacy
    /// records that predate this field (those have `password_file_b64`
    /// instead — they are re-encrypted on next write).
    ///
    /// `#[serde(default, skip_serializing_if)]` keeps backward compat with
    /// older `peers.json` files; the key is omitted when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_file_enc: Option<String>,
}

/// Find the paired-device record whose fingerprint canonicalises to the same
/// value as `fingerprint` (colon-hex stored vs colon-free P2P form both
/// match), returning a mutable reference for in-place update.
///
/// Extracted (CopyPaste-vp63.4) from the identical
/// `.iter_mut().find(|p| canonical_fp(&p.fingerprint) == target)` pattern that
/// was duplicated 4x across `update_peer_meta`, `update_peer_address`,
/// `update_peer_device_info`, and `touch_sync_times`. `pub(crate)` so it is
/// available to sibling `p2p`/`ipc` call sites that adopt it in a follow-up
/// (see the DEDUP note in the ADR-017 split sketch) without widening beyond
/// the crate.
pub(crate) fn find_mut_by_fingerprint<'a>(
    peers: &'a mut [PairedDevice],
    fingerprint: &str,
) -> Option<&'a mut PairedDevice> {
    let target = crate::ipc::canonical_fingerprint(fingerprint);
    peers
        .iter_mut()
        .find(|p| crate::ipc::canonical_fingerprint(&p.fingerprint) == target)
}

/// Remove every paired-device record whose fingerprint canonicalises to
/// `fingerprint`, retaining all others. Companion to
/// [`find_mut_by_fingerprint`] for the (rarer) remove-by-fingerprint call
/// shape.
pub(crate) fn retain_not_fingerprint(peers: &mut Vec<PairedDevice>, fingerprint: &str) {
    let target = crate::ipc::canonical_fingerprint(fingerprint);
    peers.retain(|p| crate::ipc::canonical_fingerprint(&p.fingerprint) != target);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peers::{load_peers, save_peers};
    use tempfile::tempdir;

    fn make_device(fp: &str, name: &str) -> PairedDevice {
        PairedDevice {
            fingerprint: fp.to_string(),
            name: name.to_string(),
            added_at: 1_700_000_000,
            address: Some("127.0.0.1:4242".to_string()),
            sync_key_b64: None,
            model: None,
            os_version: None,
            app_version: None,
            local_ip: None,
            device_id: None,
            public_ip: None,
            supabase_account_id: None,
            first_sync_at: None,
            last_sync_at: None,
            password_file_b64: None,
            password_file_enc: None,
        }
    }

    /// A `peers.json` written before the `address` field existed must still
    /// deserialise (the field defaults to `None`) — backward compatibility.
    #[test]
    fn legacy_record_without_address_loads_as_none() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        std::fs::write(
            &path,
            br#"[{"fingerprint":"aabbcc","name":"Old","added_at":1700000000}]"#,
        )
        .unwrap();
        let loaded = load_peers(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].fingerprint, "aabbcc");
        assert_eq!(loaded[0].address, None);
    }

    /// A `peers.json` written before the metadata / sync-time fields existed
    /// must still deserialise — all the new fields default to `None`.
    #[test]
    fn legacy_record_without_metadata_loads_as_none() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        std::fs::write(
            &path,
            br#"[{"fingerprint":"aa:bb:cc","name":"Old","added_at":1700000000,"address":"127.0.0.1:4242"}]"#,
        )
        .unwrap();
        let loaded = load_peers(&path);
        assert_eq!(loaded.len(), 1);
        let p = &loaded[0];
        assert_eq!(p.model, None);
        assert_eq!(p.os_version, None);
        assert_eq!(p.app_version, None);
        assert_eq!(p.local_ip, None);
        assert_eq!(p.first_sync_at, None);
        assert_eq!(p.last_sync_at, None);
    }

    // ─── CopyPaste-5lm: PasswordFile at-rest encryption ─────────────────────

    /// `password_file_enc` round-trips through `save_peers` / `load_peers`.
    ///
    /// We store an arbitrary encrypted blob in the field and verify it
    /// deserialises back unchanged.  The encryption itself is tested at the
    /// `ipc.rs` layer where the key is available; this test only covers the
    /// serde round-trip of the new field.
    #[test]
    fn password_file_enc_roundtrips_through_save_load() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD;

        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");

        // Simulate an encrypted blob: arbitrary bytes that look like nonce+ct.
        let fake_enc_bytes: Vec<u8> = (0u8..48).collect(); // 24-byte nonce + 24-byte ct
        let fake_enc_b64 = b64.encode(&fake_enc_bytes);

        let device = PairedDevice {
            fingerprint: "aa:bb:cc".to_string(),
            name: "Alice".to_string(),
            added_at: 1_700_000_000,
            address: None,
            sync_key_b64: None,
            model: None,
            os_version: None,
            app_version: None,
            local_ip: None,
            device_id: None,
            public_ip: None,
            supabase_account_id: None,
            first_sync_at: None,
            last_sync_at: None,
            password_file_b64: None,
            password_file_enc: Some(fake_enc_b64.clone()),
        };

        save_peers(&path, &[device]).unwrap();
        let loaded = load_peers(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded[0].password_file_enc.as_deref(),
            Some(fake_enc_b64.as_str()),
            "password_file_enc must survive save/load round-trip"
        );
        // Legacy plaintext field must remain absent when not set.
        assert!(
            loaded[0].password_file_b64.is_none(),
            "password_file_b64 must be absent when only password_file_enc is set"
        );
    }

    /// A legacy `peers.json` entry with `password_file_b64` (plaintext — the
    /// pre-CopyPaste-5lm format) must still deserialise successfully, with
    /// `password_file_b64` populated and `password_file_enc` absent.
    ///
    /// This validates the migration path: on next write, the caller re-encrypts
    /// the plaintext bytes into `password_file_enc` and writes only that field.
    #[test]
    fn legacy_password_file_b64_loads_as_plaintext_migration_entry() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD;

        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");

        // Write a legacy record with password_file_b64 (not password_file_enc).
        let fake_pf_bytes: Vec<u8> = vec![0xAB, 0xCD, 0xEF, 0x01, 0x23];
        let fake_pf_b64 = b64.encode(&fake_pf_bytes);
        std::fs::write(
            &path,
            format!(
                r#"[{{"fingerprint":"aa:bb:cc","name":"Alice","added_at":1700000000,"password_file_b64":"{fake_pf_b64}"}}]"#
            ),
        )
        .unwrap();

        let loaded = load_peers(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded[0].password_file_b64.as_deref(),
            Some(fake_pf_b64.as_str()),
            "legacy password_file_b64 must be deserialized for migration"
        );
        assert!(
            loaded[0].password_file_enc.is_none(),
            "password_file_enc must be None for a legacy record"
        );
    }

    /// A `peers.json` with `password_file_b64` must NOT re-serialize that field
    /// after a `save_peers` round-trip when `password_file_enc` is also None —
    /// i.e. loading a legacy entry and re-saving it (without the caller
    /// supplying a `password_file_enc`) leaves the `password_file_b64` in place
    /// so the caller can still detect it for migration.
    ///
    /// (Once the caller encrypts the bytes and sets `password_file_enc`, it
    /// should also clear `password_file_b64` — that clearing is done at the
    /// `pair_accept_finish` IPC site, not in `save_peers`.)
    #[test]
    fn legacy_password_file_b64_preserved_on_resave_without_enc() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD;

        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");

        let fake_pf_b64 = b64.encode(b"hello_pake");
        // Write initial record with the legacy field only.
        let device = PairedDevice {
            fingerprint: "aa:bb:cc".to_string(),
            name: "Alice".to_string(),
            added_at: 1_700_000_000,
            address: None,
            sync_key_b64: None,
            model: None,
            os_version: None,
            app_version: None,
            local_ip: None,
            device_id: None,
            public_ip: None,
            supabase_account_id: None,
            first_sync_at: None,
            last_sync_at: None,
            password_file_b64: Some(fake_pf_b64.clone()),
            password_file_enc: None,
        };
        save_peers(&path, &[device]).unwrap();
        // Re-load: the legacy field must be intact.
        let loaded = load_peers(&path);
        assert_eq!(
            loaded[0].password_file_b64.as_deref(),
            Some(fake_pf_b64.as_str()),
            "password_file_b64 must survive resave when password_file_enc is None"
        );
    }

    // ─── CopyPaste-yw2k: supabase_account_id field ──────────────────────────

    /// CopyPaste-yw2k: `supabase_account_id` must round-trip through
    /// `save_peers` / `load_peers` when set.
    #[test]
    fn supabase_account_id_roundtrips_through_save_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");

        let mut device = make_device("aa:bb:cc", "Alice");
        device.supabase_account_id =
            Some("proj_abc/uid_00000000-1111-2222-3333-444444444444".to_string());

        save_peers(&path, &[device]).unwrap();
        let loaded = load_peers(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded[0].supabase_account_id.as_deref(),
            Some("proj_abc/uid_00000000-1111-2222-3333-444444444444"),
            "supabase_account_id must survive save/load round-trip"
        );
    }

    /// CopyPaste-yw2k: a `peers.json` written before the `supabase_account_id`
    /// field existed must still deserialise — the field defaults to `None`.
    #[test]
    fn legacy_record_without_supabase_account_id_loads_as_none() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        std::fs::write(
            &path,
            br#"[{"fingerprint":"aa:bb:cc","name":"Old","added_at":1700000000}]"#,
        )
        .unwrap();
        let loaded = load_peers(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded[0].supabase_account_id, None,
            "legacy records must deserialise with supabase_account_id=None"
        );
    }

    /// CopyPaste-yw2k: when `supabase_account_id` is `None` it must NOT appear
    /// in the serialised JSON (back-compat: old daemons would ignore it but no
    /// point polluting the file).
    #[test]
    fn supabase_account_id_absent_from_json_when_none() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("peers.json");
        let device = make_device("aa:bb:cc", "Alice"); // supabase_account_id = None
        save_peers(&path, &[device]).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(
            !raw.contains("supabase_account_id"),
            "supabase_account_id must not appear in JSON when None"
        );
    }
}
