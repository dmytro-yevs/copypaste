//! Tests that exercise `crate::peers` / `crate::ipc` directly rather than any
//! `p2p` submodule.
//!
//! **Scope note (ADR-017, CopyPaste-vp63.2):** these three tests were found
//! living in the old flat `p2p/mod.rs` test module but don't test P2P code —
//! `update_peer_address_*` exercises `crate::peers::update_peer_address` and
//! `persist_paired_peer_refreshes_sync_crypto_cache_iff_handle_supplied`
//! exercises `crate::ipc::IpcServer::persist_paired_peer` +
//! `crate::sync_orch::SyncCrypto`. Relocating them to their true home
//! (`peers.rs` / `ipc/tests.rs`) is out of scope for this pass (this task is
//! restricted to `crates/copypaste-daemon/src/p2p/`); they are moved here
//! verbatim only to get them out of `p2p/mod.rs`. Tracked as a follow-up.

/// `update_peer_address` updates the `address` field of a matching peer and
/// leaves all other fields (fingerprint, name, added_at, etc.) intact.
#[test]
fn update_peer_address_updates_matching_peer_only() {
    use std::net::SocketAddr;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("peers.json");

    crate::peers::save_peers(
        &path,
        &[
            crate::peers::PairedDevice {
                fingerprint: "aabb".to_string(),
                name: "Alice".to_string(),
                added_at: 1_000,
                address: Some("10.0.0.1:1000".to_string()),
                sync_key_b64: None,
                model: None,
                os_version: None,
                app_version: None,
                local_ip: None,
                // Fresh test fixture, no prior device to carry a device_id from.
                device_id: None,
                public_ip: None,
                first_sync_at: Some(500),
                last_sync_at: Some(999),
                password_file_b64: None,
                password_file_enc: None,
                supabase_account_id: None,
            },
            crate::peers::PairedDevice {
                fingerprint: "ccdd".to_string(),
                name: "Bob".to_string(),
                added_at: 2_000,
                address: Some("10.0.0.2:2000".to_string()),
                sync_key_b64: None,
                model: None,
                os_version: None,
                app_version: None,
                local_ip: None,
                // Fresh test fixture, no prior device to carry a device_id from.
                device_id: None,
                public_ip: None,
                first_sync_at: None,
                last_sync_at: None,
                password_file_b64: None,
                password_file_enc: None,
                supabase_account_id: None,
            },
        ],
    )
    .unwrap();

    let new_addr: SocketAddr = "192.168.9.9:7777".parse().unwrap();
    crate::peers::update_peer_address(&path, "aabb", new_addr).unwrap();

    let loaded = crate::peers::load_peers(&path);
    assert_eq!(loaded.len(), 2);

    let alice = loaded.iter().find(|p| p.fingerprint == "aabb").unwrap();
    assert_eq!(
        alice.address.as_deref(),
        Some("192.168.9.9:7777"),
        "Alice's address must be updated"
    );
    // Other fields must be preserved.
    assert_eq!(alice.name, "Alice");
    assert_eq!(alice.added_at, 1_000);
    assert_eq!(alice.first_sync_at, Some(500), "first_sync_at must be kept");
    assert_eq!(alice.last_sync_at, Some(999), "last_sync_at must be kept");

    // Bob must be untouched.
    let bob = loaded.iter().find(|p| p.fingerprint == "ccdd").unwrap();
    assert_eq!(
        bob.address.as_deref(),
        Some("10.0.0.2:2000"),
        "Bob's address must be unchanged"
    );
}

/// `update_peer_address` is a no-op (and not an error) when no matching peer
/// record exists.
#[test]
fn update_peer_address_no_match_is_noop() {
    use std::net::SocketAddr;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("peers.json");
    crate::peers::save_peers(
        &path,
        &[crate::peers::PairedDevice {
            fingerprint: "aabb".to_string(),
            name: "Alice".to_string(),
            added_at: 1_000,
            address: Some("10.0.0.1:1000".to_string()),
            sync_key_b64: None,
            model: None,
            os_version: None,
            app_version: None,
            local_ip: None,
            // Fresh test fixture, no prior device to carry a device_id from.
            device_id: None,
            public_ip: None,
            first_sync_at: None,
            last_sync_at: None,
            password_file_b64: None,
            password_file_enc: None,
            supabase_account_id: None,
        }],
    )
    .unwrap();

    let new_addr: SocketAddr = "192.168.9.9:7777".parse().unwrap();
    // "deadbeef" does not match "aabb".
    crate::peers::update_peer_address(&path, "deadbeef", new_addr).unwrap();

    let loaded = crate::peers::load_peers(&path);
    // Alice's address must be untouched.
    assert_eq!(
        loaded[0].address.as_deref(),
        Some("10.0.0.1:1000"),
        "unmatched update must not modify any record"
    );
}

