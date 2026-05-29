//! P2P Phase 1 — network bootstrap PAKE pairing, end-to-end over TCP/TLS.
//!
//! This test spawns TWO real, fully-isolated `copypaste-daemon` subprocesses
//! with the P2P subsystem enabled (`COPYPASTE_P2P=1`, `COPYPASTE_EPHEMERAL_KEY=1`):
//!
//! - Daemon A is the **responder**: it generates a pairing QR. With P2P
//!   enabled, generating the QR also spins up an ephemeral, unauthenticated
//!   bootstrap TLS listener and embeds its `host:port` in the QR's `addr_hint`.
//! - Daemon B is the **initiator**: it accepts the scanned QR
//!   (`pair_accept_qr {"qr": ...}`), dials A's bootstrap address over TLS
//!   (no cert pinning), and runs the PAKE initiator handshake over that real
//!   network socket.
//!
//! Success is asserted from B's IPC response: the network branch only returns
//! `ok: true` when the OPAQUE PAKE handshake completes on BOTH endpoints (the
//! responder must finish for the initiator's final message to be consumed and
//! for the shared session key to exist). The returned `peer_fingerprint` is
//! cross-checked against A's actual cert fingerprint, proving the fingerprint
//! exchange over the bootstrap channel matched the cert A presented in TLS.
//!
//! This is NOT a single-process fake: A and B are separate OS processes, and
//! the PAKE messages + fingerprints traverse a real loopback TCP/TLS connection.

#[path = "support/mod.rs"]
mod support;

use std::time::{Duration, Instant};

use support::Daemon;

/// Strip the colons from a user-facing `XX:XX:...` fingerprint to get the
/// canonical lowercase hex the mTLS layer / bootstrap channel report.
fn canonical(fp: &str) -> String {
    fp.replace(':', "").to_lowercase()
}

/// Extract the `addr_hint` field from an encoded v1 pairing QR
/// (`CPPAIR1.<fp>.<token>.<device_id>.<name>.<addr_hint>`). addr_hint is the
/// final field and may itself contain '.' (dotted-quad IPv4) and ':', so we
/// split structurally: strip the magic, then take the 5th body field. This is
/// host-IP-agnostic (works whether the daemon advertised a LAN ip:port or the
/// 127.0.0.1 loopback fallback).
fn addr_hint_from_qr(qr: &str) -> String {
    let (_magic, body) = qr.split_once('.').expect("QR must have magic prefix");
    body.splitn(5, '.')
        .nth(4)
        .expect("QR body must have addr_hint field")
        .to_string()
}

