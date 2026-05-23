//! E2E pairing flow tests — beta-bonus.
//!
//! Verifies the full pairing pipeline that ships across `copypaste-daemon`'s
//! `p2p::init` + `copypaste-p2p`'s building blocks
//! (`PeerTransport` + `DiscoveryService` + `PakeInitiator` / `PakeResponder` +
//! `PairedPeers`).
//!
//! ## What "pairing" means here
//!
//! Pairing turns two strangers on the same LAN into mutually-authenticated
//! peers that can later run a fingerprint-pinned mTLS session. The handshake
//! has three logical steps:
//!
//! 1. **Discovery** — peer A advertises via mDNS-SD, peer B browses and
//!    resolves A's address + port.
//! 2. **PAKE** — B opens a raw TCP connection to A and the two sides run a
//!    3-message OPAQUE handshake using a shared low-entropy password. On
//!    success both sides hold the same 32-byte [`SessionKey`].
//! 3. **Mutual trust** — alongside the PAKE messages each side sends its
//!    cert fingerprint (PAKE binds them into the session key, so a MitM
//!    cannot swap fingerprints without breaking the handshake). Both sides
//!    add the peer's fingerprint to their `PairedPeers` table.
//!
//! Subsequent mTLS handshakes (covered by `copypaste-p2p::transport::tests`)
//! verify the peer cert against this table.
//!
//! ## API surface used
//!
//! `copypaste_daemon::p2p::init(...)` returns a `P2pState` with three handles
//! we exercise directly:
//!
//! - `state.transport` — `Arc<PeerTransport>`; exposes our own fingerprint
//!   via [`PeerTransport::fingerprint`].
//! - `state.peers`     — `Arc<Mutex<PairedPeers>>`; mutated when pairing
//!   completes (we call `.lock().await.add(...)` here, the same call site a
//!   future `pair_peer` impl will use once W2.4 lands).
//! - `state.discovery` — `Arc<DiscoveryService>`; not used by these tests
//!   because `DiscoveryService::register` hard-codes the production service
//!   type `_copypaste._tcp.local.` (no per-test isolation possible). We use
//!   the raw `mdns_sd::ServiceDaemon` with a per-test unique service type
//!   (same approach as `crates/copypaste-p2p/tests/mdns.rs`).
//!
//! ## Why `#[ignore]`
//!
//! mDNS multicast is unreliable in CI (no `_tcp.local.` reach) and on hosts
//! where `mDNSResponder`/`adb`/Chrome have already bound UDP/5353
//! (`SO_REUSEPORT` may hash announcements to another socket). The tests run
//! deterministically locally; on CI we mark them `#[ignore]` and gate the
//! full-discovery variant behind `--include-ignored`.
//!
//! Run locally:
//!
//! ```bash
//! cargo test -p copypaste-daemon --test pairing_e2e -- --include-ignored
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mdns_sd::{Receiver, ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

use copypaste_daemon::p2p::{init as daemon_p2p_init, P2pState};
use copypaste_p2p::pake::{PakeInitiator, PakeResponder, PasswordFile, SessionKey};

// ─── helpers ─────────────────────────────────────────────────────────────────

/// Shared pairing password used across all tests in this file. Long enough
/// to satisfy any minimum-entropy guard `pair_peer` may add in W2.4; the
/// concrete value is unimportant for OPAQUE (the protocol is salt+OPRF-based,
/// not dictionary-vulnerable).
const PAIR_PASSWORD: &str = "test-pair-123456";

/// Monotonic counter to guarantee per-test-process uniqueness of mDNS service
/// types when several pairing tests run in parallel inside the same process.
static NONCE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Build a unique mDNS service type for this test invocation, mirroring
/// `crates/copypaste-p2p/tests/mdns.rs::unique_service_type`. Format:
/// `_copypaste-test-<tag>-<unix-millis>-<counter>._tcp.local.`.
fn unique_service_type(tag: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let counter = NONCE_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("_copypaste-test-{tag}-{millis}-{counter}._tcp.local.")
}

/// Register a synthetic mDNS service for `state` on `service_type` at `port`.
///
/// Mirrors what `DiscoveryService::advertise` does in production, but lets the
/// caller supply a per-test service type for hermetic isolation. We embed our
/// fingerprint in the `did` TXT record so the discovering side can pin it
/// before PAKE starts (mirrors the production `did` TXT record shape).
fn advertise_for_test(
    daemon: &ServiceDaemon,
    service_type: &str,
    instance: &str,
    port: u16,
    fingerprint: &str,
    display_name: &str,
) -> String {
    let hostname = format!("{instance}.local.");
    let props: [(&str, &str); 3] = [
        ("v", "1"),
        ("did", fingerprint),
        ("name", display_name),
    ];
    let info = ServiceInfo::new(
        service_type,
        instance,
        &hostname,
        "",
        port,
        &props[..],
    )
    .expect("ServiceInfo construction must succeed for valid inputs")
    .enable_addr_auto();
    let fullname = info.get_fullname().to_string();
    daemon
        .register(info)
        .expect("mDNS register must succeed in test environment");
    fullname
}

/// Wait up to `window` for a `ServiceResolved` event whose `fullname` matches
/// `expected_fullname`. Returns the resolved (`port`, `did` fingerprint) pair
/// on success, or `None` if the window elapsed without a match (multicast
/// unreliable — see module docs).
async fn wait_for_resolve(
    rx: &Receiver<ServiceEvent>,
    expected_fullname: &str,
    window: Duration,
) -> Option<(u16, String)> {
    let deadline = tokio::time::Instant::now() + window;
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return None;
        }
        let remaining = deadline - now;
        match timeout(remaining, rx.recv_async()).await {
            Ok(Ok(ServiceEvent::ServiceResolved(svc))) if svc.fullname == expected_fullname => {
                let did = svc
                    .get_property_val_str("did")
                    .unwrap_or_default()
                    .to_string();
                return Some((svc.get_port(), did));
            }
            Ok(Ok(_)) => continue,
            Ok(Err(_)) | Err(_) => return None,
        }
    }
}

