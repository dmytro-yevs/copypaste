//! Manual sends reuse pairing authentication and revocation leases.
use super::channel::{NoiseChannel, SESSION_TIMEOUT};
use super::{ActiveSyncSource, Node};
use crate::{
    peers::Peer,
    protocol::ItemSummary,
    sync::{offer_for, receive_offer, send_offer, SyncError, SyncSource, SyncStats},
};

/// What one manual send to one device did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SendOutcome {
    /// Versions the policy let this device offer.
    pub offered: usize,
    /// Versions the receiver lacked and took.
    pub sent: usize,
}

impl Node {
    /// Send the chosen clips to one paired device now, whatever the automatic
    /// sync schedule or pause. The device sync policy still applies.
    ///
    /// # Errors
    ///
    /// [`SyncError`] when the device was never identified, is unreachable, or
    /// the exchange fails.
    pub async fn send_items<S: SyncSource>(
        &self,
        peer: &Peer,
        source: &S,
        item_ids: &[String],
    ) -> Result<SendOutcome, SyncError> {
        let peer = self
            .current_peer(peer)
            .ok_or_else(|| SyncError::Source("The device is no longer paired.".into()))?;
        let device_id = peer.device_id.clone().ok_or_else(|| {
            SyncError::Source("Sync with this device once before sending to it.".into())
        })?;
        let items = offer_for(source, &device_id, item_ids)?;
        if items.is_empty() {
            return Ok(SendOutcome::default());
        }
        let offered = items.len();
        let mut connected = None;
        for candidate in self.dial_candidates_with_sources(&peer, true) {
            if let Ok(session) = self.connect_peer(&candidate.endpoint, &peer).await {
                connected = Some(session);
                break;
            }
        }
        let (session, peer) = connected.ok_or_else(|| {
            SyncError::Source("The device is unavailable. Bring it online and try again.".into())
        })?;
        let active = self
            .begin_active_sync(&peer.pairing_id)
            .ok_or_else(|| SyncError::Source("The device is no longer paired.".into()))?;
        let cancel = active.cancelled();
        let source = ActiveSyncSource {
            source,
            state: std::sync::Arc::clone(&active.state),
        };
        let operation = async {
            let mut channel = NoiseChannel::new(session);
            let stats = send_offer(&mut channel, &source, items).await;
            channel.close().await;
            stats
        };
        let stats = tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(SyncError::Cancelled),
            result = tokio::time::timeout(SESSION_TIMEOUT, operation) => result
                .map_err(|_| SyncError::Source("The device stopped responding.".into()))?,
        }?;
        Ok(SendOutcome {
            offered,
            sent: stats.sent,
        })
    }

    /// Answer a manual send opened by `pairing_id` from `device_id`. No
    /// cursor moves.
    pub(super) async fn receive_sent<S: SyncSource>(
        &self,
        pairing_id: &str,
        source: &S,
        channel: &mut NoiseChannel,
        device_id: &str,
        items: Vec<ItemSummary>,
    ) -> Result<SyncStats, SyncError> {
        let active = self
            .begin_active_sync(pairing_id)
            .ok_or_else(|| SyncError::Source("The device is no longer paired.".into()))?;
        let cancel = active.cancelled();
        let source = ActiveSyncSource {
            source,
            state: std::sync::Arc::clone(&active.state),
        };
        let (stats, floor) = tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(SyncError::Cancelled),
            result = tokio::time::timeout(
                SESSION_TIMEOUT,
                receive_offer(channel, &source, device_id, items),
            ) => result
                .map_err(|_| SyncError::Source("The device stopped responding.".into()))?,
        }?;
        if let Some(floor) = floor {
            // Peers other than the sender do not have these versions yet.
            self.cursors().note_applied(pairing_id, floor);
        }
        Ok(stats)
    }
}
