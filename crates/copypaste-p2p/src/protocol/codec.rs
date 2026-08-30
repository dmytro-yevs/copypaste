//! JSON codec and fail-closed validation for sync wire messages.
//!
//! Every peer-controlled bound is enforced at `SyncMessage::decode`, the only
//! ingress path. Encoding applies the same policy so local defects fail before
//! a peer waits on an invalid frame. Negative timestamps are clamped on decode
//! at this boundary (`CopyPaste-psx7`, manifest 05 R-CLK-1); outbound negatives
//! are rejected because they can only be a local defect.

use std::net::SocketAddr;

use crate::DeviceProfile;

use super::{
    SyncMessage, MAX_CONTENT_BYTES, MAX_CONTENT_TYPE_BYTES, MAX_DEVICE_NAME_BYTES, MAX_HASH_BYTES,
    MAX_ID_BYTES, MAX_ITEMS_PER_MESSAGE, MAX_ITEM_BYTES_PER_MESSAGE, MAX_LISTEN_ADDR_BYTES,
    MAX_MESSAGE_BYTES, MAX_REQUEST_IDS_PER_MESSAGE, MAX_SUMMARIES_PER_MESSAGE, PROTOCOL_VERSION,
};

/// Wire-level failures.
///
/// No variant carries a filesystem path or item content because these strings
/// reach logs and user-facing errors (manifest 05 §4.7).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    /// Fail closed. There is no fallback to another message set.
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

    /// Outbound only; inbound negative timestamps are clamped.
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
    /// Serialises to JSON after checking every bound.
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

    /// Parses a message and enforces every bound before returning it.
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

    /// Covers timestamps in both summaries and payloads; checking only the top
    /// level is the defect fixed by `CopyPaste-psx7`.
    fn clamp_timestamps(&mut self) {
        match self {
            Self::Summary { items, .. } => {
                for summary in items {
                    summary.created_at = summary.created_at.max(0);
                    summary.pin_updated_at = summary.pin_updated_at.max(0);
                }
            }
            Self::Items { items } => {
                for item in items {
                    item.created_at = item.created_at.max(0);
                    item.pin_updated_at = item.pin_updated_at.max(0);
                }
            }
            Self::Hello { since_ms, .. } => *since_ms = (*since_ms).max(0),
            _ => {}
        }
    }

    /// Checks every bound applied by both encode and decode.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::Hello {
                protocol_version,
                device_id,
                device_name,
                profile,
                listen_addr,
                since_ms,
            } => {
                // The version check precedes all fields because a mismatch
                // means those fields may not have the semantics assumed here.
                if *protocol_version != PROTOCOL_VERSION {
                    return Err(ProtocolError::VersionMismatch {
                        ours: PROTOCOL_VERSION,
                        theirs: *protocol_version,
                    });
                }
                check_id("device_id", device_id)?;
                check_len("device_name", device_name, MAX_DEVICE_NAME_BYTES)?;
                if let Some(profile) = profile {
                    validate_device_profile(profile)?;
                }
                if *since_ms < 0 {
                    return Err(ProtocolError::NegativeTimestamp { field: "since_ms" });
                }
                if let Some(addr) = listen_addr {
                    check_len("listen_addr", addr, MAX_LISTEN_ADDR_BYTES)?;
                    if addr.parse::<SocketAddr>().is_err() {
                        return Err(ProtocolError::InvalidListenAddr);
                    }
                }
            }
            Self::Summary { items, .. } => {
                check_count("summary", items.len(), MAX_SUMMARIES_PER_MESSAGE)?;
                for summary in items {
                    check_id("item_id", &summary.item_id)?;
                    check_id("origin_device_id", &summary.origin_device_id)?;
                    check_len("content_hash", &summary.content_hash, MAX_HASH_BYTES)?;
                    check_timestamp(summary.created_at)?;
                    check_timestamp(summary.pin_updated_at)?;
                    check_pin_state(summary.pinned, summary.pin_order)?;
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
                for item in items {
                    check_id("item_id", &item.item_id)?;
                    check_id("origin_device_id", &item.origin_device_id)?;
                    check_len("content_type", &item.content_type, MAX_CONTENT_TYPE_BYTES)?;
                    if let Some(metadata) = &item.payload_metadata {
                        check_len("payload_metadata", metadata, 512)?;
                    }
                    check_len("content_hash", &item.content_hash, MAX_HASH_BYTES)?;
                    check_timestamp(item.created_at)?;
                    check_timestamp(item.pin_updated_at)?;
                    check_pin_state(item.pinned, item.pin_order)?;
                    if !item.content.is_empty() && !item.binary_content.is_empty() {
                        return Err(ProtocolError::Decode(
                            "item has both text and binary content".into(),
                        ));
                    }
                    let content_len = item.content.len().saturating_add(item.binary_content.len());
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

fn validate_device_profile(profile: &DeviceProfile) -> Result<(), ProtocolError> {
    for (field, value, max) in [
        ("app_version", profile.app_version.as_deref(), 64),
        ("os_name", profile.os_name.as_deref(), 64),
        ("os_version", profile.os_version.as_deref(), 64),
        ("model", profile.model.as_deref(), 128),
    ] {
        if let Some(value) = value {
            check_len(field, value, max)?;
        }
    }
    Ok(())
}

fn check_count(kind: &'static str, count: usize, max: usize) -> Result<(), ProtocolError> {
    if count > max {
        return Err(ProtocolError::TooManyEntries { kind, count, max });
    }
    Ok(())
}

/// Empty identifiers could silently match nothing or everything downstream.
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
    use crate::protocol::{
        plaintext_content_hash, ItemSummary, SyncItem, SyncMessage, MAX_CONTENT_BYTES,
        MAX_DEVICE_NAME_BYTES, MAX_ID_BYTES, MAX_ITEMS_PER_MESSAGE, MAX_ITEM_BYTES_PER_MESSAGE,
        MAX_LISTEN_ADDR_BYTES, MAX_MESSAGE_BYTES, MAX_REQUEST_IDS_PER_MESSAGE,
        MAX_SUMMARIES_PER_MESSAGE, PROTOCOL_VERSION,
    };
    use crate::DeviceProfile;

    use super::ProtocolError;

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

    fn hello() -> SyncMessage {
        SyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: "dev-a".into(),
            device_name: "Laptop".into(),
            profile: Some(DeviceProfile::current()),
            listen_addr: None,
            since_ms: 0,
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
                source_app_bundle_id: None,
                source_app_name: None,
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
        for message in messages {
            let bytes = message.encode().expect("encode");
            let decoded = SyncMessage::decode(&bytes).expect("decode");
            assert_eq!(
                message,
                decoded,
                "round trip changed a {} message",
                message.kind()
            );
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
    fn version_check_precedes_optional_field_defaults() {
        let raw = br#"{"t":"hello","protocol_version":2,"device_id":"d","device_name":"n"}"#;
        assert_eq!(
            SyncMessage::decode(raw),
            Err(ProtocolError::VersionMismatch {
                ours: PROTOCOL_VERSION,
                theirs: 2
            })
        );
    }

    #[test]
    fn version_mismatch_message_names_both_versions() {
        let err = ProtocolError::VersionMismatch {
            ours: 7,
            theirs: 99,
        };
        let text = err.to_string();
        assert!(text.contains("v99") && text.contains("v7"), "{text}");
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
    fn every_content_shape_has_an_inclusive_payload_boundary() {
        // Test inputs, not a second content-type policy: the IPC owner
        // classifies these shapes for all transports.
        for (name, content_type, binary, metadata) in [
            ("text", "text", false, None),
            ("rtf", "text/rtf", false, None),
            ("html", "text/html", false, None),
            ("image", "image/png", true, None),
            (
                "file",
                "file",
                true,
                Some(r#"{"filename":"report.pdf","mime_type":"application/pdf"}"#),
            ),
            ("future", "application/x-future", true, None),
        ] {
            let mut payload = vec![0; MAX_CONTENT_BYTES];
            if binary {
                payload[..3].copy_from_slice(&[0, 0xff, 0x80]);
            }
            let mut at_limit = item(name, "");
            at_limit.content_type = content_type.into();
            at_limit.payload_metadata = metadata.map(str::to_owned);
            if binary {
                at_limit.binary_content = payload.clone();
                at_limit.content_hash = plaintext_content_hash(&payload);
            } else {
                at_limit.content = String::from_utf8(payload.clone()).unwrap();
                at_limit.content_hash = plaintext_content_hash(&payload);
            }
            assert!(
                SyncMessage::Items {
                    items: vec![at_limit.clone()]
                }
                .validate()
                .is_ok(),
                "{name} at the cap"
            );

            if binary {
                at_limit.binary_content.push(1);
            } else {
                at_limit.content.push('x');
            }
            assert!(
                matches!(
                    SyncMessage::Items {
                        items: vec![at_limit]
                    }
                    .validate(),
                    Err(ProtocolError::ContentTooLarge { .. })
                ),
                "{name} one byte over must be a content error"
            );
        }
    }

    #[test]
    fn nul_text_at_the_content_cap_round_trips_within_the_frame_cap() {
        let content = "\0".repeat(MAX_CONTENT_BYTES);
        let message = SyncMessage::Items {
            items: vec![SyncItem {
                item_id: "nul-text".into(),
                content_hash: plaintext_content_hash(content.as_bytes()),
                content,
                binary_content: Vec::new(),
                payload_metadata: None,
                source_app_bundle_id: None,
                source_app_name: None,
                content_type: "text".into(),
                created_at: 1,
                deleted: false,
                origin_device_id: "device-a".into(),
                pinned: false,
                pin_order: None,
                pin_updated_at: 0,
            }],
        };
        let bytes = message.encode().expect("NUL text encodes");
        assert!(
            bytes.len() <= MAX_MESSAGE_BYTES,
            "JSON expansion exceeded frame cap"
        );
        assert_eq!(
            SyncMessage::decode(&bytes).expect("NUL text decodes"),
            message
        );
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
        let mut summary = summary("a");
        summary.created_at = -1;
        assert_eq!(
            SyncMessage::Summary {
                items: vec![summary],
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
        for message in [
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
                profile: None,
                listen_addr: None,
                since_ms: 0,
            },
        ] {
            assert!(
                matches!(message.validate(), Err(ProtocolError::FieldEmpty { .. })),
                "empty id accepted in a {} message",
                message.kind()
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
            profile: None,
            listen_addr: None,
            since_ms: 0,
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
            profile: None,
            listen_addr: Some("not-an-endpoint".into()),
            since_ms: 0,
        };
        assert_eq!(malformed.validate(), Err(ProtocolError::InvalidListenAddr));

        let oversized = SyncMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            device_id: "d".into(),
            device_name: "n".into(),
            profile: None,
            listen_addr: Some("1".repeat(MAX_LISTEN_ADDR_BYTES + 1)),
            since_ms: 0,
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
    fn omitted_optional_hello_fields_use_wire_defaults() {
        let raw = format!(
            r#"{{"t":"hello","protocol_version":{PROTOCOL_VERSION},"device_id":"d","device_name":"n"}}"#
        );
        let decoded = SyncMessage::decode(raw.as_bytes()).unwrap();
        assert!(matches!(
            decoded,
            SyncMessage::Hello {
                listen_addr: None,
                since_ms: 0,
                ..
            }
        ));
    }

    #[test]
    fn an_overlong_origin_device_id_is_rejected() {
        let mut item = item("a", "c");
        item.origin_device_id = "x".repeat(MAX_ID_BYTES + 1);
        let err = SyncMessage::Items { items: vec![item] }
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
}
