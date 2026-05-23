/// Wire protocol messages exchanged between two peers during a sync session.
///
/// Encoding: each message is a length-prefixed JSON frame.
/// Frame format on the wire: `<u32 LE length><JSON bytes>`.
///
/// The exchange sequence is:
/// ```text
/// Initiator                     Responder
///    ─── HELLO ──────────────────────▶
///    ◀─────────────────────── HELLO ──
///    ─── HAVE (my item IDs) ─────────▶
///    ◀─────────────── HAVE (peer IDs) ─
///    ─── WANT (peer IDs I'm missing) ▶
///    ◀────────── ITEMS (requested) ───
///    ─── ITEMS (wanted by peer) ─────▶
///    ◀──────────────────────── DONE ──
///    ─── DONE ───────────────────────▶
/// ```
use serde::{Deserialize, Serialize};

/// Serialisable mirror of `ClipboardItem` carried over the wire.
///
/// We define a separate struct (rather than deriving Serialize on the core
/// type) so the network representation is decoupled from the storage layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WireItem {
    /// Row primary key (UUID string).
    pub id: String,
    /// Secondary item identifier (UUID string).
    pub item_id: String,
    /// MIME-like content type, e.g. `"text"`.
    pub content_type: String,
    /// Encrypted blob bytes, base64-encoded for JSON transport.
    pub content: Option<Vec<u8>>,
    /// Encryption nonce (24 bytes for ChaCha20-Poly1305).
    pub content_nonce: Option<Vec<u8>>,
    /// Optional large-blob reference (not carried inline).
    pub blob_ref: Option<String>,
    /// Whether the item was flagged as sensitive.
    pub is_sensitive: bool,
    /// Lamport timestamp at the time of last write.
    pub lamport_ts: i64,
    /// Wall-clock time (Unix ms) at the time of last write.
    pub wall_time: i64,
    /// Optional TTL expiry (Unix ms).
    pub expires_at: Option<i64>,
    /// Source app bundle ID.
    pub app_bundle_id: Option<String>,
    /// UUID of the device that originated this item.
    pub origin_device_id: String,
}

/// Top-level protocol message enum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Message {
    /// Handshake — first message sent by each peer.
    Hello {
        /// Sender's device UUID.
        device_id: String,
        /// Sender's current Lamport clock value.
        clock: u64,
        /// How many items the sender has locally.
        item_count: u64,
    },

    /// Sender announces the IDs and Lamport clocks of all items it currently holds.
    ///
    /// Each entry is `(item_id, lamport_ts)`. The receiver uses the Lamport
    /// timestamps to decide whether to request an update even for items it
    /// already has locally (conflict detection / LWW comparison).
    Have {
        /// `(id, lamport_ts)` pairs for all items the sender holds.
        items: Vec<(String, i64)>,
    },

    /// Sender requests the listed items from its peer.
    ///
    /// Includes items the sender doesn't have *at all*, plus items where the
    /// peer's Lamport clock is higher than the sender's local copy.
    Want {
        /// IDs the sender wants to receive (new or potentially outdated).
        item_ids: Vec<String>,
    },

    /// Sender delivers the requested items.
    Items { items: Vec<WireItem> },

    /// Sender signals it has finished and will not send more data.
    Done,
}

impl Message {
    /// Encode this message as a length-prefixed JSON frame.
    ///
    /// Format: `[u32 LE length][UTF-8 JSON bytes]`
    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        let json = serde_json::to_vec(self)?;
        let len = json.len() as u32;
        let mut buf = Vec::with_capacity(4 + json.len());
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(&json);
        Ok(buf)
    }

    /// Decode a message from its raw JSON bytes (without the length prefix).
    pub fn decode(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(msg: Message) -> Message {
        let encoded = msg.encode().expect("encode must not fail");
        // Skip the 4-byte length prefix.
        Message::decode(&encoded[4..]).expect("decode must not fail")
    }

    #[test]
    fn hello_round_trips() {
        let msg = Message::Hello {
            device_id: "dev-uuid-123".to_string(),
            clock: 42,
            item_count: 7,
        };
        assert_eq!(round_trip(msg.clone()), msg);
    }

    #[test]
    fn have_round_trips() {
        let msg = Message::Have {
            items: vec![("id-1".to_string(), 5), ("id-2".to_string(), 10)],
        };
        assert_eq!(round_trip(msg.clone()), msg);
    }

    #[test]
    fn want_round_trips() {
        let msg = Message::Want {
            item_ids: vec!["id-3".to_string()],
        };
        assert_eq!(round_trip(msg.clone()), msg);
    }

    #[test]
    fn items_round_trips() {
        let item = WireItem {
            id: "abc".to_string(),
            item_id: "def".to_string(),
            content_type: "text".to_string(),
            content: Some(vec![0x01, 0x02]),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 99,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "device-a".to_string(),
        };
        let msg = Message::Items { items: vec![item] };
        assert_eq!(round_trip(msg.clone()), msg);
    }

    #[test]
    fn done_round_trips() {
        let msg = Message::Done;
        assert_eq!(round_trip(msg.clone()), msg);
    }

    #[test]
    fn encode_has_correct_length_prefix() {
        let msg = Message::Done;
        let encoded = msg.encode().unwrap();
        let prefix = u32::from_le_bytes(encoded[..4].try_into().unwrap()) as usize;
        assert_eq!(prefix, encoded.len() - 4);
    }

    #[test]
    fn empty_want_round_trips() {
        let msg = Message::Want { item_ids: vec![] };
        assert_eq!(round_trip(msg.clone()), msg);
    }
}
