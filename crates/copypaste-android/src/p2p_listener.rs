//! Inbound mTLS P2P listener — the Android analog of the macOS daemon's
//! `accept_loop` (`copypaste-daemon/src/p2p.rs`).
//!
//! Today Android only DIALS out (`sync_with_peer`) and polls the relay; it
//! never accepts an inbound connection, so macOS can never INITIATE a P2P
//! session to an Android device. This module adds a persistent, cancellable
//! accept loop that binds `0.0.0.0:<port>`, completes mutual-TLS handshakes
//! against a pinned `PairedPeers` allowlist, pushes a one-shot catch-up of the
//! local history on each accepted connection, then keeps the link open and
//! decrypts every inbound `WireItem` frame into a [`SyncedItem`] for Kotlin to
//! drain via [`poll`].
//!
//! # FFI shape
//!
//! UDL has no interface objects, so the long-lived listener is represented by a
//! `u64` handle into a process-global registry ([`LISTENER_REGISTRY`]) — the
//! same idiom `lib.rs` uses for `DB_HANDLES`. The FFI surface (in `lib.rs`):
//!   * `start_p2p_listener(..)` — bind + register + spawn on the shared runtime,
//!     returning the handle and the OS-assigned port immediately.
//!   * `poll_p2p_listener(id)` — atomically drain the received-item buffer.
//!   * `update_p2p_listener_peers(id, ..)` — live roster/denylist refresh.
//!   * `stop_p2p_listener(id)` — cancel + deregister (idempotent).
//!
//! # Security (load-bearing — mirrors macOS)
//!
//! * Allowlist pinning IS the authenticator: only fingerprints seeded into
//!   `PairedPeers` complete the TLS handshake (transport.rs rejects unpinned
//!   fingerprints at `is_known`).
//! * The denylist is re-checked **at accept, before any catch-up or frame** —
//!   the inbound analog of the dialer's revoked-peer refusal. A revoked peer
//!   must never receive the history push.
//! * Per-peer session key: each peer's frames are decrypted with the key
//!   derived from THAT peer's verified fingerprint, never a global key.
//! * Key bytes are never logged.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use copypaste_core::{decrypt_from_cloud, encrypt_for_cloud, SyncKey};
use copypaste_p2p::transport::{PairedPeers, PeerTransport};
use copypaste_sync::protocol::WireItem;

use crate::{
    is_fingerprint_revoked, shared_sync_key_from_session, CopypasteError, LocalItem, SyncedItem,
    P2P_WIRE_KEY_VERSION,
};

/// Per-peer PAKE session key, keyed by the peer's pinned cert fingerprint.
///
/// `session_key` is the 32-byte PAKE session key from the bootstrap pairing —
/// the SAME value `sync_with_peer` consumes. The shared content key is derived
/// from it per-peer (never a single global key), so a frame from peer A is only
/// ever decrypted with A's key.
///
/// # SECURITY NOTE — `session_key` crosses the FFI boundary unzeroized.
/// UniFFI copies it into a Kotlin `ByteArray`. The Kotlin layer MUST zero that
/// array after the call returns; never log it.
#[derive(Debug, Clone)]
pub struct PeerSessionKey {
    pub fingerprint: String,
    pub session_key: Vec<u8>,
}

/// FFI result of [`start`](start): the registry handle plus the OS-assigned
/// port. When the caller passes `listen_port == 0` the kernel picks a free port
/// and `actual_port` reports it so Kotlin can advertise the real bind port.
#[derive(Debug, Clone)]
pub struct P2pListenerHandle {
    pub listener_id: u64,
    pub actual_port: u16,
}

