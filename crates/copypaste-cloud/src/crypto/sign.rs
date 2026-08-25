//! Authenticating the metadata the merge orders on.
//!
//! # What this is for
//!
//! The AEAD makes a row's *content* unreadable and unforgeable. It does nothing
//! for the row's *metadata*, which travels in the clear because the backend has
//! to sort and page on it. Manifest 05 §5.3 records the consequence: an attacker
//! who holds the account password but not the sync passphrase can write rows.
//! They cannot produce content for an `item_id` — the AAD binds it — but they
//! can stamp a competing version of one, and `created_at` is the merge's first
//! key. A version they invent therefore outranks the real one on every device,
//! which silently censors that item everywhere. A forged tombstone is the same
//! attack with the payload set to "gone".
//!
//! Encryption cannot fix an ordering attack. A MAC can: every field the merge
//! reads is signed under a key derived from the sync passphrase, and a row that
//! was not produced by a passphrase holder is refused before it reaches the
//! comparator.
//!
//! # What is under the signature
//!
//! Every column a client writes, in this order:
//!
//! ```text
//! item_id | content_type | payload_metadata | source_app_bundle_id | source_app_name | origin_device_id | created_at | deleted | nonce | ciphertext
//! ```
//!
//! * `created_at`, `deleted` and `origin_device_id` are what the comparator
//!   reads (`CloudSource::apply_remote`: `created_at`, then the content hash,
//!   then `deleted`, then `origin_device_id`).
//! * The source-app fields travel with the winning version and are covered so
//!   a relay cannot relabel which application captured the item.
//! * `item_id` is the identity the ordering is *scoped to*. Without it a valid
//!   signature could be lifted onto another item, which is the metadata twin of
//!   the row-move the AAD already prevents for content.
//! * `nonce` and `ciphertext` are here because the AEAD binds the payload to the
//!   `item_id` but not to the *version*. Two real versions of one item are both
//!   openable under the same key and the same AAD, so without this an attacker
//!   could keep the newest signed stamp and swap in an older version's
//!   ciphertext — a content rollback that every device would accept. Covering
//!   them also makes the tombstone rule enforceable: a "tombstone" that carries
//!   a payload cannot be assembled out of a genuine tombstone's signature.
//! * `content_type` travels with the winning version and decides how the
//!   receiver treats the plaintext.
//!
//! The signature is over the **ciphertext**, never the plaintext. A MAC of the
//! plaintext beside the ciphertext would hand the backend an equality oracle
//! over content — the one thing this design does not give it — so the input to
//! the MAC is only ever what the backend already holds.
//!
//! # Why the key is derived and not the sync key itself
//!
//! `HKDF-SHA256-Expand(prk = sync_key, info = "…/cloud-row-signature/hmac-sha256")`.
//!
//! * **Expand only, no extract.** The sync key is already a 32-byte Argon2id
//!   output, i.e. a uniformly random pseudorandom key. RFC 5869 §3.3 says the
//!   extract step may be skipped for exactly that case, and skipping it keeps
//!   the derivation to one hash invocation.
//! * **Derived, not reused.** Using the same 32 bytes as an XChaCha20-Poly1305
//!   key *and* an HMAC key is cross-primitive key reuse. Nothing known breaks,
//!   but the security of each construction then depends on the other's, which is
//!   an argument nobody should have to make when separation costs one HKDF call.
//! * **Not a second Argon2id pass.** Argon2's cost buys resistance to guessing
//!   the *passphrase*; a second pass would cost another 19 MiB and buy nothing,
//!   since anyone who can compute the sync key can compute anything derived
//!   from it.
//!
//! # Why a MAC and not a signature
//!
//! Every device of an account holds the same sync key, so the property on offer
//! is "produced by a sync-key holder" — which is precisely the property that
//! excludes the account-compromise attacker of §5.3. Public-key signatures would
//! add per-device attribution, but they need per-device keypairs and a way to
//! decide which device keys are legitimate; that is pairing, and the cloud path
//! has none. It would also put a second signature stack in the tree
//! (`AGENTS.md` rule 1, exemption 3) for a property nothing here uses.
//!
//! The server can verify none of this and is not meant to: the signature is
//! opaque to it, exactly like the ciphertext.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use zeroize::Zeroizing;

use super::error::CloudCryptoError;
use super::key::{SyncKey, KEY_LEN};

type HmacSha256 = Hmac<Sha256>;

/// Length of the MAC, before base64.
pub const SIGNATURE_LEN: usize = 32;

/// HKDF `info` for the row-signing key. Distinct from every other `info` in the
/// crate, which is what makes this a separate key rather than the same one.
const INFO_ROW_SIGNATURE: &[u8] = b"copypaste/v2/cloud-row-signature/hmac-sha256";

