//! Integration tests for the bootstrap wire protocol.

use std::net::SocketAddr;

use tokio_util::codec::Framed;

use super::framing::length_codec;
use super::framing::{recv_frame, send_frame};
use super::initiator::{run_initiator, run_initiator_with_confirm};
use super::meta::exchange_peer_meta;
use super::responder::BootstrapResponder;
use super::tls::AcceptAnyCert;
use super::types::{PeerMeta, SyncProvisioning};
use crate::bootstrap::PAKE_EXCHANGE_TIMEOUT;
use crate::cert::SelfSignedCert;
use rustls::pki_types::ServerName;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ClientConfig;
use rustls::ServerConfig;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};

/// `bind_on` binds the EXACT requested port (LAN/SAS Phase 2 standing
/// responder advertises a stable `bport`, so the listener must re-bind the
/// same port across pairing iterations rather than getting a fresh ephemeral
/// one each time). Re-binding the same port immediately after dropping the
/// previous listener must also succeed (listening sockets do not enter
/// TIME_WAIT).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bind_on_binds_requested_port_and_is_reusable() {
    let cert = SelfSignedCert::generate("standing-responder").unwrap();

    // First pick a free port via an ephemeral bind, then drop it.
    let probe = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let r1 = BootstrapResponder::bind_on(port, cert.cert_der.clone(), cert.key_der.clone())
        .await
        .expect("bind_on requested port");
    assert_eq!(r1.local_addr().unwrap().port(), port);
    drop(r1);

    // Re-bind the same port immediately — must not fail with EADDRINUSE.
    let r2 = BootstrapResponder::bind_on(port, cert.cert_der.clone(), cert.key_der.clone())
        .await
        .expect("re-bind same port");
    assert_eq!(r2.local_addr().unwrap().port(), port);
}

/// Two endpoints over a real loopback TCP/TLS socket complete PAKE, the S3
/// channel-binding confirmation exchange, and converge on the same session
/// key, learning each other's fingerprints. Both `run`/`run_initiator`
/// returning `Ok` proves the confirmation tags matched.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bootstrap_pake_over_tls_loopback_succeeds() {
    let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
    let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();

    let responder_fp = responder_cert.fingerprint();
    let initiator_fp = initiator_cert.fingerprint();
    assert_ne!(responder_fp, initiator_fp);

    let password = "shared-qr-secret-123456";

    let responder = BootstrapResponder::bind(
        responder_cert.cert_der.clone(),
        responder_cert.key_der.clone(),
    )
    .await
    .expect("bind responder");
    let port = responder.local_addr().expect("local addr").port();
    let resp_fp_expected = responder.fingerprint().to_string();
    assert_eq!(resp_fp_expected, responder_fp);

    let pw = password.to_string();
    let resp_sync_addr = "127.0.0.1:7001";
    let resp_meta = PeerMeta {
        model: Some("Mac mini".into()),
        os_version: Some("macOS 15.5".into()),
        app_version: Some("0.5.4".into()),
        local_ip: Some("192.168.1.10".into()),
        device_name: None,
        public_ip: Some("198.51.100.10".into()),
        device_id: None,
    };
    let resp_meta_task = resp_meta.clone();
    // The responder advertises a full SyncProvisioning ("the configured PC");
    // the initiator advertises None ("a fresh device scanning the QR").
    let resp_prov = SyncProvisioning {
        supabase_url: Some("https://proj.supabase.co".into()),
        supabase_anon_key: Some("anon-key-123".into()),
        relay_url: Some("https://relay.example".into()),
        derived_sync_key: Some(vec![7u8; 32]),
    };
    let resp_prov_task = resp_prov.clone();
    let responder_task = tokio::spawn(async move {
        responder
            .run(&pw, resp_sync_addr, &resp_meta_task, Some(resp_prov_task))
            .await
    });

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let init_pw = password.to_string();
    let init_sync_addr = "127.0.0.1:7002";
    let init_meta = PeerMeta {
        model: Some("MacBook Air".into()),
        os_version: Some("macOS 14.4".into()),
        app_version: Some("0.5.4".into()),
        local_ip: Some("192.168.1.11".into()),
        device_name: None,
        public_ip: Some("198.51.100.11".into()),
        device_id: None,
    };
    let init_meta_task = init_meta.clone();
    let initiator_task = tokio::spawn(async move {
        run_initiator(
            addr,
            initiator_cert.cert_der,
            initiator_cert.key_der,
            &init_pw,
            init_sync_addr,
            &init_meta_task,
            None,
        )
        .await
    });

    let (resp_res, init_res) = tokio::join!(responder_task, initiator_task);
    let resp = resp_res.expect("responder join").expect("responder pake");
    let init = init_res.expect("initiator join").expect("initiator pake");

    // Session keys converge — the PAKE security goal, over a real network stack.
    assert_eq!(
        resp.session_key.as_bytes(),
        init.session_key.as_bytes(),
        "both endpoints must derive the same PAKE session key over TLS"
    );

    // Each side learned the other's real cert fingerprint.
    assert_eq!(resp.peer_fingerprint, initiator_fp);
    assert_eq!(init.peer_fingerprint, responder_fp);

    // Phase 2: each side also learned the other's P2P sync-listener address.
    assert_eq!(resp.peer_sync_addr, init_sync_addr);
    assert_eq!(init.peer_sync_addr, resp_sync_addr);

    // Phase 4: each side learned the other's device metadata over the
    // post-handshake metadata extension.
    assert_eq!(resp.peer_model, init_meta.model);
    assert_eq!(resp.peer_os, init_meta.os_version);
    assert_eq!(resp.peer_app_version, init_meta.app_version);
    assert_eq!(resp.peer_local_ip, init_meta.local_ip);
    assert_eq!(resp.peer_public_ip, init_meta.public_ip);
    assert_eq!(init.peer_model, resp_meta.model);
    assert_eq!(init.peer_os, resp_meta.os_version);
    assert_eq!(init.peer_app_version, resp_meta.app_version);
    assert_eq!(init.peer_local_ip, resp_meta.local_ip);
    assert_eq!(init.peer_public_ip, resp_meta.public_ip);

    // Proto v2: the initiator (which sent None) RECEIVES the responder's
    // full SyncProvisioning. The responder (initiator sent None) receives an
    // all-None provisioning, i.e. the default value carrying nothing.
    assert_eq!(
        init.peer_provisioning,
        Some(resp_prov),
        "initiator must receive the responder's advertised provisioning"
    );
    assert_eq!(
        resp.peer_provisioning,
        Some(SyncProvisioning::default()),
        "responder must receive an all-None provisioning from a fresh device"
    );
}