/// Live, mutable per-peer trust + key state shared between the accept loop and
/// the `update_p2p_listener_peers` FFI. Held behind a `std::sync::Mutex`
/// (never across an `.await`) so a roster refresh is visible to the next
/// accepted connection without restarting the listener.
struct PeerState {
    /// Pinned allowlist that the TLS verifier consults on every handshake.
    /// Shared by clone with the `PeerTransport` (interior `Arc<RwLock>`), so
    /// `add`/`remove` here take effect on subsequent handshakes.
    peers: PairedPeers,
    /// The fingerprints currently pinned in `peers`. Tracked here because
    /// `PairedPeers` is not enumerable, so a roster refresh needs to know which
    /// previously-pinned fingerprints to `remove` before adding the new set.
    allowed: Vec<String>,
    /// Current denylist (canonicalized compare happens in `is_fingerprint_revoked`).
    revoked: Vec<String>,
    /// fingerprint → 32-byte PAKE session key for deriving that peer's content key.
    session_keys: HashMap<String, Vec<u8>>,
}

/// Process-global handle to one running listener. Stored in [`LISTENER_REGISTRY`]
/// keyed by the `u64` handle the FFI hands back to Kotlin.
struct ListenerHandle {
    /// Cancel token: `stop_p2p_listener` fires it and the accept loop's
    /// `select!` breaks, dropping the listener socket.
    cancel: CancellationToken,
    /// The port the listener actually bound (resolved from `local_addr()`).
    #[allow(dead_code)]
    // retained for diagnostics / future status FFI; not read on the hot path.
    actual_port: u16,
    /// Items decrypted from inbound frames, awaiting a `poll` drain.
    received: Arc<Mutex<Vec<SyncedItem>>>,
    /// Live trust/key roster, refreshable via `update_p2p_listener_peers`.
    peer_state: Arc<Mutex<PeerState>>,
}

/// Registry of running listeners. `OnceLock<Mutex<HashMap<…>>>` mirrors the
/// `DB_HANDLES` idiom in `lib.rs`.
static LISTENER_REGISTRY: OnceLock<Mutex<HashMap<u64, ListenerHandle>>> = OnceLock::new();
/// Monotonic id source for new listener handles (starts at 1; 0 is never used
/// so a Kotlin caller can treat 0 as "no listener").
static NEXT_LISTENER_ID: AtomicU64 = AtomicU64::new(1);

fn registry() -> &'static Mutex<HashMap<u64, ListenerHandle>> {
    LISTENER_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Seed a fresh [`PairedPeers`] allowlist from the caller's fingerprint list.
fn build_paired_peers(allowed: &[String]) -> PairedPeers {
    let peers = PairedPeers::new();
    for fp in allowed {
        // Display name is cosmetic here; the verifier only checks membership.
        peers.add(fp.clone(), "p2p-peer");
    }
    peers
}

/// Build a fresh `HashMap` of fingerprint → session key from the FFI list.
fn build_session_key_map(session_keys: Vec<PeerSessionKey>) -> HashMap<String, Vec<u8>> {
    let mut map = HashMap::with_capacity(session_keys.len());
    for sk in session_keys {
        map.insert(sk.fingerprint, sk.session_key);
    }
    map
}

/// Re-key the local history into outbound `WireItem`s under `shared`, mirroring
/// `sync_with_peer`'s catch-up build EXACTLY (same wire contract: the cloud
/// blob lives in `content`, `content_nonce` is `None`, image/file types carried
/// through). `device_id` stamps the origin so the peer can dedup by origin.
fn build_catchup_wire_items(
    local_items: &[LocalItem],
    shared: &SyncKey,
    device_id: &str,
) -> Result<Vec<WireItem>, CopypasteError> {
    let mut outbound: Vec<WireItem> = Vec::with_capacity(local_items.len());
    for it in local_items {
        let wire_content_type = if it.content_type == "text" || it.content_type.starts_with("text/")
        {
            "text".to_string()
        } else if it.content_type == "image" || it.content_type.starts_with("image/") {
            it.content_type.clone()
        } else if it.content_type == "file" {
            "file".to_string()
        } else {
            continue;
        };
        // STABLE identity: reuse the caller's persisted item_id; fall back to
        // the row id only for transitional rows. Never mint a fresh UUID here.
        let item_id = if it.item_id.is_empty() {
            it.id.clone()
        } else {
            it.item_id.clone()
        };
        let id = if it.id.is_empty() {
            item_id.clone()
        } else {
            it.id.clone()
        };
        let blob = encrypt_for_cloud(shared, &item_id, &it.plaintext)
            .map_err(|_| CopypasteError::EncryptionFailed)?;
        outbound.push(WireItem {
            id,
            item_id,
            content_type: wire_content_type,
            content: Some(blob),
            content_nonce: None,
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: it.wall_time_ms,
            wall_time: it.wall_time_ms,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: device_id.to_string(),
            key_version: P2P_WIRE_KEY_VERSION,
            file_name: it.file_name.clone(),
            mime: it.mime.clone(),
            deleted: false,
            pinned: false,
            pin_order: None,
        });
    }
    Ok(outbound)
}