/// Fixed prefix of the signing input. Domain-separates a row signature from any
/// other use of the same key, present or future.
const SIGNATURE_PREFIX: &[u8] = b"copypaste/v2/cloud-row-sig|";

/// Signing-input schema version. A bump makes every previously-signed row fail
/// verification — the same fail-closed hinge the AAD has, for the same reason.
const SIGNATURE_SCHEMA_VERSION: u32 = 2;

/// The fields a row signature covers. See the module docs for why each is here.
///
/// A borrowing view rather than a copy of [`crate::rest::CloudItem`], so that
/// signing and verifying read the row itself and cannot drift from it, and so
/// that this module does not depend on the REST layer.
#[derive(Debug, Clone, Copy)]
pub struct RowMetadata<'a> {
    pub item_id: &'a str,
    pub ciphertext: &'a str,
    pub nonce: &'a str,
    pub content_type: &'a str,
    pub payload_metadata: Option<&'a str>,
    pub source_app_bundle_id: Option<&'a str>,
    pub source_app_name: Option<&'a str>,
    pub created_at: i64,
    pub deleted: bool,
    pub origin_device_id: &'a str,
}

/// Sign one row's metadata. Returns standard base64 with padding, to match the
/// two payload columns.
pub fn sign_metadata(key: &SyncKey, meta: &RowMetadata<'_>) -> String {
    B64.encode(mac(key, meta))
}

/// Verify a signature produced by [`sign_metadata`].
///
/// # Errors
///
/// [`CloudCryptoError::SignatureInvalid`] for an absent, unparseable, wrong-length
/// or simply wrong signature. The four are not distinguished: the caller's only
/// correct response to any of them is to refuse the row, and a "malformed" that
/// is distinguishable from "wrong" is one more bit than an attacker needs.
pub fn verify_metadata(
    key: &SyncKey,
    meta: &RowMetadata<'_>,
    signature: &str,
) -> Result<(), CloudCryptoError> {
    let offered = B64
        .decode(signature)
        .map_err(|_| CloudCryptoError::SignatureInvalid)?;

    // Constant time, and length-checked, inside `verify_slice`.
    let mut hmac = keyed(key);
    hmac.update(&signing_input(meta));
    hmac.verify_slice(&offered)
        .map_err(|_| CloudCryptoError::SignatureInvalid)
}

fn mac(key: &SyncKey, meta: &RowMetadata<'_>) -> [u8; SIGNATURE_LEN] {
    let mut hmac = keyed(key);
    hmac.update(&signing_input(meta));
    hmac.finalize().into_bytes().into()
}

fn keyed(key: &SyncKey) -> HmacSha256 {
    // HMAC accepts a key of any length, so this cannot fail for 32 bytes.
    HmacSha256::new_from_slice(key.row_signing_key()).expect("HMAC accepts a key of any length")
}

/// The row-signing key: HKDF-Expand over the sync key. See the module docs.
///
/// Called once, by [`SyncKey`]'s constructor. It is a pure function of the sync
/// key and this runs on every row of every push and every pull, so deriving it
/// per call was one HMAC-SHA256 per row for a value that never changes.
pub(super) fn derive_row_signing_key(material: &[u8]) -> Zeroizing<[u8; KEY_LEN]> {
    let hk = Hkdf::<Sha256>::from_prk(material)
        .expect("a 32-byte PRK is at least the hash output length");
    let mut okm = Zeroizing::new([0u8; KEY_LEN]);
    hk.expand(INFO_ROW_SIGNATURE, okm.as_mut())
        .expect("HKDF expand of 32 bytes cannot fail");
    okm
}

