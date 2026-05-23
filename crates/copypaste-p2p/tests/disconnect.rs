//! Integration tests for P2P transport resilience under disconnects.
//!
//! These tests exercise real `PeerTransport` behaviour over loopback TCP when
//! the remote side disappears at various points in the connection lifecycle.
//!
//! Test matrix (beta-bonus):
//! * peer_drops_mid_transfer_sender_gets_io_error_not_panic —
//!   server side is dropped while client is writing 1MB; client must
//!   observe an `Err` (no panic, no infinite hang).
//! * reconnect_after_disconnect_succeeds —
//!   after a clean drop on both sides, a fresh `connect()` against a
//!   freshly-bound listener completes a new mutual-TLS handshake.
//! * multiple_concurrent_streams_one_drop_doesnt_affect_others —
//!   three independent streams are established; one is dropped; the
//!   other two must still be writable.
//! * peer_offline_during_handshake_returns_timeout_within_5s —
//!   connecting to an address with no listener must fail fast (well
//!   under 5 seconds), not stall on the 10s TLS handshake timer.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bytes::Bytes;
use copypaste_p2p::{PairedPeers, PeerClientStream, PeerStream, PeerTransport, SelfSignedCert};
use futures_util::SinkExt;
use tokio::net::TcpListener;

/// Bind a loopback TCP listener on an ephemeral port.
async fn bind_loopback() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let addr = listener.local_addr().expect("local_addr");
    (listener, addr)
}

/// Build a `PeerTransport` from the given cert + peers.
fn transport_with_cert(cert: &SelfSignedCert, peers: PairedPeers) -> PeerTransport {
    PeerTransport::from_cert(cert.cert_der.clone(), cert.key_der.clone(), peers)
}

/// Generate matching server+client certs and the corresponding `PairedPeers`
/// for each side. Returns `(server_transport, client_transport, server_fp)`.
fn paired_endpoints(server_id: &str, client_id: &str) -> (PeerTransport, PeerTransport, String) {
    let server_cert = SelfSignedCert::generate(server_id).expect("gen server cert");
    let client_cert = SelfSignedCert::generate(client_id).expect("gen client cert");

    let server_fp = server_cert.fingerprint();
    let client_fp = client_cert.fingerprint();

    let mut server_peers = PairedPeers::new();
    server_peers.add(client_fp.clone(), client_id);
    let mut client_peers = PairedPeers::new();
    client_peers.add(server_fp.clone(), server_id);

    (
        transport_with_cert(&server_cert, server_peers),
        transport_with_cert(&client_cert, client_peers),
        server_fp,
    )
}

/// Drive `server.accept(&listener)` and `client.connect(addr, server_fp)`
/// concurrently and return both streams once the mutual-TLS handshake is done.
async fn establish_pair(
    server: &PeerTransport,
    client: &PeerTransport,
    server_fp: &str,
) -> (PeerStream, PeerClientStream) {
    let (listener, addr) = bind_loopback().await;
    let (srv_res, cli_res) =
        tokio::join!(server.accept(&listener), client.connect(addr, server_fp));
    let (_peer_addr, srv_stream) = srv_res.expect("server accept");
    let cli_stream = cli_res.expect("client connect");
    (srv_stream, cli_stream)
}

// ----------------------------------------------------------------------------
// 1. Peer drops mid-transfer → sender observes Err (no panic, no hang).
// ----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn peer_drops_mid_transfer_sender_gets_io_error_not_panic() {
    let (server, client, server_fp) = paired_endpoints("server", "client");
    let (server_stream, mut client_stream) = establish_pair(&server, &client, &server_fp).await;

    // Drop the server side abruptly — this closes its TLS+TCP socket. Any
    // further writes from the client must surface an I/O error rather than
    // panicking or hanging forever.
    drop(server_stream);

    // Give the OS a moment to propagate FIN/RST to the client socket. We do
    // not actually need this for correctness (the loop below will eventually
    // fail), but it keeps the test fast on healthy kernels.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Try to push up to 1 MiB worth of 64 KiB frames. The send loop must
    // terminate with `Err(_)` within a bounded number of iterations rather
    // than succeeding forever (which would mean the broken-pipe was lost).
    let chunk: Bytes = Bytes::from(vec![0xAB; 64 * 1024]);
    let max_iters = 64; // 64 * 64 KiB = 4 MiB upper bound on buffering
    let mut last_result: Result<(), std::io::Error> = Ok(());

    // Bound the whole write loop with a wall-clock timeout so a regression
    // can't hang the test indefinitely.
    let outcome = tokio::time::timeout(Duration::from_secs(5), async {
        for _ in 0..max_iters {
            match client_stream.send(chunk.clone()).await {
                Ok(()) => {
                    last_result = Ok(());
                    continue;
                }
                Err(e) => {
                    last_result = Err(e);
                    break;
                }
            }
        }
        last_result
    })
    .await;

    let result = outcome.expect("write loop must complete within 5s, not hang");
    assert!(
        result.is_err(),
        "writing into a dropped peer must eventually return Err, got Ok after {max_iters} chunks"
    );
}