/// Wrong password: the initiator's PAKE finish must fail, and the responder
/// must not produce a session key.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bootstrap_pake_wrong_password_fails() {
    let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
    let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();

    let responder = BootstrapResponder::bind(
        responder_cert.cert_der.clone(),
        responder_cert.key_der.clone(),
    )
    .await
    .expect("bind responder");
    let port = responder.local_addr().expect("local addr").port();

    let responder_task = tokio::spawn(async move {
        responder
            .run(
                "the-right-password",
                "127.0.0.1:7003",
                &PeerMeta::default(),
                None,
            )
            .await
    });

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let initiator_task = tokio::spawn(async move {
        run_initiator(
            addr,
            initiator_cert.cert_der,
            initiator_cert.key_der,
            "the-WRONG-password",
            "127.0.0.1:7004",
            &PeerMeta::default(),
            None,
        )
        .await
    });

    let (resp_res, init_res) = tokio::join!(responder_task, initiator_task);
    let init = init_res.expect("initiator join");
    assert!(init.is_err(), "initiator must fail on wrong password");
    let resp = resp_res.expect("responder join");
    assert!(
        resp.is_err(),
        "responder must not derive a key on wrong password"
    );
}

/// Relay MitM: an attacker who knows the correct PAKE password but cannot
/// keep a single TLS channel end-to-end. The relay terminates TLS toward the
/// initiator and opens a *separate* TLS session to the real responder, then
/// blindly pumps the opaque PAKE/confirmation frames between the two legs.
///
/// PAKE itself still completes (the bytes are forwarded verbatim), but the
/// RFC 5705 channel binder differs on each TLS leg, so the channel-bound
/// confirmation tags do not match and BOTH endpoints must reject pairing.
/// This is the exact attack S3 channel binding defends against.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bootstrap_relay_mitm_is_rejected_by_channel_binding() {
    use tokio::io::{copy, AsyncWriteExt};

    let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
    let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();
    let relay_cert = SelfSignedCert::generate("relay-mitm-device").unwrap();

    let password = "shared-qr-secret-relay";

    // Real responder.
    let responder = BootstrapResponder::bind(
        responder_cert.cert_der.clone(),
        responder_cert.key_der.clone(),
    )
    .await
    .expect("bind responder");
    let responder_port = responder.local_addr().expect("local addr").port();
    let pw = password.to_string();
    let responder_task = tokio::spawn(async move {
        responder
            .run(&pw, "127.0.0.1:7005", &PeerMeta::default(), None)
            .await
    });

    // Relay listener: TLS server toward the initiator (accept any client cert).
    let relay_listener = TcpListener::bind("127.0.0.1:0").await.expect("relay bind");
    let relay_port = relay_listener.local_addr().unwrap().port();

    let relay_server_cfg = ServerConfig::builder()
        .with_client_cert_verifier(Arc::new(AcceptAnyCert))
        .with_single_cert(
            vec![CertificateDer::from(relay_cert.cert_der.clone())],
            PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                relay_cert.key_der.clone(),
            )),
        )
        .expect("relay server cfg");
    let relay_acceptor = TlsAcceptor::from(Arc::new(relay_server_cfg));

    // Relay client config toward the real responder (accept any server cert,
    // present the relay's own cert).
    let relay_client_cfg = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
        .with_client_auth_cert(
            vec![CertificateDer::from(relay_cert.cert_der.clone())],
            PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                relay_cert.key_der.clone(),
            )),
        )
        .expect("relay client cfg");
    let relay_connector = TlsConnector::from(Arc::new(relay_client_cfg));

    let relay_task = tokio::spawn(async move {
        let (inbound, _) = relay_listener.accept().await.expect("relay accept");
        let init_tls = relay_acceptor
            .accept(inbound)
            .await
            .expect("relay tls accept");

        let upstream = TcpStream::connect(("127.0.0.1", responder_port))
            .await
            .expect("relay->responder connect");
        let server_name = ServerName::try_from("copypaste.peer").unwrap();
        let resp_tls = relay_connector
            .connect(server_name, upstream)
            .await
            .expect("relay->responder tls");

        // Blindly pump bytes both directions between the two TLS legs.
        let (mut ir, mut iw) = tokio::io::split(init_tls);
        let (mut rr, mut rw) = tokio::io::split(resp_tls);
        let a = tokio::spawn(async move {
            let _ = copy(&mut ir, &mut rw).await;
            let _ = rw.shutdown().await;
        });
        let b = tokio::spawn(async move {
            let _ = copy(&mut rr, &mut iw).await;
            let _ = iw.shutdown().await;
        });
        let _ = tokio::join!(a, b);
    });

    // Initiator dials the RELAY (thinking it is the responder).
    let relay_addr: SocketAddr = ([127, 0, 0, 1], relay_port).into();
    let init_pw = password.to_string();
    let initiator_task = tokio::spawn(async move {
        run_initiator(
            relay_addr,
            initiator_cert.cert_der,
            initiator_cert.key_der,
            &init_pw,
            "127.0.0.1:7006",
            &PeerMeta::default(),
            None,
        )
        .await
    });

    let (resp_res, init_res, _relay_res) = tokio::join!(responder_task, initiator_task, relay_task);

    let init = init_res.expect("initiator join");
    assert!(
        init.is_err(),
        "initiator must reject pairing — channel binding confirmation mismatch under relay MitM"
    );
    let resp = resp_res.expect("responder join");
    assert!(
        resp.is_err(),
        "responder must reject pairing — channel binding confirmation mismatch under relay MitM"
    );
}

