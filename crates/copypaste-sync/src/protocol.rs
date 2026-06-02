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
    /// HKDF key generation the `content` ciphertext + AAD were produced under
    /// (mirrors `ClipboardItem::key_version`). The sender stamps the row's real
    /// `key_version` (2 for every freshly-captured item); the receiver MUST
    /// persist this exact value so its read path (`decrypt_item_by_version`)
    /// selects the matching key + AAD. Carrying it over the wire is what makes
    /// a synced item decryptable on the receiver — see `merge::wire_to_local`.
    ///
    /// `#[serde(default = ...)]` keeps us wire-compatible with peers on a build
    /// that predates this field: an absent value defaults to
    /// [`default_key_version`] (= 2), the only version every supported build
    /// encrypts under today (the v4 sweep rotates all local rows to 2 and
    /// `merge::local_to_wire` stamps the row's real version for same-version
    /// peers). Defaulting to 1 would resurrect the original bug — decrypting a
    /// v2 ciphertext with the v1 key/AAD yields `AuthFailed`.
    #[serde(default = "default_key_version")]
    pub key_version: u8,
}

/// Default `key_version` for `WireItem`s deserialized from a peer that predates
/// the on-wire `key_version` field. See the field docs on [`WireItem`].
fn default_key_version() -> u8 {
    2
}

impl WireItem {
    /// Clamp `lamport_ts` and `wall_time` to be non-negative.
    ///
    /// Both fields are `i64` on the wire for JSON compatibility, but are
    /// semantically non-negative (they represent logical / Unix-ms timestamps).
    /// A malformed or hostile peer could send negative values; clamping at the
    /// decode boundary ensures no consumer ever sees a raw negative value,
    /// preventing silent sign-extension bugs when casting to `u64` for the
    /// Lamport clock or storing in the database.
    ///
    /// Call this once after deserialising a `WireItem` from the network, before
    /// any further processing.  See `engine.rs` ingest loop for usage.
    pub fn clamp_timestamps(&mut self) {
        if self.lamport_ts < 0 {
            self.lamport_ts = 0;
        }
        if self.wall_time < 0 {
            self.wall_time = 0;
        }
    }
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

    /// Sender announces the cross-device `item_id`s and Lamport clocks of all
    /// items it currently holds.
    ///
    /// Each entry is `(item_id, lamport_ts)` — the stable cross-device
    /// identity, NOT the per-row primary key `id` (which differs on every
    /// device). The receiver uses the Lamport timestamps to decide whether to
    /// request an update even for items it already has locally (conflict
    /// detection / LWW comparison).
    Have {
        /// `(item_id, lamport_ts)` pairs for all items the sender holds.
        items: Vec<(String, i64)>,
    },

    /// Sender requests the listed items from its peer, identified by their
    /// cross-device `item_id`s.
    ///
    /// Includes items the sender doesn't have *at all*, plus items where the
    /// peer's Lamport clock is higher than the sender's local copy.
    Want {
        /// `item_id`s the sender wants to receive (new or potentially outdated).
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
    ///
    /// Returns an error if the serialised payload exceeds `u32::MAX` bytes
    /// (≈ 4 GiB). Casting `json.len()` to `u32` without this guard would
    /// silently truncate the length prefix and corrupt every downstream read.
    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        let json = serde_json::to_vec(self)?;
        // Guard before casting: a payload larger than u32::MAX cannot be
        // represented in the 4-byte length prefix; serialise a descriptive
        // error rather than silently truncating the length.
        let json_len = json.len();
        if json_len > u32::MAX as usize {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("frame too large: {json_len} bytes exceeds u32::MAX"),
            )));
        }
        let len = json_len as u32;
        let mut buf = Vec::with_capacity(4 + json_len);
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
            key_version: 2,
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
