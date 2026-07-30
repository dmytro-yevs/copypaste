//! The peer half of the in-process backend: the node, and the five operations
//! that need it.
//!
//! Everything here is [`copypaste_p2p::Node`] driven over
//! [`copypaste_core::StoreSource`] — the same node the daemon runs and the same
//! source it runs it over. Nothing in this file decides anything about pairing,
//! sessions or the merge; if it did, this platform would have its own answer to
//! a question the other platform has already answered (ADR-0003, CLAUDE.md
//! rule 1).
//!
//! # The listener comes up once, lazily
//!
//! Binding a TCP port and registering an mDNS service are not things to do
//! while `EmbeddedBackend::open` is holding up the first paint, and neither can
//! be done at all outside a Tokio context. So the node is built on the first
//! peer operation — including the `status` probe the supervisor makes at launch,
//! which is what brings it up before any screen asks for it.
//!
//! A failure to bind is not a failure to start: this device can still dial out,
//! and refusing to pair because an inbound port is taken would be worse than
//! pairing in one direction.

use std::sync::Arc;

use copypaste_core::StoreSource;
use copypaste_ipc::{DiscoveredDevice, PairingData, PeerInfo, SyncResult};
use copypaste_p2p::discovery::Discovery;
use copypaste_p2p::peers::{Peer, PeerStore};
use copypaste_p2p::sync::SyncOutcome;
use copypaste_p2p::{Node, NodeError};
use tokio::sync::watch;

use super::rows::peer_info;
use super::{Inner, MSG_NO_PEER};
use crate::backend::{BackendError, Result};

/// What this device tells peers it is called, on first run only.
///
/// Cosmetic and peer-visible. Android has no hostname worth reading — it is
/// `localhost` — and there is no rename screen yet, so this is the honest
/// placeholder rather than a guess dressed up as a device name.
const DEVICE_NAME_HINT: &str = "CopyPaste phone";

/// The node, plus the handle that stops its listener.
pub(super) struct PeerNode {
    node: Arc<Node>,
    /// Dropped with the backend, which ends the accept loop. Held rather than
    /// discarded because a `watch::Sender` that goes out of scope closes the
    /// channel and the listener would stop immediately.
    _shutdown: watch::Sender<bool>,
}

impl PeerNode {
    /// Build the node, register the mDNS service, and start accepting.
    ///
    /// Must be called from a Tokio context.
    pub(super) async fn start(inner: &Arc<Inner>) -> Result<Self> {
        // Opened here rather than in `EmbeddedBackend::open` because the node
        // owns it by value: `PeerStore` is deliberately not `Clone` — two
        // in-memory caches over one file would disagree about which keys the
        // listener accepts.
        let peers = PeerStore::open(&inner.peers_path)
            .map_err(|e| BackendError::internal(&format!("could not open paired devices: {e}")))?;
        let pairing_ids: Vec<String> = peers
            .list()
            .iter()
            .map(|peer| peer.pairing_id.clone())
            .collect();
        // Never fatal: a network that filters multicast still pairs and still
        // syncs to an explicit address, which is the common case on a phone.
        let discovery = match Discovery::start(&inner.device_name, &pairing_ids, PORT) {
            Ok(discovery) => Some(discovery),
            Err(e) => {
                tracing::warn!(error = %e, "could not start discovery; peers must be given an address");
                None
            }
        };

        let node = Arc::new(Node::new(peers, discovery, PORT));
        let (shutdown, shutdown_rx) = watch::channel(false);

        match copypaste_p2p::node::bind(PORT).and_then(tokio::net::TcpListener::from_std) {
            Ok(listener) => {
                let listening = Arc::clone(&node);
                let source = Arc::new(source(inner));
                let named = Arc::clone(inner);
                tokio::spawn(copypaste_p2p::node::listen(
                    listening,
                    listener,
                    source,
                    move |_pairing_id: &str, outcome: &SyncOutcome| remember(&named, outcome),
                    shutdown_rx,
                ));
            }
            Err(e) => tracing::warn!(
                error = %e,
                "could not bind the peer port; this device can dial out but not be dialled"
            ),
        }

        Ok(Self {
            node,
            _shutdown: shutdown,
        })
    }

    pub(super) fn pair_create(&self, name: &str) -> Result<PairingData> {
        let pairing = self.node.pair_create(name).map_err(failed)?;
        Ok(PairingData {
            // The one and only rendering of the secret, straight into the
            // reply. Not logged, not stored, not retrievable again.
            code: pairing.code,
            pairing_id: pairing.pairing_id,
            listen_addr: pairing.listen_addr,
        })
    }

    pub(super) async fn pair_accept(
        &self,
        inner: &Arc<Inner>,
        code: &str,
        addr: &str,
    ) -> Result<Vec<PeerInfo>> {
        let accepted = self
            .node
            .pair_accept(code, addr, &source(inner))
            .await
            .map_err(failed)?;
        remember(inner, &accepted.outcome);
        let online = self.node.find(&accepted.peer.pairing_id).is_some();
        Ok(vec![peer_info(&accepted.peer, online)])
    }