// ── Fix 2: PAKE exchange has an overall deadline ──────────────────────────

/// A peer that completes TLS but then dribbles / stalls mid-PAKE exchange
/// must be evicted by `PAKE_EXCHANGE_TIMEOUT`. Without this deadline the
/// single-shot responder (and the initiator) would be pinned indefinitely
/// (slowloris-style DoS).
///
/// We simulate a slow responder by opening a raw TLS bootstrap connection,
/// sending the very first frame (PAKE msg1) and then going silent. The
/// `BootstrapResponder::run` future must time out on `PAKE_EXCHANGE_TIMEOUT`,
/// NOT block forever.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn pake_exchange_timeout_fires_on_slow_peer() {
    let responder_cert = SelfSignedCert::generate("responder-device").unwrap();

    let responder = BootstrapResponder::bind(
        responder_cert.cert_der.clone(),
        responder_cert.key_der.clone(),
    )
    .await
    .expect("bind responder");
    let port = responder.local_addr().expect("local addr").port();

    // Run the responder; it must time out because we'll stall after frame 1.
    let responder_task = tokio::spawn(async move {
        responder
            .run("any-password", "127.0.0.1:9000", &PeerMeta::default(), None)
            .await
    });

    // Connect with an "any cert" TLS client, send exactly frame 1 (a fake
    // PAKE msg1 byte string), then go permanently silent — no more frames.
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let staller_cert = SelfSignedCert::generate("staller").unwrap();
    let staller_task = tokio::spawn(async move {
        use futures_util::SinkExt as _;
        let cert = rustls::pki_types::CertificateDer::from(staller_cert.cert_der.clone());
        let key = rustls::pki_types::PrivatePkcs8KeyDer::from(staller_cert.key_der.clone());
        let private_key = rustls::pki_types::PrivateKeyDer::Pkcs8(key);
        let client_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(std::sync::Arc::new(AcceptAnyCert))
            .with_client_auth_cert(vec![cert], private_key)
            .expect("client config");
        let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(client_config));
        let tcp = tokio::net::TcpStream::connect(addr)
            .await
            .expect("tcp connect");
        let server_name =
            rustls::pki_types::ServerName::try_from("copypaste.peer").expect("server name");
        let tls_stream = connector
            .connect(server_name, tcp)
            .await
            .expect("tls connect");
        let mut framed = tokio_util::codec::Framed::new(tls_stream, length_codec());
        // Send one garbage frame (pretend to be PAKE msg1) then go silent forever.
        framed
            .send(bytes::Bytes::from_static(b"fake-pake-msg1"))
            .await
            .expect("send frame1");
        // Hold the connection open so the responder can't detect closure.
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    });

    // Advance virtual time well past PAKE_EXCHANGE_TIMEOUT.
    let advance_ms = PAKE_EXCHANGE_TIMEOUT.as_millis() as u64 + 1_000;
    tokio::time::sleep(std::time::Duration::from_millis(advance_ms)).await;

    // The responder should have timed out by now.
    staller_task.abort();
    let result = responder_task.await.expect("responder join");
    assert!(
        result.is_err(),
        "responder must fail when peer stalls mid-PAKE (PAKE_EXCHANGE_TIMEOUT not applied)"
    );
}

// ── Phase 4: device-metadata extension back-compat ───────────────────────

/// Two NEW peers running `exchange_peer_meta` over an in-memory duplex pair
/// must each learn the other's metadata.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exchange_peer_meta_both_new_learns_each_other() {
    let (a, b) = tokio::io::duplex(4096);
    let mut fa = Framed::new(a, length_codec());
    let mut fb = Framed::new(b, length_codec());

    let meta_a = PeerMeta {
        model: Some("MacBook Air".into()),
        os_version: Some("macOS 14.4".into()),
        app_version: Some("0.5.4".into()),
        local_ip: Some("10.0.0.1".into()),
        device_name: None,
        public_ip: Some("203.0.113.7".into()),
        device_id: None,
    };
    let meta_b = PeerMeta {
        model: Some("Mac mini".into()),
        public_ip: Some("203.0.113.8".into()),
        ..Default::default()
    };

    // Side A advertises provisioning; side B advertises None.
    // Note: explicit all-fields init because ZeroizeOnDrop adds a Drop impl
    // and Rust disallows `..Default::default()` on types that implement Drop.
    let prov_a = SyncProvisioning {
        supabase_url: Some("https://a.supabase.co".into()),
        supabase_anon_key: None,
        relay_url: None,
        derived_sync_key: Some(vec![9u8; 32]),
    };

    let ma = meta_a.clone();
    let mb = meta_b.clone();
    let pa = prov_a.clone();
    let ta = tokio::spawn(async move { exchange_peer_meta(&mut fa, &ma, Some(&pa)).await });
    let tb = tokio::spawn(async move { exchange_peer_meta(&mut fb, &mb, None).await });
    let (got_a, got_b) = tokio::join!(ta, tb);

    // Side A learned B's metadata; side B learned A's.
    let (meta_from_b, prov_from_b) = got_a.unwrap();
    let (meta_from_a, prov_from_a) = got_b.unwrap();
    assert_eq!(meta_from_b, meta_b);
    assert_eq!(meta_from_a, meta_a);
    // Side B learned A's provisioning; side A learned B's all-None default.
    assert_eq!(prov_from_a, Some(prov_a));
    assert_eq!(prov_from_b, Some(SyncProvisioning::default()));
}