/// The bytes that are signed.
///
/// Every field is length-prefixed as `<byte-len>:<bytes>`, so the encoding is
/// injective and no combination of caller-controlled values can be re-cut into
/// a different one (`CopyPaste-lkmy`, manifest 02 §3.2.2). This is not a
/// hypothetical here the way it is in the AAD: `content_type` and
/// `origin_device_id` are adjacent free-text fields, and a plain delimiter would
/// let one absorb the other.
fn signing_input(meta: &RowMetadata<'_>) -> Vec<u8> {
    let deleted = if meta.deleted { "1" } else { "0" };
    let created_at = meta.created_at.to_string();
    let metadata_marker = meta.payload_metadata.map_or("0", |_| "1");
    let metadata = meta.payload_metadata.unwrap_or("");
    let app_bundle_marker = meta.source_app_bundle_id.map_or("0", |_| "1");
    let app_bundle = meta.source_app_bundle_id.unwrap_or("");
    let app_name_marker = meta.source_app_name.map_or("0", |_| "1");
    let app_name = meta.source_app_name.unwrap_or("");
    let fields: [&[u8]; 13] = [
        meta.item_id.as_bytes(),
        meta.content_type.as_bytes(),
        metadata_marker.as_bytes(),
        metadata.as_bytes(),
        app_bundle_marker.as_bytes(),
        app_bundle.as_bytes(),
        app_name_marker.as_bytes(),
        app_name.as_bytes(),
        meta.origin_device_id.as_bytes(),
        created_at.as_bytes(),
        deleted.as_bytes(),
        meta.nonce.as_bytes(),
        meta.ciphertext.as_bytes(),
    ];

    let total: usize = fields.iter().map(|f| f.len() + 12).sum();
    let mut out = Vec::with_capacity(SIGNATURE_PREFIX.len() + 4 + total);
    out.extend_from_slice(SIGNATURE_PREFIX);
    out.extend_from_slice(SIGNATURE_SCHEMA_VERSION.to_string().as_bytes());
    for field in fields {
        out.push(b'|');
        out.extend_from_slice(field.len().to_string().as_bytes());
        out.push(b':');
        out.extend_from_slice(field);
    }
    out
}