/// H8 regression (CopyPaste-1w7): `standing_pairing_responder_loop` called
/// `IpcServer::persist_paired_peer` with `sync_crypto = None`, so the
/// in-memory sync-key cache was never refreshed after a button-pair — the
/// first sync after pairing silently fell back to "no key" until a daemon
/// restart. This test exercises the contract that `persist_paired_peer`
/// refreshes the cache when a `SyncCrypto` handle is supplied, and that it
/// does NOT refresh when `None` is passed (the pre-fix standing-responder
/// behaviour).
///
/// # RED → GREEN
/// Before the fix, the standing responder passed `None`, so this
/// `persist_paired_peer(... None)` branch is the buggy path.  The fix
/// threads a `SyncCrypto` clone into `standing_pairing_responder_loop` and
/// passes it to `persist_paired_peer` as `Some(…)`.  Both branches are
/// exercised here to pin the contract; the "None does not refresh" assertion
/// remains correct after the fix (None is still a valid caller-supplied
/// opt-out) while the real regression is caught by the "Some refreshes"
/// assertion which fails if the plumbing accidentally passes None again.
#[tokio::test]
// The TEST_ENV_LOCK guard is deliberately held across the awaited
// persist_paired_peer calls below: it serialises COPYPASTE_CONFIG_DIR so a
// parallel test cannot clobber the env var mid-await. Dropping the guard
// before the await would reintroduce the exact race this lock exists to
// prevent, so the await-holding-lock lint is suppressed here.
#[allow(clippy::await_holding_lock)]
async fn persist_paired_peer_refreshes_sync_crypto_cache_iff_handle_supplied() {
    // ── shared setup ────────────────────────────────────────────────────
    let tmp = tempfile::tempdir().unwrap();

    let env_lock = crate::TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let prev = std::env::var_os("COPYPASTE_CONFIG_DIR");
    // SAFETY: serialised via TEST_ENV_LOCK; restored before lock drops.
    unsafe {
        std::env::set_var("COPYPASTE_CONFIG_DIR", tmp.path());
    }

    // Dummy peers.json path inside the temp dir (same dir peers_file_path() uses).
    let peers_path = crate::ipc::peers_file_path();

    // ── test inputs ─────────────────────────────────────────────────────
    let session_key = copypaste_p2p::pake::SessionKey([0xABu8; 32]);
    let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
        model: None,
        os_version: None,
        app_version: None,
        local_ip: None,
        device_name: Some("Test Device".to_string()),
        public_ip: None,
        device_id: None,
        supabase_account_id: None,
    };
    let fp = "aa:bb:cc:dd:ee:ff:00:11";

    // ── branch 1: None (the pre-fix standing-responder path) ────────────
    // Create a SyncCrypto whose cache starts empty (no peers.json yet).
    // The seed bytes don't matter for cache-refresh; only the peers.json
    // path matters.
    let crypto_none = crate::sync_orch::SyncCrypto::new([0u8; 32], peers_path.clone());
    assert!(
        !crypto_none.has_cached_sync_key(),
        "precondition: cache is empty before any peer is persisted"
    );

    crate::ipc::IpcServer::persist_paired_peer(
        fp,
        "127.0.0.1:5001",
        &session_key,
        &peer_meta,
        None,
    )
    .await;

    // None was passed → reload_sync_key was never called → cache still empty.
    // This assertion PASSES before the fix, pinning the bug.
    assert!(
        !crypto_none.has_cached_sync_key(),
        "H8/CopyPaste-1w7: passing None must not refresh the cache (the pre-fix bug path)"
    );

    // Clean up peers.json before the second branch.
    let _ = std::fs::remove_file(&peers_path);

    // ── branch 2: Some (the fixed standing-responder path) ───────────────
    // Fresh SyncCrypto (no peers.json yet → cache starts empty).
    let crypto_some = crate::sync_orch::SyncCrypto::new([0u8; 32], peers_path.clone());
    assert!(
        !crypto_some.has_cached_sync_key(),
        "precondition: cache is empty before any peer is persisted"
    );

    crate::ipc::IpcServer::persist_paired_peer(
        fp,
        "127.0.0.1:5001",
        &session_key,
        &peer_meta,
        Some(&crypto_some),
    )
    .await;

    // Some(&crypto) was passed → reload_sync_key ran → cache is now populated.
    // This assertion FAILS before the fix because the standing responder
    // passed None; it PASSES after the fix threads the handle through.
    assert!(
        crypto_some.has_cached_sync_key(),
        "H8/CopyPaste-1w7: passing Some(&crypto) must refresh the cache \
         (standing responder must supply the handle, not None)"
    );

    // ── env restore ─────────────────────────────────────────────────────
    unsafe {
        match prev {
            Some(v) => std::env::set_var("COPYPASTE_CONFIG_DIR", v),
            None => std::env::remove_var("COPYPASTE_CONFIG_DIR"),
        }
    }
    drop(env_lock);
}