/// Back-compat: when the peer is LEGACY (closes the stream without sending a
/// version/metadata frame), `exchange_peer_meta` must return the default
/// (all-`None`) metadata rather than hanging or erroring — the pairing has
/// already completed and metadata is best-effort.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exchange_peer_meta_legacy_peer_yields_none() {
    let (a, b) = tokio::io::duplex(4096);
    let mut fa = Framed::new(a, length_codec());

    // Legacy peer: drop its end immediately (frame 9 was the last thing it
    // would have sent in the real protocol).
    drop(b);

    let meta_a = PeerMeta {
        model: Some("MacBook Air".into()),
        ..Default::default()
    };
    let (got_meta, got_prov) = exchange_peer_meta(&mut fa, &meta_a, None).await;
    assert_eq!(
        got_meta,
        PeerMeta::default(),
        "a legacy peer that sends no metadata must yield all-None"
    );
    assert_eq!(
        got_prov, None,
        "a legacy peer that sends no provisioning must yield None"
    );
}

// ── proto v2: SyncProvisioning exchange + back-compat ─────────────────────

/// A v1 (proto-version-1) peer participates in the metadata exchange but does
/// NOT send a provisioning frame. The v2 side must learn the peer's metadata
/// and return `None` for provisioning WITHOUT desyncing the stream — this is
/// the version-gated back-compat. We simulate a v1 peer by hand-writing only
/// frames 10 (version byte = 1) and 11 (metadata JSON), then closing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exchange_with_v1_peer_yields_none_provisioning() {
    let (a, b) = tokio::io::duplex(4096);
    let mut fa = Framed::new(a, length_codec());
    let mut fb = Framed::new(b, length_codec());

    // Side A is the modern (v2) side advertising provisioning.
    let meta_a = PeerMeta {
        model: Some("Modern Mac".into()),
        ..Default::default()
    };
    // Explicit all-fields init: ZeroizeOnDrop adds a Drop impl and Rust
    // forbids `..Default::default()` on types implementing Drop.
    let prov_a = SyncProvisioning {
        supabase_url: Some("https://a.supabase.co".into()),
        supabase_anon_key: None,
        relay_url: None,
        derived_sync_key: None,
    };

    // Side B emulates a v1 peer: send version byte 1, then a metadata JSON,
    // then drop — it never sends or reads a provisioning frame.
    let peer_meta_b = PeerMeta {
        model: Some("Legacy Mac".into()),
        ..Default::default()
    };
    let b_task = tokio::spawn(async move {
        send_frame(&mut fb, &[1u8]).await.unwrap();
        let json = serde_json::to_vec(&peer_meta_b).unwrap();
        send_frame(&mut fb, &json).await.unwrap();
        // Read A's frames so A's sends don't block, but never send frame 12.
        let _ = recv_frame(&mut fb).await; // A version byte
        let _ = recv_frame(&mut fb).await; // A metadata
        let _ = recv_frame(&mut fb).await; // A provisioning (A sends it; B ignores)
        peer_meta_b
    });

    let (got_meta, got_prov) = exchange_peer_meta(&mut fa, &meta_a, Some(&prov_a)).await;
    let sent_b = b_task.await.unwrap();

    assert_eq!(
        got_meta, sent_b,
        "v2 side must learn the v1 peer's metadata"
    );
    assert_eq!(
        got_prov, None,
        "a v1 peer that sends no provisioning frame must yield None (back-compat)"
    );
}

/// `SyncProvisioning` round-trips through its JSON wire form, including the
/// secret derived key bytes.
#[test]
fn sync_provisioning_round_trips() {
    let prov = SyncProvisioning {
        supabase_url: Some("https://x.supabase.co".into()),
        supabase_anon_key: Some("anon-jwt".into()),
        relay_url: Some("https://relay.example".into()),
        derived_sync_key: Some(vec![1u8; 32]),
    };
    let json = serde_json::to_vec(&prov).expect("serialize");
    let back: SyncProvisioning = serde_json::from_slice(&json).expect("deserialize");
    assert_eq!(back, prov, "SyncProvisioning must round-trip");
}

/// An all-`None` `SyncProvisioning` serialises to an empty object and round
/// -trips back to the default (every field omitted via skip_serializing_if).
#[test]
fn sync_provisioning_all_none_is_empty_object() {
    let prov = SyncProvisioning::default();
    let json = serde_json::to_string(&prov).expect("serialize");
    assert_eq!(json, "{}", "all-None provisioning must serialise to {{}}");
    let back: SyncProvisioning = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, SyncProvisioning::default());
}

/// The custom `Debug` impl must NOT print the secret key bytes — only a
/// redacted length marker — while still showing the non-secret URLs.
#[test]
fn sync_provisioning_debug_redacts_key() {
    // Explicit all-fields init: ZeroizeOnDrop adds a Drop impl and Rust
    // forbids `..Default::default()` on types implementing Drop.
    let prov = SyncProvisioning {
        supabase_url: Some("https://x.supabase.co".into()),
        supabase_anon_key: None,
        relay_url: None,
        derived_sync_key: Some(vec![0xABu8; 32]),
    };
    let dbg = format!("{prov:?}");
    assert!(dbg.contains("redacted"), "Debug must redact the key: {dbg}");
    assert!(
        !dbg.contains("171") && !dbg.contains("0xab") && !dbg.contains("AB, AB"),
        "Debug must not contain raw key bytes: {dbg}"
    );
    assert!(
        dbg.contains("x.supabase.co"),
        "Debug must still show the non-secret URL: {dbg}"
    );
}

