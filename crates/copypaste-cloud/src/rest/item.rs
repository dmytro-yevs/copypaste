//! The row shape, and what makes one valid to send.
//!
//! The base64 encoding lives here rather than at the call sites so that there
//! is one alphabet in the crate, and the validation lives here rather than in
//! the client so that a second request path cannot forget to run it.
//!
//! # Decoding is per row, never per page
//!
//! Three columns are decoded leniently: a JSON `null` or an absent field
//! becomes an empty string rather than a deserialisation failure. That is not
//! sloppiness, it is the difference between refusing one row and refusing a
//! page. `serde` fails the whole array on the first bad element, so a single
//! row with `"ciphertext": null` — which the table permits, and which anyone
//! who can write to the account can produce — would otherwise stall the cursor
//! for good. Empty is a value the ordinary checks already reject: an unsigned
//! row fails [`CloudItem::verify`] and a live row without ciphertext fails the
//! send-side validation below.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Deserializer, Serialize};

use super::error::RestError;
use crate::crypto::{sign_metadata, verify_metadata, CloudCryptoError, RowMetadata, SyncKey};

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
    #[serde(default, deserialize_with = "null_as_empty")]
    pub ciphertext: String,
    /// base64.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub nonce: String,
    pub content_type: String,
    /// File name and MIME metadata in JSON form. It is plaintext metadata, not
    /// content, and is authenticated with the rest of this row.
    #[serde(default)]
    pub payload_metadata: Option<String>,
    #[serde(default)]
    pub source_app_bundle_id: Option<String>,
    #[serde(default)]
    pub source_app_name: Option<String>,
    /// Version wall clock, ms since epoch. **Not the row's birth time**: it is
    /// the timestamp the poll cursor pages on, so a writer must restamp it on
    /// every mutation. A tombstone that kept the original creation time would
    /// sort below the watermark of every device that already saw the item, and
    /// the deletion would never propagate.
    pub created_at: i64,
    /// Tombstone flag. Always serialised, including `false` (manifest T-5).
    pub deleted: bool,
    pub origin_device_id: String,
    /// HMAC over every other column, under a key derived from the sync key.
    ///
    /// The backend can neither produce nor check it: it is what tells a
    /// receiving device that this version came from something holding the sync
    /// passphrase, rather than from something holding only the account password
    /// (manifest 05 §5.3). Set by [`CloudItem::sign`], required by the
    /// send-side validation, and checked by [`CloudItem::verify`] before a row
    /// reaches the merge.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub signature: String,
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
            payload_metadata: None,
            source_app_bundle_id: None,
            source_app_name: None,
            created_at,
            deleted: false,
            origin_device_id: origin_device_id.into(),
            signature: String::new(),
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
            payload_metadata: None,
            source_app_bundle_id: None,
            source_app_name: None,
            created_at,
            deleted: false,
            origin_device_id: origin_device_id.into(),
            signature: String::new(),
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
            payload_metadata: None,
            source_app_bundle_id: None,
            source_app_name: None,
            created_at,
            deleted: true,
            origin_device_id: origin_device_id.into(),
            signature: String::new(),
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

    /// Stamp the metadata signature. Call this on every row before it is sent;
    /// the send-side validation refuses one that has not been.
    pub fn sign(&mut self, key: &SyncKey) {
        self.signature = sign_metadata(key, &self.metadata());
    }

    /// Check the metadata signature.
    ///
    /// # Errors
    ///
    /// [`CloudCryptoError::SignatureInvalid`] if the row was not signed by a
    /// holder of the sync key. The caller must refuse the row — never merge it,
    /// never treat a refusal as a delete.
    pub fn verify(&self, key: &SyncKey) -> Result<(), CloudCryptoError> {
        verify_metadata(key, &self.metadata(), &self.signature)
    }

    /// The fields under the signature: every column except the signature
    /// itself. Built from `self`, so signing and verifying cannot read
    /// different rows.
    fn metadata(&self) -> RowMetadata<'_> {
        RowMetadata {
            item_id: &self.item_id,
            ciphertext: &self.ciphertext,
            nonce: &self.nonce,
            content_type: &self.content_type,
            payload_metadata: self.payload_metadata.as_deref(),
            source_app_bundle_id: self.source_app_bundle_id.as_deref(),
            source_app_name: self.source_app_name.as_deref(),
            created_at: self.created_at,
            deleted: self.deleted,
            origin_device_id: &self.origin_device_id,
        }
    }

    /// Client-side preconditions, checked before anything is sent.
    pub(super) fn validate(&self) -> Result<(), RestError> {
        validate_item_id(&self.item_id)?;
        // An unsigned row is refused here rather than at the receiver, where it
        // would look like an attack: every device verifies on arrival, so
        // uploading one would silently publish a version nothing can accept.
        if self.signature.is_empty() {
            return Err(RestError::InvalidItem {
                reason: "the row's metadata was never signed",
            });
        }
        if self.deleted && !self.ciphertext.is_empty() {
            return Err(RestError::InvalidItem {
                reason: "a tombstone must not carry ciphertext",
            });
        }
        if self.deleted && self.payload_metadata.is_some() {
            return Err(RestError::InvalidItem {
                reason: "a tombstone must not carry payload metadata",
            });
        }
        if self.deleted && (self.source_app_bundle_id.is_some() || self.source_app_name.is_some()) {
            return Err(RestError::InvalidItem {
                reason: "a tombstone must not carry source app metadata",
            });
        }
        if self.content_type == "file" && !self.deleted && self.payload_metadata.is_none() {
            return Err(RestError::InvalidItem {
                reason: "a file item must carry payload metadata",
            });
        }
        if let Some(metadata) = &self.payload_metadata {
            if self.content_type != "file"
                || metadata.len() > 1024
                || serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(metadata)
                    .is_err()
            {
                return Err(RestError::InvalidItem {
                    reason: "payload metadata must be bounded JSON for a file item",
                });
            }
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

/// `null` (or an absent field) reads as an empty string. See the module docs:
/// this is what keeps one bad row from failing a whole page.
fn null_as_empty<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
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

// Tests

#[cfg(test)]
mod tests {
    use super::super::testkit::{item, key};
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
        let mut same = CloudItem::from_sealed_b64(
            "a1",
            BASE64.encode(b"ct"),
            BASE64.encode(b"nc"),
            "text",
            9,
            "device-a",
        );
        assert_eq!(sealed, same, "the two constructors must agree");
        same.sign(&key());
        same.validate().expect("valid row");
    }

    #[test]
    fn a_constructed_tombstone_carries_no_payload() {
        let mut row = CloudItem::tombstone("a1", "text", 5, "device-a");
        assert!(row.deleted);
        assert!(row.ciphertext.is_empty());
        assert!(row.nonce.is_empty());
        row.sign(&key());
        row.validate().expect("a tombstone is a valid row to send");
    }

    #[test]
    fn an_unsigned_row_cannot_be_sent() {
        // The send-side half of the fail-closed rule: every device verifies on
        // arrival, so uploading an unsigned row would publish a version that
        // nothing — including this device — can accept.
        let row = CloudItem::sealed("a1", b"ct", b"nc", "text", 9, "device-a");
        assert!(row.signature.is_empty());
        assert!(matches!(row.validate(), Err(RestError::InvalidItem { .. })));
    }

    #[test]
    fn signing_covers_the_row_and_verification_refuses_a_tampered_one() {
        let row = item("a1");
        row.verify(&key()).expect("a freshly signed row verifies");

        // The attack of manifest 05 §5.3: restamp a version so it outranks the
        // real one. The signature is what makes that visible.
        let mut restamped = row.clone();
        restamped.created_at += 60_000;
        assert_eq!(
            restamped.verify(&key()),
            Err(CloudCryptoError::SignatureInvalid)
        );

        let mut forged_tombstone = row.clone();
        forged_tombstone.deleted = true;
        forged_tombstone.ciphertext = String::new();
        assert!(forged_tombstone.verify(&key()).is_err());

        let mut forged_metadata = row.clone();
        forged_metadata.payload_metadata =
            Some(r#"{"filename":"invoice.pdf","mime_type":"application/pdf"}"#.into());
        assert_eq!(
            forged_metadata.verify(&key()),
            Err(CloudCryptoError::SignatureInvalid)
        );
    }

    #[test]
    fn a_null_column_reads_as_empty_rather_than_failing_the_page() {
        // `serde` fails a whole array on its first bad element, so a single row
        // carrying `null` here would stall the cursor for good.
        let row: CloudItem = serde_json::from_str(
            r#"{"item_id":"a1","ciphertext":null,"nonce":null,"content_type":"text",
                "created_at":7,"deleted":true,"origin_device_id":"dev","signature":null}"#,
        )
        .expect("a null payload column must not fail deserialisation");
        assert!(row.ciphertext.is_empty());
        assert!(row.nonce.is_empty());
        assert!(row.signature.is_empty());
        // And it is still refused, by the check that is meant to refuse it.
        assert_eq!(row.verify(&key()), Err(CloudCryptoError::SignatureInvalid));
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
                "payload_metadata",
                "signature",
            ]
        );
        let mut selected: Vec<&str> = SELECT_COLUMNS.split(',').collect();
        selected.sort_unstable();
        assert_eq!(keys, selected, "what we write is what we read back");
    }

    #[test]
    fn a_tombstone_cannot_carry_file_metadata() {
        let mut row = CloudItem::tombstone("a1", "file", 5, "device-a");
        row.payload_metadata =
            Some(r#"{"filename":"report.pdf","mime_type":"application/pdf"}"#.into());
        row.sign(&key());
        assert!(matches!(row.validate(), Err(RestError::InvalidItem { .. })));
    }
}