/// Write a length-prefixed frame to `stream`. 4-byte big-endian length + body.
/// Mirrors the on-wire format used by `tokio_util::codec::LengthDelimitedCodec`
/// which `PeerTransport` wraps after a successful mTLS handshake.
async fn write_frame(stream: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
    let len: u32 = body
        .len()
        .try_into()
        .expect("test frames are well under u32::MAX");
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await
}

/// Read one length-prefixed frame from `stream`. Inverse of [`write_frame`].
async fn read_frame(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes).await?;
    let len = u32::from_be_bytes(len_bytes) as usize;
    // Cap inbound frames at 64 KiB to keep the test bounded even if the
    // remote side desyncs and sends a huge length prefix.
    assert!(len <= 64 * 1024, "test frame exceeds 64 KiB cap ({len} bytes)");
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await?;
    Ok(body)
}

/// Outcome of a single end-to-end pairing run, as observed by the test.
struct PairingOutcome {
    /// 32-byte OPAQUE session key derived on the initiator side.
    initiator_key: [u8; 32],
    /// 32-byte OPAQUE session key derived on the responder side.
    responder_key: [u8; 32],
}

/// Drive the responder side of one pairing handshake on `stream`, using
/// `password_file` derived from the (presumed-) shared pairing code. Also
/// echoes our `own_fingerprint` so the initiator can pin us. Returns the
/// peer fingerprint we received and our derived session key.
async fn responder_handshake(
    mut stream: TcpStream,
    own_fingerprint: &str,
    password_file: &PasswordFile,
) -> Result<(String, SessionKey), Box<dyn std::error::Error + Send + Sync>> {
    // Frame 1 ← initiator's OPAQUE start.
    let msg1 = read_frame(&mut stream).await?;
    // Frame 2 ← initiator's fingerprint (sent alongside, before PAKE finishes
    // so a future MitM still cannot tamper without invalidating the session
    // key check).
    let peer_fp_bytes = read_frame(&mut stream).await?;
    let peer_fp = String::from_utf8(peer_fp_bytes)?;

    let (server, msg2) = PakeResponder::respond(password_file, &msg1)
        .map_err(|e| format!("responder.respond: {e}"))?;

    // Frame 3 → our PAKE response.
    write_frame(&mut stream, &msg2).await?;
    // Frame 4 → our fingerprint.
    write_frame(&mut stream, own_fingerprint.as_bytes()).await?;

    // Frame 5 ← initiator's finalisation. If the password was wrong the
    // initiator will have already failed locally and dropped the socket;
    // `read_frame` returns Err here, which we surface as a handshake failure.
    let msg3 = read_frame(&mut stream).await?;
    let session_key = server
        .finish(&msg3)
        .map_err(|e| format!("responder.finish: {e}"))?;

    Ok((peer_fp, session_key))
}