/// Decrypt one inbound `WireItem` into a [`SyncedItem`], or `None` if it is a
/// legacy/non-rekeyed frame, an unknown content type, or fails to decrypt with
/// the shared key. Mirrors the inbound unwrap in `sync_with_peer` — never logs
/// key bytes.
fn decrypt_wire_item(wire: &WireItem, shared: &SyncKey) -> Option<SyncedItem> {
    // A text frame that still carries a content_nonce is a legacy / non-rekeyed
    // frame we cannot decrypt with the shared sync key — skip it.
    if wire.content_type == "text" && wire.content_nonce.is_some() {
        return None;
    }
    let is_text = wire.content_type == "text" || wire.content_type.starts_with("text/");
    let is_image = wire.content_type == "image" || wire.content_type.starts_with("image/");
    let is_file = wire.content_type == "file";
    if !(is_text || is_image || is_file) {
        return None;
    }
    let blob = wire.content.as_ref()?;
    match decrypt_from_cloud(shared, &wire.item_id, blob) {
        Ok(plaintext) => Some(SyncedItem {
            id: wire.id.clone(),
            item_id: wire.item_id.clone(),
            content_type: wire.content_type.clone(),
            plaintext,
            wall_time_ms: wire.wall_time,
            file_name: wire.file_name.clone(),
            mime: wire.mime.clone(),
        }),
        Err(_) => None,
    }
}

/// Per-connection pump: push the catch-up history once, then read inbound
/// frames with NO idle/deadline cutoff (keep the link open like the daemon)
/// until the peer drops or `cancel` fires. Every decrypted item is appended to
/// `received`.
async fn run_connection(
    mut framed: copypaste_p2p::transport::PeerStream,
    shared: SyncKey,
    catchup: Vec<WireItem>,
    received: Arc<Mutex<Vec<SyncedItem>>>,
    cancel: CancellationToken,
) {
    use bytes::Bytes;
    use futures_util::{SinkExt, StreamExt};

    // (1) Push the catch-up history once. A serialisation/write error just
    //     means the link is gone — stop, don't panic.
    for item in &catchup {
        match serde_json::to_vec(item) {
            Ok(payload) => {
                if framed.send(Bytes::from(payload)).await.is_err() {
                    return;
                }
            }
            Err(_) => return,
        }
    }

    // (2) Persistent read loop — no idle/deadline cutoff. Keep the link open
    //     like the daemon's `run_peer_connection`; exit only on peer EOF/error
    //     or listener shutdown.
    loop {
        tokio::select! {
            frame = framed.next() => {
                match frame {
                    Some(Ok(bytes)) => {
                        if let Ok(wire) = serde_json::from_slice::<WireItem>(&bytes) {
                            if let Some(item) = decrypt_wire_item(&wire, &shared) {
                                // Lock held only to push; never across an await.
                                if let Ok(mut buf) = received.lock() {
                                    buf.push(item);
                                }
                            }
                        }
                        // Unparseable / undecryptable frames are skipped (match
                        // the daemon: log-and-continue), not fatal.
                    }
                    // Frame-level error or clean EOF: peer dropped the link.
                    Some(Err(_)) | None => return,
                }
            }
            _ = cancel.cancelled() => return,
        }
    }
}

