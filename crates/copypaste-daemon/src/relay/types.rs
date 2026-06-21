//! Wire-protocol types: envelopes, request/response bodies, error variants, and
//! the public `RelayHandle` / `RelayError` surface.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Notify;

// ── Wire envelope ─────────────────────────────────────────────────────────────

/// The decoded `content_b64` envelope. `content_b64` (on the relay wire) is
/// `base64(JSON(this struct))`; `ct_b64` inside it is
/// `base64(encrypt_for_cloud(sync_key, item_id, wrapped_plaintext))` — the SAME
/// blob the Supabase path stores. This is the exact shape the Android SSE
/// receiver already decodes.
///
/// CopyPaste-cm0u / CopyPaste-ayvs / CopyPaste-bfiu: the envelope now also
/// carries `deleted` / `pinned` / `pin_order` (so deletes and pins propagate
/// over relay-only topologies) and `wall_time` / `origin_device_id` (so relay
/// LWW uses the SAME total order as P2P/cloud). All five are
/// `#[serde(default)]` OPTIONAL-by-omission fields: an envelope written by an
/// older daemon omits them and decodes to `deleted=false` / `pinned=false` /
/// `pin_order=None` / `wall_time=0` / `origin_device_id=""` — i.e. exactly the
/// pre-fix behaviour (a live, unpinned item with no origin tie-break key).
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct RelayEnvelope {
    pub(super) item_id: String,
    pub(super) lamport_ts: i64,
    /// Present for live items; a tombstone envelope sets `deleted=true` and
    /// carries an empty `ct_b64` (the content is NULL — there is nothing to
    /// decrypt). Defaulted empty so older live envelopes (no field) parse.
    #[serde(default)]
    pub(super) ct_b64: String,
    /// Soft-delete flag. Omitted (=> false) by older daemons.
    #[serde(default)]
    pub(super) deleted: bool,
    /// Pin flag. Omitted (=> false) by older daemons.
    #[serde(default)]
    pub(super) pinned: bool,
    /// Pin sort order. Omitted (=> None) by older daemons.
    #[serde(default)]
    pub(super) pin_order: Option<f64>,
    /// Wall-clock ms — the second LWW tie-break key. Omitted (=> 0) by older
    /// daemons, which makes them lose every equal-lamport tie (acceptable: the
    /// pre-fix relay path had no wall_time tie-break at all).
    #[serde(default)]
    pub(super) wall_time: i64,
    /// Originating device id — the final LWW tie-break key. Omitted (=> "") by
    /// older daemons.
    #[serde(default)]
    pub(super) origin_device_id: String,
}

/// Relay register request body.
#[derive(Debug, Serialize)]
pub(super) struct RegisterBody {
    pub(super) device_id: String,
    pub(super) device_name: String,
    pub(super) public_key_b64: String,
    /// HMAC-SHA256(sync_key, "relay-registration-pop-v1:" || device_id) base64-encoded.
    /// Proves the registrant holds the sync key matching the derived inbox id — fixes CopyPaste-n2l.
    pub(super) pop_b64: String,
}

/// Relay register response (we only need the token).
#[derive(Debug, Deserialize)]
pub(super) struct RegisterResp {
    pub(super) auth_token: String,
}

/// Relay push request body.
#[derive(Debug, Serialize)]
pub(super) struct PushBody {
    pub(super) content_type: String,
    pub(super) content_b64: String,
    pub(super) wall_time: u64,
}

/// One element of the pull response array.
#[derive(Debug, Deserialize)]
pub(super) struct PullItem {
    pub(super) id: i64,
    pub(super) content_type: String,
    pub(super) content_b64: String,
    pub(super) wall_time: u64,
}

// ── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    /// The configured `relay_url` is not a usable HTTPS (or loopback in tests) URL.
    #[error("relay_url is not a valid https URL")]
    InvalidUrl,
    /// `relay_url` was explicitly cleared (set to `""` sentinel) — relay is disabled.
    ///
    /// Returned by [`start_relay`] when the caller passes an empty string, and by
    /// [`relay_url_is_clear`] so the `set_config` IPC handler can detect the sentinel
    /// and shut down a running [`RelayHandle`] without needing to know URL internals.
    ///
    /// [`start_relay`]: super::start_relay
    /// [`relay_url_is_clear`]: super::relay_url_is_clear
    #[error("relay_url cleared — relay sync disabled")]
    Disabled,
    /// Network / transport failure talking to the relay.
    #[error("relay request failed: {0}")]
    Transport(String),
    /// Relay returned an unexpected non-success status.
    #[error("relay returned status {0}")]
    Status(u16),
    /// Could not resolve the inbox id (no sync key set).
    #[error("no sync passphrase set — relay sync inactive")]
    NoSyncKey,
}

// ── Handle ──────────────────────────────────────────────────────────────────

/// Handle to the running relay orchestrator. Drop (or call [`shutdown`]) to stop
/// the push and receive loops.
///
/// [`shutdown`]: RelayHandle::shutdown
pub struct RelayHandle {
    pub(super) shutdown: Arc<Notify>,
}

impl RelayHandle {
    /// Signal both loops to stop. Idempotent.
    pub fn shutdown(self) {
        self.shutdown.notify_waiters();
    }
}

impl Drop for RelayHandle {
    fn drop(&mut self) {
        self.shutdown.notify_waiters();
    }
}
