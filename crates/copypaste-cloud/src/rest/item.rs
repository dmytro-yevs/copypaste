//! The row shape, and what makes one valid to send.
//!
//! The base64 encoding lives here rather than at the call sites so that there
//! is one alphabet in the crate, and the validation lives here rather than in
//! the client so that a second request path cannot forget to run it.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};

use super::error::RestError;

/// One row as it travels to and from the backend.
///
/// `Debug` is derived: every field here is metadata or ciphertext, and none of
/// it is a secret the backend does not already hold. Plaintext never reaches
/// this type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudItem {
    /// Stable across devices; the upsert conflict key. Restricted to
    /// `[A-Za-z0-9_-]` so it can be placed in a PostgREST filter without
    /// quoting rules deciding what the filter means — see [`RestError::InvalidItem`].
    pub item_id: String,
    /// base64. The server cannot read this. Empty on a tombstone.
    pub ciphertext: String,
    /// base64.
    pub nonce: String,
    pub content_type: String,
    /// Version wall clock, ms since epoch. **Not the row's birth time**: it is
    /// the timestamp the poll cursor pages on, so a writer must restamp it on
    /// every mutation. A tombstone that kept the original creation time would
    /// sort below the watermark of every device that already saw the item, and
    /// the deletion would never propagate.
    pub created_at: i64,
    /// Tombstone flag. Always serialised, including `false` (manifest T-5).
    pub deleted: bool,
    pub origin_device_id: String,
}

impl CloudItem {
    /// A live row, from already-sealed bytes.
    ///
    /// The base64 encoding happens here so no caller has to pick an alphabet:
    /// one encoder, matching the one [`CloudItem::ciphertext_bytes`] decodes
    /// with.
    pub fn sealed(
        item_id: impl Into<String>,
        ciphertext: &[u8],
        nonce: &[u8],
        content_type: impl Into<String>,
        created_at: i64,
        origin_device_id: impl Into<String>,
    ) -> Self {
        Self {
            item_id: item_id.into(),
            ciphertext: BASE64.encode(ciphertext),
            nonce: BASE64.encode(nonce),
            content_type: content_type.into(),
            created_at,
            deleted: false,
            origin_device_id: origin_device_id.into(),
        }
    }

    /// A live row from a payload that is *already* base64, which is the shape
    /// [`crate::crypto::encrypt_row`] hands back.
    ///
    /// Note the argument order: `ciphertext` first, then `nonce`. The crypto
    /// side returns the pair the other way round, and a swap here would be
    /// silent — every row would encrypt fine and fail to open on the peer.
    pub fn from_sealed_b64(
        item_id: impl Into<String>,
        ciphertext_b64: impl Into<String>,
        nonce_b64: impl Into<String>,
        content_type: impl Into<String>,
        created_at: i64,
        origin_device_id: impl Into<String>,
    ) -> Self {
        Self {
            item_id: item_id.into(),
            ciphertext: ciphertext_b64.into(),
            nonce: nonce_b64.into(),
            content_type: content_type.into(),
            created_at,
            deleted: false,
            origin_device_id: origin_device_id.into(),
        }
    }

    /// A tombstone: a version of the item with the payload gone.
    ///
    /// Carrying no ciphertext is a hard precondition, not a nicety — a
    /// tombstone that shipped a stale payload would leak content the user
    /// deleted (manifest T-4).
    pub fn tombstone(
        item_id: impl Into<String>,
        content_type: impl Into<String>,
        created_at: i64,
        origin_device_id: impl Into<String>,
    ) -> Self {
        Self {
            item_id: item_id.into(),
            ciphertext: String::new(),
            nonce: String::new(),
            content_type: content_type.into(),
            created_at,
            deleted: true,
            origin_device_id: origin_device_id.into(),
        }
    }

    /// Decode the sealed payload.
    pub fn ciphertext_bytes(&self) -> Result<Vec<u8>, RestError> {
        BASE64
            .decode(&self.ciphertext)
            .map_err(|_| RestError::Malformed)
    }

