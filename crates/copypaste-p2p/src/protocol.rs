//! Wire data for one sync session; session orchestration lives in [`crate::sync`].
//!
//! [`SyncItem::content`] is plaintext inside the Noise channel. The sender's
//! ciphertext is sealed under a device-local key with the item id in its AEAD,
//! so forwarding it would produce data the receiver cannot open. Nothing in
//! this module should reach disk or a log.

use serde::{Deserialize, Serialize};

use crate::DeviceProfile;

mod codec;

pub use codec::ProtocolError;

/// Version of the message set. Bumped whenever two builds would disagree on a
/// field. No negotiation and no compatibility shim: a mismatch is a clear error
/// rather than a degraded session.
pub const PROTOCOL_VERSION: u32 = 3;

/// Most summaries one [`SyncMessage::Summary`] may carry. Sized for a full
/// local history at roughly 200 bytes each, about 2 MiB on the wire. A history
/// larger than this is sent as consecutive, lock-step summary pages; it is
/// never truncated to the newest page.
pub const MAX_SUMMARIES_PER_MESSAGE: usize = 10_000;

/// Aggregate summary ceiling for one session. Per-frame limits alone do not
/// stop a paired but hostile peer from keeping `more = true` forever.
pub const MAX_SUMMARY_PAGES_PER_SESSION: usize = 16;

/// Most ids one [`SyncMessage::Request`] may carry, and so the most items one
/// session transfers in one direction. Smaller than the summary bound: a
/// request is a promise of transfer work, and bounding it bounds how long a
/// session can run. Leftover wants go to the next session.
pub const MAX_REQUEST_IDS_PER_MESSAGE: usize = 1_000;

/// Most items one [`SyncMessage::Items`] may carry. Small on purpose: items are
/// the only messages carrying payloads, and the receiver holds a whole message
/// in memory before it can look at it.
pub const MAX_ITEMS_PER_MESSAGE: usize = 8;

/// Largest plaintext one item may carry. Chosen so one item always fits inside
/// [`MAX_ITEM_BYTES_PER_MESSAGE`] and cannot wedge a session by itself.
pub const MAX_CONTENT_BYTES: usize = 4 * 1024 * 1024;

/// Content-byte budget for one [`SyncMessage::Items`], summed over its items.
///
/// [`MAX_ITEMS_PER_MESSAGE`] alone would allow eight maximal items in one
/// message; this caps the actual memory a single message can cost.
pub const MAX_ITEM_BYTES_PER_MESSAGE: usize = 4 * 1024 * 1024;

/// Hard ceiling on one encoded message, checked before any parsing. Well above
/// [`MAX_ITEM_BYTES_PER_MESSAGE`] because JSON escaping inflates control
/// characters to six bytes (`\u00XX`) each; 8× the content budget covers that
/// worst case and still refuses an unbounded stream before parsing.
pub const MAX_MESSAGE_BYTES: usize = 32 * 1024 * 1024;

/// Longest item id, device id, or origin device id.
pub const MAX_ID_BYTES: usize = 128;

/// Longest human-facing device name. Shown in the UI; not an identifier.
pub const MAX_DEVICE_NAME_BYTES: usize = 128;

/// Longest numeric `host:port` an authenticated peer may advertise.
pub const MAX_LISTEN_ADDR_BYTES: usize = 64;

/// Longest content-type string (`text`, `image`, …).
pub const MAX_CONTENT_TYPE_BYTES: usize = 64;

/// Longest content hash. A hex SHA-256 is 64.
pub const MAX_HASH_BYTES: usize = 128;

/// What one side knows about an item, without the content — the whole input to
/// the merge. `created_at` is the **version** stamp, not the birth of the item:
/// a tombstone carries the time of the deletion, which is what lets a delete
/// beat the version it deleted ([`crate::sync::merge_decision`]).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemSummary {
    pub item_id: String,
    /// Milliseconds since the Unix epoch.
    pub created_at: i64,
    /// Tombstone. A deleted item is a *version* of the item, not the absence of
    /// one, so it takes part in the merge like any other version.
    pub deleted: bool,
    pub content_hash: String,
    /// The final LWW tie-break. It has to be present here, not only on the
    /// payload, because planning otherwise cannot distinguish two otherwise
    /// equal versions and the replicas can keep opposite winners forever.
    pub origin_device_id: String,
    /// Pin state is part of the version on P2P. It is authenticated by the
    /// Noise record carrying this summary and the matching item payload.
    pub pinned: bool,
    /// Explicit `None` clears a previous order on unpin; omission would retain
    /// stale state at the receiver.
    pub pin_order: Option<f64>,
    /// P2P-only LWW stamp for pin state. It deliberately differs from
    /// `created_at`: cloud has no pin fields, so a pin must not republish an
    /// older content version.
    #[serde(default)]
    pub pin_updated_at: i64,
}