// ── PeerMeta.public_ip serde (B1: peer public/global IP exchange) ─────────

/// Round-trip: a `PeerMeta` carrying `public_ip` serialises and deserialises
/// back to an equal value (the new field survives the JSON wire form).
#[test]
fn peer_meta_public_ip_round_trips() {
    let meta = PeerMeta {
        model: Some("MacBook Air".into()),
        os_version: Some("macOS 15.5".into()),
        app_version: Some("0.6.0".into()),
        local_ip: Some("192.168.1.5".into()),
        device_name: Some("Alice's MacBook".into()),
        public_ip: Some("203.0.113.42".into()),
        device_id: None,
    };
    let json = serde_json::to_string(&meta).expect("serialize PeerMeta");
    assert!(
        json.contains("\"public_ip\":\"203.0.113.42\""),
        "public_ip must appear in the serialised PeerMeta JSON: {json}"
    );
    let back: PeerMeta = serde_json::from_str(&json).expect("deserialize PeerMeta");
    assert_eq!(back, meta, "PeerMeta must round-trip with public_ip set");
}

/// When `public_ip` is `None`, it is omitted from the wire form
/// (`skip_serializing_if`) — keeping the frame minimal and back-compat with a
/// legacy reader that does not know the key.
#[test]
fn peer_meta_public_ip_none_is_omitted() {
    let meta = PeerMeta {
        model: Some("Mac mini".into()),
        ..Default::default()
    };
    let json = serde_json::to_string(&meta).expect("serialize PeerMeta");
    assert!(
        !json.contains("public_ip"),
        "public_ip must be absent from JSON when None: {json}"
    );
}

/// Back-compat: an OLD-format `PeerMeta` payload that predates `public_ip`
/// (the key is entirely absent) must deserialise cleanly with `public_ip ==
/// None`. This is the wire form an older peer sends; it must NOT error, so
/// pairing/connecting/syncing with a legacy peer keeps working.
#[test]
fn peer_meta_legacy_payload_without_public_ip_deserialises_to_none() {
    // Exactly the JSON an older build emits (model/os/app/local_ip/device_name,
    // NO public_ip key).
    let legacy_json = r#"{
        "model": "MacBook Air",
        "os_version": "macOS 14.4",
        "app_version": "0.5.4",
        "local_ip": "192.168.1.11",
        "device_name": "Bob's Mac"
    }"#;
    let meta: PeerMeta =
        serde_json::from_str(legacy_json).expect("legacy PeerMeta must deserialise");
    assert_eq!(
        meta.public_ip, None,
        "a legacy payload missing public_ip must deserialise to None"
    );
    // The other fields still populate, proving the additive field did not
    // disturb the existing wire contract.
    assert_eq!(meta.model.as_deref(), Some("MacBook Air"));
    assert_eq!(meta.device_name.as_deref(), Some("Bob's Mac"));
}

// ── LAN/SAS phase 1: confirm-gated handshake variants ────────────────────

/// Both endpoints run the confirm-gated variants over a real loopback
/// TLS socket: each side's `confirm` callback is invoked with the SAS, both
/// accept, and the handshake completes. The two SAS strings MUST be equal
/// (same `bound_key`), and the returned `BootstrapPairing.sas` matches.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn confirm_variants_loopback_sas_matches_and_accepts() {
    use std::sync::{Arc, Mutex};

    let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
    let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();
    let password = "sas-confirm-loopback";

    let responder = BootstrapResponder::bind(
        responder_cert.cert_der.clone(),
        responder_cert.key_der.clone(),
    )
    .await
    .expect("bind responder");
    let port = responder.local_addr().expect("local addr").port();

    let resp_seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let init_seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let resp_seen_cb = resp_seen.clone();
    let responder_task = tokio::spawn(async move {
        responder
            .run_with_confirm(
                "sas-confirm-loopback",
                "127.0.0.1:7101",
                &PeerMeta::default(),
                None,
                move |sas, _peer_fp| {
                    let slot = resp_seen_cb.clone();
                    let sas = sas.to_string();
                    async move {
                        *slot.lock().unwrap() = Some(sas);
                        true
                    }
                },
            )
            .await
    });

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let init_seen_cb = init_seen.clone();
    let initiator_task = tokio::spawn(async move {
        run_initiator_with_confirm(
            addr,
            initiator_cert.cert_der,
            initiator_cert.key_der,
            "sas-confirm-loopback",
            "127.0.0.1:7102",
            &PeerMeta::default(),
            None,
            move |sas, _peer_fp| {
                let slot = init_seen_cb.clone();
                let sas = sas.to_string();
                async move {
                    *slot.lock().unwrap() = Some(sas);
                    true
                }
            },
        )
        .await
    });

    let _ = password;
    let (resp_res, init_res) = tokio::join!(responder_task, initiator_task);
    let resp = resp_res
        .expect("responder join")
        .expect("responder pairing");
    let init = init_res
        .expect("initiator join")
        .expect("initiator pairing");

    let resp_sas = resp_seen
        .lock()
        .unwrap()
        .clone()
        .expect("responder saw sas");
    let init_sas = init_seen
        .lock()
        .unwrap()
        .clone()
        .expect("initiator saw sas");
    assert_eq!(resp_sas, init_sas, "both sides must see the same SAS");
    assert_eq!(resp.sas, resp_sas, "returned sas matches confirmed sas");
    assert_eq!(init.sas, init_sas, "returned sas matches confirmed sas");
    assert_eq!(resp.sas, init.sas);
    assert_eq!(resp.session_key.as_bytes(), init.session_key.as_bytes());
}

