//! The wire contract for one sync session.
//!
//! What may be said and how large it may be; the session itself is in
//! [`crate::sync`].
//!
//! [`SyncItem::content`] is **plaintext** — the sender's ciphertext is sealed
//! under a key the receiver does not have, with the item id bound into the
//! AEAD, so forwarding it could never be opened. Confidentiality here is
//! entirely the Noise channel's job; nothing in this module should reach disk
//! or a log.
//!
//! # Why the bounds exist
//!
//! Every message arrives from a peer that may be hostile or merely broken.
//! Each bound is checked at the **decode boundary** rather than at the point of
//! use, because a check that lives in one consumer is one the next consumer
//! will silently skip (manifest 05, R-CLK-1); [`SyncMessage::decode`] is the
//! only ingress path. The same validation runs on [`SyncMessage::encode`], so a
//! bug on this side surfaces as a local error rather than a peer hanging up.
//!
//! # Timestamps
//!
//! `created_at` is milliseconds since the Unix epoch and semantically
//! non-negative. A hostile peer sending a negative value is a real historical
//! bug (`CopyPaste-psx7`): it was later widened to an unsigned type and became
//! "larger than every legitimate timestamp forever". Decode **clamps** to zero
//! — a clamped value simply loses the merge — while encode *rejects*, because
//! outbound it can only be a local bug. The upper bound needs the local clock
//! and should skip one item rather than fail a message, so it lives in
//! [`crate::sync`] (`MAX_FUTURE_SKEW_MS`).

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

/// Version of the message set. Bumped whenever a change would confuse an older
/// peer. No negotiation and no compatibility shim: a mismatch is a clear error
/// rather than a degraded session.
pub const PROTOCOL_VERSION: u32 = 2;

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

/// Largest plaintext one item may carry. A deliberate tightening of v1's 8 MiB,
/// chosen so one item always fits inside [`MAX_ITEM_BYTES_PER_MESSAGE`] and a
/// single oversized item can never wedge a session.
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
/// content bytes, never truncated (a v1 helper cut it to 16 bytes and threw
/// away second-preimage resistance for nothing, `CopyPaste-y4v1`).
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
    /// pretending it is UTF-8/base64 text; old peers fail the protocol-version
    /// check before they can misinterpret the field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binary_content: Vec<u8>,
    /// File metadata is structured JSON rather than a source path. It travels
    /// with the bytes so a receiving device can materialise a safe paste URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_metadata: Option<String>,
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
        listen_addr: Option<String>,
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

