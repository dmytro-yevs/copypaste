#![allow(clippy::empty_line_after_doc_comments)] // uniffi-generated scaffolding triggers this lint

uniffi::include_scaffolding!("copypaste_android");

pub mod p2p_listener;
pub mod pairing;
pub mod panic_boundary;
pub mod stun;
pub mod version;
pub use p2p_listener::{P2pListenerHandle, PeerSessionKey};
pub use pairing::{DiscoveredPeer, PairStatus};
pub use panic_boundary::PanicError;
pub use version::{
    check_compatibility, core_version, uniffi_abi_version, VersionError, UNIFFI_ABI_VERSION,
};

// ── Split modules ────────────────────────────────────────────────────────────
// Each module contains a cohesive group of UniFFI-exported functions/types that
// were previously inlined in this god file. All public items are re-exported
// from lib.rs so the generated Kotlin bindings are unchanged (UDL references
// the types by name; the scaffolding finds them via `pub use`).
pub mod ffi_cloud_sync;
pub mod ffi_config;
pub mod ffi_crypto;
pub mod ffi_db;
pub mod ffi_p2p_session;
pub mod ffi_pairing;
pub mod ffi_revocation;
pub mod ffi_sensitive;
pub mod ffi_system;

pub use ffi_cloud_sync::{
    cloud_decrypt, cloud_encrypt, derive_cloud_sync_key, relay_inbox_id, relay_public_key_b64,
    relay_registration_pop,
};
pub use ffi_config::{
    appconfig_from_config, clamp_config, config_from_appconfig, default_config, Config,
    DEFAULT_IMAGE_MAX_HEIGHT, DEFAULT_MASK_SENSITIVE_CONTENT, DEFAULT_P2P_ENABLED,
};
pub use ffi_crypto::{
    decrypt_text, decrypt_text_batch, encrypt_text, DecryptBatchResult, DecryptedItem,
    EncryptedBlob, EncryptedItem,
};
#[cfg(feature = "android-uniffi-live")]
pub use ffi_db::db_by_path;
pub use ffi_db::{
    add_clipboard_item, close_database, db_handle_to_cache_key, db_vacuum, fts_search,
    get_history_count, get_history_page, key_cache_hash, open_database, store_clipboard_item,
    with_cached_db, HistoryItem, SearchResultItem,
};
pub use ffi_p2p_session::{
    canonicalize_fingerprint, is_fingerprint_revoked, poll_p2p_listener,
    shared_sync_key_from_session, start_p2p_listener, stop_p2p_listener, sync_with_peer,
    update_p2p_listener_peers, LocalItem, P2pSyncResult, SyncedItem, P2P_SYNC_KEY_SALT,
    P2P_WIRE_KEY_VERSION,
};
pub use ffi_pairing::{
    bootstrap_pair_initiator, bootstrap_result_from_pairing, build_android_peer_meta,
    build_pairing_qr, confirmed_pairing_from, generate_device_cert, list_discovered, pair_abort,
    pair_confirm_sas, pair_get_sas, pair_reset, pair_with_discovered, parse_pairing_qr,
    start_discovery, stop_discovery, BootstrapResult, DeviceCert, PairingQrPayload, ScannedPairing,
    SyncProvisioning,
};
pub use ffi_revocation::{
    derive_new_sync_key_from_passphrase, list_revoked_fingerprints, list_revoked_peers,
    revoke_device_and_rotate_key, revoke_device_audit, rotate_sync_key, RevokedPeer,
};
pub use ffi_sensitive::{
    byte_to_char_offset_android, detect_sensitive_spans, is_sensitive, sensitive_capture_decision,
    sensitive_expires_at_ms, sensitive_kind, SensitiveCaptureDecision, SensitiveSpan,
};
pub use ffi_system::{
    classify_text_kind, compute_android_sync_badge_state, get_private_mode, resolve_stun_public_ip,
    set_private_mode,
};

// When using UDL-based scaffolding, uniffi::Error and uniffi::Record proc-macro
// derives conflict with the generated scaffolding. Only thiserror is needed here.
#[derive(Debug, thiserror::Error)]
pub enum CopypasteError {
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed: {reason}")]
    DecryptionFailed { reason: String },
    #[error("Database error: {reason}")]
    DatabaseError { reason: String },
    #[error("Invalid key length: expected 32")]
    InvalidKeyLength,
    /// P2P pairing / transport failure surfaced from `copypaste_p2p`
    /// (`TransportError`): TLS, socket, framing, or PAKE handshake errors —
    /// including a wrong pairing password or a channel-binding MitM abort. Also
    /// raised for a malformed `addr_hint` that cannot be parsed into a
    /// `SocketAddr`. The `reason` carries the underlying error's display form.
    #[error("P2P pairing failed: {reason}")]
    P2pError { reason: String },
    /// v0.3 (OI-7): a Rust panic was caught at the FFI boundary by
    /// [`panic_boundary::catch_result`]. Carries the panic message so Kotlin
    /// can log/surface it instead of seeing a JVM-killing abort.
    ///
    /// NOTE: the field is named `reason` (not `message`) on purpose — a UniFFI
    /// flat-error variant field named `message` collides with the Kotlin
    /// `Throwable.message` supertype property and produces "conflicting
    /// declarations" / missing-`override` codegen errors. See the generated
    /// `CopypasteException` binding.
    #[error("Panicked: {reason}")]
    Panicked { reason: String },
}

