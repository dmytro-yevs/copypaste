//! P2P Phase 3 — REAL two-daemon clipboard sync over a live mTLS link.
//!
//! This is the end-to-end proof for the Phase 3 outbound connector + the
//! cross-device content-key mechanism. It spawns TWO real `copypaste-daemon`
//! subprocesses with P2P enabled (`COPYPASTE_P2P=1`, `COPYPASTE_EPHEMERAL_KEY=1`
//! — so each has a DIFFERENT per-device local-storage key), pairs them over the
//! network (the Phase 1/2 PAKE bootstrap flow), then:
//!
//!   1. imports a clipboard item with a KNOWN plaintext on daemon A via the
//!      `import` IPC method (which broadcasts it into the sync pipeline);
//!   2. polls daemon B's `history_page` until that item's plaintext PREVIEW
//!      appears — i.e. B not only received a row but DECRYPTED it to the same
//!      plaintext A held.
//!
//! Why this proves the hard part: items are stored encrypted under each
//! device's own local key. Without the shared content sync key established at
//! pairing, B would receive an opaque blob it could never decrypt. Asserting
//! the plaintext (not merely "a row arrived") is the whole point.
//!
//! A negative check confirms an UNPAIRED third daemon never receives the item.

#[path = "support/mod.rs"]
mod support;

use std::time::{Duration, Instant};

use support::Daemon;

fn canonical(fp: &str) -> String {
    fp.replace(':', "").to_lowercase()
}

/// Poll `daemon`'s `peers.json` until it contains a record whose canonical
/// fingerprint equals `want`, then return that record.
fn wait_for_persisted_peer(daemon: &Daemon, want: &str) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let peers = daemon.read_peers_json();
        if let Some(arr) = peers.as_array() {
            for p in arr {
                if let Some(fp) = p.get("fingerprint").and_then(|v| v.as_str()) {
                    if canonical(fp) == want {
                        return p.clone();
                    }
                }
            }
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for peers.json to contain {want}; last: {peers}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Drive the full network PAKE pairing: A generates a QR, B accepts it over the
/// network. Returns `(fp_a_canonical, fp_b_canonical)`.
fn pair(daemon_a: &Daemon, daemon_b: &Daemon) -> (String, String) {
    let fp_a = daemon_a.request(r#"{"id":"fa","method":"get_own_fingerprint","params":{}}"#);
    let fp_a_canonical = canonical(fp_a["data"]["fingerprint"].as_str().expect("A fp"));
    let fp_b = daemon_b.request(r#"{"id":"fb","method":"get_own_fingerprint","params":{}}"#);
    let fp_b_canonical = canonical(fp_b["data"]["fingerprint"].as_str().expect("B fp"));

    let qr_resp = daemon_a.request(r#"{"id":"qa","method":"pair_generate_qr","params":{}}"#);
    assert_eq!(qr_resp["ok"], true, "pair_generate_qr failed: {qr_resp}");
    let qr = qr_resp["data"]["qr"]
        .as_str()
        .expect("QR string")
        .to_string();

    let accept_body = serde_json::json!({
        "id": "qb",
        "method": "pair_accept_qr",
        "params": { "qr": qr },
    })
    .to_string();
    let accept_resp = daemon_b.request(&accept_body);
    assert_eq!(
        accept_resp["ok"], true,
        "network PAKE pairing must succeed, got: {accept_resp}"
    );

    // Both sides must persist the peer (fingerprint + address) before the
    // connector can dial. The responder (A) persists from a detached task.
    let a_rec = wait_for_persisted_peer(daemon_b, &fp_a_canonical);
    let b_rec = wait_for_persisted_peer(daemon_a, &fp_b_canonical);

    // P2P Phase 3: each side must also have derived + persisted the shared
    // content sync key — without it cross-device decryption is impossible.
    assert!(
        a_rec.get("sync_key_b64").and_then(|v| v.as_str()).is_some(),
        "B's record of A must carry the shared sync key: {a_rec}"
    );
    assert!(
        b_rec.get("sync_key_b64").and_then(|v| v.as_str()).is_some(),
        "A's record of B must carry the shared sync key: {b_rec}"
    );
    // Both sides must derive the IDENTICAL shared key (PAKE session key converges).
    assert_eq!(
        a_rec.get("sync_key_b64").and_then(|v| v.as_str()),
        b_rec.get("sync_key_b64").and_then(|v| v.as_str()),
        "both daemons must derive the same shared content sync key"
    );

    (fp_a_canonical, fp_b_canonical)
}

/// Import a single text item with `plaintext` on `daemon`, returning its
/// IPC response. The `import` handler encrypts it under the daemon's local key
/// and broadcasts it into the sync pipeline.
fn import_text(daemon: &Daemon, plaintext: &str) -> serde_json::Value {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(plaintext.as_bytes());
    let body = serde_json::json!({
        "id": "imp",
        "method": "import",
        "params": {
            "items": [{
                "content_type": "text",
                "content_bytes_b64": b64,
                "created_at_ms": 1_700_000_123_456i64,
            }],
        },
    })
    .to_string();
    daemon.request(&body)
}

/// Poll `daemon`'s `history_page` until a text item whose preview exactly
/// equals `want_plaintext` appears, or the deadline elapses. Returns true on
/// match.
fn wait_for_synced_plaintext(daemon: &Daemon, want_plaintext: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        let resp = daemon
            .request(r#"{"id":"hp","method":"history_page","params":{"limit":50,"offset":0}}"#);
        if let Some(items) = resp["data"]["items"].as_array() {
            for it in items {
                if it.get("content_type").and_then(|v| v.as_str()) == Some("text") {
                    if let Some(preview) = it.get("preview").and_then(|v| v.as_str()) {
                        if preview == want_plaintext {
                            return true;
                        }
                    }
                }
            }
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// THE Phase 3 end-to-end test: copy-on-A → same-plaintext-on-B over live P2P.
#[test]
fn copy_on_a_syncs_same_plaintext_to_paired_b_over_p2p() {
    let daemon_a = Daemon::spawn_with_p2p();
    let daemon_b = Daemon::spawn_with_p2p();
    // An unpaired third daemon — the negative control.
    let daemon_c = Daemon::spawn_with_p2p();

    // Pair A <-> B over the real network bootstrap PAKE flow.
    pair(&daemon_a, &daemon_b);

    // A unique, recognisable plaintext so the assertion is unambiguous.
    let plaintext = "phase3-live-p2p-secret-2f9a1c7e";

    // Import it on A. This encrypts under A's local key AND broadcasts it into
    // the sync pipeline; sync_orch re-keys it under the shared sync key and the
    // (now-connected) connector/fanout pushes it to B.
    let imp = import_text(&daemon_a, plaintext);
    assert_eq!(imp["ok"], true, "import on A must succeed: {imp}");
    assert_eq!(imp["data"]["inserted"], 1, "A must insert the item: {imp}");

    // The connector dials every few seconds; allow a generous window for the
    // link to come up and the item to traverse it.
    let got = wait_for_synced_plaintext(&daemon_b, plaintext, Duration::from_secs(30));
    assert!(
        got,
        "B must receive AND decrypt A's item to the same plaintext over live P2P"
    );

    // Negative: the unpaired daemon C must NOT have the item. C was never paired
    // with A, so the mTLS verifier rejects any connection and no item flows.
    let leaked = wait_for_synced_plaintext(&daemon_c, plaintext, Duration::from_secs(3));
    assert!(
        !leaked,
        "an UNPAIRED daemon must never receive A's clipboard item"
    );
}
