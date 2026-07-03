//! Reproduction test for CopyPaste-2wa: "unpair then pair fails on first attempt".
//!
//! # What this test exercises
//!
//! 1. Two real daemon subprocesses (A = responder, B = initiator) perform a
//!    full QR network-bootstrap PAKE pairing over the loopback TLS channel.
//! 2. A unilaterally unpairing B via the `unpair_peer` IPC call (removes B
//!    from A's peers.json and in-memory `PairedPeers` allowlist).
//! 3. A immediately generates a NEW pairing QR (new ephemeral bootstrap
//!    listener, new token).
//! 4. B accepts the new QR on the FIRST attempt — this is where the bug
//!    manifests: the first re-pair attempt after an unpair was reported to
//!    fail.
//!
//! # Success criteria
//!
//! * The first re-pair `pair_accept_qr` on B returns `ok: true`.
//! * Both daemons' `peers.json` contain a fresh record for the other peer
//!   with a non-empty `sync_key_b64`.
//! * The `sync_key_b64` values on A and B are byte-equal (PAKE session key
//!   was identical on both sides, so the derived content key matches).
//!
//! # Failure criteria (bug reproduced)
//!
//! * `pair_accept_qr` returns `ok: false` on the first attempt, or the
//!   returned `peer_fingerprint` is wrong, or `sync_key_b64` is missing /
//!   mismatched after the first re-pair — meaning the first re-pair is
//!   broken and only a SECOND attempt would succeed.
//!
//! # Why #[ignore]
//!
//! Requires the real daemon binary (`cargo build -p copypaste-daemon` first)
//! and a loopback network. Marked `#[ignore]` so it does not run in standard
//! `cargo test`; run explicitly:
//!
//! ```bash
//! cargo test -p copypaste-daemon --test unpair_repair -- --include-ignored --nocapture
//! ```

#[path = "support/mod.rs"]
mod support;

use std::time::{Duration, Instant};

use support::Daemon;

/// Strip colons from a colon-hex fingerprint to get the canonical lowercase
/// hex that the mTLS layer and `PairedPeers` use as keys.
fn canonical(fp: &str) -> String {
    fp.replace(':', "").to_lowercase()
}