    /// Decode the nonce.
    pub fn nonce_bytes(&self) -> Result<Vec<u8>, RestError> {
        BASE64.decode(&self.nonce).map_err(|_| RestError::Malformed)
    }

    /// Client-side preconditions, checked before anything is sent.
    pub(super) fn validate(&self) -> Result<(), RestError> {
        validate_item_id(&self.item_id)?;
        if self.deleted && !self.ciphertext.is_empty() {
            return Err(RestError::InvalidItem {
                reason: "a tombstone must not carry ciphertext",
            });
        }
        if !self.deleted && self.ciphertext.is_empty() {
            return Err(RestError::InvalidItem {
                reason: "a live item must carry ciphertext",
            });
        }
        if self.content_type.is_empty() || self.origin_device_id.is_empty() {
            return Err(RestError::InvalidItem {
                reason: "content_type and origin_device_id are required",
            });
        }
        Ok(())
    }
}

/// An `item_id` goes into a PostgREST filter (`item_id=in.(…)`), where quoting
/// and commas are syntax. Rather than inventing an escaping scheme, restrict
/// the identifier to characters that have no meaning there — uuids, which is
/// what these are, fit comfortably.
pub(super) fn validate_item_id(item_id: &str) -> Result<(), RestError> {
    if item_id.is_empty() {
        return Err(RestError::InvalidItem {
            reason: "item_id must not be empty",
        });
    }
    let ok = item_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(RestError::InvalidItem {
            reason: "item_id must be [A-Za-z0-9_-]",
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::testkit::item;
    use super::super::SELECT_COLUMNS;
    use super::*;

    #[test]
    fn sealed_bytes_round_trip_through_base64() {
        let ciphertext: Vec<u8> = (0u8..=255).collect();
        let nonce = [7u8; 12];
        let row = CloudItem::sealed("a1", &ciphertext, &nonce, "image", 1, "device-a");

        assert_eq!(row.ciphertext_bytes().expect("decode"), ciphertext);
        assert_eq!(row.nonce_bytes().expect("decode"), nonce);
        assert!(!row.deleted);
        // Standard alphabet with padding — one encoder for the whole crate.
        assert_eq!(row.nonce, BASE64.encode(nonce));
    }

    #[test]
    fn junk_base64_decodes_to_malformed_rather_than_panicking() {
        let mut row = CloudItem::sealed("a1", b"x", b"y", "text", 1, "device-a");
        row.ciphertext = "not base64 !!".to_string();
        assert!(matches!(row.ciphertext_bytes(), Err(RestError::Malformed)));
    }

    #[test]
    fn a_row_can_be_built_from_payloads_that_are_already_base64() {
        let sealed = CloudItem::sealed("a1", b"ct", b"nc", "text", 9, "device-a");
        let same = CloudItem::from_sealed_b64(
            "a1",
            BASE64.encode(b"ct"),
            BASE64.encode(b"nc"),
            "text",
            9,
            "device-a",
        );
        assert_eq!(sealed, same, "the two constructors must agree");
        same.validate().expect("valid row");
    }

    #[test]
    fn a_constructed_tombstone_carries_no_payload() {
        let row = CloudItem::tombstone("a1", "text", 5, "device-a");
        assert!(row.deleted);
        assert!(row.ciphertext.is_empty());
        assert!(row.nonce.is_empty());
        row.validate().expect("a tombstone is a valid row to send");
    }

    #[test]
    fn a_row_serialises_with_exactly_the_columns_the_table_has() {
        let json = serde_json::to_value(item("a1")).expect("serialise");
        let object = json.as_object().expect("object");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "ciphertext",
                "content_type",
                "created_at",
                "deleted",
                "item_id",
                "nonce",
                "origin_device_id",
            ]
        );
        let mut selected: Vec<&str> = SELECT_COLUMNS.split(',').collect();
        selected.sort_unstable();
        assert_eq!(keys, selected, "what we write is what we read back");
    }
}