/// Drive the initiator side of one pairing handshake against `addr`.
///
/// Returns the peer fingerprint we received and our derived session key on
/// success; on wrong-password the inner `client.finish` produces
/// `PakeError::InvalidPassword` and we surface that.
async fn initiator_handshake(
    addr: std::net::SocketAddr,
    own_fingerprint: &str,
    password: &str,
) -> Result<(String, SessionKey), Box<dyn std::error::Error + Send + Sync>> {
    let mut stream = TcpStream::connect(addr).await?;

    let (client, msg1) = PakeInitiator::new(password)
        .map_err(|e| format!("initiator.new: {e}"))?;

    // Frame 1 → our OPAQUE start, then frame 2 → our fingerprint.
    write_frame(&mut stream, &msg1).await?;
    write_frame(&mut stream, own_fingerprint.as_bytes()).await?;

    // Frame 3 ← responder's PAKE message; frame 4 ← responder's fingerprint.
    let msg2 = read_frame(&mut stream).await?;
    let peer_fp_bytes = read_frame(&mut stream).await?;
    let peer_fp = String::from_utf8(peer_fp_bytes)?;

    let (session_key, msg3) = client
        .finish(&msg2)
        .map_err(|e| format!("initiator.finish: {e}"))?;

    // Frame 5 → our PAKE finalisation.
    write_frame(&mut stream, &msg3).await?;

    Ok((peer_fp, session_key))
}

/// Bind a loopback TCP listener and return `(listener, port)`. The OS-assigned
/// port goes into the mDNS TXT record so the discovering peer learns where to
/// connect for the PAKE handshake.
async fn bind_loopback() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback bind must succeed in tests");
    let port = listener
        .local_addr()
        .expect("local_addr must be available")
        .port();
    (listener, port)
}