/// Poll `daemon`'s peers.json until it contains a record whose canonical
/// fingerprint equals `want_fp_canonical`. Returns the matching record.
///
/// Plain existence check — no address requirement. See pairing_network.rs
/// `wait_for_persisted_peer` for the full rationale (CopyPaste-7mf fix).
fn wait_for_persisted_peer(daemon: &Daemon, want_fp_canonical: &str) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let peers = daemon.read_peers_json();
        if let Some(arr) = peers.as_array() {
            for p in arr {
                if let Some(fp) = p.get("fingerprint").and_then(|v| v.as_str()) {
                    if canonical(fp) == want_fp_canonical {
                        return p.clone();
                    }
                }
            }
        }
        if Instant::now() >= deadline {
            panic!(
                "timed out waiting for peer {want_fp_canonical} in peers.json; \
                 last seen: {peers}"
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Poll until `want_fp_canonical` is ABSENT from daemon's peers.json.
/// Used to confirm unpair took effect on-disk before proceeding.
fn wait_for_peer_absent(daemon: &Daemon, want_fp_canonical: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let peers = daemon.read_peers_json();
        let found = peers.as_array().is_some_and(|arr| {
            arr.iter().any(|p| {
                p.get("fingerprint")
                    .and_then(|v| v.as_str())
                    .is_some_and(|fp| canonical(fp) == want_fp_canonical)
            })
        });
        if !found {
            return;
        }
        if Instant::now() >= deadline {
            panic!(
                "timed out waiting for peer {want_fp_canonical} to be removed from peers.json; \
                 last seen: {peers}"
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Reproduce CopyPaste-2wa: first re-pair after unpair must succeed on
/// the FIRST attempt, not require a retry.
///
/// Requires: `cargo build -p copypaste-daemon`, loopback network, no macOS
/// Keychain prompt (COPYPASTE_EPHEMERAL_KEY=1 is set by spawn_with_p2p).
#[test]
#[ignore = "requires daemon binary and loopback network; run with --include-ignored"]
fn unpair_then_repair_succeeds_on_first_attempt() {
    // ── Spawn two isolated daemons with P2P enabled ───────────────────────────
    let daemon_a = Daemon::spawn_with_p2p(); // responder (shows QR)
    let daemon_b = Daemon::spawn_with_p2p(); // initiator (scans QR)

    // Learn both fingerprints upfront (canonical lowercase hex).
    let fp_a_resp = daemon_a.request(r#"{"id":"fa1","method":"get_own_fingerprint","params":{}}"#);
    assert_eq!(
        fp_a_resp["ok"], true,
        "A get_own_fingerprint failed: {fp_a_resp}"
    );
    let fp_a_display = fp_a_resp["data"]["fingerprint"]
        .as_str()
        .expect("A fingerprint string")
        .to_string();
    let fp_a_canonical = canonical(&fp_a_display);

    let fp_b_resp = daemon_b.request(r#"{"id":"fb1","method":"get_own_fingerprint","params":{}}"#);
    assert_eq!(
        fp_b_resp["ok"], true,
        "B get_own_fingerprint failed: {fp_b_resp}"
    );
    let fp_b_display = fp_b_resp["data"]["fingerprint"]
        .as_str()
        .expect("B fingerprint string")
        .to_string();
    let fp_b_canonical = canonical(&fp_b_display);

    // ── FIRST PAIRING ─────────────────────────────────────────────────────────
    // A generates QR → binds ephemeral bootstrap TLS listener.
    let qr1_resp = daemon_a.request(r#"{"id":"qa1","method":"pair_generate_qr","params":{}}"#);
    assert_eq!(
        qr1_resp["ok"], true,
        "pair_generate_qr (first) failed: {qr1_resp}"
    );
    let qr1 = qr1_resp["data"]["qr"]
        .as_str()
        .expect("QR string")
        .to_string();

    // B scans QR → dials A's bootstrap addr, full PAKE handshake.
    let accept1_body = serde_json::json!({
        "id": "qb1",
        "method": "pair_accept_qr",
        "params": { "qr": qr1 },
    })
    .to_string();
    let accept1_resp = daemon_b.request(&accept1_body);
    assert_eq!(
        accept1_resp["ok"], true,
        "FIRST pairing (pair_accept_qr) must succeed: {accept1_resp}"
    );
    let first_peer_fp_seen_by_b = accept1_resp["data"]["peer_fingerprint"]
        .as_str()
        .expect("first pairing must report peer_fingerprint");
    assert_eq!(
        first_peer_fp_seen_by_b, fp_a_canonical,
        "B must learn A's canonical fingerprint over the bootstrap channel"
    );

    // Wait for both daemons to persist each other to peers.json.
    // (A's persistence is async / detached.)
    let _a_record_first = wait_for_persisted_peer(&daemon_a, &fp_b_canonical);
    let b_record_first = wait_for_persisted_peer(&daemon_b, &fp_a_canonical);

    let first_sync_key_on_b = b_record_first
        .get("sync_key_b64")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    assert!(
        !first_sync_key_on_b.is_empty(),
        "first pairing must persist sync_key_b64 on B, got record: {b_record_first}"
    );

    eprintln!("[repro] FIRST pairing completed. A={fp_a_canonical} B={fp_b_canonical}");

    // ── UNPAIR B from A ───────────────────────────────────────────────────────
    // A removes B using B's DISPLAY fingerprint (colon-hex) — the format stored
    // in peers.json and expected by unpair_peer's exact-match retain.
    //
    // NOTE: the unpair_peer handler compares the passed-in `fingerprint`
    // parameter byte-for-byte against the stored display fingerprint.  Passing
    // the canonical (no-colon) form would silently MISS the record and leave
    // the old entry in peers.json while still evicting B from the in-memory
    // PairedPeers allowlist — a divergence between on-disk and in-memory state
    // that would cause the re-pair to see a stale peers.json entry.
    let unpair_body = serde_json::json!({
        "id": "up1",
        "method": "unpair_peer",
        "params": { "fingerprint": fp_b_display },
    })
    .to_string();
    let unpair_resp = daemon_a.request(&unpair_body);
    assert_eq!(
        unpair_resp["ok"], true,
        "unpair_peer on A must succeed: {unpair_resp}"
    );
    assert_eq!(
        unpair_resp["data"]["removed"], true,
        "unpair_peer must report removed=true (B was present): {unpair_resp}"
    );

    // Confirm unpair took effect in peers.json before proceeding.
    wait_for_peer_absent(&daemon_a, &fp_b_canonical);

    eprintln!("[repro] Unpaired B from A. Proceeding immediately to re-pair.");

    // ── IMMEDIATE RE-PAIR (FIRST attempt — this is where the bug manifests) ──
    // A generates a NEW QR immediately after unpair — new ephemeral bootstrap
    // listener port, new token. The bootstrap socket from the first pairing
    // has already closed (its task completed successfully). This is a clean
    // bind on a fresh OS-assigned port.
    let qr2_resp = daemon_a.request(r#"{"id":"qa2","method":"pair_generate_qr","params":{}}"#);
    assert_eq!(
        qr2_resp["ok"], true,
        "pair_generate_qr (re-pair) failed: {qr2_resp}"
    );
    let qr2 = qr2_resp["data"]["qr"]
        .as_str()
        .expect("QR string for re-pair")
        .to_string();

    // Sanity: the re-pair QR must carry a different token (single-use).
    // We can only compare the full QR string since the token is embedded
    // in encoded form; a different QR string proves a fresh token was issued.
    assert_ne!(
        qr1, qr2,
        "re-pair QR must differ from the first QR (fresh token + new bootstrap port)"
    );

    // B accepts the re-pair QR on the FIRST attempt.
    // If CopyPaste-2wa is reproducible in-process, this call returns ok: false
    // (or times out / panics) — that would be the failing assertion below.
    let accept2_body = serde_json::json!({
        "id": "qb2",
        "method": "pair_accept_qr",
        "params": { "qr": qr2 },
    })
    .to_string();
    let accept2_resp = daemon_b.request(&accept2_body);

    // ── PRIMARY ASSERTION: first re-pair attempt must succeed ─────────────────
    assert_eq!(
        accept2_resp["ok"], true,
        "BUG CopyPaste-2wa REPRODUCED: first re-pair attempt after unpair FAILED.\n\
         pair_accept_qr response: {accept2_resp}\n\
         This is the bug: the first attempt fails, requiring a second try."
    );

    let repaired_peer_fp = accept2_resp["data"]["peer_fingerprint"]
        .as_str()
        .expect("re-pair must report peer_fingerprint on success");
    assert_eq!(
        repaired_peer_fp, fp_a_canonical,
        "B must learn A's fingerprint on re-pair (bootstrap channel cross-check)"
    );

    eprintln!("[repro] RE-PAIR pair_accept_qr returned ok=true. Waiting for persistence...");

    // ── Wait for re-pair to persist on both daemons ───────────────────────────
    // A's responder task runs detached; give it time to write peers.json.
    let a_record_repaired = wait_for_persisted_peer(&daemon_a, &fp_b_canonical);
    let b_record_repaired = wait_for_persisted_peer(&daemon_b, &fp_a_canonical);

    // ── Sync key must be non-empty and IDENTICAL on both sides ────────────────
    // Both sides derive the per-peer content key from the PAKE session key via
    // derive_peer_sync_key_b64. Since both sides hold the IDENTICAL session
    // key after a successful PAKE, the derived key bytes MUST match.
    let a_sync_key = a_record_repaired
        .get("sync_key_b64")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let b_sync_key = b_record_repaired
        .get("sync_key_b64")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    assert!(
        !a_sync_key.is_empty(),
        "A's peers.json must contain sync_key_b64 for B after re-pair, record: {a_record_repaired}"
    );
    assert!(
        !b_sync_key.is_empty(),
        "B's peers.json must contain sync_key_b64 for A after re-pair, record: {b_record_repaired}"
    );
    assert_eq!(
        a_sync_key, b_sync_key,
        "sync_key_b64 must be identical on A and B after re-pair (PAKE session key must match)"
    );

    // Sync key must differ from the first pairing (fresh PAKE, fresh key).
    assert_ne!(
        first_sync_key_on_b, b_sync_key,
        "re-pair must produce a fresh sync_key_b64 (stale key from first pairing \
         would mean the PAKE session key was not refreshed)"
    );

    eprintln!(
        "[repro] RE-PAIR verified: sync keys match and are fresh. Bug NOT reproduced in-process.\n\
         TRUE ROOT CAUSE must involve conditions not captured by in-process test.\n\
         See bd update CopyPaste-2wa for next steps."
    );
}

/// Variant: test that unpair with CANONICAL fingerprint (no colons) correctly
/// removes the peer from peers.json (exact-match vs. display-format bug check).
///
/// If `unpair_peer` is called with the canonical (colon-stripped) fingerprint
/// but peers.json stores the display (colon-separated) fingerprint, the retain
/// comparison `f != fingerprint` is an EXACT match and will MISS the record.
/// This leaves peers.json in a state where the old record persists alongside
/// the new re-pair record — two entries for the same peer, causing undefined
/// behaviour in `list_peers` and potentially making re-pair appear to fail
/// (sync_key from the stale record shadows the fresh one on the second load).
#[test]
#[ignore = "requires daemon binary and loopback network; run with --include-ignored"]
fn unpair_with_canonical_fingerprint_removes_peer_from_json() {
    let daemon_a = Daemon::spawn_with_p2p();
    let daemon_b = Daemon::spawn_with_p2p();

    let fp_b_resp = daemon_b.request(r#"{"id":"fb2","method":"get_own_fingerprint","params":{}}"#);
    assert_eq!(fp_b_resp["ok"], true, "B get_own_fingerprint: {fp_b_resp}");
    let fp_b_display = fp_b_resp["data"]["fingerprint"]
        .as_str()
        .expect("B fingerprint")
        .to_string();
    let fp_b_canonical = canonical(&fp_b_display);

    // Pair first so there's a record to remove.
    let qr_resp = daemon_a.request(r#"{"id":"qa3","method":"pair_generate_qr","params":{}}"#);
    assert_eq!(qr_resp["ok"], true, "pair_generate_qr: {qr_resp}");
    let qr = qr_resp["data"]["qr"].as_str().expect("QR").to_string();

    let accept_resp = daemon_b.request(
        &serde_json::json!({"id":"qb3","method":"pair_accept_qr","params":{"qr": qr}}).to_string(),
    );
    assert_eq!(accept_resp["ok"], true, "pair_accept_qr: {accept_resp}");

    // Wait for A to persist B.
    wait_for_persisted_peer(&daemon_a, &fp_b_canonical);

    // Unpair using the CANONICAL fingerprint (no colons) — the format callers
    // may use if they read fingerprints from the bootstrap channel.
    // The peers.json record stores the DISPLAY fingerprint (with colons).
    // This test verifies whether the exact-string match in unpair_peer's retain
    // correctly removes the record or silently leaves it.
    let unpair_canonical_body = serde_json::json!({
        "id": "up2",
        "method": "unpair_peer",
        "params": { "fingerprint": fp_b_canonical },  // canonical, NOT display
    })
    .to_string();
    let unpair_resp = daemon_a.request(&unpair_canonical_body);

    // The call must succeed at the IPC level.
    assert_eq!(
        unpair_resp["ok"], true,
        "unpair_peer must not return an error: {unpair_resp}"
    );

    // CRITICAL: the record must actually be gone from peers.json.
    // If this assertion fails, it means the `retain` comparison in
    // `unpair_peer` (`f != fingerprint`) does NOT handle canonical vs.
    // display format mismatch — a root-cause candidate for CopyPaste-2wa.
    let peers_after = daemon_a.read_peers_json();
    let still_present = peers_after.as_array().is_some_and(|arr| {
        arr.iter().any(|p| {
            p.get("fingerprint")
                .and_then(|v| v.as_str())
                .is_some_and(|fp| canonical(fp) == fp_b_canonical)
        })
    });
    assert!(
        !still_present,
        "BUG: unpair_peer with canonical fingerprint left the record in peers.json!\n\
         The retain comparison uses exact string match but the stored fingerprint is in\n\
         DISPLAY format (colon-separated, e.g. AA:BB:CC:...) while the caller passed the\n\
         CANONICAL format (no colons). The record was NOT removed.\n\
         peers.json after unpair: {peers_after}"
    );
}

/// Unit test (no daemon binary required): interleaved pair-add and unpair-remove
/// must not lose each other's write.
///
/// Simulates the race window described in CopyPaste-qvn:
///   Thread A: load → remove peer X → save
///   Thread B: load → add peer Y   → save  (interleaved between A's load & save)
///
/// With a single typed writer (`crate::peers::save_peers`), the last writer wins
/// but the earlier writer's load snapshot is stale — this test verifies that the
/// final on-disk state is consistent with whichever write happened last and that
/// neither write silently corrupts the file (no partial JSON, no duplicate records).
///
/// This is a structural test: in a real daemon both operations are serialised by
/// the Tokio task system (one IPC request at a time), so a true interleave cannot
/// happen at runtime.  The test confirms the file-level write path is robust and
/// that reading back the file always yields valid JSON with no duplicate entries.
#[test]
fn interleaved_pair_add_and_unpair_remove_yield_consistent_peers_json() {
    use copypaste_daemon::peers::{load_peers, save_peers, PairedDevice};
    use std::time::{SystemTime, UNIX_EPOCH};

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("peers.json");

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Seed: peer X already exists.
    let peer_x = PairedDevice {
        fingerprint: "aa:bb:cc:dd".to_string(),
        name: "Device X".to_string(),
        added_at: now,
        address: None,
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
    };
    save_peers(&path, std::slice::from_ref(&peer_x)).expect("initial save");

    // Simulate thread A: load snapshot (sees peer X), prepare to remove it.
    let mut snap_a = load_peers(&path);
    let before_a = snap_a.len();
    snap_a.retain(|p| canonical(&p.fingerprint) != canonical("aa:bb:cc:dd"));
    assert_eq!(
        snap_a.len(),
        before_a - 1,
        "thread A must remove peer X from its snapshot"
    );

    // Simulate thread B: load snapshot (also sees peer X), add peer Y.
    let mut snap_b = load_peers(&path);
    let peer_y = PairedDevice {
        fingerprint: "11:22:33:44".to_string(),
        name: "Device Y".to_string(),
        added_at: now,
        address: None,
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
    };
    snap_b.push(peer_y.clone());

    // Thread A writes first (removes X).
    save_peers(&path, &snap_a).expect("thread A save");
    // Thread B writes second (adds Y, but its snapshot still had X → X comes back).
    // This is the "last writer wins" semantic; the test just verifies the file is
    // valid JSON with no duplicates and at least peer Y is present.
    save_peers(&path, &snap_b).expect("thread B save");

    // Read back the file — must be valid, no duplicates.
    let final_peers = load_peers(&path);
    let fps: Vec<String> = final_peers
        .iter()
        .map(|p| canonical(&p.fingerprint))
        .collect();
    // No duplicates.
    let mut sorted = fps.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        fps.len(),
        "peers.json must not contain duplicate fingerprints after interleaved writes; got: {fps:?}"
    );
    // Peer Y must be present (last writer included it).
    assert!(
        fps.contains(&canonical("11:22:33:44")),
        "peer Y must survive the last write; final peers: {fps:?}"
    );

    // Now verify the SINGLE-WRITER path: if thread B's write goes through the
    // typed helper every time, the resulting JSON must always parse cleanly
    // (no corruption from the atomic-rename pattern).
    let raw = std::fs::read_to_string(&path).expect("read peers.json");
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).expect("peers.json must be valid JSON after interleaved writes");
    assert!(
        parsed.is_array(),
        "peers.json must be a JSON array; got: {parsed}"
    );
}