/// Poll `daemon`'s `peers.json` until it contains a record whose canonical
/// fingerprint equals `want_fp_canonical`, returning that record. The responder
/// side persists from a detached task after PAKE completes, so a short poll
/// avoids a flaky race without an arbitrary fixed sleep.
fn wait_for_persisted_peer(daemon: &Daemon, want_fp_canonical: &str) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(5);
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
                "timed out waiting for peers.json to contain peer {want_fp_canonical}; \
                 last seen: {peers}"
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn two_daemons_complete_pake_over_network_bootstrap() {
    // Both daemons run with P2P enabled so they have a live mTLS cert and the
    // bootstrap pairing channel.
    let daemon_a = Daemon::spawn_with_p2p();
    let daemon_b = Daemon::spawn_with_p2p();

    // A's advertised (colon-hex) cert fingerprint — what the QR carries and
    // what B should learn over the bootstrap channel (in canonical form).
    let fp_resp = daemon_a.request(r#"{"id":"fa","method":"get_own_fingerprint","params":{}}"#);
    assert_eq!(
        fp_resp["ok"], true,
        "A get_own_fingerprint failed: {fp_resp}"
    );
    let fp_a_display = fp_resp["data"]["fingerprint"]
        .as_str()
        .expect("A fingerprint string")
        .to_string();
    let fp_a_canonical = canonical(&fp_a_display);

    // Step 1 — A generates a QR. With P2P on, this binds A's bootstrap TLS
    // listener and embeds its host:port in addr_hint.
    let qr_resp = daemon_a.request(r#"{"id":"qa","method":"pair_generate_qr","params":{}}"#);
    assert_eq!(qr_resp["ok"], true, "pair_generate_qr failed: {qr_resp}");
    let qr = qr_resp["data"]["qr"]
        .as_str()
        .expect("QR string in response")
        .to_string();
    assert!(qr.starts_with("CPPAIR1."), "QR must use the v1 magic: {qr}");

    // The QR must carry a non-empty, reachable addr_hint (the bootstrap listener
    // address). The daemon advertises the host's primary LAN-routable IPv4 when
    // one exists and falls back to 127.0.0.1 only on loopback-only hosts, so we
    // assert the hint is a valid host:port (LAN ip:port OR loopback) rather than
    // pinning it to 127.0.0.1. The listener binds 0.0.0.0, so either form is
    // reachable from this same host and the end-to-end PAKE below still succeeds.
    let hint = addr_hint_from_qr(&qr);
    assert!(
        hint.parse::<std::net::SocketAddr>().is_ok(),
        "QR addr_hint must be a valid reachable host:port, got: {hint:?} (QR: {qr})"
    );

    // Step 2 — B accepts the QR over the NETWORK: decode, dial A's addr_hint
    // over TLS, run the PAKE initiator. This single IPC call drives the whole
    // network handshake; it returns ok only if PAKE completed on both ends.
    let accept_body = serde_json::json!({
        "id": "qb",
        "method": "pair_accept_qr",
        "params": { "qr": qr },
    })
    .to_string();
    let accept_resp = daemon_b.request(&accept_body);

    assert_eq!(
        accept_resp["ok"], true,
        "network PAKE pairing must succeed end-to-end, got: {accept_resp}"
    );

    // B learned A's real cert fingerprint over the bootstrap channel, and it
    // matches the cert A actually presented (and advertised in the QR).
    let peer_fp = accept_resp["data"]["peer_fingerprint"]
        .as_str()
        .expect("network accept must report the peer fingerprint");
    assert_eq!(
        peer_fp, fp_a_canonical,
        "B's PAKE-confirmed peer fingerprint must equal A's cert fingerprint"
    );
}

/// Wrong/garbage QR token: if B dials with a QR whose token does not match the
/// one A's bootstrap responder registered, the PAKE handshake must fail and the
/// network accept must report an error — never a false-positive pairing.
///
/// We simulate a mismatch by having B accept a QR generated by A but pointed at
/// a *second* generation's listener with a different token: regenerating on A
/// replaces the active bootstrap responder/token, so the QR from the first
/// generation now targets a stale port. The cleaner deterministic check is that
/// a structurally-valid QR with an unreachable addr_hint fails fast.
#[test]
fn network_pairing_fails_on_unreachable_addr_hint() {
    let daemon_b = Daemon::spawn_with_p2p();

    // Build a syntactically valid QR by hand whose addr_hint points at a closed
    // loopback port (nothing is listening), so the bootstrap dial must fail.
    // Reuse B's own fingerprint format for the fingerprint field (any valid
    // colon-hex 32-byte fingerprint works; pairing fails at the dial/PAKE step).
    let fp_resp = daemon_b.request(r#"{"id":"fb","method":"get_own_fingerprint","params":{}}"#);
    let fp_display = fp_resp["data"]["fingerprint"]
        .as_str()
        .expect("B fingerprint")
        .to_string();

    // A fresh QR from a throwaway daemon gives us a real token + wire shape;
    // we then rewrite its addr_hint to an unreachable port.
    let daemon_tmp = Daemon::spawn_with_p2p();
    let qr_resp = daemon_tmp.request(r#"{"id":"qt","method":"pair_generate_qr","params":{}}"#);
    let qr = qr_resp["data"]["qr"].as_str().expect("tmp QR").to_string();

    // Replace the trailing addr_hint field (whatever host the daemon advertised
    // — LAN ip:port or loopback) with an almost-certainly-closed loopback port so
    // the bootstrap dial is refused. addr_hint is the final field; strip it off
    // structurally (host-IP-agnostic) and re-append an unreachable one.
    let real_hint = addr_hint_from_qr(&qr);
    let prefix_len = qr.len() - real_hint.len();
    let bad_qr = format!("{}127.0.0.1:1", &qr[..prefix_len]);
    let _ = &fp_display;

    let accept_body = serde_json::json!({
        "id": "qbad",
        "method": "pair_accept_qr",
        "params": { "qr": bad_qr },
    })
    .to_string();
    let accept_resp = daemon_b.request(&accept_body);

    assert_eq!(
        accept_resp["ok"], false,
        "pairing against an unreachable bootstrap addr must fail, got: {accept_resp}"
    );
}

/// P2P Phase 2: after a successful network PAKE pairing, BOTH daemons must
/// durably persist the *other* peer to their own `peers.json`, recording the
/// peer's canonical cert fingerprint AND a non-empty P2P sync-listener address
/// (exchanged in-band over the bootstrap channel). The Phase 3 outbound
/// connector relies on that persisted address rather than mDNS (loopback mDNS
/// filters 127.0.0.1 and is unreliable).
#[test]
fn pairing_persists_peer_fingerprint_and_address_on_both_sides() {
    let daemon_a = Daemon::spawn_with_p2p(); // responder
    let daemon_b = Daemon::spawn_with_p2p(); // initiator

    // Both daemons' advertised cert fingerprints, in canonical form.
    let fp_a = daemon_a.request(r#"{"id":"fa","method":"get_own_fingerprint","params":{}}"#);
    let fp_a_canonical = canonical(fp_a["data"]["fingerprint"].as_str().expect("A fp"));
    let fp_b = daemon_b.request(r#"{"id":"fb","method":"get_own_fingerprint","params":{}}"#);
    let fp_b_canonical = canonical(fp_b["data"]["fingerprint"].as_str().expect("B fp"));

    // A generates a QR (binds its bootstrap listener + embeds addr_hint).
    let qr_resp = daemon_a.request(r#"{"id":"qa","method":"pair_generate_qr","params":{}}"#);
    assert_eq!(qr_resp["ok"], true, "pair_generate_qr failed: {qr_resp}");
    let qr = qr_resp["data"]["qr"]
        .as_str()
        .expect("QR string")
        .to_string();

    // B accepts the QR over the network: full PAKE over real TCP/TLS.
    let accept_body = serde_json::json!({
        "id": "qb",
        "method": "pair_accept_qr",
        "params": { "qr": qr },
    })
    .to_string();
    let accept_resp = daemon_b.request(&accept_body);
    assert_eq!(
        accept_resp["ok"], true,
        "network PAKE pairing must succeed end-to-end, got: {accept_resp}"
    );

    // ── B (initiator) persisted A ──────────────────────────────────────────
    let a_record = wait_for_persisted_peer(&daemon_b, &fp_a_canonical);
    let a_addr = a_record
        .get("address")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        !a_addr.is_empty(),
        "B's peers.json must record A's sync address, got record: {a_record}"
    );
    assert!(
        a_addr.contains(':'),
        "A's persisted address must be host:port, got: {a_addr}"
    );

    // ── A (responder) persisted B ──────────────────────────────────────────
    // A persists from a detached task after the responder PAKE completes.
    let b_record = wait_for_persisted_peer(&daemon_a, &fp_b_canonical);
    let b_addr = b_record
        .get("address")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        !b_addr.is_empty(),
        "A's peers.json must record B's sync address, got record: {b_addr}"
    );
    assert!(
        b_addr.contains(':'),
        "B's persisted address must be host:port, got: {b_addr}"
    );

    // ── Shared content sync key MUST match on both sides ─────────────────────
    // REGRESSION (live emulator↔macOS): both daemons derive the per-peer content
    // sync key (`derive_peer_sync_key_b64`) from the PAKE session key they each
    // hold and persist it as `sync_key_b64`. After a successful pairing both
    // sides hold the IDENTICAL session key, so the persisted keys MUST be
    // byte-equal — otherwise the responder's catch-up blobs (encrypted under its
    // key) fail to decrypt on the initiator (the live `itemsReceived=N, items=[]`
    // symptom). `SyncCrypto::shared_sync_key` reads exactly this value back.
    let a_sync_key = a_record
        .get("sync_key_b64")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let b_sync_key = b_record
        .get("sync_key_b64")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        !a_sync_key.is_empty(),
        "B's peers.json must record a content sync key for A, got record: {a_record}"
    );
    assert!(
        !b_sync_key.is_empty(),
        "A's peers.json must record a content sync key for B, got record: {b_record}"
    );
    assert_eq!(
        a_sync_key, b_sync_key,
        "both daemons must persist the IDENTICAL content sync key after pairing \
         (responder's catch-up encryption key == initiator's decryption key)"
    );
}