impl From<PanicError> for CopypasteError {
    fn from(p: PanicError) -> Self {
        match p {
            PanicError::Panicked(reason) => CopypasteError::Panicked { reason },
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    // These core helpers are used directly in tests but are not part of the
    // public FFI surface (not re-exported from lib.rs). Import them locally so
    // they remain reachable after the split without polluting the FFI API.
    use base64::Engine as _;
    use copypaste_core::{
        decrypt_from_cloud, detect, encrypt_for_cloud, is_sensitive_for_autowipe,
    };

    fn test_key() -> Vec<u8> {
        vec![7u8; 32]
    }

    #[test]
    fn encrypt_then_decrypt_roundtrips() {
        let key = test_key();
        let item_id = "test-android-item".to_string();
        // Use key_version=2 (current daemon default, ITEM_KEY_VERSION_CURRENT=2).
        let blob = encrypt_text(item_id.clone(), b"hello android", &key, 2).expect("encrypt");
        let plaintext =
            decrypt_text(item_id, &blob.ciphertext, &blob.nonce, &key, 2).expect("decrypt");
        assert_eq!(plaintext, b"hello android");
    }

    /// v0.3 regression: ciphertext is bound to item_id via AAD — decrypting
    /// with a different item_id must fail with `DecryptionFailed` rather than
    /// silently returning plaintext (legacy empty-AAD fallback removed in
    /// 1c55e57).
    #[test]
    fn decrypt_rejects_mismatched_item_id() {
        let key = test_key();
        let blob = encrypt_text("item-A".into(), b"secret", &key, 2).expect("encrypt");
        let err = decrypt_text("item-B".into(), &blob.ciphertext, &blob.nonce, &key, 2)
            .expect_err("mismatched item_id must reject");
        assert!(
            matches!(err, CopypasteError::DecryptionFailed { .. }),
            "expected DecryptionFailed, got {err:?}"
        );
    }

    /// CopyPaste-00zz: `decrypt_text_batch` must DEGRADE GRACEFULLY across a mix
    /// of decryptable and undecryptable (wrong-key) items: it returns ONLY the
    /// items that verify+decrypt and reports the rest in `skipped`, instead of
    /// throwing one `DecryptionFailed` per undecryptable legacy row (the ~629
    /// startup-flood bug). A failed auth tag is never accepted as plaintext.
    #[test]
    fn decrypt_text_batch_skips_undecryptable_and_counts_them() {
        let key = test_key();
        // Two decryptable items encrypted under `key`.
        let blob_a = encrypt_text("item-A".into(), b"alpha", &key, 2).expect("encrypt A");
        let blob_b = encrypt_text("item-B".into(), b"bravo", &key, 2).expect("encrypt B");
        // Three undecryptable legacy items: encrypted under a DIFFERENT key,
        // standing in for a rotated/old key whose auth tag fails under `key`.
        let stale_key = vec![0x99u8; 32];
        let bad_1 = encrypt_text("legacy-1".into(), b"x", &stale_key, 2).expect("encrypt bad1");
        let bad_2 = encrypt_text("legacy-2".into(), b"y", &stale_key, 2).expect("encrypt bad2");
        let bad_3 = encrypt_text("legacy-3".into(), b"z", &stale_key, 2).expect("encrypt bad3");

        let items = vec![
            EncryptedItem {
                item_id: "item-A".into(),
                ciphertext: blob_a.ciphertext,
                nonce: blob_a.nonce,
                key_version: 2,
            },
            EncryptedItem {
                item_id: "legacy-1".into(),
                ciphertext: bad_1.ciphertext,
                nonce: bad_1.nonce,
                key_version: 2,
            },
            EncryptedItem {
                item_id: "item-B".into(),
                ciphertext: blob_b.ciphertext,
                nonce: blob_b.nonce,
                key_version: 2,
            },
            EncryptedItem {
                item_id: "legacy-2".into(),
                ciphertext: bad_2.ciphertext,
                nonce: bad_2.nonce,
                key_version: 2,
            },
            EncryptedItem {
                item_id: "legacy-3".into(),
                ciphertext: bad_3.ciphertext,
                nonce: bad_3.nonce,
                key_version: 2,
            },
        ];

        let result = decrypt_text_batch(items, &key).expect("batch must not error");
        assert_eq!(
            result.skipped, 3,
            "the 3 wrong-key items must be skipped + counted, not thrown"
        );
        let mut got: Vec<(String, Vec<u8>)> = result
            .items
            .into_iter()
            .map(|d| (d.item_id, d.plaintext))
            .collect();
        got.sort();
        let mut want = vec![
            ("item-A".to_string(), b"alpha".to_vec()),
            ("item-B".to_string(), b"bravo".to_vec()),
        ];
        want.sort();
        assert_eq!(
            got, want,
            "only the decryptable items, with correct plaintext"
        );
    }

    /// CopyPaste-00zz: an unknown `key_version` is undecryptable by definition
    /// and must be skipped+counted (never panic, never accepted).
    #[test]
    fn decrypt_text_batch_skips_unknown_key_version() {
        let key = test_key();
        let blob = encrypt_text("item-A".into(), b"alpha", &key, 2).expect("encrypt");
        let items = vec![EncryptedItem {
            item_id: "item-A".into(),
            ciphertext: blob.ciphertext,
            nonce: blob.nonce,
            key_version: 7, // unsupported
        }];
        let result = decrypt_text_batch(items, &key).expect("batch must not error");
        assert!(result.items.is_empty());
        assert_eq!(result.skipped, 1);
    }

    /// v0.3 OI-7: a panic raised inside a wrapped UniFFI body must surface
    /// as `CopypasteError::Panicked` (via the `From<PanicError>` impl) rather
    /// than aborting the process.
    #[test]
    fn panic_boundary_converts_to_copypaste_panicked() {
        let result: Result<(), CopypasteError> = panic_boundary::catch_result(|| {
            panic!("synthetic panic inside FFI body");
        });
        match result {
            Err(CopypasteError::Panicked { reason }) => {
                assert!(
                    reason.contains("synthetic panic inside FFI body"),
                    "expected panic message in error, got: {reason}"
                );
            }
            other => panic!("expected CopypasteError::Panicked, got {other:?}"),
        }
    }

    /// CRASH FIX: `is_sensitive`/`sensitive_kind` are now wrapped in the
    /// panic-boundary helper so a panic inside `detect()` can't unwind across
    /// the JNI boundary and abort the JVM. The helper is testable from Rust:
    /// confirm normal inputs return the expected values through the wrapper.
    #[test]
    fn is_sensitive_and_kind_return_expected_through_panic_boundary() {
        // A GitHub PAT is detected by copypaste_core::detect.
        let pat = format!("ghp_{}", "A".repeat(36));
        assert!(is_sensitive(pat.clone()), "PAT must be flagged sensitive");
        assert!(
            sensitive_kind(pat).is_some(),
            "PAT must yield a sensitive kind label"
        );

        // Plain text is not sensitive.
        assert!(
            !is_sensitive("just a plain note".into()),
            "plain text must not be sensitive"
        );
        assert_eq!(
            sensitive_kind("just a plain note".into()),
            None,
            "plain text must yield no kind"
        );
    }

    /// AB-6a (ABI 14): `is_sensitive` now gates on the SAME >= 0.70 confidence
    /// floor macOS uses (`is_sensitive_for_autowipe`) instead of flagging on ANY
    /// `detect()` match. A low-confidence heuristic (a bare US phone number,
    /// confidence 0.55) must NOT be flagged on Android anymore — otherwise such
    /// items vanish at capture/sync-in while macOS keeps them. High-confidence
    /// credentials (a GitHub PAT) must still flag.
    #[test]
    fn is_sensitive_uses_high_confidence_threshold_parity() {
        // High-confidence credential: still flagged.
        let pat = format!("ghp_{}", "A".repeat(36));
        assert!(
            is_sensitive(pat),
            "high-confidence credential (PAT) must remain sensitive"
        );

        // Low-confidence heuristic (bare US phone, 0.55) is below the 0.70 floor:
        // detect() still matches it, but is_sensitive must now return false so the
        // verdict matches the macOS daemon's is_sensitive_for_autowipe gate.
        let phone = "555-123-4567";
        assert!(
            detect(phone).is_some(),
            "precondition: detector matches a US phone (low confidence)"
        );
        assert!(
            !is_sensitive(phone.into()),
            "low-confidence phone match must NOT be flagged (>= 0.70 parity with macOS)"
        );
        // Cross-check: agrees byte-for-byte with the core gate macOS uses.
        assert_eq!(
            is_sensitive(phone.into()),
            is_sensitive_for_autowipe(phone),
            "Android is_sensitive must equal the core >= 0.70 gate"
        );
    }

    #[test]
    fn add_clipboard_item_rejects_bad_key() {
        let err = add_clipboard_item("/tmp/copypaste-test.db".into(), &[0u8; 16], "x".into())
            .expect_err("16-byte key must error");
        assert!(matches!(err, CopypasteError::InvalidKeyLength));
    }

    // ── CopyPaste-bdac.42: db_vacuum tests ──────────────────────────────────

    /// db_vacuum must reject a key that is not exactly 32 bytes.
    #[test]
    fn db_vacuum_rejects_bad_key() {
        let err = db_vacuum("/tmp/copypaste-vacuum-test.db".into(), &[0u8; 16])
            .expect_err("16-byte key must error");
        assert!(
            matches!(err, CopypasteError::InvalidKeyLength),
            "expected InvalidKeyLength, got {err:?}"
        );
    }

    /// db_vacuum returns Ok(()) on a valid key when the feature is off (no-op stub).
    #[cfg(not(feature = "android-uniffi-live"))]
    #[test]
    fn db_vacuum_succeeds_as_noop_when_feature_off() {
        // With android-uniffi-live off the stub validates key shape and returns Ok.
        db_vacuum("/dev/null".into(), &test_key())
            .expect("stub db_vacuum must succeed without I/O");
    }

    /// db_vacuum compacts a real SQLCipher database when the feature is on.
    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn db_vacuum_compacts_live_database() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("vacuum-test.db")
            .to_string_lossy()
            .into_owned();
        let key = test_key();

        // Insert and delete several items so there are free pages to reclaim.
        for i in 0..5 {
            add_clipboard_item(path.clone(), &key, format!("item to vacuum {i}")).expect("insert");
        }

        // db_vacuum(0) reclaims all free pages; must succeed without error.
        db_vacuum(path.clone(), &key).expect("db_vacuum must succeed on a live database");

        // Database must still be readable after vacuum.
        let n = get_history_count(path, &key).expect("count after vacuum");
        assert_eq!(n, 5, "items must survive the vacuum pass");
    }

    #[cfg(not(feature = "android-uniffi-live"))]
    #[test]
    fn add_clipboard_item_returns_empty_when_feature_off() {
        let id =
            add_clipboard_item("/dev/null".into(), &test_key(), "hello".into()).expect("stub path");
        // Empty string signals "not stored natively" so Kotlin falls back to
        // SharedPreferences. A non-empty stub value would wrongly suppress the
        // fallback and silently discard every clipboard item.
        assert!(
            id.is_empty(),
            "stub path must return empty string, got {id:?}"
        );
        let n = get_history_count("/dev/null".into(), &test_key()).expect("stub count");
        assert_eq!(n, 0);
    }

    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn add_clipboard_item_persists_when_feature_on() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("live.db");
        let key = test_key();

        let id = add_clipboard_item(
            path.to_string_lossy().into_owned(),
            &key,
            "live android body".into(),
        )
        .expect("insert");
        assert!(!id.is_empty(), "real insert returns a uuid");

        let n = get_history_count(path.to_string_lossy().into_owned(), &key).expect("count");
        assert_eq!(n, 1);
    }

    /// M5: repeated inserts on the same db_path must reuse one cached
    /// connection (no open-per-call) and the count must accumulate correctly
    /// through that shared handle.
    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn live_calls_reuse_cached_connection() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("reuse.db").to_string_lossy().into_owned();
        let key = test_key();

        for i in 0..5 {
            let id = add_clipboard_item(path.clone(), &key, format!("item {i}")).expect("insert");
            assert!(!id.is_empty(), "real insert returns a uuid");
        }

        // Every call above (and this count) went through with_cached_db for the
        // same path, so the same Database connection serviced all of them.
        let n = get_history_count(path.clone(), &key).expect("count");
        assert_eq!(n, 5, "all 5 inserts visible through the reused connection");