// Tests

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::super::key::derive_sync_key;
    use super::super::testkit::{key, ACCOUNT, ITEM, PASS};
    use super::*;

    fn meta() -> RowMetadata<'static> {
        RowMetadata {
            item_id: ITEM,
            ciphertext: "c2VhbGVk",
            nonce: "bm9uY2U=",
            content_type: "text",
            payload_metadata: None,
            source_app_bundle_id: None,
            source_app_name: None,
            created_at: 1_700_000_000_000,
            deleted: false,
            origin_device_id: "device-a",
        }
    }

    #[test]
    fn a_signature_verifies_under_the_same_key() {
        let k = key();
        let signature = sign_metadata(&k, &meta());
        assert_eq!(verify_metadata(&k, &meta(), &signature), Ok(()));
    }

    #[test]
    fn a_signature_verifies_on_a_device_that_only_shares_the_passphrase() {
        // The whole point: no shared state but the passphrase and the account.
        let signature = sign_metadata(&key(), &meta());
        let other_device = derive_sync_key(PASS, ACCOUNT).unwrap();
        assert_eq!(verify_metadata(&other_device, &meta(), &signature), Ok(()));
    }

    #[test]
    fn the_signing_key_is_not_the_sync_key() {
        // Cross-primitive reuse is what the HKDF step exists to avoid; if this
        // ever became an identity the separation would be gone silently.
        let k = key();
        assert_ne!(k.row_signing_key(), k.to_bytes().as_ref());
        // And it is deterministic, or two devices would not agree — including
        // across the hoist onto `SyncKey`, which must derive what the free
        // function derived.
        assert_eq!(
            k.row_signing_key(),
            derive_row_signing_key(k.to_bytes().as_ref()).as_ref()
        );
        assert_eq!(key().row_signing_key(), k.row_signing_key());
    }

    #[test]
    fn another_accounts_key_does_not_verify() {
        let signature = sign_metadata(&key(), &meta());
        let other_account = derive_sync_key(PASS, "3f2b1c0a-0000-4000-8000-000000000002").unwrap();
        assert_eq!(
            verify_metadata(&other_account, &meta(), &signature),
            Err(CloudCryptoError::SignatureInvalid)
        );
    }

    #[test]
    fn every_signed_field_is_actually_covered() {
        // The table of what must be under the signature, asserted one field at
        // a time. A field dropped from `signing_input` fails exactly here.
        let k = key();
        let signature = sign_metadata(&k, &meta());

        let mutations: Vec<(&str, RowMetadata<'_>)> = vec![
            (
                "item_id",
                RowMetadata {
                    item_id: "another-item",
                    ..meta()
                },
            ),
            (
                "created_at",
                RowMetadata {
                    created_at: 1_700_000_000_001,
                    ..meta()
                },
            ),
            (
                "deleted",
                RowMetadata {
                    deleted: true,
                    ..meta()
                },
            ),
            (
                "origin_device_id",
                RowMetadata {
                    origin_device_id: "device-b",
                    ..meta()
                },
            ),
            (
                "content_type",
                RowMetadata {
                    content_type: "image",
                    ..meta()
                },
            ),
            (
                "payload_metadata",
                RowMetadata {
                    payload_metadata: Some("{\"filename\":\"report.pdf\"}"),
                    ..meta()
                },
            ),
            (
                "nonce",
                RowMetadata {
                    nonce: "bm9uY2Uy",
                    ..meta()
                },
            ),
            (
                "ciphertext",
                RowMetadata {
                    ciphertext: "c2VhbGVkMg==",
                    ..meta()
                },
            ),
        ];

        for (field, tampered) in mutations {
            assert_eq!(
                verify_metadata(&k, &tampered, &signature),
                Err(CloudCryptoError::SignatureInvalid),
                "{field} is not covered by the signature"
            );
        }
    }

    #[test]
    fn a_forged_tombstone_does_not_verify() {
        // The most destructive shape: a version that claims the item is gone.
        // It has to be refused before it can win the merge, or one write into
        // the account deletes an item on every device.
        let k = key();
        let live = meta();
        let signature = sign_metadata(&k, &live);

        let forged = RowMetadata {
            deleted: true,
            ciphertext: "",
            nonce: "",
            created_at: live.created_at + 60_000,
            ..live
        };
        assert!(verify_metadata(&k, &forged, &signature).is_err());
        // And a genuine tombstone signs and verifies, so refusing forgeries
        // does not also refuse deletes.
        let genuine = sign_metadata(&k, &forged);
        assert_eq!(verify_metadata(&k, &forged, &genuine), Ok(()));
    }

    #[test]
    fn a_payload_cannot_be_rolled_back_under_a_newer_stamp() {
        // Two real versions of one item. Both open under the same key and the
        // same AAD, so the AEAD alone cannot tell them apart; the signature has
        // to, or an attacker can keep the newest metadata and restore the older
        // content.
        let k = key();
        let newer = RowMetadata {
            ciphertext: "bmV3ZXI=",
            created_at: 2_000,
            ..meta()
        };
        let older = RowMetadata {
            ciphertext: "b2xkZXI=",
            created_at: 1_000,
            ..meta()
        };
        let newer_signature = sign_metadata(&k, &newer);

        let spliced = RowMetadata {
            ciphertext: older.ciphertext,
            ..newer
        };
        assert_eq!(
            verify_metadata(&k, &spliced, &newer_signature),
            Err(CloudCryptoError::SignatureInvalid)
        );
    }

    #[test]
    fn an_absent_or_junk_signature_is_refused_without_panicking() {
        let k = key();
        for offered in ["", "not base64!!", "c2hvcnQ=", &"A".repeat(1_000)] {
            assert_eq!(
                verify_metadata(&k, &meta(), offered),
                Err(CloudCryptoError::SignatureInvalid),
                "{offered:?} was not refused"
            );
        }
    }

    #[test]
    fn a_signature_is_the_documented_size_and_alphabet() {
        let signature = sign_metadata(&key(), &meta());
        assert_eq!(B64.decode(&signature).unwrap().len(), SIGNATURE_LEN);
        assert_eq!(signature.len(), 44, "base64 of 32 bytes with padding");
        assert!(signature
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=')));
    }

    #[test]
    fn the_signing_input_has_the_documented_layout() {
        let input = signing_input(&RowMetadata {
            item_id: "a1",
            ciphertext: "Y3Q=",
            nonce: "bm8=",
            content_type: "text",
            payload_metadata: None,
            source_app_bundle_id: None,
            source_app_name: None,
            created_at: 7,
            deleted: true,
            origin_device_id: "dev",
        });
        assert_eq!(
            input,
            b"copypaste/v2/cloud-row-sig|2|2:a1|4:text|1:0|0:|1:0|0:|1:0|0:|3:dev|1:7|1:1|4:bm8=|4:Y3Q=".to_vec()
        );
    }

    #[test]
    fn the_encoding_is_injective_across_delimiter_abuse() {
        // Adjacent free-text fields: without the length prefixes, moving a
        // character from one to the next would produce the same input and
        // therefore the same signature.
        let base = meta();
        let mut seen = HashSet::new();
        for (content_type, origin) in [
            ("text", "dev"),
            ("tex", "tdev"),
            ("text|", "dev"),
            ("text", "|dev"),
            ("4:text", "3:dev"),
            ("", "textdev"),
        ] {
            let input = signing_input(&RowMetadata {
                content_type,
                origin_device_id: origin,
                ..base
            });
            assert!(
                seen.insert(input),
                "signing-input collision for {content_type:?}/{origin:?}"
            );
        }
    }

    #[test]
    fn the_schema_version_is_bound_so_a_bump_fails_closed() {
        let k = key();
        let signature = sign_metadata(&k, &meta());

        // Same key, same fields, a signing input that differs only in the
        // version byte: it must not verify.
        let mut hmac = keyed(&k);
        let mut input = signing_input(&meta());
        input[SIGNATURE_PREFIX.len()] = b'1';
        hmac.update(&input);
        let v2 = B64.encode(hmac.finalize().into_bytes());

        assert_ne!(v2, signature);
        assert_eq!(
            verify_metadata(&k, &meta(), &v2),
            Err(CloudCryptoError::SignatureInvalid)
        );
    }
}