/// The accept loop, ported from the daemon's `accept_loop` (no outbound
/// fanout). `select!`s an accept against the cancel token; per accepted
/// connection re-checks the denylist BEFORE any catch-up, derives the per-peer
/// shared key, and spawns [`run_connection`].
async fn accept_loop(
    listener: TcpListener,
    transport: Arc<PeerTransport>,
    peer_state: Arc<Mutex<PeerState>>,
    local_items: Arc<Vec<LocalItem>>,
    device_id: Arc<String>,
    received: Arc<Mutex<Vec<SyncedItem>>>,
    cancel: CancellationToken,
) {
    loop {
        tokio::select! {
            result = transport.accept(&listener) => {
                match result {
                    Ok((_peer_addr, peer_fp, framed)) => {
                        // ── SECURITY: re-check the denylist AT ACCEPT, before
                        //    catch-up or ANY frame. A revoked peer must never
                        //    receive the history push (inbound analog of the
                        //    dialer's revoked-peer refusal). ──
                        let (is_revoked, session_key) = {
                            // std::Mutex held briefly, never across an await.
                            let Ok(state) = peer_state.lock() else { continue };
                            let revoked = is_fingerprint_revoked(&peer_fp, &state.revoked);
                            let key = state.session_keys.get(&peer_fp).cloned();
                            (revoked, key)
                        };
                        if is_revoked {
                            // Drop the connection without sending anything.
                            drop(framed);
                            continue;
                        }

                        // Derive the per-peer shared content key from the
                        // VERIFIED peer fingerprint's session key. Without a
                        // session key we cannot decrypt/encrypt for this peer —
                        // drop the connection.
                        let Some(session_key) = session_key else {
                            drop(framed);
                            continue;
                        };
                        let shared = match shared_sync_key_from_session(&session_key) {
                            Ok(k) => k,
                            Err(_) => {
                                drop(framed);
                                continue;
                            }
                        };

                        // Build the catch-up history under THIS peer's key.
                        let catchup =
                            match build_catchup_wire_items(&local_items, &shared, &device_id) {
                                Ok(c) => c,
                                Err(_) => {
                                    drop(framed);
                                    continue;
                                }
                            };

                        let received = Arc::clone(&received);
                        let conn_cancel = cancel.clone();
                        tokio::spawn(async move {
                            run_connection(framed, shared, catchup, received, conn_cancel).await;
                        });
                    }
                    Err(_e) => {
                        // Accept/handshake error (unknown peer, TLS failure,
                        // handshake timeout). Not fatal — keep accepting.
                    }
                }
            }
            _ = cancel.cancelled() => break,
        }
    }
}