    pub(super) fn peers(&self) -> Vec<PeerInfo> {
        self.node
            .peers()
            .list()
            .iter()
            .map(|peer| peer_info(peer, self.node.find(&peer.pairing_id).is_some()))
            .collect()
    }

    pub(super) fn unpair(&self, pairing_id: &str) -> Result<bool> {
        self.node.unpair(pairing_id).map_err(failed)
    }

    /// Sync with one peer, or with every known peer.
    ///
    /// One peer failing must not abort the rest: each gets its own
    /// [`SyncResult`], and a failure is reported in that peer's `error` field
    /// while the run continues. The whole request only fails when a named peer
    /// does not exist.
    pub(super) async fn sync(
        &self,
        inner: &Arc<Inner>,
        pairing_id: Option<&str>,
    ) -> Result<Vec<SyncResult>> {
        let targets = match pairing_id {
            Some(pairing_id) => match self.node.peers().get(pairing_id) {
                Some(peer) => vec![peer],
                None => return Err(failed(NodeError::NoPeer)),
            },
            None => self.node.peers().list(),
        };

        let source = source(inner);
        let mut results = Vec::with_capacity(targets.len());
        for peer in &targets {
            results.push(self.sync_one(inner, &source, peer).await);
        }
        Ok(results)
    }

    /// One peer, start to finish. Never returns `Err`: a failure is a field.
    async fn sync_one(&self, inner: &Arc<Inner>, source: &StoreSource, peer: &Peer) -> SyncResult {
        match self.node.sync_one(peer, source).await {
            Ok(outcome) => {
                remember(inner, &outcome);
                SyncResult {
                    pairing_id: peer.pairing_id.clone(),
                    name: outcome.peer_device_name,
                    sent: u32::try_from(outcome.stats.sent).unwrap_or(u32::MAX),
                    received: u32::try_from(outcome.stats.received).unwrap_or(u32::MAX),
                    error: None,
                }
            }
            Err(e) => SyncResult {
                pairing_id: peer.pairing_id.clone(),
                name: peer.name.clone(),
                sent: 0,
                received: 0,
                error: Some(e.to_string()),
            },
        }
    }

    /// LAN devices, paired or not.
    ///
    /// Never an error, and an empty list is a normal answer: the network may
    /// filter multicast. A client must offer "enter an address" as an equal
    /// path rather than as a fallback (manifest 04 §4.12).
    pub(super) fn discovered(&self) -> Vec<DiscoveredDevice> {
        self.node
            .seen()
            .into_iter()
            .map(|found| DiscoveredDevice {
                // Resolved locally rather than taken from the advertisement:
                // anyone on the LAN can claim any pairing id, and `paired`
                // decides whether the UI offers "pair" or "sync"
                // (`CopyPaste-vgpy`).
                paired: self.node.peers().get(&found.pairing_id).is_some(),
                addr: found.addr.to_string(),
                pairing_id: found.pairing_id,
                name: found.name,
                last_seen_ms: found.last_seen_ms,
            })
            .collect()
    }

    pub(super) fn republish(&self) {
        self.node.republish();
    }
}

/// The TCP port the peer listener binds.
const PORT: u16 = copypaste_p2p::DEFAULT_PORT;

/// This device's history, as a session sees it.
///
/// Built per operation, as the daemon builds it: it is three `Arc` clones and a
/// pool handle, and a source parked on the backend it reads is a cycle.
fn source(inner: &Arc<Inner>) -> StoreSource {
    StoreSource::new(
        inner.store.clone(),
        Arc::clone(&inner.keyring),
        Arc::clone(&inner.detector),
        inner.device_id.clone(),
        inner.device_name.clone(),
    )
}

/// Remember what the device on the other end of a session calls itself.
///
/// The origin column holds a device *id*, which is a UUID, so without this a
/// row that arrived from the laptop could be attributed but not named. Recorded
/// from both sides of a session, because either may be the first to speak to a
/// given device. Best-effort: a failure costs a label, not an item.
fn remember(inner: &Arc<Inner>, outcome: &SyncOutcome) {
    if let Err(e) = inner
        .store
        .record_device_name(&outcome.peer_device_id, &outcome.peer_device_name)
    {
        tracing::warn!(error = ?e, "could not record a peer device name");
    }
}

/// The one mapping from the node's failures onto this backend's taxonomy.
///
/// The node owns the sentences, so what a user reads does not depend on which
/// platform asked.
fn failed(error: NodeError) -> BackendError {
    match error {
        NodeError::NoPeer => BackendError::NotFound(MSG_NO_PEER),
        // `BackendError` serialises as its `Display`, so what a user reads is
        // the node's own sentence either way — the same words the daemon
        // backend renders for the same failure, which is the point of the node
        // owning them. `internal` scrubs, which costs nothing on text that has
        // no path in it.
        e => BackendError::internal(&e.to_string()),
    }
}

/// The device identity, minted on first run and then reused.
pub(super) fn identity(store: &copypaste_core::Store) -> Result<copypaste_core::DeviceIdentity> {
    store
        .device_identity(DEVICE_NAME_HINT)
        .map_err(|e| BackendError::internal(&format!("could not resolve this device: {e}")))
}
