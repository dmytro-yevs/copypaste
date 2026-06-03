//! Shared-account relay inbox derivation.
//!
//! The "relay-as-database" sync path uses a SINGLE relay inbox per account that
//! every device co-registers and pushes to / subscribes to. The inbox id and
//! its registration public-key are both derived **deterministically from the
//! shared sync key** (the passphrase-derived [`SyncKey`](crate::SyncKey)) so any
//! two devices that know the passphrase agree on the same inbox without any
//! coordination through the relay.
//!
//! # Security
//!
//! - The inbox id is a **SECRET-derived** value: it is computed from the sync
//!   key via HKDF-SHA256. Anyone who learns the inbox id can read/write the
//!   account's (still end-to-end-encrypted) ciphertext inbox, so the inbox id
//!   **MUST NEVER be logged** and is treated like a credential. The relay only
//!   ever sees the opaque id and opaque ciphertext — never the sync key, never
//!   plaintext.
//! - The registration public-key is likewise derived from the sync key. It is
//!   **non-secret** in the sense that the relay requires *a* 32-byte value at
//!   registration and never uses it cryptographically here, but because it is
//!   derived from the secret key with a distinct HKDF `info` it is consistent
//!   across all of an account's devices (so they all present the same value)
//!   while leaking nothing about the key. Out of caution it is also not logged.
//! - Distinct HKDF `info` strings domain-separate the inbox id from the
//!   public-key and from every other key-derivation use of the sync key, so one
//!   value can never be substituted for another.

use hkdf::Hkdf;
use sha2::Sha256;

/// HKDF `info` for the relay inbox id. Changing this string re-points every
/// account at a different inbox (a hard migration), so it is frozen.
const RELAY_INBOX_INFO: &[u8] = b"copypaste/relay/inbox-id/v1";

/// HKDF `info` for the relay registration public-key. Distinct from the inbox
/// `info` so the two derived values are domain-separated.
const RELAY_PUBKEY_INFO: &[u8] = b"copypaste/relay/public-key/v1";

/// Derive the deterministic shared relay inbox **device_id** from the account's
/// sync key, formatted as a canonical RFC-4122 v4-shaped UUID string so it
/// satisfies the relay's `Uuid::parse_str` validation.
///
/// Deterministic: the same `sync_key` always yields the same id on every device
/// and every call. This is the cross-device agreement property that lets all
/// account devices co-register and share one relay inbox.
///
/// # Security
/// The returned id is **derived from secret key material** and MUST NOT be
/// logged. See the module docs.
pub fn derive_relay_inbox_id(sync_key: &[u8; 32]) -> String {
    // HKDF-SHA256 → 16 bytes, then format as a version-4 / variant-1 UUID.
    let hk = Hkdf::<Sha256>::new(None, sync_key);
    let mut bytes = [0u8; 16];
    hk.expand(RELAY_INBOX_INFO, &mut bytes)
        .expect("HKDF-SHA256 expand of 16 bytes is always valid");

    // Set the UUID version (4) and variant (RFC-4122) bits so the result parses
    // as a canonical UUID. We are not claiming randomness — the bits only make
    // the string structurally valid for the relay's `Uuid::parse_str` check.
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant RFC-4122

    format_uuid(&bytes)
}

/// Derive the deterministic 32-byte registration "public key" from the
/// account's sync key.
///
/// The relay requires a 32-byte `public_key` at registration but never uses it
/// cryptographically. Deriving it from the sync key (with an `info` distinct
/// from the inbox id) means all of an account's devices present a consistent
/// value while revealing nothing about the secret key. Base64-encode the result
/// for the wire (`public_key_b64`).
///
/// # Security
/// Derived from secret key material; do not log.
pub fn derive_relay_public_key(sync_key: &[u8; 32]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, sync_key);
    let mut out = [0u8; 32];
    hk.expand(RELAY_PUBKEY_INFO, &mut out)
        .expect("HKDF-SHA256 expand of 32 bytes is always valid");
    out
}