/// Bind `0.0.0.0:listen_port`, register a listener, and spawn its accept loop on
/// the shared runtime. Returns immediately with the handle + actual bound port.
///
/// `listen_port == 0` lets the kernel assign a free port (read back from
/// `local_addr()`).
#[allow(clippy::too_many_arguments)] // FFI contract mirrors `sync_with_peer`'s shape.
pub fn start(
    runtime: &'static tokio::runtime::Runtime,
    listen_port: u16,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    allowed_fingerprints: Vec<String>,
    revoked_fingerprints: Vec<String>,
    session_keys: Vec<PeerSessionKey>,
    local_items: Vec<LocalItem>,
    device_id: String,
) -> Result<P2pListenerHandle, CopypasteError> {
    // Bind on the runtime so the TcpListener is created inside a tokio context.
    let listener = runtime
        .block_on(async move { TcpListener::bind(("0.0.0.0", listen_port)).await })
        .map_err(|e| CopypasteError::P2pError {
            reason: format!("failed to bind P2P listener on 0.0.0.0:{listen_port}: {e}"),
        })?;
    let actual_port = listener
        .local_addr()
        .map_err(|e| CopypasteError::P2pError {
            reason: format!("failed to read P2P listener local_addr: {e}"),
        })?
        .port();

    let peers = build_paired_peers(&allowed_fingerprints);
    // The transport clones the same `PairedPeers` Arc, so later add/remove on
    // `peer_state.peers` are reflected on subsequent handshakes.
    let transport = Arc::new(PeerTransport::from_cert(cert_der, key_der, peers.clone()));

    let peer_state = Arc::new(Mutex::new(PeerState {
        peers,
        allowed: allowed_fingerprints,
        revoked: revoked_fingerprints,
        session_keys: build_session_key_map(session_keys),
    }));
    let received: Arc<Mutex<Vec<SyncedItem>>> = Arc::new(Mutex::new(Vec::new()));
    let cancel = CancellationToken::new();

    let listener_id = NEXT_LISTENER_ID.fetch_add(1, Ordering::SeqCst);

    {
        let mut reg = registry().lock().map_err(|_| CopypasteError::P2pError {
            reason: "listener registry mutex poisoned".to_string(),
        })?;
        reg.insert(
            listener_id,
            ListenerHandle {
                cancel: cancel.clone(),
                actual_port,
                received: Arc::clone(&received),
                peer_state: Arc::clone(&peer_state),
            },
        );
    }

    let local_items = Arc::new(local_items);
    let device_id = Arc::new(device_id);
    runtime.spawn(accept_loop(
        listener,
        transport,
        peer_state,
        local_items,
        device_id,
        received,
        cancel,
    ));

    Ok(P2pListenerHandle {
        listener_id,
        actual_port,
    })
}

/// Atomically drain the received-item buffer for `listener_id`. Returns an empty
/// vec for an unknown id (the listener may have been stopped between polls).
pub fn poll(listener_id: u64) -> Result<Vec<SyncedItem>, CopypasteError> {
    let reg = registry().lock().map_err(|_| CopypasteError::P2pError {
        reason: "listener registry mutex poisoned".to_string(),
    })?;
    let Some(handle) = reg.get(&listener_id) else {
        return Ok(Vec::new());
    };
    let mut buf = handle
        .received
        .lock()
        .map_err(|_| CopypasteError::P2pError {
            reason: "received-item buffer mutex poisoned".to_string(),
        })?;
    Ok(std::mem::take(&mut *buf))
}

/// Live roster/denylist/session-key refresh without restarting the listener.
/// A revoked fingerprint is removed from the pinned allowlist immediately (so
/// the TLS verifier rejects it on the next handshake) and added to the
/// denylist (so an in-flight accept is refused). No-op for an unknown id.
pub fn update_peers(
    listener_id: u64,
    allowed: Vec<String>,
    revoked: Vec<String>,
    session_keys: Vec<PeerSessionKey>,
) -> Result<(), CopypasteError> {
    let reg = registry().lock().map_err(|_| CopypasteError::P2pError {
        reason: "listener registry mutex poisoned".to_string(),
    })?;
    let Some(handle) = reg.get(&listener_id) else {
        return Ok(());
    };
    let mut state = handle
        .peer_state
        .lock()
        .map_err(|_| CopypasteError::P2pError {
            reason: "peer-state mutex poisoned".to_string(),
        })?;

    // Reconcile the pinned allowlist in place: the `PeerTransport` holds a clone
    // of this same `PairedPeers` (shared Arc), so add/remove take effect on the
    // next handshake without rebuilding the transport.
    let desired: std::collections::HashSet<&String> = allowed.iter().collect();
    // Remove any previously-pinned fingerprint that is no longer allowed.
    for fp in &state.allowed {
        if !desired.contains(fp) {
            state.peers.remove(fp);
        }
    }
    // Add every desired fingerprint (add is idempotent).
    for fp in &allowed {
        state.peers.add(fp.clone(), "p2p-peer");
    }
    // Evict revoked fingerprints from the pinned allowlist immediately so an
    // in-flight or next handshake from a revoked peer is rejected at TLS.
    for fp in &revoked {
        state.peers.remove(fp);
    }

    state.allowed = allowed;
    state.revoked = revoked;
    state.session_keys = build_session_key_map(session_keys);
    Ok(())
}