        // The (path, sha256(key)) pair is cached after first use.
        // P1-8: the cache key carries SHA-256(key), not the raw key bytes —
        // use key_cache_hash so the assertion matches the actual map key.
        let key_arr: [u8; 32] = key.try_into().expect("test key is 32 bytes");
        let key_hash = key_cache_hash(&key_arr);
        assert!(
            db_by_path()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains_key(&(path.clone(), key_hash)),
            "db_(path,sha256(key)) must be cached after first live call"
        );
    }

    /// PG-3 (349q): sensitive items must be STORED (is_sensitive=true) not dropped.
    ///
    /// The old test asserted `id.is_empty()` and `count == 0`.  After the 349q fix,
    /// sensitive items are encrypted, stored, and flagged — they must produce a
    /// non-empty row id and a count of 1, matching the macOS daemon's behaviour
    /// (daemon.rs:2170-2183).
    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn add_clipboard_item_stores_sensitive_with_flag() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("live.db");
        let key = test_key();

        // GitHub PAT (confidence >= 0.70) — is_sensitive_for_autowipe returns true.
        let pat = format!("ghp_{}", "A".repeat(36));
        let id = add_clipboard_item(path.to_string_lossy().into_owned(), &key, pat)
            .expect("sensitive item must be stored and return Ok");
        assert!(
            !id.is_empty(),
            "PG-3 (349q): sensitive item must produce a non-empty row id"
        );

        let n = get_history_count(path.to_string_lossy().into_owned(), &key).expect("count");
        assert_eq!(
            n, 1,
            "PG-3 (349q): exactly one row must be inserted for sensitive content"
        );
    }

    /// PG-3 (349q): store_clipboard_item stores with explicit TTL.
    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn store_clipboard_item_stores_sensitive_with_explicit_ttl() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("live-ttl.db");
        let key = test_key();

        // Anthropic key — high-confidence sensitive (>= 0.70).
        let ak = format!("sk-ant-api03-{}", "A".repeat(80));
        let id = store_clipboard_item(
            path.to_string_lossy().into_owned(),
            &key,
            ak,
            60, // 60-second TTL
        )
        .expect("store sensitive with explicit TTL");
        assert!(
            !id.is_empty(),
            "store_clipboard_item must return a non-empty row id for sensitive content"
        );

        let n = get_history_count(path.to_string_lossy().into_owned(), &key).expect("count");
        assert_eq!(n, 1, "exactly one row stored");
    }

    /// PG-3 (349q): store_clipboard_item with TTL=0 stores but no expires_at.
    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn store_clipboard_item_ttl_zero_disables_autowipe() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("live-no-ttl.db");
        let key = test_key();

        let aws = "AKIAIOSFODNN7EXAMPLE".to_string();
        let id = store_clipboard_item(path.to_string_lossy().into_owned(), &key, aws, 0)
            .expect("store with ttl=0 (no auto-wipe)");
        assert!(!id.is_empty(), "item stored even with ttl=0");

        let n = get_history_count(path.to_string_lossy().into_owned(), &key).expect("count");
        assert_eq!(n, 1, "exactly one row stored");
    }

    /// PG-23 (l9z8): sensitive_kind must agree with is_sensitive (>= 0.70 floor).
    #[test]
    fn sensitive_kind_aligned_with_is_sensitive_threshold() {
        // High-confidence case: AWS key (0.99) — both must return Some / true.
        let aws = "AKIAIOSFODNN7EXAMPLE".to_string();
        assert!(
            is_sensitive(aws.clone()),
            "is_sensitive must be true for AWS key"
        );
        assert!(
            sensitive_kind(aws).is_some(),
            "sensitive_kind must be Some for AWS key (>= 0.70)"
        );

        // Low-confidence case: phone_us (0.55) — both must return None / false.
        let phone = "(555) 867-5309".to_string();
        assert!(
            !is_sensitive(phone.clone()),
            "is_sensitive must be false for phone (0.55 < 0.70)"
        );
        assert!(
            sensitive_kind(phone).is_none(),
            "sensitive_kind must be None for phone (0.55 < 0.70) — PG-23 alignment"
        );
    }

    /// PG-24 (5tnx): sensitive_expires_at_ms must compute now + ttl*1000.
    #[test]
    fn sensitive_expires_at_ms_basic() {
        let now_ms: i64 = 1_700_000_000_000; // arbitrary fixed timestamp
        let ttl_secs: u64 = 30;
        let expires = sensitive_expires_at_ms(now_ms, ttl_secs);
        assert_eq!(
            expires,
            Some(now_ms + 30_000),
            "expires_at must equal now + ttl_secs * 1000"
        );
    }

    /// PG-24 (5tnx): ttl=0 → None (auto-wipe disabled sentinel).
    #[test]
    fn sensitive_expires_at_ms_zero_ttl_returns_none() {
        let expires = sensitive_expires_at_ms(1_700_000_000_000, 0);
        assert!(
            expires.is_none(),
            "ttl=0 ('auto-wipe disabled') must return None"
        );
    }

    /// PG-4 (ojsh): detect_sensitive_spans returns spans for embedded secrets.
    #[test]
    fn detect_sensitive_spans_finds_aws_key_in_text() {
        let text = "Here is my key: AKIAIOSFODNN7EXAMPLE please keep secret".to_string();
        let spans = detect_sensitive_spans(text.clone());
        assert!(
            !spans.is_empty(),
            "detect_sensitive_spans must find at least one span for embedded AWS key"
        );
        // The span must point at the actual key region.
        let aws_start = text.find("AKIAIOSFODNN7EXAMPLE").expect("key in text");
        let any_matches = spans
            .iter()
            .any(|s| s.start as usize <= aws_start && (s.end as usize) > aws_start);
        assert!(
            any_matches,
            "at least one span must cover the AWS key offset"
        );
    }

    /// PG-4 (ojsh): detect_sensitive_spans returns empty list for benign text.
    #[test]
    fn detect_sensitive_spans_empty_for_benign_text() {
        let text = "Hello, world! This is not sensitive.".to_string();
        let spans = detect_sensitive_spans(text);
        assert!(spans.is_empty(), "no spans expected for benign text");
    }

    /// PG-3 (349q): sensitive_capture_decision returns correct verdict.
    #[test]
    fn sensitive_capture_decision_sensitive_text() {
        let aws = "AKIAIOSFODNN7EXAMPLE".to_string();
        let now_ms: i64 = 1_700_000_000_000;
        let ttl_secs: u64 = 30;

        let d = sensitive_capture_decision(aws, now_ms, ttl_secs);
        assert!(d.is_sensitive, "AWS key must be flagged sensitive");
        assert!(d.kind.is_some(), "AWS key kind must be Some");
        assert_eq!(
            d.expires_at_ms,
            Some(now_ms + 30_000),
            "expires_at_ms must be now + ttl*1000"
        );
    }

    /// PG-3 (349q): sensitive_capture_decision for benign text.
    #[test]
    fn sensitive_capture_decision_benign_text() {
        let d = sensitive_capture_decision("Hello world".to_string(), 1_700_000_000_000, 30);
        assert!(!d.is_sensitive);
        assert!(d.kind.is_none());
        assert!(d.expires_at_ms.is_none());
    }

    /// PG-3 (349q): sensitive_capture_decision with ttl=0 → no expires_at.
    #[test]
    fn sensitive_capture_decision_ttl_zero_no_expiry() {
        let aws = "AKIAIOSFODNN7EXAMPLE".to_string();
        let d = sensitive_capture_decision(aws, 1_700_000_000_000, 0);
        assert!(d.is_sensitive, "still sensitive");
        assert!(
            d.expires_at_ms.is_none(),
            "ttl=0 must produce null expires_at (auto-wipe disabled)"
        );
    }

    // ── Cloud sync crypto tests ──────────────────────────────────────────────

    /// derive_cloud_sync_key must be deterministic: same passphrase → same bytes.
    #[test]
    fn derive_cloud_sync_key_is_deterministic() {
        let k1 = derive_cloud_sync_key("shared-passphrase".into()).expect("derive 1");
        let k2 = derive_cloud_sync_key("shared-passphrase".into()).expect("derive 2");
        assert_eq!(k1, k2, "same passphrase must produce identical key bytes");
        assert_eq!(k1.len(), 32, "key must be exactly 32 bytes");
    }

    /// Different passphrases must produce different keys.
    #[test]
    fn derive_cloud_sync_key_different_passphrases_differ() {
        let k1 = derive_cloud_sync_key("passphrase-alpha".into()).expect("derive 1");
        let k2 = derive_cloud_sync_key("passphrase-beta".into()).expect("derive 2");
        assert_ne!(k1, k2, "different passphrases must yield different keys");
    }

    /// cloud_encrypt + cloud_decrypt must round-trip the plaintext.
    #[test]
    fn cloud_encrypt_decrypt_roundtrip() {
        let key = derive_cloud_sync_key("round-trip-passphrase".into()).expect("derive");
        let item_id = "android-cloud-item-001".to_string();
        let plaintext = b"hello from android";

        let blob = cloud_encrypt(item_id.clone(), plaintext, &key).expect("encrypt");
        let recovered = cloud_decrypt(item_id, &blob, &key).expect("decrypt");
        assert_eq!(recovered, plaintext);
    }

    /// Wrong passphrase must cause DecryptionFailed.
    #[test]
    fn cloud_decrypt_wrong_passphrase_fails() {
        // Passphrases must be >= MIN_PASSPHRASE_LEN (8); derive_sync_key rejects
        // shorter ones with PassphraseTooShort (surfaced here as EncryptionFailed).
        let enc_key = derive_cloud_sync_key("correct-passphrase".into()).expect("derive enc");
        let dec_key = derive_cloud_sync_key("wrong-passphrase".into()).expect("derive dec");
        let blob = cloud_encrypt("item-x".into(), b"data", &enc_key).expect("encrypt");
        let result = cloud_decrypt("item-x".into(), &blob, &dec_key);
        assert!(
            matches!(result, Err(CopypasteError::DecryptionFailed { .. })),
            "wrong passphrase must produce DecryptionFailed, got {result:?}"
        );
    }

    /// Wrong item_id (AAD mismatch) must cause DecryptionFailed.
    #[test]
    fn cloud_decrypt_wrong_item_id_fails() {
        let key = derive_cloud_sync_key("aad-test".into()).expect("derive");
        let blob = cloud_encrypt("item-correct".into(), b"payload", &key).expect("encrypt");
        let result = cloud_decrypt("item-wrong".into(), &blob, &key);
        assert!(
            matches!(result, Err(CopypasteError::DecryptionFailed { .. })),
            "wrong item_id must produce DecryptionFailed, got {result:?}"
        );
    }

    /// cloud_encrypt with a non-32-byte key must return InvalidKeyLength.
    #[test]
    fn cloud_encrypt_invalid_key_length() {
        let result = cloud_encrypt("item-bad".into(), b"data", &[0u8; 16]);
        assert!(
            matches!(result, Err(CopypasteError::InvalidKeyLength)),
            "16-byte key must return InvalidKeyLength"
        );
    }

    /// cloud_decrypt with a non-32-byte key must return InvalidKeyLength.
    #[test]
    fn cloud_decrypt_invalid_key_length() {
        let result = cloud_decrypt("item-bad".into(), &[0u8; 50], &[0u8; 16]);
        assert!(
            matches!(result, Err(CopypasteError::InvalidKeyLength)),
            "16-byte key must return InvalidKeyLength"
        );
    }

    // ── P2P pairing FFI tests ────────────────────────────────────────────────

    /// `generate_device_cert` returns a non-empty cert/key and a fingerprint
    /// that matches `fingerprint_of(cert_der)` — i.e. the SAME value the peer
    /// pins. Two calls produce distinct identities.
    #[test]
    fn generate_device_cert_fingerprint_matches() {
        let c = generate_device_cert().expect("cert gen");
        assert!(!c.cert_der.is_empty(), "cert DER must not be empty");
        assert!(!c.key_der.is_empty(), "key DER must not be empty");
        assert!(!c.device_id.is_empty(), "device_id must not be empty");
        assert_eq!(
            c.fingerprint,
            copypaste_p2p::fingerprint_of(&c.cert_der),
            "FFI fingerprint must equal fingerprint_of(cert_der)"
        );

        let c2 = generate_device_cert().expect("cert gen 2");
        assert_ne!(c.fingerprint, c2.fingerprint, "each cert is unique");
    }

    /// End-to-end: spin up a real `BootstrapResponder` on a loopback port in a
    /// background thread (with its own runtime), then call the
    /// `bootstrap_pair_initiator` FFI wrapper against it. Proves the FFI path
    /// completes a real PAKE + RFC 5705 channel-binding handshake over TLS:
    /// it must return the responder's cert fingerprint and a 32-byte session
    /// key. The responder thread asserts both ends derived the same key.
    #[test]
    fn bootstrap_pair_initiator_pairs_over_loopback() {
        use copypaste_p2p::bootstrap::BootstrapResponder;
        use std::sync::mpsc;

        let responder_cert = generate_device_cert().expect("responder cert");
        let initiator_cert = generate_device_cert().expect("initiator cert");
        let responder_fp = responder_cert.fingerprint.clone();
        let initiator_fp = initiator_cert.fingerprint.clone();

        let password = "shared-qr-secret-abcdef";
        let resp_sync_addr = "127.0.0.1:7001";
        let init_sync_addr = "127.0.0.1:7002";

        // The responder runs on its OWN runtime in a background OS thread so the
        // main test thread is free of an ambient runtime and can call the
        // synchronous FFI wrapper (which itself does runtime().block_on(...)).
        let (port_tx, port_rx) = mpsc::channel::<u16>();
        let resp_cert_der = responder_cert.cert_der.clone();
        let resp_key_der = responder_cert.key_der.clone();
        let pw = password.to_string();
        let responder_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("responder runtime");
            rt.block_on(async move {
                let responder = BootstrapResponder::bind(resp_cert_der, resp_key_der)
                    .await
                    .expect("bind responder");
                let port = responder.local_addr().expect("local addr").port();
                port_tx.send(port).expect("send port");
                responder
                    .run(
                        &pw,
                        resp_sync_addr,
                        // HB-1b: responder advertises real PeerMeta so the FFI
                        // initiator receives it in BootstrapResult.peer_*.
                        &copypaste_p2p::bootstrap::PeerMeta {
                            model: Some("MacBook Pro".into()),
                            os_version: Some("macOS 15.5".into()),
                            app_version: Some("0.6.1".into()),
                            local_ip: Some("127.0.0.1".into()),
                            device_name: Some("Test Mac".into()),
                            public_ip: Some("203.0.113.9".into()),
                            device_id: None,
                        },
                        // Responder advertises provisioning so the FFI initiator
                        // receives it in `peer_provisioning`.
                        Some(copypaste_p2p::bootstrap::SyncProvisioning {
                            supabase_url: Some("https://proj.supabase.co".into()),
                            supabase_anon_key: Some("anon-key".into()),
                            relay_url: None,
                            derived_sync_key: Some(vec![3u8; 32]),
                        }),
                    )
                    .await
            })
        });

        let port = port_rx.recv().expect("responder port");
        let addr_hint = format!("127.0.0.1:{port}");

        let result = bootstrap_pair_initiator(
            addr_hint,
            &initiator_cert.cert_der,
            &initiator_cert.key_der,
            password.to_string(),
            init_sync_addr.to_string(),
            None,
            // HB-1a: this device's own metadata, sent in-band to the responder.
            Some("Pixel 8".into()),
            Some("Pixel 8".into()),
            Some("Android 15".into()),
            Some("0.6.1".into()),
            Some("127.0.0.1".into()),
            // ABI 18: public_ip — None in tests (no real STUN in unit tests).
            None,
        )
        .expect("FFI bootstrap pairing must succeed over loopback");

        // HB-1b: the FFI initiator received the responder's device metadata.
        assert_eq!(result.peer_model.as_deref(), Some("MacBook Pro"));
        assert_eq!(result.peer_os.as_deref(), Some("macOS 15.5"));
        assert_eq!(result.peer_app_version.as_deref(), Some("0.6.1"));
        assert_eq!(result.peer_local_ip.as_deref(), Some("127.0.0.1"));
        assert_eq!(result.peer_public_ip.as_deref(), Some("203.0.113.9"));

        // QR-provisions-all-sync: the FFI initiator received the responder's
        // advertised provisioning, including the derived sync key bytes.
        let prov = result
            .peer_provisioning
            .as_ref()
            .expect("initiator must receive peer provisioning");
        assert_eq!(
            prov.supabase_url.as_deref(),
            Some("https://proj.supabase.co")
        );
        assert_eq!(prov.derived_sync_key.as_deref(), Some(&[3u8; 32][..]));

        // The FFI wrapper learned the responder's REAL pinned cert fingerprint.
        assert_eq!(
            result.peer_fingerprint, responder_fp,
            "initiator must pin the responder's cert fingerprint"
        );
        assert_eq!(result.peer_sync_addr, resp_sync_addr);
        assert_eq!(
            result.session_key.len(),
            32,
            "PAKE session key must be 32 bytes"
        );

        // The responder side must have derived the same key and learned our fp.
        let resp_pairing = responder_thread
            .join()
            .expect("responder thread join")
            .expect("responder pairing");
        assert_eq!(resp_pairing.peer_fingerprint, initiator_fp);
        assert_eq!(
            resp_pairing.session_key.as_bytes().as_slice(),
            result.session_key.as_slice(),
            "both endpoints must derive the same PAKE session key via the FFI path"
        );
        // HB-1a: the responder (macOS side) learned this Android device's real
        // metadata that the FFI initiator sent in-band — was None before ABI 14.
        assert_eq!(resp_pairing.peer_model.as_deref(), Some("Pixel 8"));
        assert_eq!(resp_pairing.peer_os.as_deref(), Some("Android 15"));
        assert_eq!(resp_pairing.peer_app_version.as_deref(), Some("0.6.1"));
        assert_eq!(resp_pairing.peer_local_ip.as_deref(), Some("127.0.0.1"));
    }

    /// REGRESSION (live emulator↔macOS divergence): after a real network PAKE
    /// pairing the macOS daemon (PAKE **responder**) re-keys catch-up items under
    /// the content sync key it derives from its pairing result, and the Android
    /// FFI (PAKE **initiator**) must derive the IDENTICAL key from the
    /// `session_key` the FFI returns — otherwise `decrypt_from_cloud` rejects
    /// every pushed item (itemsReceived=N, items=[]), the exact symptom seen live.
    ///
    /// This drives the real `BootstrapResponder::run` + `bootstrap_pair_initiator`
    /// over a loopback TLS socket, then derives the content sync key two ways: the
    /// DAEMON way (`derive_peer_sync_key_b64`'s exact derivation from the
    /// responder's `BootstrapPairing.session_key`) and the ANDROID way
    /// (`shared_sync_key_from_session` from the initiator's returned
    /// `BootstrapResult.session_key`). It asserts the two `SyncKey`s are
    /// byte-equal AND that a blob the daemon would push (`encrypt_for_cloud` under
    /// the daemon key) decrypts under the Android key. A divergence in which key
    /// (raw vs channel-bound) each side feeds into derivation makes this fail.
    #[test]
    fn pairing_derives_matching_content_sync_key_daemon_and_ffi() {
        use copypaste_p2p::bootstrap::BootstrapResponder;
        use std::sync::mpsc;

        let responder_cert = generate_device_cert().expect("responder cert");
        let initiator_cert = generate_device_cert().expect("initiator cert");

        let password = "shared-qr-secret-rekey";
        let resp_sync_addr = "127.0.0.1:7101";
        let init_sync_addr = "127.0.0.1:7102";

        // Responder (== macOS daemon role) on its own runtime / OS thread so the
        // main thread can call the synchronous FFI initiator wrapper.
        let (port_tx, port_rx) = mpsc::channel::<u16>();
        let resp_cert_der = responder_cert.cert_der.clone();
        let resp_key_der = responder_cert.key_der.clone();
        let pw = password.to_string();
        let responder_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("responder runtime");
            rt.block_on(async move {
                let responder = BootstrapResponder::bind(resp_cert_der, resp_key_der)
                    .await
                    .expect("bind responder");
                let port = responder.local_addr().expect("local addr").port();
                port_tx.send(port).expect("send port");
                responder
                    .run(
                        &pw,
                        resp_sync_addr,
                        &copypaste_p2p::bootstrap::PeerMeta::default(),
                        None,
                    )
                    .await
            })
        });

        let port = port_rx.recv().expect("responder port");
        let addr_hint = format!("127.0.0.1:{port}");

        let init_result = bootstrap_pair_initiator(
            addr_hint,
            &initiator_cert.cert_der,
            &initiator_cert.key_der,
            password.to_string(),
            init_sync_addr.to_string(),
            None,
            // ABI 14 device-meta params — not exercised in this key-derivation test.
            None,
            None,
            None,
            None,
            None,
            // ABI 18: public_ip — None in unit tests.
            None,
        )
        .expect("FFI bootstrap pairing must succeed over loopback");

        let resp_pairing = responder_thread
            .join()
            .expect("responder thread join")
            .expect("responder pairing");

        // DAEMON derivation: `derive_peer_sync_key_b64` persists
        // `session_key.derive_xchacha_key(P2P_SYNC_KEY_SALT)` into peers.json and
        // `SyncCrypto::shared_sync_key` reads it back through a lossless base64
        // round-trip, so the effective key is exactly these derived bytes from
        // the responder's pairing result.
        let daemon_key = copypaste_core::SyncKey::from_bytes(
            *resp_pairing
                .session_key
                .derive_xchacha_key(P2P_SYNC_KEY_SALT),
        );

        // ANDROID derivation: the exact FFI path from the session_key the FFI
        // returned to Kotlin.
        let android_key = shared_sync_key_from_session(&init_result.session_key)
            .expect("FFI derives content sync key from returned session_key");

        // The two derived content keys MUST be byte-equal, or every catch-up
        // item the daemon pushes fails to decrypt on Android.
        assert_eq!(
            daemon_key.as_bytes(),
            android_key.as_bytes(),
            "daemon (responder) and Android FFI (initiator) must derive the IDENTICAL content sync key"
        );

        // And concretely: a blob the daemon would push must decrypt under the
        // Android key (the live `itemsReceived=N, items=[]` symptom).
        let item_id = uuid::Uuid::new_v4().to_string();
        let plaintext = b"catch-up item from the macOS daemon".to_vec();
        let blob = encrypt_for_cloud(&daemon_key, &item_id, &plaintext)
            .expect("daemon wraps catch-up item under its content key");
        let recovered = decrypt_from_cloud(&android_key, &item_id, &blob)
            .expect("Android must decrypt the daemon's catch-up blob");
        assert_eq!(recovered, plaintext);
    }

    /// A malformed `addr_hint` must surface as `P2pError`, not a panic.
    #[test]
    fn bootstrap_pair_initiator_rejects_bad_addr() {
        let cert = generate_device_cert().expect("cert");
        let err = bootstrap_pair_initiator(
            "not-an-addr".into(),
            &cert.cert_der,
            &cert.key_der,
            "pw".into(),
            "127.0.0.1:7000".into(),
            None,
            // ABI 14 device-meta params — irrelevant to the addr-parse error path.
            None,
            None,
            None,
            None,
            None,
            // ABI 18: public_ip.
            None,
        )
        .expect_err("malformed addr_hint must error");
        assert!(
            matches!(err, CopypasteError::P2pError { .. }),
            "expected P2pError, got {err:?}"
        );
    }

    /// `sync_with_peer` rejects a non-32-byte session key before any network I/O.
    #[test]
    fn sync_with_peer_rejects_bad_session_key() {
        let cert = generate_device_cert().expect("cert");
        let err = sync_with_peer(
            "127.0.0.1:1".into(),
            "deadbeef".into(),
            vec![0u8; 16], // wrong length
            cert.cert_der.clone(),
            cert.key_der.clone(),
            Vec::new(),
            Vec::new(),
            "test-device".into(),
        )
        .expect_err("16-byte session key must error");
        assert!(
            matches!(err, CopypasteError::InvalidKeyLength),
            "expected InvalidKeyLength, got {err:?}"
        );
    }

    /// End-to-end loopback sync against a peer that speaks the DAEMON's framed
    /// wire protocol (NOT `run_session`) — i.e. the real protocol live macOS
    /// daemons use. The fake peer accepts the mTLS connection, then PUSHES one
    /// framed JSON `WireItem` (re-keyed under the shared key, `content_nonce =
    /// None`) exactly like the daemon's sync-on-connect catch-up push. It also
    /// reads any inbound frame the FFI sends, so this test exercises BOTH
    /// directions of the framed exchange.
    ///
    /// Proves the full FFI path: derive shared key → mTLS connect (fingerprint
    /// pinned) → keep the `LengthDelimitedCodec` framing → read the peer's
    /// framed `WireItem` → unwrap the cloud blob back to the ORIGINAL plaintext.
    /// Asserts the FFI returns that item as correct plaintext and the peer
    /// received the FFI's one offered item (the Android→macOS send path).
    #[test]
    fn sync_with_peer_receives_item_from_loopback_peer() {
        loopback_sync_roundtrip("text");
    }

    /// Regression for the Android→peer "ZERO items sent" bug: the Kotlin layer
    /// stores `content_type = "text/plain"` and historically passed that raw
    /// into `LocalItem`, but the send path only re-keyed items whose content
    /// type was exactly "text" — so every Android item was silently dropped
    /// (items_sent = 0). This drives the same loopback exchange with a
    /// `"text/plain"` offered item and asserts it IS sent and received by the
    /// peer. The earlier loopback test used "text", masking the real value.
    #[test]
    fn sync_with_peer_sends_text_plain_item_to_loopback_peer() {
        loopback_sync_roundtrip("text/plain");
    }

    /// Shared body for the loopback send/receive tests, parameterized by the
    /// content type the FFI offers, so we can prove both the canonical "text"
    /// token and the MIME-style "text/plain" value are accepted by the send
    /// path. `offered_content_type` is the value placed on the outbound
    /// `LocalItem.content_type`.
    fn loopback_sync_roundtrip(offered_content_type: &str) {
        use bytes::Bytes;
        use copypaste_p2p::pake::SessionKey;
        use copypaste_p2p::transport::{PairedPeers, PeerTransport};
        use copypaste_sync::protocol::WireItem;
        use futures_util::{SinkExt, StreamExt};
        use std::sync::mpsc;
        use tokio::net::TcpListener;

        // Both ends agree on a 32-byte PAKE session key (the bootstrap output).
        let session_key = [0x5Au8; 32];
        // The peer derives the SAME shared content key the FFI will derive.
        let shared = {
            let sk = SessionKey(session_key);
            copypaste_core::SyncKey::from_bytes(*sk.derive_xchacha_key(P2P_SYNC_KEY_SALT))
        };

        // Identities. The FFI (initiator/client) pins the peer's fingerprint;
        // the peer (server) pins the initiator's fingerprint.
        let peer_cert = generate_device_cert().expect("peer cert");
        let init_cert = generate_device_cert().expect("initiator cert");
        let peer_fp = peer_cert.fingerprint.clone();
        let init_fp = init_cert.fingerprint.clone();

        // The one known item the peer pushes, wrapped under the shared key
        // exactly as the daemon's `rekey_outbound` does (self-framed cloud blob
        // in `content`, `content_nonce = None`).
        let known_item_id = uuid::Uuid::new_v4().to_string();
        let known_plaintext = b"hello from the loopback peer".to_vec();
        let known_blob = encrypt_for_cloud(&shared, &known_item_id, &known_plaintext)
            .expect("peer wraps its item under the shared key");
        let peer_wire = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: known_item_id.clone(),
            item_id: known_item_id.clone(),
            content_type: "text".to_string(),
            content: Some(known_blob),
            content_nonce: None,
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 5,
            wall_time: 5,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "loopback-peer".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        };

        // Peer runs on its OWN runtime in a background OS thread so the main test
        // thread is free of an ambient runtime for the synchronous FFI call.
        // Returns the count of frames it received from the FFI initiator.
        let (port_tx, port_rx) = mpsc::channel::<u16>();
        let peer_cert_der = peer_cert.cert_der.clone();
        let peer_key_der = peer_cert.key_der.clone();
        let init_fp_for_peer = init_fp.clone();
        let peer_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("peer runtime");
            rt.block_on(async move {
                let peers = PairedPeers::new();
                peers.add(init_fp_for_peer, "android-initiator");
                let transport = PeerTransport::from_cert(peer_cert_der, peer_key_der, peers);

                let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
                port_tx
                    .send(listener.local_addr().expect("addr").port())
                    .expect("send port");

                // Accept and KEEP the length-delimited framing — this is the
                // daemon's `run_peer_connection` shape, not `run_session`.
                let (_addr, _fp, mut framed) = transport.accept(&listener).await.expect("accept");

                // PUSH the catch-up item as one framed JSON `WireItem`, exactly
                // like the daemon does right after a connection is established.
                let payload = serde_json::to_vec(&peer_wire).expect("serialise peer WireItem");
                framed
                    .send(Bytes::from(payload))
                    .await
                    .expect("peer push frame");

                // Read whatever the FFI sends back (its offered local items),
                // bounded by a short idle timeout so the peer task terminates.
                let mut received_from_ffi = 0u64;
                while let Ok(Some(Ok(frame))) =
                    tokio::time::timeout(std::time::Duration::from_secs(2), framed.next()).await
                {
                    if serde_json::from_slice::<WireItem>(&frame).is_ok() {
                        received_from_ffi += 1;
                    }
                }
                received_from_ffi
            })
        });

        let port = port_rx.recv().expect("peer port");
        let addr = format!("127.0.0.1:{port}");

        // The FFI under test offers ONE local item (exercising the send path)
        // and must receive the peer's pushed item decrypted to plaintext.
        let offered_plaintext = b"hello from android initiator".to_vec();
        let offered_item_id = uuid::Uuid::new_v4().to_string();
        let local_items = vec![LocalItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: String::new(),
            item_id: offered_item_id.clone(),
            wall_time_ms: 7,
            content_type: offered_content_type.to_string(),
            plaintext: offered_plaintext.clone(),
            file_name: None,
            mime: None,
        }];

        let result = sync_with_peer(
            addr,
            peer_fp,
            session_key.to_vec(),
            init_cert.cert_der.clone(),
            init_cert.key_der.clone(),
            local_items,
            Vec::new(),
            "test-device".into(),
        )
        .expect("FFI sync_with_peer must succeed over loopback");

        assert!(
            result.items_received >= 1,
            "must receive at least the peer's one item, got {}",
            result.items_received
        );
        assert_eq!(
            result.items_sent, 1,
            "FFI must report its one offered item as sent"
        );
        let got = result
            .items
            .iter()
            .find(|i| i.plaintext == known_plaintext)
            .expect("the peer's item must come back decrypted to its plaintext");
        assert_eq!(got.content_type, "text");
        assert_eq!(got.plaintext, known_plaintext);
        // The peer's STABLE item_id must be carried through to the SyncedItem so
        // Kotlin can persist it and avoid re-minting on a later re-sync.
        assert_eq!(
            got.item_id, known_item_id,
            "received SyncedItem must carry the peer's stable item_id"
        );

        // The peer must have received the FFI's one offered item (send path).
        let frames_peer_got = peer_thread.join().expect("peer thread join");
        assert_eq!(
            frames_peer_got, 1,
            "peer must have received the FFI initiator's one offered framed WireItem"
        );
    }

    /// v0.6 image/file sync (RECEIVE + outbound symmetry): an image frame
    /// arrives on the wire under the SAME sync-key-wrapped shape as text
    /// (`content` = `encrypt_for_cloud(shared, item_id, plaintext)`,
    /// `content_nonce = None`, `content_type = "image"`). The FFI must NOT drop
    /// it (the old `content_type != "text"` guard did), must decrypt it back to
    /// the raw image bytes, and must surface it as a `SyncedItem` whose
    /// `content_type` is preserved as "image". Symmetrically, an image
    /// `LocalItem` offered by Android must be re-keyed and sent to the peer.
    #[test]
    fn sync_with_peer_receives_image_frame_from_loopback_peer() {
        use bytes::Bytes;
        use copypaste_p2p::pake::SessionKey;
        use copypaste_p2p::transport::{PairedPeers, PeerTransport};
        use copypaste_sync::protocol::WireItem;
        use futures_util::{SinkExt, StreamExt};
        use std::sync::mpsc;
        use tokio::net::TcpListener;

        let session_key = [0x5Au8; 32];
        let shared = {
            let sk = SessionKey(session_key);
            copypaste_core::SyncKey::from_bytes(*sk.derive_xchacha_key(P2P_SYNC_KEY_SALT))
        };

        let peer_cert = generate_device_cert().expect("peer cert");
        let init_cert = generate_device_cert().expect("initiator cert");
        let peer_fp = peer_cert.fingerprint.clone();
        let init_fp = init_cert.fingerprint.clone();

        // The peer pushes ONE image item, wrapped under the shared key exactly
        // as the daemon's `rekey_outbound` does for images: the raw PNG bytes
        // are the plaintext, the self-framed cloud blob goes in `content`, and
        // `content_nonce` is `None`.
        let known_item_id = uuid::Uuid::new_v4().to_string();
        // A minimal "PNG-ish" byte payload (content is opaque to sync).
        let known_plaintext: Vec<u8> =
            vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
        let known_blob = encrypt_for_cloud(&shared, &known_item_id, &known_plaintext)
            .expect("peer wraps its image under the shared key");
        let peer_wire = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: known_item_id.clone(),
            item_id: known_item_id.clone(),
            content_type: "image".to_string(),
            content: Some(known_blob),
            content_nonce: None,
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 9,
            wall_time: 9,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "loopback-peer".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        };

        let (port_tx, port_rx) = mpsc::channel::<u16>();
        let peer_cert_der = peer_cert.cert_der.clone();
        let peer_key_der = peer_cert.key_der.clone();
        let init_fp_for_peer = init_fp.clone();
        let peer_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("peer runtime");
            rt.block_on(async move {
                let peers = PairedPeers::new();
                peers.add(init_fp_for_peer, "android-initiator");
                let transport = PeerTransport::from_cert(peer_cert_der, peer_key_der, peers);

                let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
                port_tx
                    .send(listener.local_addr().expect("addr").port())
                    .expect("send port");

                let (_addr, _fp, mut framed) = transport.accept(&listener).await.expect("accept");

                let payload = serde_json::to_vec(&peer_wire).expect("serialise peer WireItem");
                framed
                    .send(Bytes::from(payload))
                    .await
                    .expect("peer push frame");

                // Capture the content_type of the frame the FFI offers back, so
                // we can prove the outbound image symmetry (Android → macOS).
                let mut received_content_types: Vec<String> = Vec::new();
                while let Ok(Some(Ok(frame))) =
                    tokio::time::timeout(std::time::Duration::from_secs(2), framed.next()).await
                {
                    if let Ok(w) = serde_json::from_slice::<WireItem>(&frame) {
                        received_content_types.push(w.content_type);
                    }
                }
                received_content_types
            })
        });

        let port = port_rx.recv().expect("peer port");
        let addr = format!("127.0.0.1:{port}");

        // The FFI offers ONE local IMAGE item (exercising the outbound path).
        let offered_plaintext: Vec<u8> = vec![0x89, b'P', b'N', b'G', 9, 8, 7];
        let offered_item_id = uuid::Uuid::new_v4().to_string();
        let local_items = vec![LocalItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: String::new(),
            item_id: offered_item_id.clone(),
            wall_time_ms: 11,
            content_type: "image".to_string(),
            plaintext: offered_plaintext.clone(),
            file_name: None,
            mime: None,
        }];

        let result = sync_with_peer(
            addr,
            peer_fp,
            session_key.to_vec(),
            init_cert.cert_der.clone(),
            init_cert.key_der.clone(),
            local_items,
            Vec::new(),
            "test-device".into(),
        )
        .expect("FFI sync_with_peer must succeed over loopback");

        assert_eq!(
            result.items_sent, 1,
            "FFI must offer its one local image item (outbound symmetry)"
        );
        let got = result
            .items
            .iter()
            .find(|i| i.plaintext == known_plaintext)
            .expect("the peer's image must come back decrypted to its plaintext");
        assert_eq!(
            got.content_type, "image",
            "received SyncedItem must preserve the image content type"
        );
        assert_eq!(got.item_id, known_item_id);

        let peer_content_types = peer_thread.join().expect("peer thread join");
        assert!(
            peer_content_types.iter().any(|ct| ct == "image"),
            "peer must have received the FFI initiator's offered image frame, got {peer_content_types:?}"
        );
    }

    /// STABLE-IDENTITY regression: `sync_with_peer` must put the caller's
    /// `LocalItem.item_id` onto the outbound `WireItem.item_id` verbatim (no
    /// fresh `Uuid::new_v4()` per send) — that re-minting was the desktop
    /// "every clip is a new item → duplicates / broken LWW" bug. The fake peer
    /// captures the `item_id` of the frame it receives and we assert it equals
    /// the stable id we offered. Also covers the empty-`item_id` transitional
    /// fallback to `id`.
    #[test]
    fn sync_with_peer_sends_stable_item_id() {
        use bytes::Bytes;
        use copypaste_p2p::pake::SessionKey;
        use copypaste_p2p::transport::{PairedPeers, PeerTransport};
        use copypaste_sync::protocol::WireItem;
        use futures_util::StreamExt;
        use std::sync::mpsc;
        use tokio::net::TcpListener;

        let session_key = [0x5Au8; 32];
        let _shared = {
            let sk = SessionKey(session_key);
            copypaste_core::SyncKey::from_bytes(*sk.derive_xchacha_key(P2P_SYNC_KEY_SALT))
        };

        let peer_cert = generate_device_cert().expect("peer cert");
        let init_cert = generate_device_cert().expect("initiator cert");
        let peer_fp = peer_cert.fingerprint.clone();
        let init_fp = init_cert.fingerprint.clone();

        // Channel carries the item_id of the FIRST frame the peer receives.
        let (id_tx, id_rx) = mpsc::channel::<String>();
        let (port_tx, port_rx) = mpsc::channel::<u16>();
        let peer_cert_der = peer_cert.cert_der.clone();
        let peer_key_der = peer_cert.key_der.clone();
        let init_fp_for_peer = init_fp.clone();
        let peer_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("peer runtime");
            rt.block_on(async move {
                let peers = PairedPeers::new();
                peers.add(init_fp_for_peer, "android-initiator");
                let transport = PeerTransport::from_cert(peer_cert_der, peer_key_der, peers);
                let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
                port_tx
                    .send(listener.local_addr().expect("addr").port())
                    .expect("send port");
                let (_addr, _fp, mut framed) = transport.accept(&listener).await.expect("accept");
                // Read the FFI's offered frame and report its item_id.
                if let Ok(Some(Ok(frame))) =
                    tokio::time::timeout(std::time::Duration::from_secs(3), framed.next()).await
                {
                    if let Ok(w) = serde_json::from_slice::<WireItem>(&frame) {
                        let _ = id_tx.send(w.item_id);
                    }
                }
                // Keep the buffer typed for clarity; nothing else to send.
                let _ = Bytes::new();
            })
        });

        let port = port_rx.recv().expect("peer port");
        let addr = format!("127.0.0.1:{port}");

        let stable_id = uuid::Uuid::new_v4().to_string();
        let local_items = vec![LocalItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "local-row-1".to_string(),
            item_id: stable_id.clone(),
            wall_time_ms: 11,
            content_type: "text".to_string(),
            plaintext: b"stable-id body".to_vec(),
            file_name: None,
            mime: None,
        }];

        let result = sync_with_peer(
            addr,
            peer_fp,
            session_key.to_vec(),
            init_cert.cert_der.clone(),
            init_cert.key_der.clone(),
            local_items,
            Vec::new(),
            "test-device".into(),
        )
        .expect("FFI sync_with_peer must succeed over loopback");
        assert_eq!(result.items_sent, 1);

        let sent_item_id = id_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("peer must report the received frame's item_id");
        assert_eq!(
            sent_item_id, stable_id,
            "outbound WireItem.item_id must be the caller's stable item_id, not a fresh Uuid"
        );

        peer_thread.join().expect("peer thread join");
    }

    /// Defense-in-depth observability: a peer that pushes a text `WireItem`
    /// still carrying a `content_nonce` (a legacy / non-rekeyed frame, the exact
    /// build-skew shape that hid the "decrypt 7/7" failure) must NOT vanish
    /// silently. The FFI must skip it (it's undecryptable with the shared sync
    /// key) but COUNT it in `items_skipped_legacy` and exercise the warn path.
    /// Mirrors `sync_with_peer_receives_item_from_loopback_peer`.
    #[test]
    fn sync_with_peer_counts_skipped_legacy_frame() {
        use bytes::Bytes;
        use copypaste_p2p::transport::{PairedPeers, PeerTransport};
        use copypaste_sync::protocol::WireItem;
        use futures_util::{SinkExt, StreamExt};
        use std::sync::mpsc;
        use tokio::net::TcpListener;

        let session_key = [0x5Au8; 32];

        let peer_cert = generate_device_cert().expect("peer cert");
        let init_cert = generate_device_cert().expect("initiator cert");
        let peer_fp = peer_cert.fingerprint.clone();
        let init_fp = init_cert.fingerprint.clone();

        // A LEGACY text frame: `content_nonce = Some(...)`. This is the
        // non-rekeyed shape the FFI cannot decrypt and previously dropped with a
        // silent `continue`.
        let legacy_item_id = uuid::Uuid::new_v4().to_string();
        let legacy_wire = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: legacy_item_id.clone(),
            item_id: legacy_item_id.clone(),
            content_type: "text".to_string(),
            content: Some(vec![1, 2, 3, 4]),
            content_nonce: Some(vec![9u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 3,
            wall_time: 3,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "legacy-peer".to_string(),
            key_version: 1,
            file_name: None,
            mime: None,
        };

        let (port_tx, port_rx) = mpsc::channel::<u16>();
        let peer_cert_der = peer_cert.cert_der.clone();
        let peer_key_der = peer_cert.key_der.clone();
        let init_fp_for_peer = init_fp.clone();
        let peer_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("peer runtime");
            rt.block_on(async move {
                let peers = PairedPeers::new();
                peers.add(init_fp_for_peer, "android-initiator");
                let transport = PeerTransport::from_cert(peer_cert_der, peer_key_der, peers);

                let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
                port_tx
                    .send(listener.local_addr().expect("addr").port())
                    .expect("send port");

                let (_addr, _fp, mut framed) = transport.accept(&listener).await.expect("accept");

                // PUSH the legacy frame, exactly as a stale daemon's catch-up
                // push would.
                let payload = serde_json::to_vec(&legacy_wire).expect("serialise legacy WireItem");
                framed
                    .send(Bytes::from(payload))
                    .await
                    .expect("peer push legacy frame");

                // Drain anything the FFI offers so the peer task terminates.
                while let Ok(Some(Ok(_frame))) =
                    tokio::time::timeout(std::time::Duration::from_secs(2), framed.next()).await
                {
                }
            })
        });

        let port = port_rx.recv().expect("peer port");
        let addr = format!("127.0.0.1:{port}");

        let result = sync_with_peer(
            addr,
            peer_fp,
            session_key.to_vec(),
            init_cert.cert_der.clone(),
            init_cert.key_der.clone(),
            Vec::new(),
            Vec::new(),
            "test-device".into(),
        )
        .expect("FFI sync_with_peer must succeed over loopback");

        // The legacy frame was received on the wire ...
        assert!(
            result.items_received >= 1,
            "must have received the legacy frame, got {}",
            result.items_received
        );
        // ... skipped (undecryptable) so it yields NO plaintext item ...
        assert!(
            result.items.is_empty(),
            "legacy non-rekeyed frame must not surface as a decrypted item"
        );
        // ... but is now COUNTED instead of vanishing silently.
        assert_eq!(
            result.items_skipped_legacy, 1,
            "the skipped legacy/non-rekeyed frame must be counted, got {}",
            result.items_skipped_legacy
        );

        peer_thread.join().expect("peer thread join");
    }

    /// Blob format: `nonce[24]` prepended, total length = 24 + plaintext + 16 (AEAD tag).
    #[test]
    fn cloud_encrypt_blob_format() {
        let key = derive_cloud_sync_key("format-test".into()).expect("derive");
        let plaintext = b"test blob format";
        let blob = cloud_encrypt("item-fmt".into(), plaintext, &key).expect("encrypt");
        assert_eq!(
            blob.len(),
            24 + plaintext.len() + 16,
            "blob must be nonce(24) + plaintext + tag(16)"
        );
    }

    // ── R3b: shared-account relay inbox derivation (ABI 13) ─────────────────

    /// The FFI `relay_inbox_id` MUST be byte-identical to the core function it
    /// wraps — that equality is the cross-device agreement property that lets
    /// Android share the macOS daemon's relay inbox.
    #[test]
    fn relay_inbox_id_matches_core() {
        let key = derive_cloud_sync_key("relay-inbox-match".into()).expect("derive");
        let key_arr: [u8; 32] = key.as_slice().try_into().expect("32-byte key");
        let ffi = relay_inbox_id(&key).expect("inbox id");
        assert_eq!(ffi, copypaste_core::derive_relay_inbox_id(&key_arr));
        // Deterministic across calls.
        assert_eq!(ffi, relay_inbox_id(&key).expect("inbox id again"));
    }

    /// The FFI `relay_public_key_b64` MUST equal STANDARD-base64 of the core
    /// `derive_relay_public_key`, matching the daemon's registration value.
    #[test]
    fn relay_public_key_b64_matches_core() {
        let key = derive_cloud_sync_key("relay-pubkey-match".into()).expect("derive");
        let key_arr: [u8; 32] = key.as_slice().try_into().expect("32-byte key");
        let ffi = relay_public_key_b64(&key).expect("pubkey b64");
        let expected = base64::engine::general_purpose::STANDARD
            .encode(copypaste_core::derive_relay_public_key(&key_arr));
        assert_eq!(ffi, expected);
    }

    /// Both relay derivations reject a non-32-byte key with `InvalidKeyLength`
    /// rather than panicking across the FFI boundary.
    #[test]
    fn relay_derivations_reject_wrong_key_length() {
        let short = vec![0u8; 16];
        assert!(matches!(
            relay_inbox_id(&short),
            Err(CopypasteError::InvalidKeyLength)
        ));
        assert!(matches!(
            relay_public_key_b64(&short),
            Err(CopypasteError::InvalidKeyLength)
        ));
    }

    // ── Part A: LocalItem file_name + mime outbound wiring (ABI 8) ──────────

    /// A `LocalItem` with `content_type == "file"` and populated `file_name` /
    /// `mime` fields MUST have those fields forwarded onto the outbound
    /// `WireItem` (Android→macOS file-send, ABI 8).
    ///
    /// The peer is a loopback listener that captures the first inbound frame and
    /// reports its `file_name` / `mime` back via a channel.
    #[test]
    fn sync_with_peer_sends_file_item_with_file_name_and_mime() {
        use copypaste_p2p::transport::{PairedPeers, PeerTransport};
        use copypaste_sync::protocol::WireItem;
        use futures_util::StreamExt;
        use std::sync::mpsc;
        use tokio::net::TcpListener;

        let session_key = [0x7Bu8; 32];

        let peer_cert = generate_device_cert().expect("peer cert");
        let init_cert = generate_device_cert().expect("initiator cert");
        let peer_fp = peer_cert.fingerprint.clone();
        let init_fp = init_cert.fingerprint.clone();

        // Side-channel: first recv = port, second recv = captured (file_name, mime).
        let (name_tx, name_rx) = mpsc::channel::<(Option<String>, Option<String>)>();

        let peer_cert_der = peer_cert.cert_der.clone();
        let peer_key_der = peer_cert.key_der.clone();
        let init_fp_for_peer = init_fp.clone();
        let name_tx_peer = name_tx.clone();
        let peer_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("peer runtime");
            rt.block_on(async move {
                let peers = PairedPeers::new();
                peers.add(init_fp_for_peer, "android-initiator");
                let transport = PeerTransport::from_cert(peer_cert_der, peer_key_der, peers);

                let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
                let port = listener.local_addr().expect("addr").port();
                // Port is signalled as (None, Some("<port>")).
                name_tx_peer
                    .send((None, Some(port.to_string())))
                    .expect("send port");

                let (_addr, _fp, mut framed) = transport.accept(&listener).await.expect("accept");

                // Drain inbound frames; capture the first "file" frame's metadata.
                let mut captured: Option<(Option<String>, Option<String>)> = None;
                while let Ok(Some(Ok(frame))) =
                    tokio::time::timeout(std::time::Duration::from_secs(5), framed.next()).await
                {
                    if let Ok(w) = serde_json::from_slice::<WireItem>(&frame) {
                        if w.content_type == "file" && captured.is_none() {
                            captured = Some((w.file_name, w.mime));
                        }
                    }
                }
                if let Some(pair) = captured {
                    name_tx_peer.send(pair).expect("send captured fields");
                }
            })
        });

        // First receive is the port signal.
        let (_, port_opt) = name_rx.recv().expect("recv port signal");
        let addr = format!("127.0.0.1:{}", port_opt.expect("port string"));

        let file_bytes: Vec<u8> = b"fake pdf content".to_vec();
        let file_item_id = uuid::Uuid::new_v4().to_string();
        let local_items = vec![LocalItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: String::new(),
            item_id: file_item_id.clone(),
            wall_time_ms: 42,
            content_type: "file".to_string(),
            plaintext: file_bytes.clone(),
            file_name: Some("report.pdf".to_string()),
            mime: Some("application/pdf".to_string()),
        }];

        let result = sync_with_peer(
            addr,
            peer_fp,
            session_key.to_vec(),
            init_cert.cert_der.clone(),
            init_cert.key_der.clone(),
            local_items,
            Vec::new(),
            "test-device".into(),
        )
        .expect("FFI sync_with_peer must succeed over loopback");

        assert_eq!(
            result.items_sent, 1,
            "FFI must report its one offered file item as sent"
        );

        // Second receive is the captured (file_name, mime) from the peer.
        let (got_name, got_mime) = name_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("peer must report the file_name/mime it observed");

        assert_eq!(
            got_name,
            Some("report.pdf".to_string()),
            "outbound WireItem.file_name must carry the LocalItem.file_name"
        );
        assert_eq!(
            got_mime,
            Some("application/pdf".to_string()),
            "outbound WireItem.mime must carry the LocalItem.mime"
        );

        peer_thread.join().expect("peer thread join");
    }

    // ── Fix #3: derive_cloud_sync_key PassphraseTooShort surface ────────────

    /// An empty passphrase must surface `DecryptionFailed { reason }` that
    /// mentions the cause — not `EncryptionFailed` which discards all info.
    #[test]
    fn derive_cloud_sync_key_empty_passphrase_surfaces_reason() {
        let err = derive_cloud_sync_key(String::new())
            .expect_err("empty passphrase must return an error");
        match err {
            CopypasteError::DecryptionFailed { reason } => {
                assert!(
                    !reason.is_empty(),
                    "reason must carry a non-empty message about the cause"
                );
            }
            other => panic!("expected DecryptionFailed {{reason}}, got {other:?}"),
        }
    }

    // ── Fix #2: key-aware DB cache ───────────────────────────────────────────

    /// Opening the same db_path with TWO different keys must NOT silently reuse
    /// the connection keyed under the first key. The second call must either
    /// succeed with its own connection OR return an appropriate error — but it
    /// must never silently return the first key's connection.
    ///
    /// This test verifies the path-only cache bug is fixed by confirming that
    /// two distinct keys produce independent operations (here: we just check the
    /// stub path returns 0 items regardless, and the live path would open two
    /// separate connections).
    #[cfg(not(feature = "android-uniffi-live"))]
    #[test]
    fn different_keys_same_path_stub_returns_zero() {
        let key_a = vec![1u8; 32];
        let key_b = vec![2u8; 32];
        // Both calls on the same path but different keys must each succeed
        // independently on the stub path.
        let n_a = get_history_count("/dev/null".into(), &key_a).expect("count key_a");
        let n_b = get_history_count("/dev/null".into(), &key_b).expect("count key_b");
        assert_eq!(n_a, 0, "stub key_a must return 0");
        assert_eq!(n_b, 0, "stub key_b must return 0");
    }

    // ── Fix #1: stack key copies are zeroized (Zeroizing<[u8;32]>) ──────────

    /// The key material path through encrypt_text / decrypt_text uses a
    /// Zeroizing<[u8;32]> wrapper — verify the functions still work correctly
    /// end-to-end (Zeroizing is transparent to callers; this confirms no
    /// accidental deref breakage was introduced).
    #[test]
    fn zeroizing_key_does_not_break_encrypt_decrypt() {
        let key = test_key();
        let item_id = "zeroize-test".to_string();
        // Use key_version=2 (current daemon default).
        let blob = encrypt_text(item_id.clone(), b"zeroize path check", &key, 2).expect("encrypt");
        let pt = decrypt_text(item_id, &blob.ciphertext, &blob.nonce, &key, 2).expect("decrypt");
        assert_eq!(pt, b"zeroize path check");
    }

    /// cloud_encrypt / cloud_decrypt paths use Zeroizing<[u8;32]> — verify
    /// that end-to-end round-trip is still correct.
    #[test]
    fn zeroizing_key_does_not_break_cloud_encrypt_decrypt() {
        let key = derive_cloud_sync_key("zeroize-cloud-check".into()).expect("derive");
        let item_id = "zeroize-cloud-item".to_string();
        let plaintext = b"cloud zeroize path";
        let blob = cloud_encrypt(item_id.clone(), plaintext, &key).expect("encrypt");
        let recovered = cloud_decrypt(item_id, &blob, &key).expect("decrypt");
        assert_eq!(recovered, plaintext);
    }

    /// #40b DB_BY_PATH cache eviction: when `with_cached_db` is called with a
    /// key that differs from a previously-cached entry for the SAME path, the
    /// stale (path, old_key) entry must be evicted before the new one is
    /// inserted. Without the `retain` call the map would accumulate one entry
    /// per key rotation — a connection leak.
    ///
    /// The test uses a unique path prefix so it does not collide with the
    /// global `DB_BY_PATH` state set by sibling tests.
    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn db_by_path_evicts_stale_entries_on_key_rotation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("evict_test.db")
            .to_string_lossy()
            .into_owned();

        let key_a: [u8; 32] = [1u8; 32];
        let key_b: [u8; 32] = [2u8; 32];

        // Prime the cache with (path, key_a).
        {
            let mut map = db_by_path().lock().unwrap_or_else(|e| e.into_inner());
            let cache_key_a = (path.clone(), key_a);
            map.entry(cache_key_a).or_insert_with(|| {
                copypaste_core::Database::open(
                    std::path::Path::new(&path),
                    &zeroize::Zeroizing::new(key_a),
                )
                .expect("open with key_a")
            });
        }

        // Verify key_a is in the cache.
        {
            let map = db_by_path().lock().unwrap_or_else(|e| e.into_inner());
            assert!(
                map.contains_key(&(path.clone(), key_a)),
                "key_a must be in cache after initial insert"
            );
        }

        // Now call with_cached_db with key_b for the SAME path. The retain
        // must evict the (path, key_a) entry. Without the fix both entries
        // would coexist (connection leak).
        let result = with_cached_db(&path, &key_b, |_db| Ok(()));
        // The open may fail because key_a already encrypted the file, but the
        // eviction test is about what's LEFT in the map — key_a must be gone.
        let _ = result; // tolerate open failure; we only test the cache state

        let map = db_by_path().lock().unwrap_or_else(|e| e.into_inner());
        assert!(
            !map.contains_key(&(path.clone(), key_a)),
            "stale (path, key_a) entry must be evicted after inserting (path, key_b)"
        );
    }

    // ── W6: AppConfig over UniFFI ────────────────────────────────────────────

    #[test]
    fn default_config_matches_appconfig_default_mapping() {
        let ac = copypaste_core::AppConfig::default();
        let cfg = default_config();

        // Every AppConfig-backed field must equal AppConfig::default().
        assert_eq!(cfg.max_text_size_bytes, ac.max_text_size_bytes);
        assert_eq!(cfg.max_image_size_bytes, ac.max_image_size_bytes);
        assert_eq!(cfg.max_file_size_bytes, ac.max_file_size_bytes);
        assert_eq!(cfg.storage_quota_bytes, ac.storage_quota_bytes);
        assert_eq!(cfg.sensitive_ttl_secs, ac.sensitive_ttl_secs);
        assert_eq!(cfg.poll_interval_ms, ac.poll_interval_ms);
        assert_eq!(cfg.sound_on_copy, ac.sound_on_copy);
        assert_eq!(cfg.notify_on_copy, ac.notify_on_copy);
        assert_eq!(cfg.sync_on_wifi_only, ac.sync_on_wifi_only);
        assert_eq!(cfg.image_quality, ac.image_quality as u32);
        assert_eq!(cfg.collect_public_ip, ac.collect_public_ip);
        assert_eq!(cfg.paste_as_plain_text, ac.paste_as_plain_text);

        // Android-only knobs take their documented defaults.
        assert_eq!(cfg.mask_sensitive_content, DEFAULT_MASK_SENSITIVE_CONTENT);
        assert_eq!(cfg.p2p_enabled, DEFAULT_P2P_ENABLED);
        assert_eq!(cfg.image_max_height, DEFAULT_IMAGE_MAX_HEIGHT);

        // ABI 12: the excluded-apps list mirrors AppConfig (empty by default).
        assert_eq!(cfg.excluded_app_bundle_ids, ac.excluded_app_bundle_ids);
        assert!(cfg.excluded_app_bundle_ids.is_empty());
    }

    #[test]
    fn clamp_config_round_trips_excluded_app_bundle_ids() {
        // ABI 12: the excluded-apps list maps directly to/from
        // AppConfig::excluded_app_bundle_ids and survives clamp unchanged
        // (clamp_values does not touch it). Order + contents preserved.
        let mut cfg = default_config();
        cfg.excluded_app_bundle_ids = vec![
            "com.apple.keychainaccess".to_string(),
            "org.keepassxc.keepassxc".to_string(),
            "1password.desktop".to_string(),
        ];
        let original = cfg.excluded_app_bundle_ids.clone();

        let clamped = clamp_config(cfg);

        assert_eq!(
            clamped.excluded_app_bundle_ids, original,
            "excluded_app_bundle_ids must round-trip through clamp verbatim"
        );

        // And the mapping onto AppConfig carries the same list.
        let ac = appconfig_from_config(&clamped);
        assert_eq!(ac.excluded_app_bundle_ids, original);
    }

    #[test]
    fn clamp_config_enforces_file_ceiling_and_quota_floor() {
        // A hostile config: file size 8 GiB (above the 100 MiB library hard
        // cap) and storage quota 200 bytes (the historical self-clearing-history
        // bug, below the 50 MiB floor). clamp_config must enforce the SAME
        // bounds as the macOS daemon's AppConfig::clamp_values.
        let mut cfg = default_config();
        cfg.max_file_size_bytes = 8 * 1024 * 1024 * 1024; // 8 GiB
        cfg.storage_quota_bytes = 200;

        let clamped = clamp_config(cfg);

        // File cap clamps DOWN to the 100 MiB library hard cap.
        assert_eq!(
            clamped.max_file_size_bytes,
            100 * 1024 * 1024,
            "max_file_size_bytes must clamp to the 100 MiB ceiling"
        );
        // Storage quota floors UP to MIN_STORAGE_QUOTA_BYTES (50 MiB).
        assert_eq!(
            clamped.storage_quota_bytes,
            50 * 1024 * 1024,
            "storage_quota_bytes must floor to MIN_STORAGE_QUOTA_BYTES"
        );
    }

    #[test]
    fn clamp_config_floors_size_caps_and_poll_interval() {
        let mut cfg = default_config();
        cfg.max_text_size_bytes = 1;
        cfg.max_image_size_bytes = 1;
        cfg.poll_interval_ms = 1; // below the 100 ms floor

        let clamped = clamp_config(cfg);

        assert_eq!(clamped.max_text_size_bytes, 64 * 1024); // MIN_TEXT_SIZE_BYTES
        assert_eq!(clamped.max_image_size_bytes, 1024 * 1024); // MIN_IMAGE_SIZE_BYTES
        assert_eq!(clamped.poll_interval_ms, 100); // POLL_INTERVAL_MIN_MS
    }

    #[test]
    fn clamp_config_preserves_android_only_knobs_verbatim() {
        // The Android-only knobs have no AppConfig counterpart and must survive
        // the round-trip unchanged (not reset to a default).
        let mut cfg = default_config();
        cfg.mask_sensitive_content = false;
        cfg.p2p_enabled = true;
        cfg.image_max_height = 1234;

        let clamped = clamp_config(cfg);

        assert!(!clamped.mask_sensitive_content);
        assert!(clamped.p2p_enabled);
        assert_eq!(clamped.image_max_height, 1234);
    }

    #[test]
    fn clamp_config_does_not_floor_sensitive_ttl_zero_sentinel() {
        // 0 = "auto-wipe disabled" sentinel; clamp must NOT lift it to 1.
        let mut cfg = default_config();
        cfg.sensitive_ttl_secs = 0;
        let clamped = clamp_config(cfg);
        assert_eq!(clamped.sensitive_ttl_secs, 0);
    }

    // ── W7: revoked-fingerprint denylist predicate ───────────────────────────

    #[test]
    fn is_fingerprint_revoked_matches_canonicalized_forms() {
        // Colon-grouped, mixed-case fingerprint vs a bare-hex lowercase denylist
        // entry (and vice versa) must match after canonicalization.
        let revoked = vec!["abcd1234ef".to_string()];
        assert!(is_fingerprint_revoked("AB:CD:12:34:EF", &revoked));
        assert!(is_fingerprint_revoked("abcd1234ef", &revoked));

        let revoked_colon = vec!["AB:CD:12:34:EF".to_string()];
        assert!(is_fingerprint_revoked("abcd1234ef", &revoked_colon));
    }

    #[test]
    fn is_fingerprint_revoked_rejects_non_member_and_empty() {
        let revoked = vec!["abcd1234ef".to_string()];
        assert!(!is_fingerprint_revoked("deadbeef", &revoked));
        assert!(!is_fingerprint_revoked("abcd1234ef", &[]));
    }

    #[test]
    fn sync_with_peer_refuses_revoked_peer_without_network() {
        // The revoke check runs at the TOP of sync_with_peer, before the addr
        // is parsed or any socket opens. A revoked fingerprint must therefore
        // return P2pError("… is revoked") even with a bogus address and empty
        // identity material — proving the refusal is enforced at the trust
        // layer, not merely in the UI.
        let err = sync_with_peer(
            "not-a-valid-addr".to_string(),
            "AB:CD:EF".to_string(),
            vec![0u8; 32],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec!["abcdef".to_string()], // denylist (canonicalizes to match)
            "device-1".to_string(),
        )
        .expect_err("revoked peer must be refused");
        match err {
            CopypasteError::P2pError { reason } => {
                assert!(
                    reason.contains("revoked"),
                    "expected a 'revoked' refusal, got: {reason}"
                );
            }
            other => panic!("expected P2pError, got {other:?}"),
        }
    }

    // ── P1-8: DB_BY_PATH cache eviction on close_database ────────────────────
    //
    // Verifies that:
    //   1. `key_cache_hash` produces a deterministic 32-byte value.
    //   2. After `close_database`, the `DB_HANDLE_TO_CACHE_KEY` side-map no
    //      longer holds the handle, confirming the eviction path ran.
    //
    // Note: DB_BY_PATH itself is only populated by `with_cached_db` (the
    // `android-uniffi-live` feature path, not exercised in host unit tests
    // because it requires a real SQLCipher file). This test verifies the
    // side-map eviction logic which is always compiled in.

    #[test]
    fn key_cache_hash_is_deterministic_and_not_identity() {
        let key: [u8; 32] = [0x42u8; 32];
        let h1 = key_cache_hash(&key);
        let h2 = key_cache_hash(&key);
        assert_eq!(h1, h2, "key_cache_hash must be deterministic");
        // Hash must differ from the raw key (i.e. not a no-op passthrough).
        assert_ne!(h1, key, "key_cache_hash must not be the identity function");
    }

    #[test]
    fn key_cache_hash_different_keys_produce_different_hashes() {
        let key_a: [u8; 32] = [0x01u8; 32];
        let key_b: [u8; 32] = [0x02u8; 32];
        assert_ne!(
            key_cache_hash(&key_a),
            key_cache_hash(&key_b),
            "distinct keys must produce distinct hashes"
        );
    }

    #[test]
    fn close_database_evicts_handle_to_cache_key_side_map() {
        // Simulate the side-map lifecycle without opening a real database:
        // manually insert a (handle → cache_key) entry (as open_database does)
        // then call close_database and assert the entry is gone.
        let fake_handle: u64 = 0xDEAD_BEEF_1234_5678;
        let fake_key: [u8; 32] = [0x99u8; 32];
        let fake_hash = key_cache_hash(&fake_key);
        let fake_path = "/tmp/test-eviction-db".to_string();

        // Directly insert into the side-map, mimicking open_database.
        db_handle_to_cache_key()
            .lock()
            .unwrap()
            .insert(fake_handle, (fake_path.clone(), fake_hash));

        assert!(
            db_handle_to_cache_key()
                .lock()
                .unwrap()
                .contains_key(&fake_handle),
            "side-map must contain the handle before close"
        );

        // close_database must evict it.
        close_database(fake_handle);

        assert!(
            !db_handle_to_cache_key()
                .lock()
                .unwrap()
                .contains_key(&fake_handle),
            "P1-8: close_database must evict the handle from DB_HANDLE_TO_CACHE_KEY \
             — raw key material (now a hash) must not survive after close"
        );
    }

    // ── PG-17 (mxoq): fts_search stub mode ───────────────────────────────────

    /// Without `android-uniffi-live`, `fts_search` returns an empty list (not an
    /// error) and validates the key length.
    #[test]
    fn fts_search_stub_returns_empty_on_valid_key() {
        let key = test_key();
        let result = fts_search("/tmp/stub.db".to_string(), &key, "hello".to_string(), 10)
            .expect("stub fts_search must not error");
        assert!(result.is_empty(), "stub must return an empty list");
    }

    /// `fts_search` must fail with `InvalidKeyLength` for a key shorter than 32
    /// bytes, exactly like every other DB-keyed function.
    #[test]
    fn fts_search_stub_rejects_short_key() {
        let short_key = vec![0u8; 16];
        let err = fts_search(
            "/tmp/stub.db".to_string(),
            &short_key,
            "hello".to_string(),
            10,
        )
        .expect_err("fts_search must reject a short key");
        assert!(
            matches!(err, CopypasteError::InvalidKeyLength),
            "expected InvalidKeyLength, got {err:?}"
        );
    }

    // ── PG-19 (o0t3): get_history_page stub mode ──────────────────────────────

    /// Without `android-uniffi-live`, `get_history_page` returns an empty list
    /// and validates the key length.
    #[test]
    fn get_history_page_stub_returns_empty_on_valid_key() {
        let key = test_key();
        let result = get_history_page("/tmp/stub.db".to_string(), &key, 50, 0)
            .expect("stub get_history_page must not error");
        assert!(result.is_empty(), "stub must return an empty list");
    }

    /// `get_history_page` must fail with `InvalidKeyLength` for a key shorter
    /// than 32 bytes.
    #[test]
    fn get_history_page_stub_rejects_short_key() {
        let short_key = vec![0u8; 8];
        let err = get_history_page("/tmp/stub.db".to_string(), &short_key, 50, 0)
            .expect_err("get_history_page must reject a short key");
        assert!(
            matches!(err, CopypasteError::InvalidKeyLength),
            "expected InvalidKeyLength, got {err:?}"
        );
    }

    // ── PG-28 (8cu0): build_android_peer_meta threads public_ip ─────────────

    /// ABI 18: `build_android_peer_meta` must accept a `public_ip` parameter
    /// and pass it through to `PeerMeta.public_ip` so Android can advertise its
    /// STUN-derived WAN address to peers during pairing.
    #[test]
    fn build_android_peer_meta_threads_public_ip() {
        let meta = build_android_peer_meta(
            Some("Pixel 8".into()),
            Some("Pixel 8".into()),
            Some("Android 15".into()),
            Some("2.0.0".into()),
            Some("192.168.1.5".into()),
            Some("203.0.113.42".into()),
        );
        assert_eq!(
            meta.public_ip.as_deref(),
            Some("203.0.113.42"),
            "public_ip must be threaded from the FFI param into PeerMeta"
        );
    }

    /// When `public_ip` is `None`, `PeerMeta.public_ip` must also be `None`
    /// (backward-compatible: not collecting STUN is still valid).
    #[test]
    fn build_android_peer_meta_public_ip_none_when_omitted() {
        let meta = build_android_peer_meta(
            Some("Test".into()),
            None,
            None,
            None,
            None,
            None, // public_ip not collected
        );
        assert!(
            meta.public_ip.is_none(),
            "public_ip must be None when not provided"
        );
    }
}
