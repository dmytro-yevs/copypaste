//! Post-handshake device-metadata and sync-provisioning exchange (P2P Phase 4).
//!
//! [`exchange_peer_meta`] runs symmetrically on both endpoints AFTER the 9-frame
//! PAKE + channel-binding handshake has fully completed. All errors are swallowed
//! (pairing already succeeded; metadata is best-effort).

use tokio_util::codec::{Framed, LengthDelimitedCodec};

use super::framing::{recv_frame, send_frame, MAX_META_BYTES, MAX_PROVISIONING_BYTES};
use super::types::{PeerMeta, SyncProvisioning};
use crate::bootstrap::BOOTSTRAP_PROTO_VERSION;

/// Minimum protocol version a peer must advertise (in the frame-10 version byte)
/// to participate in the [`SyncProvisioning`] exchange (frames 12/13). A peer
/// advertising less than this — or no version frame at all — is treated as
/// not-provisioning-capable and the step is skipped with `peer_provisioning =
/// None` (back-compat).
pub(super) const SYNC_PROVISIONING_MIN_VERSION: u8 = 2;

/// Exchange optional device metadata over the framed stream AFTER the PAKE
/// handshake has fully completed.
///
/// Symmetric on both endpoints (so it cannot deadlock): each side SENDS its own
/// version byte then its metadata JSON, then RECEIVES the peer's version byte
/// and metadata. Sending first, before any receive, keeps the two sides in
/// lock-step over the duplex stream.
///
/// Back-compat: a legacy peer terminates the protocol at frame 9 and never reads
/// or writes these frames. When we try to receive its version frame the stream
/// is closed → `recv_frame` errors → we return [`PeerMeta::default`] (all
/// `None`). Likewise an explicit version `< BOOTSTRAP_PROTO_VERSION` skips the
/// metadata read. ALL errors are swallowed: pairing already succeeded, so a
/// metadata hiccup must never turn it into a failure.
pub(super) async fn exchange_peer_meta<S>(
    framed: &mut Framed<S, LengthDelimitedCodec>,
    own_meta: &PeerMeta,
    own_provisioning: Option<&SyncProvisioning>,
) -> (PeerMeta, Option<SyncProvisioning>)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    // ── Send half (always send-first to stay in lock-step over the duplex) ──
    //
    // Frame 10: our version byte. Frame 11: our metadata JSON. When we advertise
    // proto >= 2 we ALSO send frame 12: our sync-provisioning JSON. The version
    // byte tells the peer whether to expect frame 12, so a v1 peer never reads
    // it. Swallow send errors (a legacy peer may have closed the read half).
    if send_frame(framed, &[BOOTSTRAP_PROTO_VERSION])
        .await
        .is_err()
    {
        return (PeerMeta::default(), None);
    }
    let own_json = serde_json::to_vec(own_meta).unwrap_or_default();
    if send_frame(framed, &own_json).await.is_err() {
        return (PeerMeta::default(), None);
    }
    // Frame 12 (proto >= 2): our sync-provisioning JSON. We always send a frame
    // when our advertised version supports it — an unconfigured side sends an
    // all-`None` value so the peer's read stays in lock-step. NOTE: the JSON is
    // produced via serde; `serde_json::to_vec` does not log field values, so the
    // secret `derived_sync_key` is never written to a log here.
    if BOOTSTRAP_PROTO_VERSION >= SYNC_PROVISIONING_MIN_VERSION {
        let prov = own_provisioning.cloned().unwrap_or_default();
        let prov_json = serde_json::to_vec(&prov).unwrap_or_default();
        if send_frame(framed, &prov_json).await.is_err() {
            // We already sent meta; treat a provisioning send failure as "no
            // provisioning exchange" but still return whatever meta we read.
            // Fall through to the receive half so we can still learn peer meta.
        }
    }

    // ── Receive half ──
    //
    // Frame 10 ← peer version byte. Absent / malformed → legacy peer.
    let peer_version = match recv_frame(framed).await {
        Ok(bytes) if bytes.len() == 1 => bytes[0],
        _ => return (PeerMeta::default(), None),
    };
    if peer_version < 1 {
        // Should not happen (version 0 is never advertised); be defensive.
        return (PeerMeta::default(), None);
    }

    // Frame 11 ← peer metadata JSON.
    let peer_meta = match recv_frame(framed).await {
        Ok(b) if b.len() <= MAX_META_BYTES => {
            serde_json::from_slice::<PeerMeta>(&b).unwrap_or_default()
        }
        _ => return (PeerMeta::default(), None),
    };

    // Frame 12 ← peer sync-provisioning JSON — ONLY when the peer advertised a
    // version that includes it. A v1 (or unknown-lower) peer never sent it, so
    // we must NOT try to read it (that would desync the stream); we return
    // `None` for provisioning and the meta we already learned. This is the
    // version-gated back-compat, mirroring the additive `PeerMeta` pattern.
    if peer_version < SYNC_PROVISIONING_MIN_VERSION {
        return (peer_meta, None);
    }
    let peer_provisioning = match recv_frame(framed).await {
        Ok(b) if b.len() <= MAX_PROVISIONING_BYTES => {
            serde_json::from_slice::<SyncProvisioning>(&b).ok()
        }
        // A missing/oversized/garbled provisioning frame must not fail the
        // already-complete pairing — just yield no provisioning.
        _ => None,
    };
    (peer_meta, peer_provisioning)
}