/// The wire definition of content identity: lowercase hex SHA-256 of the
/// content bytes, never truncated (`CopyPaste-y4v1`).
///
#[must_use]
pub fn content_hash(content: &str) -> String {
    plaintext_content_hash(content.as_bytes())
}

/// The shared plaintext identity for P2P transfer and local deduplication.
#[must_use]
pub fn plaintext_content_hash(content: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(content))
}

pub use plaintext_content_hash as content_hash_bytes;

/// An item being transferred. `content` is plaintext — see the module docs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncItem {
    pub item_id: String,
    /// UTF-8 text payload. Empty for a binary payload or tombstone.
    pub content: String,
    /// Raw binary payload. JSON serialises this as a byte array rather than
    /// pretending it is UTF-8/base64 text. Incompatible message sets fail the
    /// protocol-version check before interpreting it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binary_content: Vec<u8>,
    /// File metadata is structured JSON rather than a source path. It travels
    /// with the bytes so a receiving device can materialise a safe paste URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_metadata: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_app_bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_app_name: Option<String>,
    pub content_type: String,
    pub created_at: i64,
    pub deleted: bool,
    /// [`content_hash`] of `content` for a live item; for a tombstone, the hash
    /// of the item it deletes. **A peer's word, until it is checked** — the
    /// receiving session recomputes it, because this is comparator key 2 and a
    /// hostile peer that controls it can force a dedup collision on a targeted
    /// item.
    pub content_hash: String,
    /// The device the item was *first* captured on — never the forwarding
    /// device. Restamping this on every hop destroys the tie-break's
    /// determinism across a three-device circle.
    pub origin_device_id: String,
    /// See [`ItemSummary::pinned`].
    pub pinned: bool,
    /// See [`ItemSummary::pin_order`].
    pub pin_order: Option<f64>,
    #[serde(default)]
    pub pin_updated_at: i64,
}

impl SyncItem {
    /// The metadata view of this item, which is all the merge ever sees.
    pub fn summary(&self) -> ItemSummary {
        ItemSummary {
            item_id: self.item_id.clone(),
            created_at: self.created_at,
            deleted: self.deleted,
            content_hash: self.content_hash.clone(),
            origin_device_id: self.origin_device_id.clone(),
            pinned: self.pinned,
            pin_order: self.pin_order,
            pin_updated_at: self.pin_updated_at,
        }
    }
}

/// One message on the wire.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum SyncMessage {
    Hello {
        protocol_version: u32,
        device_id: String,
        device_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<DeviceProfile>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        listen_addr: Option<String>,
        #[serde(default)]
        since_ms: i64,
    },
    Summary {
        items: Vec<ItemSummary>,
        /// More pages follow from this peer. Both sides exchange one page per
        /// turn, so a large history cannot fill the channel before either end
        /// reads.
        more: bool,
    },
    Request {
        item_ids: Vec<String>,
    },
    Items {
        items: Vec<SyncItem>,
    },
    Done,
}

impl SyncMessage {
    /// A short name for this variant, for error messages and tracing. Never
    /// includes any field value.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Hello { .. } => "hello",
            Self::Summary { .. } => "summary",
            Self::Request { .. } => "request",
            Self::Items { .. } => "items",
            Self::Done => "done",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_is_the_full_sha256_hex() {
        assert_eq!(
            content_hash(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            content_hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(content_hash("abc").len() <= MAX_HASH_BYTES);
    }

    fn item(id: &str, content: &str) -> SyncItem {
        SyncItem {
            item_id: id.into(),
            content: content.into(),
            binary_content: Vec::new(),
            payload_metadata: None,
            source_app_bundle_id: None,
            source_app_name: None,
            content_type: "text".into(),
            created_at: 1_000,
            deleted: false,
            content_hash: "h".into(),
            origin_device_id: "dev-a".into(),
            pinned: false,
            pin_order: None,
            pin_updated_at: 0,
        }
    }

    #[test]
    fn summary_view_of_an_item_matches_its_fields() {
        let i = item("a", "c");
        let s = i.summary();
        assert_eq!(s.item_id, i.item_id);
        assert_eq!(s.created_at, i.created_at);
        assert_eq!(s.deleted, i.deleted);
        assert_eq!(s.content_hash, i.content_hash);
    }
}