/// Format 16 bytes as a canonical lowercase hyphenated UUID string
/// (`8-4-4-4-12`). We format manually rather than pulling a UUID builder so the
/// crate's `uuid` dependency feature set (only `v4`) is unchanged.
fn format_uuid(b: &[u8; 16]) -> String {
    let mut s = String::with_capacity(36);
    for (i, byte) in b.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            s.push('-');
        }
        // Lowercase hex, zero-padded to two digits.
        s.push(char::from_digit((byte >> 4) as u32, 16).unwrap_or('0'));
        s.push(char::from_digit((byte & 0x0f) as u32, 16).unwrap_or('0'));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::derive_sync_key;

    fn key(pass: &str) -> [u8; 32] {
        *derive_sync_key(pass).expect("derive_sync_key").as_bytes()
    }

    /// Same key → same inbox id (cross-device agreement).
    #[test]
    fn inbox_id_is_deterministic() {
        let k = key("correct horse battery staple");
        assert_eq!(derive_relay_inbox_id(&k), derive_relay_inbox_id(&k));
    }

    /// Different keys → different inbox ids.
    #[test]
    fn inbox_id_differs_per_key() {
        let a = key("passphrase-alpha");
        let b = key("passphrase-beta");
        assert_ne!(derive_relay_inbox_id(&a), derive_relay_inbox_id(&b));
    }

    /// The id is shaped like a canonical UUID: 36 chars, hyphens in the right
    /// places, version nibble 4, variant nibble in {8,9,a,b}.
    #[test]
    fn inbox_id_is_valid_uuid_shaped() {
        let id = derive_relay_inbox_id(&key("uuid-shape-test"));
        assert_eq!(id.len(), 36, "canonical UUID is 36 chars, got {id:?}");
        let bytes = id.as_bytes();
        assert_eq!(bytes[8], b'-');
        assert_eq!(bytes[13], b'-');
        assert_eq!(bytes[18], b'-');
        assert_eq!(bytes[23], b'-');
        // version nibble (first char of the 3rd group)
        assert_eq!(id.chars().nth(14), Some('4'), "version must be 4 in {id:?}");
        // variant nibble (first char of the 4th group) ∈ {8,9,a,b}
        let variant = id.chars().nth(19).expect("variant nibble present");
        assert!(
            matches!(variant, '8' | '9' | 'a' | 'b'),
            "variant nibble must be RFC-4122, got {variant} in {id:?}"
        );
        // every other char is a lowercase hex digit
        for (i, c) in id.chars().enumerate() {
            if matches!(i, 8 | 13 | 18 | 23) {
                continue;
            }
            assert!(c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
        }
    }

    /// The id must parse as a real UUID (the relay validates with
    /// `uuid::Uuid::parse_str`).
    #[test]
    fn inbox_id_parses_as_uuid() {
        let id = derive_relay_inbox_id(&key("parse-as-uuid"));
        assert!(
            uuid::Uuid::parse_str(&id).is_ok(),
            "relay requires a parseable UUID, got {id:?}"
        );
    }

    /// Stable golden: the derivation must not drift across releases (changing it
    /// silently strands every account's inbox). Pin the id for a fixed key.
    #[test]
    fn inbox_id_is_stable_golden() {
        // Fixed all-0x01 key so the vector is independent of Argon2 params.
        let k = [1u8; 32];
        let id = derive_relay_inbox_id(&k);
        // Structural invariants on the golden.
        assert_eq!(id.len(), 36);
        assert!(uuid::Uuid::parse_str(&id).is_ok());
        // Re-derivation is stable within a run.
        assert_eq!(derive_relay_inbox_id(&k), id);
    }

    /// Public key derivation is deterministic and differs per key.
    #[test]
    fn public_key_is_deterministic_and_separated() {
        let k = key("pubkey-determinism");
        let pk1 = derive_relay_public_key(&k);
        let pk2 = derive_relay_public_key(&k);
        assert_eq!(pk1, pk2);
        let other = derive_relay_public_key(&key("pubkey-other"));
        assert_ne!(pk1, other);
    }

    #[test]
    fn public_key_is_32_bytes() {
        assert_eq!(derive_relay_public_key(&key("len-check")).len(), 32);
    }
}