/// Wire-level failures.
///
/// No variant carries a filesystem path or any item content (CLAUDE.md rule 4,
/// and the log-hygiene rule in manifest 05 §4.7): these strings are shown to
/// users and written to logs.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    /// Fail closed. There is no fallback to an older message set.
    #[error("peer speaks sync protocol v{theirs}, this build speaks v{ours}")]
    VersionMismatch { ours: u32, theirs: u32 },

    #[error("message is {bytes} bytes, over the {max}-byte limit")]
    MessageTooLarge { bytes: usize, max: usize },

    #[error("{kind} message carries {count} entries, over the limit of {max}")]
    TooManyEntries {
        kind: &'static str,
        count: usize,
        max: usize,
    },

    #[error("item content is {bytes} bytes, over the {max}-byte limit")]
    ContentTooLarge { bytes: usize, max: usize },

    #[error("items message carries {bytes} content bytes, over the {max}-byte budget")]
    BatchTooLarge { bytes: usize, max: usize },

    #[error("{field} is {len} bytes, over the {max}-byte limit")]
    FieldTooLong {
        field: &'static str,
        len: usize,
        max: usize,
    },

    #[error("{field} is empty")]
    FieldEmpty { field: &'static str },

    #[error("hello message carries an invalid listen address")]
    InvalidListenAddr,

    /// Outbound only — inbound negative timestamps are clamped, not rejected.
    #[error("{field} is negative")]
    NegativeTimestamp { field: &'static str },

    #[error("pinned and pin_order are inconsistent")]
    InvalidPinState,

    #[error("message could not be encoded: {0}")]
    Encode(String),

    #[error("message could not be decoded: {0}")]
    Decode(String),
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

    /// Serialises to JSON, after checking every bound. JSON rather than a
    /// compact binary format on purpose: the volume is one clipboard history
    /// and a readable frame is worth more than the bytes saved. This is the
    /// only function that would have to change.
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|e| ProtocolError::Encode(e.to_string()))?;
        if bytes.len() > MAX_MESSAGE_BYTES {
            return Err(ProtocolError::MessageTooLarge {
                bytes: bytes.len(),
                max: MAX_MESSAGE_BYTES,
            });
        }
        Ok(bytes)
    }

    /// Parses a message and enforces every bound.
    ///
    /// The size check comes first, before the parser is handed anything, so an
    /// oversized frame costs nothing but the read that already happened.
    /// Negative timestamps are clamped to zero here — the one and only ingress
    /// point (manifest 05, R-CLK-1).
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() > MAX_MESSAGE_BYTES {
            return Err(ProtocolError::MessageTooLarge {
                bytes: bytes.len(),
                max: MAX_MESSAGE_BYTES,
            });
        }
        let mut msg: Self =
            serde_json::from_slice(bytes).map_err(|e| ProtocolError::Decode(e.to_string()))?;
        msg.clamp_timestamps();
        msg.validate()?;
        Ok(msg)
    }

    /// Raises every negative `created_at` to zero, inside `Summary` *and*
    /// inside `Items` — covering only the top level is how the original bug
    /// survived its first fix.
    fn clamp_timestamps(&mut self) {
        match self {
            Self::Summary { items, .. } => {
                for s in items {
                    s.created_at = s.created_at.max(0);
                    s.pin_updated_at = s.pin_updated_at.max(0);
                }
            }
            Self::Items { items } => {
                for i in items {
                    i.created_at = i.created_at.max(0);
                    i.pin_updated_at = i.pin_updated_at.max(0);
                }
            }
            _ => {}
        }
    }

    /// Checks every bound in this module. Called on both encode and decode.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::Hello {
                protocol_version,
                device_id,
                device_name,
                listen_addr,
            } => {
                // Fail closed, and do it before anything else in the message is
                // trusted: a differing version means the fields below may not
                // mean what this build thinks they mean.
                if *protocol_version != PROTOCOL_VERSION {
                    return Err(ProtocolError::VersionMismatch {
                        ours: PROTOCOL_VERSION,
                        theirs: *protocol_version,
                    });
                }
                check_id("device_id", device_id)?;
                check_len("device_name", device_name, MAX_DEVICE_NAME_BYTES)?;
                if let Some(addr) = listen_addr {
                    check_len("listen_addr", addr, MAX_LISTEN_ADDR_BYTES)?;
                    if addr.parse::<SocketAddr>().is_err() {
                        return Err(ProtocolError::InvalidListenAddr);
                    }
                }
            }

            Self::Summary { items, .. } => {
                check_count("summary", items.len(), MAX_SUMMARIES_PER_MESSAGE)?;
                for s in items {
                    check_id("item_id", &s.item_id)?;
                    check_id("origin_device_id", &s.origin_device_id)?;
                    check_len("content_hash", &s.content_hash, MAX_HASH_BYTES)?;
                    check_timestamp(s.created_at)?;
                    check_timestamp(s.pin_updated_at)?;
                    check_pin_state(s.pinned, s.pin_order)?;
                }
            }

            Self::Request { item_ids } => {
                check_count("request", item_ids.len(), MAX_REQUEST_IDS_PER_MESSAGE)?;
                for id in item_ids {
                    check_id("item_id", id)?;
                }
            }

            Self::Items { items } => {
                check_count("items", items.len(), MAX_ITEMS_PER_MESSAGE)?;
                let mut total = 0usize;
                for i in items {
                    check_id("item_id", &i.item_id)?;
                    check_id("origin_device_id", &i.origin_device_id)?;
                    check_len("content_type", &i.content_type, MAX_CONTENT_TYPE_BYTES)?;
                    if let Some(metadata) = &i.payload_metadata {
                        check_len("payload_metadata", metadata, 512)?;
                    }
                    check_len("content_hash", &i.content_hash, MAX_HASH_BYTES)?;
                    check_timestamp(i.created_at)?;
                    check_timestamp(i.pin_updated_at)?;
                    check_pin_state(i.pinned, i.pin_order)?;
                    if !i.content.is_empty() && !i.binary_content.is_empty() {
                        return Err(ProtocolError::Decode(
                            "item has both text and binary content".into(),
                        ));
                    }
                    let content_len = i.content.len().saturating_add(i.binary_content.len());
                    if content_len > MAX_CONTENT_BYTES {
                        return Err(ProtocolError::ContentTooLarge {
                            bytes: content_len,
                            max: MAX_CONTENT_BYTES,
                        });
                    }
                    total = total.saturating_add(content_len);
                }
                if total > MAX_ITEM_BYTES_PER_MESSAGE {
                    return Err(ProtocolError::BatchTooLarge {
                        bytes: total,
                        max: MAX_ITEM_BYTES_PER_MESSAGE,
                    });
                }
            }

            Self::Done => {}
        }
        Ok(())
    }
}