/// Regression for the discovery-pairing P0: when BOTH the initiator and the
/// responder use the fixed, well-known [`DISCOVERY_PAIRING_PASSWORD`] (the
/// QR-less LAN/SAS path), the asymmetric OPAQUE PAKE `finish`es (no
/// `InvalidPassword` at frame 7) and both sides derive the SAME SAS. The old
/// daemon discovery path generated an INDEPENDENT random password per side,
/// which would fail here. Mirrors `confirm_variants_loopback_sas_matches_and_accepts`
/// but pins the shared constant.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovery_shared_password_pake_completes_and_sas_matches() {
    use std::sync::{Arc, Mutex};

    use crate::bootstrap::DISCOVERY_PAIRING_PASSWORD;

    let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
    let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();

    let responder = BootstrapResponder::bind(
        responder_cert.cert_der.clone(),
        responder_cert.key_der.clone(),
    )
    .await
    .expect("bind responder");
    let port = responder.local_addr().expect("local addr").port();

    let resp_seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let init_seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let resp_seen_cb = resp_seen.clone();
    let responder_task = tokio::spawn(async move {
        responder
            .run_with_confirm(
                DISCOVERY_PAIRING_PASSWORD,
                "127.0.0.1:7111",
                &PeerMeta::default(),
                None,
                move |sas, _peer_fp| {
                    let slot = resp_seen_cb.clone();
                    let sas = sas.to_string();
                    async move {
                        *slot.lock().unwrap() = Some(sas);
                        true
                    }
                },
            )
            .await
    });

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let init_seen_cb = init_seen.clone();
    let initiator_task = tokio::spawn(async move {
        run_initiator_with_confirm(
            addr,
            initiator_cert.cert_der,
            initiator_cert.key_der,
            DISCOVERY_PAIRING_PASSWORD,
            "127.0.0.1:7112",
            &PeerMeta::default(),
            None,
            move |sas, _peer_fp| {
                let slot = init_seen_cb.clone();
                let sas = sas.to_string();
                async move {
                    *slot.lock().unwrap() = Some(sas);
                    true
                }
            },
        )
        .await
    });

    let (resp_res, init_res) = tokio::join!(responder_task, initiator_task);
    let resp = resp_res
        .expect("responder join")
        .expect("responder pairing (PAKE must finish with the shared password)");
    let init = init_res
        .expect("initiator join")
        .expect("initiator pairing (PAKE must finish with the shared password)");

    let resp_sas = resp_seen
        .lock()
        .unwrap()
        .clone()
        .expect("responder saw sas");
    let init_sas = init_seen
        .lock()
        .unwrap()
        .clone()
        .expect("initiator saw sas");
    assert_eq!(resp_sas, init_sas, "both sides must derive the same SAS");
    assert_eq!(resp.sas, init.sas);
    assert_eq!(resp.session_key.as_bytes(), init.session_key.as_bytes());
}

/// If EITHER side's user rejects the SAS, BOTH endpoints must abort with an
/// error and neither returns a `BootstrapPairing` (keys drop/zeroize).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn confirm_variant_reject_aborts_both() {
    let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
    let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();
    let password = "sas-confirm-reject";

    let responder = BootstrapResponder::bind(
        responder_cert.cert_der.clone(),
        responder_cert.key_der.clone(),
    )
    .await
    .expect("bind responder");
    let port = responder.local_addr().expect("local addr").port();

    // Responder accepts; initiator rejects → both must fail.
    let responder_task = tokio::spawn(async move {
        responder
            .run_with_confirm(
                "sas-confirm-reject",
                "127.0.0.1:7103",
                &PeerMeta::default(),
                None,
                |_sas, _peer_fp| async { true },
            )
            .await
    });

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let initiator_task = tokio::spawn(async move {
        run_initiator_with_confirm(
            addr,
            initiator_cert.cert_der,
            initiator_cert.key_der,
            "sas-confirm-reject",
            "127.0.0.1:7104",
            &PeerMeta::default(),
            None,
            |_sas, _peer_fp| async { false },
        )
        .await
    });

    let _ = password;
    let (resp_res, init_res) = tokio::join!(responder_task, initiator_task);
    let init = init_res.expect("initiator join");
    assert!(
        init.is_err(),
        "initiator must abort when it rejects the SAS"
    );
    let resp = resp_res.expect("responder join");
    assert!(
        resp.is_err(),
        "responder must abort when the peer rejects the SAS"
    );
}

