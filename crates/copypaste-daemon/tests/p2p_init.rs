//! P2P init / shutdown integration tests — beta-bonus.
//!
//! ## API gaps (documented per task scope)
//!
//! 1. **Binary-only crate.** `copypaste-daemon` has no `src/lib.rs`, so
//!    `crate::p2p::{init, P2pState}` cannot be reached from `tests/*.rs`
//!    integration files.  Per the task scope ("DO NOT modify src/*") we
//!    exercise the **same construction contract** that `p2p::init` uses,
//!    against the public `copypaste-p2p` APIs (`DiscoveryService`,
//!    `PeerTransport`).
//!
//! 2. **Field naming drift.** The task spec asks to "assert `mdns_handle`,
//!    `tls_acceptor` present", but the actual `P2pState` (see
//!    `crates/copypaste-daemon/src/p2p.rs`) exposes three fields:
//!
//!    - `discovery:  Arc<DiscoveryService>`  ← mDNS-SD service (not "mdns_handle")
//!    - `transport:  Arc<PeerTransport>`     ← mTLS transport (not "tls_acceptor")
//!    - `peers:      Arc<Mutex<PairedPeers>>`
//!
//!    The spec's terminology is from a pre-W2.1 design.  This test asserts the
//!    real fields' construction succeeds and their public observables look
//!    sane.
//!
//! 3. **No tempdir needed for keys.** `p2p::init` generates a fresh
//!    self-signed cert in-memory (`PeerTransport::new_with_generated_cert`)
//!    and the discovery service does not write to disk either.  The task
//!    spec's "use tempdir for keys" is moot for this code path — a tempdir is
//!    created anyway to document the expected pattern and prove the call site
//!    tolerates an isolated working directory.
//!
//! ## Resolution path (post-beta)
//!
//! Add `crates/copypaste-daemon/src/lib.rs` with `pub mod p2p;` and switch
//! these tests to call `copypaste_daemon::p2p::init(...)` directly, which
//! would also let us assert against the exact `P2pState` struct.

use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;
use tokio::sync::Mutex;
use tokio::time::timeout;

use copypaste_p2p::{
    discovery::DiscoveryService,
    transport::{PairedPeers, PeerTransport},
};

/// Mirror of `daemon::p2p::P2pState`, built from the same public APIs.  Used
/// here so the test asserts against an identical shape without needing the
/// binary crate's private module.
struct P2pStateStandIn {
    discovery: Arc<DiscoveryService>,
    transport: Arc<PeerTransport>,
    #[allow(dead_code)]
    peers: Arc<Mutex<PairedPeers>>,
}

/// Mirror of `daemon::p2p::init` — same construction sequence, same error
/// surfaces, public-API-only.
fn init_stand_in(
    listen_port: u16,
    device_id: &str,
    device_name: &str,
) -> anyhow::Result<P2pStateStandIn> {
    let peers = PairedPeers::new();
    let transport = PeerTransport::new_with_generated_cert(device_id, peers.clone())
        .map_err(|e| anyhow::anyhow!("transport: {e}"))?;
    let discovery = DiscoveryService::new();
    discovery
        .register(listen_port, device_id, device_name)
        .map_err(|e| anyhow::anyhow!("discovery: {e}"))?;
    Ok(P2pStateStandIn {
        discovery: Arc::new(discovery),
        transport: Arc::new(transport),
        peers: Arc::new(Mutex::new(peers)),
    })
}

/// Contract: `init` returns a fully-constructed state with the discovery
/// service and mTLS transport both ready.  (Spec calls these `mdns_handle` /
/// `tls_acceptor` — see module doc for the rename.)
#[tokio::test(flavor = "multi_thread")]
async fn p2p_init_yields_discovery_and_transport() {
    // Tempdir is created to honour the spec's "use tempdir for keys" intent —
    // even though `init` is in-memory, this proves the call tolerates an
    // isolated CWD (useful when sandboxing is added later).
    let tmp = TempDir::new().expect("tempdir");
    let _guard = std::env::set_current_dir(tmp.path()).ok();

    let state = init_stand_in(0, "test-device", "Test Device").expect("init must succeed");

    // ── "mdns_handle present" → discovery service constructed and registered.
    // DiscoveryService has no public liveness probe before `start()`, but the
    // post-register `peers()` snapshot must be empty-not-panicking.
    let peers_now = state.discovery.peers();
    assert!(
        peers_now.is_empty(),
        "fresh discovery must have zero known peers before start()"
    );

    // ── "tls_acceptor present" → PeerTransport built with a self-signed cert.
    let fp = state.transport.fingerprint();
    assert!(
        !fp.is_empty(),
        "transport must expose a non-empty cert fingerprint"
    );
    assert!(
        fp.chars().all(|c| c.is_ascii_hexdigit()),
        "fingerprint must be hex (SHA-256 of cert DER), got: {fp}"
    );
    assert!(
        fp.len() >= 32,
        "fingerprint must be at least 32 hex chars, got {} chars",
        fp.len()
    );
}

/// Contract: dropping `P2pState` must be clean — no panics, no hangs,
/// completes synchronously within a tight timeout.  `init` does not spawn any
/// background tasks (those land in `start_p2p`), so drop should be immediate.
///
/// Note: `tokio::runtime::Handle::metrics()` is not enabled in the workspace
/// (the `metrics` feature is off), so we assert via a short-budget bounded
/// drop rather than a literal task count.
#[tokio::test(flavor = "multi_thread")]
async fn p2p_state_drop_is_clean_no_leaked_tasks() {
    let tmp = TempDir::new().expect("tempdir");
    let _guard = std::env::set_current_dir(tmp.path()).ok();

    // Bound the entire init+drop cycle. If init spawned and leaked tasks that
    // somehow blocked drop (they don't, by design), this would hang.
    timeout(Duration::from_millis(500), async {
        let state = init_stand_in(0, "drop-test", "Drop Test").expect("init");
        // Use the state so it's not optimised away.
        assert!(!state.transport.fingerprint().is_empty());
        // Explicit drop to make the intent obvious.
        drop(state);
        // Yield so the runtime gets a chance to reap anything (it shouldn't
        // need to — init spawns nothing — but this proves the runtime is live).
        tokio::task::yield_now().await;
    })
    .await
    .expect("p2p init + drop must complete within 500ms (no leaked/blocking tasks)");
}

/// Contract: repeated `init` calls (e.g. daemon restart in the same process)
/// must each yield independent, working state with distinct cert fingerprints.
#[tokio::test(flavor = "multi_thread")]
async fn p2p_init_produces_unique_fingerprints_per_call() {
    let tmp = TempDir::new().expect("tempdir");
    let _guard = std::env::set_current_dir(tmp.path()).ok();

    let a = init_stand_in(0, "device-a", "A").expect("init a");
    let b = init_stand_in(0, "device-b", "B").expect("init b");

    assert_ne!(
        a.transport.fingerprint(),
        b.transport.fingerprint(),
        "each init() must generate a fresh self-signed cert"
    );
}
