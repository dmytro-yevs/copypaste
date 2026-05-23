//! Integration tests for mutual-TLS peer-to-peer transport.
//!
//! These tests exercise the public `PeerTransport` API end-to-end over loopback
//! TCP. The transport pins identity via SHA-256 fingerprint of the certificate
//! DER (see `cert::fingerprint_of`), so verification logic is exercised through
//! real TLS handshakes — not via mocks.
//!
//! Test matrix (beta-bonus):
//!   * mtls_handshake_with_matching_certs_succeeds — happy path
//!   * reject_client_with_untrusted_cert            — server rejects unknown client
//!   * reject_server_with_untrusted_cert            — client rejects unknown server
//!   * cert_rotation_old_cert_after_rotation_fails  — rotated server, old client cert no good
//!   * fingerprint_pinning_matches                  — SHA-256(DER) pin matches local hash

use std::net::SocketAddr;

use copypaste_p2p::{fingerprint_of, PairedPeers, PeerTransport, SelfSignedCert, TransportError};
use tokio::net::TcpListener;

/// Bind a loopback listener on an ephemeral port.
async fn bind_loopback() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let addr = listener.local_addr().expect("local_addr");
    (listener, addr)
}

/// Build a `PeerTransport` from a freshly-generated cert and the given peer set.
fn transport_with_cert(cert: &SelfSignedCert, peers: PairedPeers) -> PeerTransport {
    PeerTransport::from_cert(cert.cert_der.clone(), cert.key_der.clone(), peers)
}

// ----------------------------------------------------------------------------
// 1. Happy path — matching certs on both sides, handshake completes.
// ----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mtls_handshake_with_matching_certs_succeeds() {
    let server_cert = SelfSignedCert::generate("server").unwrap();
    let client_cert = SelfSignedCert::generate("client").unwrap();

    let server_fp = server_cert.fingerprint();
    let client_fp = client_cert.fingerprint();

    // Mutual trust: each side knows the other's fingerprint.
    let mut server_peers = PairedPeers::new();
    server_peers.add(client_fp.clone(), "client");
    let mut client_peers = PairedPeers::new();
    client_peers.add(server_fp.clone(), "server");

    let server = transport_with_cert(&server_cert, server_peers);
    let client = transport_with_cert(&client_cert, client_peers);

    let (listener, addr) = bind_loopback().await;

    let (srv_res, cli_res) = tokio::join!(server.accept(&listener), client.connect(addr, &server_fp));

    let (peer_addr, _srv_stream) = srv_res.expect("server accept succeeds");
    let _cli_stream = cli_res.expect("client connect succeeds");
    assert_eq!(peer_addr.ip().to_string(), "127.0.0.1");
}

// ----------------------------------------------------------------------------
// 2. Server rejects a client whose cert fingerprint is not paired.
// ----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reject_client_with_untrusted_cert() {
    let server_cert = SelfSignedCert::generate("server").unwrap();
    let stranger_cert = SelfSignedCert::generate("stranger").unwrap();

    let server_fp = server_cert.fingerprint();

    // Server has no paired peers — stranger is not allow-listed.
    let server_peers = PairedPeers::new();

    // Stranger blindly trusts the server (to make connect() proceed to the
    // point where the *server* gets to reject it).
    let mut stranger_peers = PairedPeers::new();
    stranger_peers.add(server_fp.clone(), "server");

    let server = transport_with_cert(&server_cert, server_peers);
    let stranger = transport_with_cert(&stranger_cert, stranger_peers);

    let (listener, addr) = bind_loopback().await;

    let (srv_res, _cli_res) =
        tokio::join!(server.accept(&listener), stranger.connect(addr, &server_fp));

    let err = srv_res.expect_err("server must reject untrusted client cert");
    // Must be a TLS-layer failure (TlsConfig wraps rustls::Error, Io covers the
    // handshake-driven I/O reset). It must NOT be a panic and it must NOT be
    // `UnknownPeer` — the verifier in the handshake fires before our manual
    // fingerprint check.
    assert!(
        matches!(err, TransportError::Io(_) | TransportError::TlsConfig(_)),
        "expected TLS/IO error rejecting client, got {err:?}"
    );
}