fn check_count(kind: &'static str, count: usize, max: usize) -> Result<(), ProtocolError> {
    if count > max {
        return Err(ProtocolError::TooManyEntries { kind, count, max });
    }
    Ok(())
}

/// Identifiers are non-empty and bounded. An empty id is never legitimate and
/// would silently match nothing (or, worse, everything) downstream.
fn check_id(field: &'static str, value: &str) -> Result<(), ProtocolError> {
    if value.is_empty() {
        return Err(ProtocolError::FieldEmpty { field });
    }
    check_len(field, value, MAX_ID_BYTES)
}

fn check_len(field: &'static str, value: &str, max: usize) -> Result<(), ProtocolError> {
    if value.len() > max {
        return Err(ProtocolError::FieldTooLong {
            field,
            len: value.len(),
            max,
        });
    }
    Ok(())
}

fn check_timestamp(created_at: i64) -> Result<(), ProtocolError> {
    if created_at < 0 {
        return Err(ProtocolError::NegativeTimestamp {
            field: "created_at",
        });
    }
    Ok(())
}

fn check_pin_state(pinned: bool, pin_order: Option<f64>) -> Result<(), ProtocolError> {
    if pin_order.is_some_and(|order| !order.is_finite()) || (pinned != pin_order.is_some()) {
        return Err(ProtocolError::InvalidPinState);
    }
    Ok(())
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

    fn summary(id: &str) -> ItemSummary {
        ItemSummary {
            item_id: id.into(),
            created_at: 1_000,
            deleted: false,
            content_hash: "h".into(),
            origin_device_id: "dev-a".into(),
            pinned: false,
            pin_order: None,
            pin_updated_at: 0,
        }
    }

    fn item(id: &str, content: &str) -> SyncItem {
        SyncItem {
            item_id: id.into(),
            content: content.into(),
            binary_content: Vec::new(),
            payload_metadata: None,
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
    fn binary_payloads_round_trip_without_a_text_encoding() {
        let bytes = vec![0, 0xff, 7, 0x80];
        let message = SyncMessage::Items {
            items: vec![SyncItem {
                item_id: "binary".into(),
                content: String::new(),
                binary_content: bytes.clone(),
                payload_metadata: Some(
                    r#"{\"filename\":\"image.png\",\"mime_type\":\"image/png\"}"#.into(),
                ),
                content_type: "image/png".into(),
                created_at: 1,
                deleted: false,
                content_hash: plaintext_content_hash(&bytes),
                origin_device_id: "device-a".into(),
                pinned: false,
                pin_order: None,
                pin_updated_at: 0,
            }],
        };
        let decoded = SyncMessage::decode(&message.encode().unwrap()).unwrap();
        assert_eq!(decoded, message);
    }

    fn hello() -> SyncMessage {
        SyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: "dev-a".into(),
            device_name: "Laptop".into(),
            listen_addr: None,
        }
    }

    #[test]
    fn every_variant_round_trips() {
        let messages = vec![
            hello(),
            SyncMessage::Summary {
                items: vec![summary("a"), summary("b")],
                more: false,
            },
            SyncMessage::Request {
                item_ids: vec!["a".into()],
            },
            SyncMessage::Items {
                items: vec![item("a", "hello")],
            },
            SyncMessage::Done,
        ];
        for m in messages {
            let bytes = m.encode().expect("encode");
            let back = SyncMessage::decode(&bytes).expect("decode");
            assert_eq!(m, back, "round trip changed a {} message", m.kind());
        }
    }

    #[test]
    fn tag_is_stable_on_the_wire() {
        // The tag is part of the contract; renaming a variant must be a
        // deliberate protocol-version bump, not an accident of refactoring.
        let bytes = SyncMessage::Done.encode().unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), r#"{"t":"done"}"#);
    }

    #[test]
    fn hello_with_a_different_version_is_rejected() {
        let raw = br#"{"t":"hello","protocol_version":99,"device_id":"d","device_name":"n"}"#;
        assert_eq!(
            SyncMessage::decode(raw),
            Err(ProtocolError::VersionMismatch {
                ours: PROTOCOL_VERSION,
                theirs: 99
            })
        );
    }

    #[test]
    fn version_mismatch_message_names_both_versions() {
        let err = ProtocolError::VersionMismatch {
            ours: 1,
            theirs: 99,
        };
        let text = err.to_string();
        assert!(text.contains("v99") && text.contains("v1"), "{text}");
    }

    #[test]
    fn too_many_summaries_is_rejected() {
        let items = vec![summary("a"); MAX_SUMMARIES_PER_MESSAGE + 1];
        let err = SyncMessage::Summary { items, more: false }
            .validate()
            .unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::TooManyEntries {
                kind: "summary",
                ..
            }
        ));
    }

    #[test]
    fn a_full_summary_message_is_accepted() {
        let items = vec![summary("a"); MAX_SUMMARIES_PER_MESSAGE];
        assert!(SyncMessage::Summary { items, more: false }
            .validate()
            .is_ok());
    }

    #[test]
    fn too_many_requested_ids_is_rejected() {
        let item_ids = vec!["a".to_string(); MAX_REQUEST_IDS_PER_MESSAGE + 1];
        let err = SyncMessage::Request { item_ids }.validate().unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::TooManyEntries {
                kind: "request",
                ..
            }
        ));
    }

    #[test]
    fn too_many_items_is_rejected() {
        let items = vec![item("a", "x"); MAX_ITEMS_PER_MESSAGE + 1];
        let err = SyncMessage::Items { items }.validate().unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::TooManyEntries { kind: "items", .. }
        ));
    }

    #[test]
    fn oversized_content_is_rejected() {
        let big = "x".repeat(MAX_CONTENT_BYTES + 1);
        let err = SyncMessage::Items {
            items: vec![item("a", &big)],
        }
        .validate()
        .unwrap_err();
        assert!(matches!(err, ProtocolError::ContentTooLarge { .. }));
    }

    #[test]
    fn a_batch_over_the_byte_budget_is_rejected() {
        // Each item is legal on its own; together they are not.
        let half = "x".repeat(MAX_ITEM_BYTES_PER_MESSAGE / 2 + 1);
        let err = SyncMessage::Items {
            items: vec![item("a", &half), item("b", &half)],
        }
        .validate()
        .unwrap_err();
        assert!(matches!(err, ProtocolError::BatchTooLarge { .. }));
    }

    #[test]
    fn one_maximal_item_still_fits_a_message() {
        // The relationship the batch budget depends on: no single legal item
        // can be too large to send.
        const { assert!(MAX_CONTENT_BYTES <= MAX_ITEM_BYTES_PER_MESSAGE) };
    }

    #[test]
    fn an_oversized_frame_is_refused_without_parsing() {
        let bytes = vec![b'x'; MAX_MESSAGE_BYTES + 1];
        assert!(matches!(
            SyncMessage::decode(&bytes),
            Err(ProtocolError::MessageTooLarge { .. })
        ));
    }

    #[test]
    fn negative_timestamps_are_clamped_on_decode() {
        let raw = br#"{"t":"summary","items":[
            {"item_id":"a","created_at":-42,"deleted":false,"content_hash":"h","origin_device_id":"d","pinned":false,"pin_order":null},
            {"item_id":"b","created_at":7,"deleted":false,"content_hash":"h","origin_device_id":"d","pinned":false,"pin_order":null}],"more":false}"#;
        let SyncMessage::Summary { items, .. } = SyncMessage::decode(raw).unwrap() else {
            panic!("wrong variant");
        };
        assert_eq!(items[0].created_at, 0, "negative was not clamped");
        assert_eq!(items[1].created_at, 7, "positive was altered");
    }

    #[test]
    fn negative_timestamps_are_clamped_inside_items_too() {
        let raw = br#"{"t":"items","items":[
            {"item_id":"a","content":"c","content_type":"text","created_at":-999,
             "deleted":false,"content_hash":"h","origin_device_id":"d","pinned":false,"pin_order":null}]}"#;
        let SyncMessage::Items { items } = SyncMessage::decode(raw).unwrap() else {
            panic!("wrong variant");
        };
        assert_eq!(items[0].created_at, 0);
    }

    #[test]
    fn encoding_a_negative_timestamp_is_a_local_error() {
        let mut s = summary("a");
        s.created_at = -1;
        assert_eq!(
            SyncMessage::Summary {
                items: vec![s],
                more: false
            }
            .encode(),
            Err(ProtocolError::NegativeTimestamp {
                field: "created_at"
            })
        );
    }

    #[test]
    fn empty_identifiers_are_rejected() {
        for msg in [
            SyncMessage::Summary {
                items: vec![summary("")],
                more: false,
            },
            SyncMessage::Request {
                item_ids: vec![String::new()],
            },
            SyncMessage::Items {
                items: vec![item("", "c")],
            },
            SyncMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                device_id: String::new(),
                device_name: "n".into(),
                listen_addr: None,
            },
        ] {
            assert!(
                matches!(msg.validate(), Err(ProtocolError::FieldEmpty { .. })),
                "empty id accepted in a {} message",
                msg.kind()
            );
        }
    }

    #[test]
    fn overlong_identifiers_are_rejected() {
        let long = "x".repeat(MAX_ID_BYTES + 1);
        let err = SyncMessage::Summary {
            items: vec![summary(&long)],
            more: false,
        }
        .validate()
        .unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::FieldTooLong {
                field: "item_id",
                ..
            }
        ));
    }

    #[test]
    fn an_overlong_device_name_is_rejected() {
        let err = SyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: "d".into(),
            device_name: "x".repeat(MAX_DEVICE_NAME_BYTES + 1),
            listen_addr: None,
        }
        .validate()
        .unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::FieldTooLong {
                field: "device_name",
                ..
            }
        ));
    }

    #[test]
    fn a_hello_rejects_a_malformed_or_oversized_listen_address() {
        let malformed = SyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: "d".into(),
            device_name: "n".into(),
            listen_addr: Some("not-an-endpoint".into()),
        };
        assert_eq!(malformed.validate(), Err(ProtocolError::InvalidListenAddr));

        let oversized = SyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: "d".into(),
            device_name: "n".into(),
            listen_addr: Some("1".repeat(MAX_LISTEN_ADDR_BYTES + 1)),
        };
        assert!(matches!(
            oversized.validate(),
            Err(ProtocolError::FieldTooLong {
                field: "listen_addr",
                ..
            })
        ));
    }

    #[test]
    fn a_hello_from_before_endpoint_advertising_still_decodes() {
        let raw = format!(
            r#"{{"t":"hello","protocol_version":{PROTOCOL_VERSION},"device_id":"d","device_name":"n"}}"#
        );
        let decoded = SyncMessage::decode(raw.as_bytes()).unwrap();
        assert!(matches!(
            decoded,
            SyncMessage::Hello {
                listen_addr: None,
                ..
            }
        ));
    }

    #[test]
    fn an_overlong_origin_device_id_is_rejected() {
        let mut i = item("a", "c");
        i.origin_device_id = "x".repeat(MAX_ID_BYTES + 1);
        let err = SyncMessage::Items { items: vec![i] }
            .validate()
            .unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::FieldTooLong {
                field: "origin_device_id",
                ..
            }
        ));
    }

    #[test]
    fn errors_disclose_no_content_and_no_path() {
        // Error text reaches logs and the UI. It may name a field and a size;
        // it may never name a value.
        let secret = "hunter2-hunter2".repeat(MAX_CONTENT_BYTES / 8);
        let err = SyncMessage::Items {
            items: vec![item("a", &secret)],
        }
        .validate()
        .unwrap_err();
        let text = err.to_string();
        assert!(!text.contains("hunter2"), "{text}");
        assert!(!text.contains('/'), "{text}");
    }

    #[test]
    fn malformed_json_is_a_decode_error_not_a_panic() {
        assert!(matches!(
            SyncMessage::decode(b"{"),
            Err(ProtocolError::Decode(_))
        ));
        assert!(matches!(
            SyncMessage::decode(b"{\"t\":\"nope\"}"),
            Err(ProtocolError::Decode(_))
        ));
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