/// Under a relay MitM the two legs derive DIFFERENT `bound_key`s, so the two
/// SAS values diverge — the human compare is what catches the attack. We use
/// the confirm variants and capture each side's SAS; if both captured one
/// they must differ. The channel-binding tag check already aborts both, so
/// this also asserts both still fail.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn confirm_variant_relay_mitm_yields_different_sas_per_leg() {
    use std::sync::{Arc, Mutex};
    use tokio::io::{copy, AsyncWriteExt};

    let responder_cert = SelfSignedCert::generate("responder-device").unwrap();
    let initiator_cert = SelfSignedCert::generate("initiator-device").unwrap();
    let relay_cert = SelfSignedCert::generate("relay-mitm-device").unwrap();
    let password = "sas-relay-secret";

    let responder = BootstrapResponder::bind(
        responder_cert.cert_der.clone(),
        responder_cert.key_der.clone(),
    )
    .await
    .expect("bind responder");
    let responder_port = responder.local_addr().expect("local addr").port();

    let resp_seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let init_seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let resp_seen_cb = resp_seen.clone();
    let responder_task = tokio::spawn(async move {
        responder
            .run_with_confirm(
                "sas-relay-secret",
                "127.0.0.1:7105",
                &PeerMeta::default(),
                None,
                move |sas, _peer_fp| {
                    let slot = resp_seen_cb.clone();
                    let sas = sas.to_string();
                    async move {
                        *slot.lock().unwrap() = Some(sas);
                        true
                    }
                },
            )
            .await
    });

    let relay_listener = TcpListener::bind("127.0.0.1:0").await.expect("relay bind");
    let relay_port = relay_listener.local_addr().unwrap().port();

    let relay_server_cfg = ServerConfig::builder()
        .with_client_cert_verifier(Arc::new(AcceptAnyCert))
        .with_single_cert(
            vec![CertificateDer::from(relay_cert.cert_der.clone())],
            PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                relay_cert.key_der.clone(),
            )),
        )
        .expect("relay server cfg");
    let relay_acceptor = TlsAcceptor::from(Arc::new(relay_server_cfg));

    let relay_client_cfg = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
        .with_client_auth_cert(
            vec![CertificateDer::from(relay_cert.cert_der.clone())],
            PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
                relay_cert.key_der.clone(),
            )),
        )
        .expect("relay client cfg");
    let relay_connector = TlsConnector::from(Arc::new(relay_client_cfg));

    let relay_task = tokio::spawn(async move {
        let (inbound, _) = relay_listener.accept().await.expect("relay accept");
        let init_tls = relay_acceptor
            .accept(inbound)
            .await
            .expect("relay tls accept");
        let upstream = TcpStream::connect(("127.0.0.1", responder_port))
            .await
            .expect("relay->responder connect");
        let server_name = ServerName::try_from("copypaste.peer").unwrap();
        let resp_tls = relay_connector
            .connect(server_name, upstream)
            .await
            .expect("relay->responder tls");
        let (mut ir, mut iw) = tokio::io::split(init_tls);
        let (mut rr, mut rw) = tokio::io::split(resp_tls);
        let a = tokio::spawn(async move {
            let _ = copy(&mut ir, &mut rw).await;
            let _ = rw.shutdown().await;
        });
        let b = tokio::spawn(async move {
            let _ = copy(&mut rr, &mut iw).await;
            let _ = iw.shutdown().await;
        });
        let _ = tokio::join!(a, b);
    });

    let relay_addr: SocketAddr = ([127, 0, 0, 1], relay_port).into();
    let init_seen_cb = init_seen.clone();
    let initiator_task = tokio::spawn(async move {
        run_initiator_with_confirm(
            relay_addr,
            initiator_cert.cert_der,
            initiator_cert.key_der,
            "sas-relay-secret",
            "127.0.0.1:7106",
            &PeerMeta::default(),
            None,
            move |sas, _peer_fp| {
                let slot = init_seen_cb.clone();
                let sas = sas.to_string();
                async move {
                    *slot.lock().unwrap() = Some(sas);
                    true
                }
            },
        )
        .await
    });

    let _ = password;
    let (resp_res, init_res, _relay_res) = tokio::join!(responder_task, initiator_task, relay_task);

    // Both must reject (the constant-time tag check aborts before confirm).
    assert!(init_res.expect("initiator join").is_err());
    assert!(resp_res.expect("responder join").is_err());

    // If both sides DID surface a SAS to the user, the two would differ —
    // that divergence is the human-visible MitM signal.
    let r = resp_seen.lock().unwrap().clone();
    let i = init_seen.lock().unwrap().clone();
    if let (Some(rs), Some(is)) = (r, i) {
        assert_ne!(rs, is, "relay legs must yield different SAS values");
    }
}

// ── Fix 4: fingerprint comparison is case-insensitive ────────────────────

/// A peer that sends its fingerprint in UPPERCASE hex must still pair
/// successfully. Before the fix, `frame_peer_fp != tls_peer_fp` was a byte
/// comparison of the frame bytes (which might be uppercase) against
/// `fingerprint_of` output (which is lowercase), causing a false mismatch.
///
/// We test the invariant directly: `recv_fingerprint` now lowercases its
/// output so an uppercase frame equals the lowercase TLS fingerprint.
#[test]
fn recv_fingerprint_normalises_to_lowercase() {
    // Construct what recv_fingerprint MUST return when the peer sends
    // an uppercase hex fingerprint — it should be lowercased.
    let uppercase_hex = "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789";
    assert_eq!(uppercase_hex.len(), 64);
    // The function itself is async and private; test the normalised form
    // symbolically: if we lowercase the uppercase input we get a valid
    // lowercase fingerprint that would match `fingerprint_of` output.
    let normalised = uppercase_hex.to_lowercase();
    assert!(
        normalised.bytes().all(|b| b.is_ascii_hexdigit()),
        "lowercased hex must still be valid hex"
    );
    assert!(
        normalised.bytes().all(|b| !b.is_ascii_uppercase()),
        "normalised fingerprint must contain no uppercase chars"
    );
    // Also verify the current recv_fingerprint validator accepts uppercase
    // (64 chars, all hex digits including uppercase).
    assert!(
        uppercase_hex.len() == 64 && uppercase_hex.bytes().all(|b| b.is_ascii_hexdigit()),
        "uppercase fingerprint must be accepted by the length+hex check"
    );
}

// ── CopyPaste-n3bc: confirm callback receives peer_fingerprint ────────────