// ----------------------------------------------------------------------------
// 2. Reconnect after disconnect → fresh handshake succeeds with new session.
// ----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconnect_after_disconnect_succeeds() {
    let (server, client, server_fp) = paired_endpoints("server", "client");

    // First connection — establish, then tear both sides down cleanly.
    {
        let (srv_stream, cli_stream) = establish_pair(&server, &client, &server_fp).await;
        drop(srv_stream);
        drop(cli_stream);
    }

    // Small pause so the kernel can reclaim the previous socket pair.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Second connection — uses the same `PeerTransport`s (same cert/key
    // material) but goes through a completely new TLS handshake on a freshly
    // bound listener. Must succeed.
    let (srv_stream2, cli_stream2) = establish_pair(&server, &client, &server_fp).await;

    // Sanity check: streams are usable. Send one small message client→server
    // and just confirm `send` completes successfully (we don't read it back —
    // we only need to prove the new session is alive).
    let mut cli_stream2 = cli_stream2;
    cli_stream2
        .send(Bytes::from_static(b"ping"))
        .await
        .expect("reconnected stream must accept a write");

    drop(srv_stream2);
    drop(cli_stream2);
}

// ----------------------------------------------------------------------------
// 3. Three concurrent streams; drop one; the other two must still work.
// ----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multiple_concurrent_streams_one_drop_doesnt_affect_others() {
    // Three completely independent (server, client) pairs — each with its
    // own paired identity so cross-talk is impossible.
    let (srv_a, cli_a, fp_a) = paired_endpoints("srv-a", "cli-a");
    let (srv_b, cli_b, fp_b) = paired_endpoints("srv-b", "cli-b");
    let (srv_c, cli_c, fp_c) = paired_endpoints("srv-c", "cli-c");

    let (s_a, c_a) = establish_pair(&srv_a, &cli_a, &fp_a).await;
    let (s_b, c_b) = establish_pair(&srv_b, &cli_b, &fp_b).await;
    let (s_c, c_c) = establish_pair(&srv_c, &cli_c, &fp_c).await;

    // Kill stream B (both sides) — this must not affect A or C in any way.
    drop(s_b);
    drop(c_b);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // A and C must still be writable.
    let mut c_a = c_a;
    let mut c_c = c_c;
    c_a.send(Bytes::from_static(b"hello-a"))
        .await
        .expect("stream A must remain writable after B is dropped");
    c_c.send(Bytes::from_static(b"hello-c"))
        .await
        .expect("stream C must remain writable after B is dropped");

    drop(s_a);
    drop(s_c);
    drop(c_a);
    drop(c_c);
}

// ----------------------------------------------------------------------------
// 4. Peer offline during handshake → connect() fails fast (< 5s), no hang.
// ----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn peer_offline_during_handshake_returns_timeout_within_5s() {
    // Bind and immediately drop a listener to grab an ephemeral port that
    // is guaranteed to be free on this host. Once the listener is gone,
    // connect attempts to that port will be refused by the kernel almost
    // immediately on Linux/macOS loopback — which is exactly the "peer
    // offline" semantic from the user's perspective.
    let (listener, addr) = bind_loopback().await;
    drop(listener);

    let client_cert = SelfSignedCert::generate("client").unwrap();
    // The expected fingerprint never matters because the connection will
    // fail at the TCP layer well before TLS verification.
    let bogus_fp = "0".repeat(64);
    let mut client_peers = PairedPeers::new();
    client_peers.add(bogus_fp.clone(), "ghost-peer");

    let client = PeerTransport::from_cert(client_cert.cert_der, client_cert.key_der, client_peers);

    let start = Instant::now();
    let result =
        tokio::time::timeout(Duration::from_secs(5), client.connect(addr, &bogus_fp)).await;
    let elapsed = start.elapsed();

    let inner =
        result.expect("connect must complete (with Err) within 5s — must not hang on offline peer");
    assert!(
        inner.is_err(),
        "connect to an offline peer must return Err, got Ok"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "connect to offline peer must fail in <5s, took {:?}",
        elapsed
    );
}