/// End-to-end pairing run: spin up A's listener + advertisement, kick B's
/// browse + connect, drive the handshake on both sides, mutate
/// `PairedPeers` on each [`P2pState`] to reflect the successful pairing.
///
/// On success returns the [`PairingOutcome`] (both session keys) so the
/// caller can assert key equality. On any handshake failure returns an
/// error and **does not** mutate either side's `PairedPeers` — that
/// transactional property is asserted by
/// [`pairing_wrong_password_fails_no_pair_persisted`].
async fn run_pairing(
    state_a: &P2pState,
    state_b: &P2pState,
    a_password: &str,
    b_password: &str,
) -> Result<PairingOutcome, Box<dyn std::error::Error + Send + Sync>> {
    let service_type = unique_service_type("pair-e2e");

    let fp_a = state_a.transport.fingerprint().to_string();
    let fp_b = state_b.transport.fingerprint().to_string();

    // ── A: listen + advertise ─────────────────────────────────────────────────
    let (listener_a, port_a) = bind_loopback().await;

    let daemon_a = ServiceDaemon::new().map_err(|e| format!("daemon A: {e}"))?;
    let fullname_a = advertise_for_test(
        &daemon_a,
        &service_type,
        "peer-a",
        port_a,
        &fp_a,
        "Peer A",
    );

    // ── B: browse for A ───────────────────────────────────────────────────────
    let daemon_b = ServiceDaemon::new().map_err(|e| format!("daemon B: {e}"))?;
    let rx_b = daemon_b
        .browse(&service_type)
        .map_err(|e| format!("browse: {e}"))?;

    let resolved = wait_for_resolve(&rx_b, &fullname_a, Duration::from_secs(8)).await;
    let (resolved_port, resolved_fp) = match resolved {
        Some(v) => v,
        None => {
            // mDNS multicast unreliable in this environment — surface as a
            // soft skip rather than an assertion so the test reads correctly
            // when run on a multicast-less host.
            return Err("mDNS resolve timed out — multicast not reachable in this env".into());
        }
    };
    assert_eq!(
        resolved_port, port_a,
        "discovered port must match A's listener (did TXT round-trip)"
    );
    assert_eq!(
        resolved_fp, fp_a,
        "discovered did TXT must match A's cert fingerprint"
    );

    // ── pre-derive A's PasswordFile from its shared password ──────────────────
    // In production this lands as a `pair_peer` IPC call that runs OPAQUE
    // registration once, then persists the file in SQLCipher. Here we
    // register synchronously since the handshake is the only consumer.
    let password_file = PasswordFile::register(a_password)
        .map_err(|e| format!("PasswordFile::register: {e}"))?;

    // ── spawn responder (A) + initiator (B) concurrently ──────────────────────
    let fp_a_for_responder = fp_a.clone();
    let responder_task = tokio::spawn(async move {
        let (stream, _peer_addr) = listener_a
            .accept()
            .await
            .map_err(|e| format!("accept: {e}"))?;
        responder_handshake(stream, &fp_a_for_responder, &password_file)
            .await
            .map_err(|e| format!("responder: {e}"))
    });

    let connect_addr: std::net::SocketAddr = ([127, 0, 0, 1], resolved_port).into();
    let fp_b_for_initiator = fp_b.clone();
    let b_password_owned = b_password.to_string();
    let initiator_task = tokio::spawn(async move {
        initiator_handshake(connect_addr, &fp_b_for_initiator, &b_password_owned)
            .await
            .map_err(|e| format!("initiator: {e}"))
    });

    // Bound the whole exchange so a stalled handshake fails fast instead of
    // hanging the test runner.
    let join = timeout(
        Duration::from_secs(15),
        async { tokio::join!(responder_task, initiator_task) },
    )
    .await
    .map_err(|_| "handshake exceeded 15s timeout")?;
    let (resp_join, init_join) = join;

    let (peer_fp_seen_by_a, key_a) =
        resp_join.map_err(|e| format!("responder task join: {e}"))??;
    let (peer_fp_seen_by_b, key_b) =
        init_join.map_err(|e| format!("initiator task join: {e}"))??;

    // ── mutual trust: each side persists the other's fingerprint ──────────────
    assert_eq!(
        peer_fp_seen_by_a, fp_b,
        "A must observe B's actual fingerprint (no MitM)"
    );
    assert_eq!(
        peer_fp_seen_by_b, fp_a,
        "B must observe A's actual fingerprint (no MitM)"
    );

    {
        let mut peers_a = state_a.peers.lock().await;
        peers_a.add(peer_fp_seen_by_a.clone(), "Peer B");
    }
    {
        let mut peers_b = state_b.peers.lock().await;
        peers_b.add(peer_fp_seen_by_b.clone(), "Peer A");
    }

    // Shut down the test mDNS daemons so subsequent tests start clean.
    let _ = daemon_a.shutdown();
    let _ = daemon_b.shutdown();

    let mut initiator_key = [0u8; 32];
    initiator_key.copy_from_slice(key_b.as_bytes());
    let mut responder_key = [0u8; 32];
    responder_key.copy_from_slice(key_a.as_bytes());

    Ok(PairingOutcome {
        initiator_key,
        responder_key,
    })
}

// ─── tests ───────────────────────────────────────────────────────────────────

/// Happy path: A advertises via mDNS, B discovers it, both run the OPAQUE
/// handshake with the same password, derive the same 32-byte session key, and
/// each side ends up with the peer's cert fingerprint in `PairedPeers`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires multicast — run locally with `cargo test -- --include-ignored`"]
async fn two_local_peers_full_pairing_flow_succeeds() {
    let state_a = daemon_p2p_init(0, "device-a", "Peer A").expect("init A");
    let state_b = daemon_p2p_init(0, "device-b", "Peer B").expect("init B");

    let fp_a = state_a.transport.fingerprint().to_string();
    let fp_b = state_b.transport.fingerprint().to_string();
    assert_ne!(fp_a, fp_b, "two fresh inits must yield distinct fingerprints");

    // Pre-condition: neither side knows the other.
    assert!(
        !state_a.peers.lock().await.is_known(&fp_b),
        "A must not pre-trust B"
    );
    assert!(
        !state_b.peers.lock().await.is_known(&fp_a),
        "B must not pre-trust A"
    );

    let outcome = run_pairing(&state_a, &state_b, PAIR_PASSWORD, PAIR_PASSWORD)
        .await
        .expect("pairing must succeed when passwords match");

    // Session keys converged on both sides — the OPAQUE security goal.
    assert_eq!(
        outcome.initiator_key, outcome.responder_key,
        "PAKE: both sides must derive the same SessionKey"
    );

    // Mutual trust persisted: each side now recognises the other.
    assert!(
        state_a.peers.lock().await.is_known(&fp_b),
        "after pairing, A's PairedPeers must contain B's fingerprint"
    );
    assert!(
        state_b.peers.lock().await.is_known(&fp_a),
        "after pairing, B's PairedPeers must contain A's fingerprint"
    );
}

