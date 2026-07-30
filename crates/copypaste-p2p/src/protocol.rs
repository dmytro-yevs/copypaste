//! The wire contract for one sync session.
//!
//! Five message types, five bounds, and nothing else. The session itself lives
//! in [`crate::sync`]; this module only says what may be said and how large it
//! may be.
//!
//! # Content is plaintext here
//!
//! [`SyncItem::content`] is the item's plaintext, for the reason set out in the
//! crate docs: the sender's ciphertext is sealed under a key the receiver does
//! not have, with the item id bound into the AEAD, so forwarding it could never
//! be opened. The confidentiality of this module's bytes is entirely the Noise
//! channel's job — nothing here should be written to disk or a log.
//!
//! # Why the bounds exist
//!
//! Every message arrives from a peer, and a peer is a program on the local
//! network that may be hostile or merely broken. Each bound below is checked at
//! the **decode boundary** rather than at the point of use, because a check that
//! lives in one consumer is a check the next consumer will silently skip
//! (manifest 05, rule R-CLK-1). [`SyncMessage::decode`] is the only ingress
//! path, so validating there covers every caller.
//!
//! The same validation runs on [`SyncMessage::encode`]. We do not send what we
//! would refuse to receive: a bug on this side should surface as a clear local
//! error, not as a peer dropping the connection.
//!
//! # Timestamps
//!
//! `created_at` is milliseconds since the Unix epoch and is semantically
//! non-negative. A hostile peer sending a negative value is a real historical
//! bug (`CopyPaste-psx7`): the value was later widened to an unsigned type and
//! became "larger than every legitimate timestamp forever". Decode **clamps** to
//! zero rather than rejecting — a clamped value simply loses the merge, which is
//! the safe outcome. Encode *rejects* a negative timestamp, because on the
//! outbound side it can only mean a local bug.
//!
//! The upper bound is not enforced here. It needs the local clock, and the
//! sensible response is to skip one item rather than to fail the whole message,
//! so it lives in [`crate::sync`] (see `MAX_FUTURE_SKEW_MS`).

use serde::{Deserialize, Serialize};

/// Version of the message set in this module.
///
/// Bumped whenever a change would confuse an older peer. There is no
/// negotiation and no compatibility shim: v2 has no old peers to be compatible
/// with, and a mismatch is a clear error rather than a degraded session.
pub const PROTOCOL_VERSION: u32 = 1;

/// Most summaries one [`SyncMessage::Summary`] may carry.
///
/// Sized for a full local history (the store caps itself well below this) at
/// roughly 200 bytes per summary — about 2 MiB on the wire. A peer with more
/// than this syncs its newest 10,000 items this session and the rest on the
/// next one; sessions repeat, so convergence is reached either way.
pub const MAX_SUMMARIES_PER_MESSAGE: usize = 10_000;

/// Most ids one [`SyncMessage::Request`] may carry, and therefore the most
/// items one session transfers in one direction.
///
/// Deliberately smaller than the summary bound: a request is a promise of
/// transfer work, and bounding it bounds how long one session can run. Leftover
/// wants are picked up by the next session.
pub const MAX_REQUEST_IDS_PER_MESSAGE: usize = 1_000;

/// Most items one [`SyncMessage::Items`] may carry.
///
/// Small on purpose. Items are the only messages that carry payloads, and the
/// receiver holds a whole message in memory before it can look at it.
pub const MAX_ITEMS_PER_MESSAGE: usize = 8;

/// Largest plaintext one item may carry.
///
/// A clipboard entry larger than this is pathological; v1 capped text uploads
/// at 8 MiB and this is a deliberate tightening, chosen so that one item always
/// fits inside [`MAX_ITEM_BYTES_PER_MESSAGE`] and a single oversized item can
/// never wedge a session.
pub const MAX_CONTENT_BYTES: usize = 4 * 1024 * 1024;

/// Content-byte budget for one [`SyncMessage::Items`], summed over its items.
///
/// [`MAX_ITEMS_PER_MESSAGE`] alone would allow eight maximal items in one
/// message; this caps the actual memory a single message can cost.
pub const MAX_ITEM_BYTES_PER_MESSAGE: usize = 4 * 1024 * 1024;

/// Hard ceiling on one encoded message, checked before any parsing.
///
/// Well above [`MAX_ITEM_BYTES_PER_MESSAGE`] because JSON string escaping
/// inflates: a payload of control characters becomes six bytes (`\u00XX`) per
/// byte. Eight times the content budget covers that worst case with room for
/// the envelope, while still refusing an unbounded stream before a single byte
/// is parsed.
pub const MAX_MESSAGE_BYTES: usize = 32 * 1024 * 1024;

/// Longest item id, device id, or origin device id.
pub const MAX_ID_BYTES: usize = 128;

/// Longest human-facing device name. Shown in the UI; not an identifier.
pub const MAX_DEVICE_NAME_BYTES: usize = 128;

/// Longest content-type string (`text`, `image`, …).
pub const MAX_CONTENT_TYPE_BYTES: usize = 64;