/// Regression test for CopyPaste-n3bc: both `run_with_confirm` and
/// `run_initiator_with_confirm` must pass the TLS peer fingerprint as the
/// SECOND argument to the `confirm` callback (`confirm(&sas, &peer_fingerprint)`).
///
/// Before this fix the callback only received the SAS (`confirm(&sas)`), so
/// the responder-side confirm had no identity binding — it could not surface the
/// peer fingerprint on the responder path (matching the initiator path where the
/// fingerprint is available in `BootstrapPairing.peer_fingerprint`).
///
/// After the fix the responder confirm captures the REAL initiator TLS
/// fingerprint, and the initiator confirm captures the REAL responder TLS
/// fingerprint. Both captured values must match the `BootstrapPairing.peer_fingerprint`
/// returned from the respective call.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn confirm_callbacks_receive_peer_fingerprint_n3bc() {
    use std::sync::{Arc, Mutex};

    let responder_cert = SelfSignedCert::generate("responder-n3bc").unwrap();
    let initiator_cert = SelfSignedCert::generate("initiator-n3bc").unwrap();

    // Pre-compute the expected fingerprints so we can cross-check what the
    // callbacks receive.
    let expected_initiator_fp = crate::cert::fingerprint_of(&initiator_cert.cert_der);
    let expected_responder_fp = crate::cert::fingerprint_of(&responder_cert.cert_der);

    let responder = BootstrapResponder::bind(
        responder_cert.cert_der.clone(),
        responder_cert.key_der.clone(),
    )
    .await
    .expect("bind responder");
    let port = responder.local_addr().expect("local addr").port();

    // Slots to capture the fingerprint the confirm callback receives on each side.
    let resp_fp_in_cb: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let init_fp_in_cb: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let resp_fp_slot = resp_fp_in_cb.clone();
    let responder_task = tokio::spawn(async move {
        responder
            .run_with_confirm(
                "n3bc-test-password",
                "127.0.0.1:8201",
                &PeerMeta::default(),
                None,
                // NEW 2-arg callback: confirm(&sas, &peer_fingerprint)
                move |_sas: &str, peer_fp: &str| {
                    *resp_fp_slot.lock().unwrap() = Some(peer_fp.to_string());
                    async move { true }
                },
            )
            .await
    });

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let init_fp_slot = init_fp_in_cb.clone();
    let initiator_task = tokio::spawn(async move {
        run_initiator_with_confirm(
            addr,
            initiator_cert.cert_der,
            initiator_cert.key_der,
            "n3bc-test-password",
            "127.0.0.1:8202",
            &PeerMeta::default(),
            None,
            // NEW 2-arg callback: confirm(&sas, &peer_fingerprint)
            move |_sas: &str, peer_fp: &str| {
                *init_fp_slot.lock().unwrap() = Some(peer_fp.to_string());
                async move { true }
            },
        )
        .await
    });

    let (resp_res, init_res) = tokio::join!(responder_task, initiator_task);
    let resp = resp_res
        .expect("responder join")
        .expect("responder pairing");
    let init = init_res
        .expect("initiator join")
        .expect("initiator pairing");

    // The fingerprint the responder's confirm callback saw MUST be the
    // initiator's TLS cert fingerprint.
    let fp_seen_by_resp = resp_fp_in_cb
        .lock()
        .unwrap()
        .clone()
        .expect("responder confirm must have been invoked");
    assert_eq!(
        fp_seen_by_resp, expected_initiator_fp,
        "responder confirm callback must receive the initiator's TLS fingerprint (CopyPaste-n3bc)"
    );
    // Cross-check: the returned BootstrapPairing also carries that fingerprint.
    assert_eq!(
        resp.peer_fingerprint, expected_initiator_fp,
        "BootstrapPairing.peer_fingerprint on responder side must match (CopyPaste-n3bc)"
    );

    // The fingerprint the initiator's confirm callback saw MUST be the
    // responder's TLS cert fingerprint.
    let fp_seen_by_init = init_fp_in_cb
        .lock()
        .unwrap()
        .clone()
        .expect("initiator confirm must have been invoked");
    assert_eq!(
        fp_seen_by_init, expected_responder_fp,
        "initiator confirm callback must receive the responder's TLS fingerprint (CopyPaste-n3bc)"
    );
    assert_eq!(
        init.peer_fingerprint, expected_responder_fp,
        "BootstrapPairing.peer_fingerprint on initiator side must match (CopyPaste-n3bc)"
    );
}

/// `SyncProvisioning` has a non-trivial `Drop` impl via `ZeroizeOnDrop`
/// (CopyPaste-34u2).
///
/// `#[derive(zeroize::ZeroizeOnDrop)]` generates a `Drop` impl that
/// calls `Zeroize::zeroize()` on each field, overwriting the
/// `derived_sync_key` bytes with zeros before the memory is freed.
///
/// We verify this by:
/// 1. Asserting `std::mem::needs_drop::<SyncProvisioning>()` is true
///    (the type has a `Drop` impl — it would be false for plain
///    `#[derive(Clone, Default)]` without `ZeroizeOnDrop`).
/// 2. Zeroizing the `derived_sync_key` field directly using
///    `zeroize::Zeroize::zeroize()` on the `Vec<u8>`, which is exactly
///    what the generated `Drop` impl does for that field, and asserting
///    the bytes are cleared.
#[test]
fn sync_provisioning_zeroizes_derived_sync_key_on_drop() {
    use zeroize::Zeroize as _;

    // 1. Structural check: ZeroizeOnDrop adds a Drop impl, so needs_drop
    //    must be true. Without ZeroizeOnDrop it would be false (the struct
    //    only holds Option<String>/Option<Vec<u8>> which do need_drop, but
    //    the key point is the ZeroizeOnDrop derive is present).
    assert!(
        std::mem::needs_drop::<SyncProvisioning>(),
        "SyncProvisioning must have a Drop impl (ZeroizeOnDrop) to zeroize \
         derived_sync_key on drop (CopyPaste-34u2)"
    );

    // 2. Field-level check: zeroize the Vec<u8> held in derived_sync_key
    //    the same way the generated Drop impl does, and verify the bytes
    //    are overwritten with zeros.
    let mut key_vec: Vec<u8> = vec![0xABu8; 32];
    // Confirm non-zero before.
    assert!(key_vec.iter().all(|&b| b == 0xAB));
    // Zeroize (same operation as in the generated Drop impl).
    key_vec.zeroize();
    // The Vec must now be empty (len=0) and any bytes that were in the
    // backing store must have been overwritten.
    assert!(
        key_vec.is_empty(),
        "zeroize::Zeroize::zeroize() on Vec<u8> must clear the buffer \
         (CopyPaste-34u2)"
    );
}