/// Cancel and deregister `listener_id`. Idempotent: a second call (or an unknown
/// id) is a no-op. Firing the cancel token breaks the accept loop's `select!`
/// and drops the listener socket; in-flight per-connection tasks also observe
/// the cancel and exit.
pub fn stop(listener_id: u64) -> Result<(), CopypasteError> {
    let handle = {
        let mut reg = registry().lock().map_err(|_| CopypasteError::P2pError {
            reason: "listener registry mutex poisoned".to_string(),
        })?;
        reg.remove(&listener_id)
    };
    if let Some(handle) = handle {
        handle.cancel.cancel();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate_device_cert;
    use bytes::Bytes;
    use copypaste_p2p::pake::SessionKey;
    use copypaste_p2p::transport::PeerTransport as ClientTransport;
    use futures_util::SinkExt;
    use std::time::Duration;

    /// Spin up the shared test runtime once. Tests must not create nested
    /// runtimes when they call `start` (which blocks on the runtime to bind).
    fn test_runtime() -> &'static tokio::runtime::Runtime {
        crate::runtime().expect("test tokio runtime builds")
    }

    /// A fixed 32-byte PAKE session key both ends agree on (bootstrap output).
    const TEST_SESSION_KEY: [u8; 32] = [0x5Au8; 32];

    /// Derive the shared content key the listener and the dialer both use, the
    /// SAME way `shared_sync_key_from_session` does.
    fn shared_test_key() -> SyncKey {
        let sk = SessionKey(TEST_SESSION_KEY);
        SyncKey::from_bytes(*sk.derive_xchacha_key(crate::P2P_SYNC_KEY_SALT))
    }

    /// Drive a single dial against the running listener from a dedicated OS
    /// thread + runtime: connect over mTLS pinning `listener_fp`, send one
    /// framed `WireItem`, then hold the link briefly so the listener can read
    /// it. Returns `Ok(())` if the handshake + send succeeded, `Err` if the
    /// handshake was rejected (e.g. revoked/unpinned at accept).
    fn dial_and_send(
        addr: std::net::SocketAddr,
        listener_fp: String,
        client_cert_der: Vec<u8>,
        client_key_der: Vec<u8>,
        wire: WireItem,
    ) -> Result<(), String> {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("client runtime");
            rt.block_on(async move {
                let peers = PairedPeers::new();
                peers.add(listener_fp.clone(), "listener");
                let transport = ClientTransport::from_cert(client_cert_der, client_key_der, peers);
                let mut framed = transport
                    .connect(addr, &listener_fp)
                    .await
                    .map_err(|e| format!("connect failed: {e}"))?;
                let payload = serde_json::to_vec(&wire).map_err(|e| e.to_string())?;
                framed
                    .send(Bytes::from(payload))
                    .await
                    .map_err(|e| format!("send failed: {e}"))?;
                // Keep the link open briefly so the listener reads the frame.
                tokio::time::sleep(Duration::from_millis(300)).await;
                Ok::<(), String>(())
            })
        })
        .join()
        .expect("client thread")
    }

    /// Build a sync-key-wrapped text `WireItem` carrying `plaintext` under
    /// `shared` (the on-wire shape the listener decrypts).
    fn make_wire_item(shared: &SyncKey, plaintext: &[u8]) -> WireItem {
        let item_id = uuid::Uuid::new_v4().to_string();
        let blob = encrypt_for_cloud(shared, &item_id, plaintext).expect("wrap item");
        WireItem {
            id: item_id.clone(),
            item_id,
            content_type: "text".to_string(),
            content: Some(blob),
            content_nonce: None,
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 7,
            wall_time: 7,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "test-dialer".to_string(),
            key_version: P2P_WIRE_KEY_VERSION,
            file_name: None,
            mime: None,
        }
    }

    /// Allowlist pinning + loopback handshake: a dialer whose fingerprint IS in
    /// the allowlist completes the handshake; the listener decrypts its framed
    /// item and surfaces it via `poll`.
    #[test]
    fn allowlist_pinned_peer_handshakes_and_item_is_received() {
        let listener_cert = generate_device_cert().expect("listener cert");
        let client_cert = generate_device_cert().expect("client cert");
        let client_fp = client_cert.fingerprint.clone();
        let listener_fp = listener_cert.fingerprint.clone();

        let handle = start(
            test_runtime(),
            0,
            listener_cert.cert_der.clone(),
            listener_cert.key_der.clone(),
            vec![client_fp.clone()],
            Vec::new(), // no revocations
            vec![PeerSessionKey {
                fingerprint: client_fp.clone(),
                session_key: TEST_SESSION_KEY.to_vec(),
            }],
            Vec::new(), // no catch-up history
            "listener-device".to_string(),
        )
        .expect("listener starts");

        let addr: std::net::SocketAddr = format!("127.0.0.1:{}", handle.actual_port)
            .parse()
            .expect("addr");
        let shared = shared_test_key();
        let plaintext = b"hello from the macOS dialer".to_vec();
        let wire = make_wire_item(&shared, &plaintext);

        dial_and_send(
            addr,
            listener_fp,
            client_cert.cert_der.clone(),
            client_cert.key_der.clone(),
            wire,
        )
        .expect("pinned peer handshake must succeed");

        // Poll for the decrypted item (give the accept task a moment to run).
        let mut got = Vec::new();
        for _ in 0..50 {
            got = poll(handle.listener_id).expect("poll");
            if !got.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        stop(handle.listener_id).expect("stop");

        assert_eq!(got.len(), 1, "listener must surface the dialer's item");
        assert_eq!(got[0].plaintext, plaintext, "decrypted plaintext mismatch");
    }

    /// Denylist enforced AT ACCEPT: a dialer whose (pinned) fingerprint is also
    /// in the revoked list is dropped before any catch-up/frame, so `poll`
    /// never yields its item.
    #[test]
    fn revoked_peer_is_rejected_at_accept() {
        let listener_cert = generate_device_cert().expect("listener cert");
        let client_cert = generate_device_cert().expect("client cert");
        let client_fp = client_cert.fingerprint.clone();
        let listener_fp = listener_cert.fingerprint.clone();

        // The fingerprint is BOTH allowed (so TLS would complete) AND revoked
        // (so the accept-time denylist check must drop it).
        let handle = start(
            test_runtime(),
            0,
            listener_cert.cert_der.clone(),
            listener_cert.key_der.clone(),
            vec![client_fp.clone()],
            vec![client_fp.clone()], // revoked
            vec![PeerSessionKey {
                fingerprint: client_fp.clone(),
                session_key: TEST_SESSION_KEY.to_vec(),
            }],
            Vec::new(),
            "listener-device".to_string(),
        )
        .expect("listener starts");

        let addr: std::net::SocketAddr = format!("127.0.0.1:{}", handle.actual_port)
            .parse()
            .expect("addr");
        let shared = shared_test_key();
        let wire = make_wire_item(&shared, b"should never be received");

        // The TLS handshake itself may complete (fp is pinned) — the rejection
        // is at the application layer (denylist) right after accept. The dialer
        // may therefore see send succeed; what matters is the listener drops the
        // connection and never decrypts the item.
        let _ = dial_and_send(
            addr,
            listener_fp,
            client_cert.cert_der.clone(),
            client_cert.key_der.clone(),
            wire,
        );

        // Give the accept task time to run and (correctly) drop the connection.
        std::thread::sleep(Duration::from_millis(400));
        let got = poll(handle.listener_id).expect("poll");
        stop(handle.listener_id).expect("stop");

        assert!(
            got.is_empty(),
            "a revoked peer's item must NOT be received (denylist enforced at accept)"
        );
    }

    /// Registry lifecycle: start registers a handle, poll on a live id returns
    /// (empty) Ok, stop deregisters, and a poll/stop after stop is a safe no-op.
    #[test]
    fn start_poll_stop_registry_lifecycle() {
        let cert = generate_device_cert().expect("cert");
        let handle = start(
            test_runtime(),
            0,
            cert.cert_der.clone(),
            cert.key_der.clone(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            "lifecycle-device".to_string(),
        )
        .expect("listener starts");

        assert!(handle.listener_id >= 1, "ids start at 1");
        assert!(handle.actual_port > 0, "OS assigned a real port");

        // Live id: poll yields an empty drain without error.
        let drained = poll(handle.listener_id).expect("poll live id");
        assert!(drained.is_empty());

        // Stop deregisters.
        stop(handle.listener_id).expect("stop");

        // After stop: poll returns empty (unknown id), stop is idempotent.
        let after = poll(handle.listener_id).expect("poll after stop is Ok");
        assert!(after.is_empty(), "unknown id drains empty");
        stop(handle.listener_id).expect("second stop is a no-op");

        // An entirely unknown id is also a safe no-op for update_peers.
        update_peers(999_999, Vec::new(), Vec::new(), Vec::new())
            .expect("update on unknown id is a no-op");
    }

    /// `update_peers` on a running listener swaps the roster: a newly-revoked
    /// fingerprint is removed from the pinned allowlist so its next handshake is
    /// rejected at TLS.
    #[test]
    fn update_peers_evicts_revoked_from_allowlist() {
        let listener_cert = generate_device_cert().expect("listener cert");
        let client_cert = generate_device_cert().expect("client cert");
        let client_fp = client_cert.fingerprint.clone();
        let listener_fp = listener_cert.fingerprint.clone();

        let handle = start(
            test_runtime(),
            0,
            listener_cert.cert_der.clone(),
            listener_cert.key_der.clone(),
            vec![client_fp.clone()],
            Vec::new(),
            vec![PeerSessionKey {
                fingerprint: client_fp.clone(),
                session_key: TEST_SESSION_KEY.to_vec(),
            }],
            Vec::new(),
            "listener-device".to_string(),
        )
        .expect("listener starts");

        // Now revoke the client: remove from allowlist + add to denylist.
        update_peers(
            handle.listener_id,
            Vec::new(),              // no longer allowed
            vec![client_fp.clone()], // revoked
            Vec::new(),
        )
        .expect("update_peers");

        let addr: std::net::SocketAddr = format!("127.0.0.1:{}", handle.actual_port)
            .parse()
            .expect("addr");
        let shared = shared_test_key();
        let wire = make_wire_item(&shared, b"post-revocation item");

        // After the live update, the dialer's fingerprint is no longer pinned
        // AND is on the denylist. The handshake is rejected at the verifier
        // and/or the connection is dropped at accept by the denylist re-check —
        // either way the load-bearing guarantee is that NO item is received.
        // (The client-side connect may or may not observe the rejection
        // depending on TLS alert timing, so we assert on the security outcome
        // — no item delivered — rather than on the dialer's error.)
        let _ = dial_and_send(
            addr,
            listener_fp,
            client_cert.cert_der.clone(),
            client_cert.key_der.clone(),
            wire,
        );

        std::thread::sleep(Duration::from_millis(300));
        let got = poll(handle.listener_id).expect("poll");
        stop(handle.listener_id).expect("stop");

        assert!(
            got.is_empty(),
            "a revoked/unpinned dialer's item must NOT be received after a live roster update"
        );
    }
}