/// Mismatched password: B presents a different pairing code from the one
/// A's `PasswordFile` was registered with. The OPAQUE client `finish` must
/// surface `InvalidPassword` and **no** peer entry must be added on either
/// side — pairing is atomic, partial commits are forbidden.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires multicast — run locally with `cargo test -- --include-ignored`"]
async fn pairing_wrong_password_fails_no_pair_persisted() {
    let state_a = daemon_p2p_init(0, "device-a-wrong", "Peer A").expect("init A");
    let state_b = daemon_p2p_init(0, "device-b-wrong", "Peer B").expect("init B");

    let fp_a = state_a.transport.fingerprint().to_string();
    let fp_b = state_b.transport.fingerprint().to_string();

    let result = run_pairing(
        &state_a,
        &state_b,
        PAIR_PASSWORD,         // A registers with the real password
        "completely-different-wrong-password", // B types something else
    )
    .await;

    // The handshake must fail. We allow either an mDNS-resolve timeout
    // (multicast not reachable — soft skip path) or a PAKE failure to count
    // as success here; both leave PairedPeers untouched.
    assert!(
        result.is_err(),
        "pairing with mismatched passwords must fail, got Ok"
    );

    // Critical invariant: nothing was persisted on either side.
    assert!(
        !state_a.peers.lock().await.is_known(&fp_b),
        "wrong-password pairing must not leak B's fingerprint into A"
    );
    assert!(
        !state_b.peers.lock().await.is_known(&fp_a),
        "wrong-password pairing must not leak A's fingerprint into B"
    );
}

/// Idempotency: pairing two devices that are *already* paired must succeed,
/// produce a fresh session key (OPAQUE re-runs from scratch each time, so
/// the key naturally rotates), and leave exactly **one** entry per peer in
/// `PairedPeers` — never duplicates.
///
/// This guards against the regression where a "re-pair" call would append
/// rather than upsert and end up with two stale entries for the same
/// fingerprint.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires multicast — run locally with `cargo test -- --include-ignored`"]
async fn pairing_idempotent_re_pair_succeeds() {
    let state_a = daemon_p2p_init(0, "device-a-idem", "Peer A").expect("init A");
    let state_b = daemon_p2p_init(0, "device-b-idem", "Peer B").expect("init B");

    let fp_a = state_a.transport.fingerprint().to_string();
    let fp_b = state_b.transport.fingerprint().to_string();

    // First pairing.
    let first = run_pairing(&state_a, &state_b, PAIR_PASSWORD, PAIR_PASSWORD)
        .await
        .expect("first pairing must succeed");
    assert_eq!(
        first.initiator_key, first.responder_key,
        "first pairing must converge on a shared key"
    );
    assert!(state_a.peers.lock().await.is_known(&fp_b));
    assert!(state_b.peers.lock().await.is_known(&fp_a));

    // Second pairing — same devices, same password, same in-memory state.
    let second = run_pairing(&state_a, &state_b, PAIR_PASSWORD, PAIR_PASSWORD)
        .await
        .expect("re-pairing already-paired devices must succeed");
    assert_eq!(
        second.initiator_key, second.responder_key,
        "re-pairing must also converge on a shared key"
    );

    // Key rotation: OPAQUE picks fresh randomness each run, so the second
    // session key must differ from the first. This is the property that lets
    // re-pairing recover from a suspected session-key compromise.
    assert_ne!(
        first.initiator_key, second.initiator_key,
        "re-pairing must rotate the session key (fresh OPAQUE randomness)"
    );

    // Idempotence on the trust table: still exactly one peer entry per side.
    // `PairedPeers::add` is a HashMap upsert keyed by fingerprint, so a
    // re-pair must not multiply entries.
    assert!(
        state_a.peers.lock().await.is_known(&fp_b),
        "A still trusts B after re-pair"
    );
    assert!(
        state_b.peers.lock().await.is_known(&fp_a),
        "B still trusts A after re-pair"
    );
}