/// Longest content hash. A hex SHA-256 is 64.
pub const MAX_HASH_BYTES: usize = 128;

/// What one side knows about an item, without the content.
///
/// This is the whole input to the merge. `created_at` is the **version** stamp,
/// not the birth of the item: a tombstone carries the time of the deletion, and
/// that is what lets a delete beat the version it deleted (see
/// [`crate::sync::merge_decision`]).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemSummary {
    pub item_id: String,
    /// Milliseconds since the Unix epoch.
    pub created_at: i64,
    /// Tombstone. A deleted item is a *version* of the item, not the absence of
    /// one, so it takes part in the merge like any other version.
    pub deleted: bool,
    pub content_hash: String,
}

/// An item being transferred. `content` is plaintext — see the module docs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncItem {
    pub item_id: String,
    pub content: String,
    pub content_type: String,
    pub created_at: i64,
    pub deleted: bool,
    pub content_hash: String,
    /// The device the item was *first* captured on — never the forwarding
    /// device. Restamping this on every hop destroys the tie-break's
    /// determinism across a three-device circle.
    pub origin_device_id: String,
}

impl SyncItem {
    /// The metadata view of this item, which is all the merge ever sees.
    pub fn summary(&self) -> ItemSummary {
        ItemSummary {
            item_id: self.item_id.clone(),
            created_at: self.created_at,
            deleted: self.deleted,
            content_hash: self.content_hash.clone(),
        }
    }
}

/// One message on the wire.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum SyncMessage {
    Hello {
        protocol_version: u32,
        device_id: String,
        device_name: String,
    },
    Summary {
        items: Vec<ItemSummary>,
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

    /// Outbound only — inbound negative timestamps are clamped, not rejected.
    #[error("{field} is negative")]
    NegativeTimestamp { field: &'static str },

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

    /// Serialises to JSON, after checking every bound.
    ///
    /// JSON rather than a compact binary format on purpose: the volume is one
    /// clipboard history, the channel already compresses nothing and encrypts
    /// everything, and a readable frame is worth more than the bytes saved. If
    /// that ever stops being true, this is the only function that has to change.
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

    /// Raises every negative `created_at` to zero.
    ///
    /// Applies inside `Summary` *and* inside `Items`; a check that covered only
    /// the top level would miss the nested case, which is exactly how the
    /// original bug survived its first fix.
    fn clamp_timestamps(&mut self) {
        match self {
            Self::Summary { items } => {
                for s in items {
                    s.created_at = s.created_at.max(0);
                }
            }
            Self::Items { items } => {
                for i in items {
                    i.created_at = i.created_at.max(0);
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
            }

            Self::Summary { items } => {
                check_count("summary", items.len(), MAX_SUMMARIES_PER_MESSAGE)?;
                for s in items {
                    check_id("item_id", &s.item_id)?;
                    check_len("content_hash", &s.content_hash, MAX_HASH_BYTES)?;
                    check_timestamp(s.created_at)?;
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
                    check_len("content_hash", &i.content_hash, MAX_HASH_BYTES)?;
                    check_timestamp(i.created_at)?;
                    if i.content.len() > MAX_CONTENT_BYTES {
                        return Err(ProtocolError::ContentTooLarge {
                            bytes: i.content.len(),
                            max: MAX_CONTENT_BYTES,
                        });
                    }
                    total = total.saturating_add(i.content.len());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(id: &str) -> ItemSummary {
        ItemSummary {
            item_id: id.into(),
            created_at: 1_000,
            deleted: false,
            content_hash: "h".into(),
        }
    }

    fn item(id: &str, content: &str) -> SyncItem {
        SyncItem {
            item_id: id.into(),
            content: content.into(),
            content_type: "text".into(),
            created_at: 1_000,
            deleted: false,
            content_hash: "h".into(),
            origin_device_id: "dev-a".into(),
        }
    }

    fn hello() -> SyncMessage {
        SyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: "dev-a".into(),
            device_name: "Laptop".into(),
        }
    }

    #[test]
    fn every_variant_round_trips() {
        let messages = vec![
            hello(),
            SyncMessage::Summary {
                items: vec![summary("a"), summary("b")],
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
        let err = SyncMessage::Summary { items }.validate().unwrap_err();
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
        assert!(SyncMessage::Summary { items }.validate().is_ok());
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
            {"item_id":"a","created_at":-42,"deleted":false,"content_hash":"h"},
            {"item_id":"b","created_at":7,"deleted":false,"content_hash":"h"}]}"#;
        let SyncMessage::Summary { items } = SyncMessage::decode(raw).unwrap() else {
            panic!("wrong variant");
        };
        assert_eq!(items[0].created_at, 0, "negative was not clamped");
        assert_eq!(items[1].created_at, 7, "positive was altered");
    }

    #[test]
    fn negative_timestamps_are_clamped_inside_items_too() {
        let raw = br#"{"t":"items","items":[
            {"item_id":"a","content":"c","content_type":"text","created_at":-999,
             "deleted":false,"content_hash":"h","origin_device_id":"d"}]}"#;
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
            SyncMessage::Summary { items: vec![s] }.encode(),
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