// ----------------------------------------------------------------------------
// 3. Client rejects a server whose cert fingerprint is not paired.
// ----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reject_server_with_untrusted_cert() {
    let server_cert = SelfSignedCert::generate("server").unwrap();
    let client_cert = SelfSignedCert::generate("client").unwrap();

    let server_fp = server_cert.fingerprint();
    let client_fp = client_cert.fingerprint();

    // Server happens to know the client (so it would accept), but the client
    // has NOT paired with this server's fingerprint.
    let mut server_peers = PairedPeers::new();
    server_peers.add(client_fp.clone(), "client");
    let client_peers = PairedPeers::new(); // client trusts nobody

    let server = transport_with_cert(&server_cert, server_peers);
    let client = transport_with_cert(&client_cert, client_peers);

    let (listener, addr) = bind_loopback().await;

    let (_srv_res, cli_res) =
        tokio::join!(server.accept(&listener), client.connect(addr, &server_fp));

    let err = cli_res.expect_err("client must reject untrusted server cert");
    assert!(
        matches!(err, TransportError::Io(_) | TransportError::TlsConfig(_)),
        "expected TLS/IO error rejecting server, got {err:?}"
    );
}

// ----------------------------------------------------------------------------
// 4. Cert rotation — after the server rotates its cert, a client still pinned
//    to the OLD server fingerprint must fail to connect.
// ----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cert_rotation_old_cert_after_rotation_fails() {
    // Original server cert — known to the client.
    let old_server_cert = SelfSignedCert::generate("server").unwrap();
    let old_server_fp = old_server_cert.fingerprint();

    // Rotated server cert — same "device", brand new key material.
    let new_server_cert = SelfSignedCert::generate("server").unwrap();
    let new_server_fp = new_server_cert.fingerprint();
    assert_ne!(
        old_server_fp, new_server_fp,
        "rotation must produce a different fingerprint"
    );

    let client_cert = SelfSignedCert::generate("client").unwrap();
    let client_fp = client_cert.fingerprint();

    // Server (post-rotation) trusts the client.
    let mut server_peers = PairedPeers::new();
    server_peers.add(client_fp.clone(), "client");

    // Client still has only the OLD fingerprint in its paired list and pins
    // the OLD fingerprint during connect.
    let mut client_peers = PairedPeers::new();
    client_peers.add(old_server_fp.clone(), "server");

    let server_after_rotation = transport_with_cert(&new_server_cert, server_peers);
    let client = transport_with_cert(&client_cert, client_peers);

    let (listener, addr) = bind_loopback().await;

    let (_srv_res, cli_res) = tokio::join!(
        server_after_rotation.accept(&listener),
        // Client pins the OLD fingerprint — server now presents NEW cert →
        // mismatch must trip the verifier.
        client.connect(addr, &old_server_fp),
    );

    let err = cli_res.expect_err("client must reject rotated server cert (pin mismatch)");
    assert!(
        matches!(err, TransportError::Io(_) | TransportError::TlsConfig(_)),
        "expected TLS/IO error on rotated cert, got {err:?}"
    );
}

// ----------------------------------------------------------------------------
// 5. Fingerprint pinning — pin matches SHA-256(DER) computed locally and is
//    stable across `SelfSignedCert::fingerprint()` and `fingerprint_of(&DER)`.
// ----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fingerprint_pinning_matches() {
    let cert = SelfSignedCert::generate("pin-target").unwrap();
    let fp_from_helper = cert.fingerprint();
    let fp_from_der = fingerprint_of(&cert.cert_der);

    assert_eq!(fp_from_helper, fp_from_der, "two fingerprint paths must agree");
    assert_eq!(fp_from_helper.len(), 64, "SHA-256 hex must be 64 chars");
    assert!(
        fp_from_helper.chars().all(|c| c.is_ascii_hexdigit()),
        "fingerprint must be lowercase hex"
    );
    assert!(
        fp_from_helper.chars().all(|c| !c.is_ascii_uppercase()),
        "fingerprint must be lowercase (got {fp_from_helper})"
    );

    // Pinning end-to-end: a client that pins the exact fingerprint succeeds.
    let client_cert = SelfSignedCert::generate("client").unwrap();
    let client_fp = client_cert.fingerprint();

    let mut server_peers = PairedPeers::new();
    server_peers.add(client_fp.clone(), "client");
    let mut client_peers = PairedPeers::new();
    client_peers.add(fp_from_helper.clone(), "pin-target");

    let server = transport_with_cert(&cert, server_peers);
    let client = transport_with_cert(&client_cert, client_peers);

    let (listener, addr) = bind_loopback().await;

    let (srv_res, cli_res) = tokio::join!(
        server.accept(&listener),
        client.connect(addr, &fp_from_helper)
    );

    srv_res.expect("server accept with matching pin");
    cli_res.expect("client connect with matching pin");
}
