//! QR-code pairing payload: a compact, versioned transport for the material a
//! scanning device needs to drive the existing PAKE pairing handshake.
//!
//! # What this is (and is not)
//!
//! This module does **not** define a new cryptographic protocol. The pairing
//! authenticity still rests entirely on the existing OPAQUE PAKE handshake
//! (`copypaste_p2p::pake`) plus mTLS cert-fingerprint pinning. A QR code is
//! merely a more convenient *transport* for the two pieces of pairing material
//! the user previously had to relay by hand:
//!
//! 1. The displaying device's **certificate fingerprint** (so the scanner can
//!    pin the right peer), and
//! 2. A **short-lived, high-entropy pairing token** that is fed into the PAKE
//!    handshake exactly where the manually-typed 6-character password used to
//!    go. Because the PAKE is password-authenticated, the token is the shared
//!    secret both sides must agree on; an attacker who cannot read the QR code
//!    cannot complete the handshake.
//!
//! # Security properties preserved
//!
//! * **No long-term secrets in the QR.** The token is ephemeral (single
//!   pairing, TTL-bounded by the daemon's PAKE-session expiry) and high
//!   entropy (256 bits), so it is strictly stronger than the 6-char password
//!   it replaces. No private key, no `SyncKey`, no `PasswordFile`, and no
//!   long-term identity secret is ever encoded.
//! * **No downgrade.** The payload is versioned (`PAIRING_QR_MAGIC`). A
//!   decoder rejects unknown versions rather than silently falling back, so a
//!   tampered "v0 / no-token" payload cannot strip the token field.
//! * **Channel binding unchanged.** The QR carries the *same* fingerprint that
//!   the mTLS verifier pins; it does not bypass or weaken the PAKE↔TLS channel
//!   binding work (`SessionKey::bind_to_tls_channel`). The token simply seeds
//!   the PAKE password, leaving every downstream property intact.
//! * **Token entropy.** [`PairingToken::generate`] draws 32 bytes from the OS
//!   CSPRNG. The token is compared in constant time via [`subtle`] to avoid a
//!   timing side channel on any equality check a caller might perform.
//!
//! # Wire format
//!
//! Two versions are recognised:
//!
//! ## v2 (current — emitted by [`PairingPayload::encode`])
//!
//! ```text
//! CPPAIR2.<fp_b64url43>.<token_b64url>.<device_id_b64url22>.<name_b64url>.<addr_b64url>[.<prov_b64url>]
//! ```
//!
//! * `CPPAIR2` — magic + version (`2`).
//! * `fp_b64url43` — the 32 raw fingerprint bytes encoded as base64url **without**
//!   padding (43 chars). On encode the hex/colon-hex fingerprint is first
//!   hex-decoded (colons stripped) to 32 bytes; on decode the bytes are
//!   hex-encoded back to the lowercase bare-hex form callers expect.
//! * `token_b64url` — the 32-byte pairing token, base64url **without** padding
//!   (43 chars).
//! * `device_id_b64url22` — the UUID's 16 raw bytes encoded as base64url **without**
//!   padding (22 chars). On encode the UUID string is parsed to its 16-byte wire
//!   form; on decode the bytes are formatted back as a standard UUID string.
//! * `name_b64url` — the human-readable device name, base64url.
//! * `addr_b64url` — the discovery hint (`host:port`), base64url-encoded so
//!   that IPv4 dots in the address cannot collide with the `.` field delimiter.
//!   An empty hint encodes to the base64url of an empty byte string.
//! * `prov_b64url` — **optional** sync-provisioning JSON, base64url **without**
//!   padding. Present when the generating device has at least one of relay URL,
//!   Supabase URL, or Supabase anon key configured.
//!
//! Compared to v1, `fp_b64url43` saves 21 chars (64→43) and `device_id_b64url22`
//! saves 14 chars (36→22), for a total saving of 35 chars per payload (addr_hint
//! base64url adds a few chars for IPv4 addresses, but the net saving is positive).
//!
//! ## v1 (legacy — still accepted by [`PairingPayload::decode`])
//!
//! ```text
//! CPPAIR1.<fp_hex>.<token_b64url>.<device_id>.<name_b64url>.<host:port>[.<prov_b64url>]
//! ```
//!
//! * `CPPAIR1` — magic + version (`1`). Decoders accept this for backward compat.
//! * `fp_hex` — the cert fingerprint in lowercase hex (colons optional).
//! * `token_b64url` — the 32-byte pairing token, base64url **without** padding.
//! * `device_id` — the displaying device's UUID string.
//! * `name_b64url` — the human-readable device name, base64url.
//! * `host:port` — optional discovery hint (`host` may be empty → mDNS only).
//!   This is the raw, un-encoded form — safe in v1 because it is the terminal
//!   field and the raw `host:port` string contains `:` but not `.`. Provisioning
//!   is not supported in the v1 wire format because IPv4 dots in `host:port`
//!   would collide with the field delimiter; use CPPAIR2 for provisioning.

mod payload;
mod token;
mod wire;

pub use payload::{PairingPayload, QrProvisioning};
pub use token::PairingToken;
pub use wire::strip_deeplink;

use thiserror::Error;

/// Magic prefix for the v1 (legacy) QR pairing payload.
///
/// Still accepted by [`PairingPayload::decode`] for backward compatibility with
/// devices that scanned a v1 QR code. [`PairingPayload::encode`] now emits
/// [`PAIRING_QR_MAGIC_V2`] instead.
pub const PAIRING_QR_MAGIC: &str = "CPPAIR1";

/// Magic prefix for the v2 (current) QR pairing payload.
///
/// v2 encodes the fingerprint and device_id as base64url (raw bytes) instead of
/// hex/UUID strings, saving 35 chars per payload and reducing QR code density.
/// addr_hint is also base64url-encoded in v2 to avoid dot collision with the
/// optional 6th provisioning field.
pub const PAIRING_QR_MAGIC_V2: &str = "CPPAIR2";

/// Number of `.`-separated fields in the mandatory part of the payload body
/// (after the magic prefix). A 6th optional field carries provisioning JSON.
const PAIRING_QR_FIELD_COUNT: usize = 5;

/// Expected byte length of a SHA-256 certificate fingerprint.
const FP_BYTE_LEN: usize = 32;

/// Expected byte length of a UUID (version-agnostic, just the 16 wire bytes).
const UUID_BYTE_LEN: usize = 16;

/// Deep-link URI prefix wrapping the bare [`PAIRING_QR_MAGIC`] payload so that
/// external QR scanners (e.g. Google Lens, the iOS/Android camera app) treat the
/// QR as an actionable link and offer "open in app" instead of plain text.
///
/// The wrapped form is `cppair://pair?p=<percent-encoded CPPAIR2...>`. The bare
/// payload is the value of the `p` query parameter.
pub const PAIRING_DEEPLINK_PREFIX: &str = "cppair://pair?p=";

/// Length in bytes of the pairing token.
///
/// 32 bytes = 256 bits of entropy, drawn from the OS CSPRNG.
pub const PAIRING_TOKEN_LEN: usize = 32;

/// base64url engine (URL-safe alphabet, no padding) used for binary fields.
fn b64() -> base64::engine::general_purpose::GeneralPurpose {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
}

// ─────────────────────────────────────────────────────────────────────────────
// Errors
// ─────────────────────────────────────────────────────────────────────────────

/// Errors from QR pairing payload encode/decode.
#[derive(Debug, Error)]
pub enum PairingQrError {
    /// The magic + version prefix was missing or not a recognised version
    /// (`CPPAIR1` or `CPPAIR2`). Rejecting here is the anti-downgrade guard.
    #[error("missing or unsupported pairing QR magic/version")]
    BadMagic,

    /// The payload body had the wrong number of `.`-separated fields.
    #[error("expected {PAIRING_QR_FIELD_COUNT} fields, found {0}")]
    FieldCount(usize),

    /// A base64url-encoded field failed to decode.
    #[error("base64 decode error: {0}")]
    Base64(String),

    /// A decoded field was not valid UTF-8.
    #[error("utf-8 decode error: {0}")]
    Utf8(String),

    /// The token field was not exactly [`PAIRING_TOKEN_LEN`] bytes.
    #[error("pairing token must be {PAIRING_TOKEN_LEN} bytes, got {0}")]
    TokenLength(usize),

    /// The fingerprint field was empty.
    #[error("fingerprint must not be empty")]
    EmptyFingerprint,

    /// (CPPAIR2) The fingerprint b64url field decoded to the wrong number of bytes.
    /// Expected exactly `FP_BYTE_LEN` (32).
    #[error("fingerprint must be {FP_BYTE_LEN} bytes, got {0}")]
    FingerprintLength(usize),

    /// (CPPAIR2) The device_id b64url field decoded to the wrong number of bytes.
    /// Expected exactly `UUID_BYTE_LEN` (16).
    #[error("device_id must be {UUID_BYTE_LEN} bytes, got {0}")]
    DeviceIdLength(usize),

    /// (CPPAIR2) The addr_hint b64url field failed to decode or was not valid UTF-8.
    #[error("addr_hint base64url decode failed")]
    AddrHintDecode,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    /// `from_bytes` test helper that panics with a clear message.
    fn token(bytes: [u8; PAIRING_TOKEN_LEN]) -> PairingToken {
        match PairingToken::from_bytes(&bytes) {
            Ok(t) => t,
            Err(e) => panic!("from_bytes must succeed for {PAIRING_TOKEN_LEN} bytes: {e}"),
        }
    }

    fn sample_with_provisioning() -> PairingPayload {
        PairingPayload {
            fingerprint: "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899"
                .to_string(),
            token: token([7u8; PAIRING_TOKEN_LEN]),
            device_id: "11112222-3333-4444-5555-666677778888".to_string(),
            device_name: "Dmytro's MacBook".to_string(),
            addr_hint: "192.168.1.5:54321".to_string(),
            provisioning: Some(QrProvisioning {
                relay_url: Some("https://relay.example.com".to_string()),
                supabase_url: Some("https://abcd.supabase.co".to_string()),
                supabase_anon_key: Some("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.anon".to_string()),
            }),
        }
    }

    #[test]
    fn token_generate_is_correct_length_and_random() {
        let a = PairingToken::generate();
        let b = PairingToken::generate();
        assert_eq!(a.as_bytes().len(), PAIRING_TOKEN_LEN);
        assert_ne!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn token_eq_is_constant_time_and_correct() {
        let t1 = token([1u8; PAIRING_TOKEN_LEN]);
        let t2 = token([1u8; PAIRING_TOKEN_LEN]);
        let t3 = token([2u8; PAIRING_TOKEN_LEN]);
        assert!(t1 == t2);
        assert!(t1 != t3);
    }

    #[test]
    fn token_from_bytes_rejects_wrong_length() {
        let err = match PairingToken::from_bytes(&[0u8; 16]) {
            Ok(_) => panic!("16-byte token must be rejected"),
            Err(e) => e,
        };
        assert!(matches!(err, PairingQrError::TokenLength(16)));
    }

    #[test]
    fn pake_password_is_stable_for_same_token() {
        let t = token([42u8; PAIRING_TOKEN_LEN]);
        let p1 = t.to_pake_password();
        let p2 = t.to_pake_password();
        assert_eq!(p1, p2, "same token must derive the same PAKE password");
        assert!(p1.len() >= 6);
    }

    /// Decode helper that panics with a clear message.
    fn decode(s: &str) -> PairingPayload {
        match PairingPayload::decode(s) {
            Ok(p) => p,
            Err(e) => panic!("decode must succeed: {e}"),
        }
    }

    /// Returns the error from a decode that is expected to fail.
    fn decode_err(s: &str) -> PairingQrError {
        match PairingPayload::decode(s) {
            Ok(_) => panic!("decode was expected to fail but succeeded"),
            Err(e) => e,
        }
    }

    /// A sample payload with a valid 64-char (32-byte) hex fingerprint and a
    /// proper UUID device_id, as required by the CPPAIR2 format.
    fn sample_v2() -> PairingPayload {
        PairingPayload {
            fingerprint: "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899"
                .to_string(),
            token: token([7u8; PAIRING_TOKEN_LEN]),
            device_id: "11112222-3333-4444-5555-666677778888".to_string(),
            device_name: "Dmytro's MacBook".to_string(),
            addr_hint: "192.168.1.5:54321".to_string(),
            provisioning: None,
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        // Use a proper 32-byte (64-char hex) fingerprint and UUID device_id so
        // the CPPAIR2 encoder/decoder can validate field lengths.
        let original = sample_v2();
        let encoded = original.encode();
        let decoded = decode(&encoded);

        assert_eq!(decoded.fingerprint, original.fingerprint);
        assert!(decoded.token == original.token);
        assert_eq!(decoded.device_id, original.device_id);
        assert_eq!(decoded.device_name, original.device_name);
        assert_eq!(decoded.addr_hint, original.addr_hint);
        assert!(decoded.provisioning.is_none());
    }

    #[test]
    fn encoded_starts_with_magic() {
        // encode() now emits CPPAIR2 (slimmer format).
        let encoded = sample_v2().encode();
        assert!(encoded.starts_with("CPPAIR2."));
    }

    #[test]
    fn decode_rejects_bad_magic() {
        // encode() emits CPPAIR2; replace with an unknown version to trigger BadMagic.
        let encoded = sample_v2().encode().replacen("CPPAIR2", "CPPAIR0", 1);
        let err = decode_err(&encoded);
        assert!(
            matches!(err, PairingQrError::BadMagic),
            "downgrade/unknown version must be rejected, got {err:?}"
        );
    }

    #[test]
    fn decode_rejects_missing_magic() {
        let err = decode_err("not-a-pairing-code");
        assert!(matches!(err, PairingQrError::BadMagic));
    }

    #[test]
    fn decode_rejects_wrong_field_count() {
        let err = decode_err("CPPAIR1.aabb.tok");
        assert!(matches!(err, PairingQrError::FieldCount(_)));
    }

    #[test]
    fn decode_rejects_short_token() {
        // Craft a CPPAIR1 string directly with a too-short token field.
        // We use CPPAIR1 (v1) format here to test the token-length validation
        // path in decode_v1 with arbitrary (non-UUID/non-32fp) field values.
        let fp = "aabbccdd";
        let short_token = b64().encode([0u8; 8]);
        let device_id = "some-device-id";
        let name_b64 = b64().encode(b"TestDevice");
        let tampered = format!(
            "{}.{}.{}.{}.{}.{}",
            PAIRING_QR_MAGIC, fp, short_token, device_id, name_b64, ""
        );
        let err = decode_err(&tampered);
        assert!(matches!(err, PairingQrError::TokenLength(8)));
    }

    #[test]
    fn fingerprint_is_lowercased_in_v2() {
        // In CPPAIR2, the fingerprint is round-tripped through bytes, so colons
        // are stripped and the result is bare lowercase hex. This documents the
        // v2 behavior: callers passing colon-hex get bare hex back.
        let payload = PairingPayload {
            // 32 bytes = 64 hex chars with colons (32 groups × "XX:")
            fingerprint: "aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99:\
                          aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99"
                .to_string(),
            token: token([0u8; PAIRING_TOKEN_LEN]),
            device_id: "11112222-3333-4444-5555-666677778888".to_string(),
            device_name: "n".to_string(),
            addr_hint: String::new(),
            provisioning: None,
        };
        let decoded = decode(&payload.encode());
        // Colons stripped, lowercase hex only.
        assert_eq!(
            decoded.fingerprint,
            "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899"
        );
        assert!(!decoded.fingerprint.contains(':'));
    }

    #[test]
    fn empty_addr_hint_roundtrips() {
        // Use valid 32-byte fingerprint and UUID so CPPAIR2 encode/decode succeeds.
        let payload = PairingPayload {
            fingerprint: "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
                .to_string(),
            token: token([3u8; PAIRING_TOKEN_LEN]),
            device_id: "11112222-3333-4444-5555-666677778888".to_string(),
            device_name: "Phone".to_string(),
            addr_hint: String::new(),
            provisioning: None,
        };
        let decoded = decode(&payload.encode());
        assert_eq!(decoded.addr_hint, "");
    }

    #[test]
    fn device_name_with_special_chars_roundtrips() {
        // Use valid 32-byte fingerprint and UUID so CPPAIR2 encode/decode succeeds.
        let payload = PairingPayload {
            fingerprint: "cafecafecafecafecafecafecafecafecafecafecafecafecafecafecafecafe"
                .to_string(),
            token: token([9u8; PAIRING_TOKEN_LEN]),
            device_id: "11112222-3333-4444-5555-666677778888".to_string(),
            // Dots, colons and unicode would break naive splitting; base64url
            // of the name field protects against that.
            device_name: "A.B:C — café".to_string(),
            addr_hint: "host:1234".to_string(),
            provisioning: None,
        };
        let decoded = decode(&payload.encode());
        assert_eq!(decoded.device_name, "A.B:C — café");
    }

    #[test]
    fn new_rejects_empty_fingerprint() {
        let err = match PairingPayload::new("", "id", "name", "") {
            Ok(_) => panic!("empty fingerprint must be rejected"),
            Err(e) => e,
        };
        assert!(matches!(err, PairingQrError::EmptyFingerprint));
    }

    #[test]
    fn deeplink_wrap_strip_decode_roundtrip() {
        // Full wrap → strip → decode cycle must recover the original payload,
        // exercising the cppair:// envelope external scanners (Google Lens) need.
        // Use sample_v2() so the CPPAIR2 format is exercised end-to-end.
        let original = sample_v2();
        let wrapped = original.encode_deeplink();
        assert!(
            wrapped.starts_with(PAIRING_DEEPLINK_PREFIX),
            "deep-link must carry the cppair://pair?p= prefix: {wrapped}"
        );
        // The bare CPPAIR2 magic must NOT appear before the prefix (i.e. it is
        // wrapped, not concatenated), so external scanners see a URI.
        assert!(wrapped.starts_with("cppair://pair?p="));

        let stripped = strip_deeplink(&wrapped);
        assert!(
            stripped.starts_with("CPPAIR2."),
            "stripping the wrapper must yield the bare CPPAIR2 payload: {stripped}"
        );
        assert_eq!(stripped, original.encode(), "strip must invert wrap");

        let decoded = decode(&stripped);
        assert_eq!(decoded.fingerprint, original.fingerprint);
        assert!(decoded.token == original.token);
        assert_eq!(decoded.device_id, original.device_id);
        assert_eq!(decoded.device_name, original.device_name);
        assert_eq!(decoded.addr_hint, original.addr_hint);
    }

    #[test]
    fn strip_deeplink_passes_through_bare_payload() {
        // Back-compat: a bare (unwrapped) CPPAIR2 string must be returned as-is.
        let bare = sample_v2().encode();
        assert_eq!(strip_deeplink(&bare), bare);
        let padded = format!("  {bare}  ");
        assert_eq!(strip_deeplink(&padded), bare);
    }

    #[test]
    fn deeplink_encodes_addr_hint_as_b64() {
        // In CPPAIR2 the addr_hint is base64url-encoded, so its IPv4 dots are
        // gone from the encoded string and the deep-link has no raw ':'.
        // The decoded addr_hint must still round-trip correctly.
        let original = sample_v2(); // addr_hint = "192.168.1.5:54321"
        let encoded = original.encode();
        // addr_hint bytes are base64url in the encoded string — no raw dots from IPv4.
        // Verify the encoded body contains no raw "192.168" sequence.
        let body = encoded.strip_prefix("CPPAIR2.").unwrap();
        let fields: Vec<&str> = body.splitn(6, '.').collect();
        // field[4] is addr_b64 — it must decode back to the original addr_hint.
        let addr_decoded = String::from_utf8(b64().decode(fields[4]).unwrap()).unwrap();
        assert_eq!(addr_decoded, "192.168.1.5:54321");

        let decoded = decode(&strip_deeplink(&original.encode_deeplink()));
        assert_eq!(decoded.addr_hint, "192.168.1.5:54321");
    }

    #[test]
    fn new_generates_fresh_tokens() {
        let a = match PairingPayload::new("fp", "id", "name", "") {
            Ok(p) => p,
            Err(e) => panic!("new must succeed: {e}"),
        };
        let b = match PairingPayload::new("fp", "id", "name", "") {
            Ok(p) => p,
            Err(e) => panic!("new must succeed: {e}"),
        };
        assert!(a.token != b.token, "each payload must get a fresh token");
    }

    // ── CPPAIR2 tests ───────────────────────────────────────────────────────────

    #[test]
    fn cppair2_encode_starts_with_v2_magic() {
        let encoded = sample_v2().encode();
        assert!(
            encoded.starts_with("CPPAIR2."),
            "encode must emit CPPAIR2 prefix, got: {encoded}"
        );
    }

    #[test]
    fn cppair2_roundtrip() {
        let original = sample_v2();
        let encoded = original.encode();
        let decoded = decode(&encoded);

        assert_eq!(decoded.fingerprint, original.fingerprint);
        assert!(decoded.token == original.token);
        assert_eq!(decoded.device_id, original.device_id);
        assert_eq!(decoded.device_name, original.device_name);
        assert_eq!(decoded.addr_hint, original.addr_hint);
        assert!(decoded.provisioning.is_none());
    }

    #[test]
    fn cppair2_is_shorter_than_cppair1_equivalent() {
        // For the same data, a CPPAIR2 string must be shorter than its CPPAIR1
        // equivalent. CPPAIR2 saves 21 chars on fp (43 vs 64) and 14 chars on
        // device_id (22 vs 36) = 35 chars saved; addr_hint base64url adds a few
        // chars but the net saving is still positive.
        let payload = sample_v2();
        let v2_len = payload.encode().len();

        // Build an equivalent v1 string manually using the same fields.
        let v1_string = format!(
            "CPPAIR1.{}.{}.{}.{}.{}",
            payload.fingerprint,
            b64().encode(payload.token.inner()),
            payload.device_id,
            b64().encode(payload.device_name.as_bytes()),
            payload.addr_hint,
        );
        let v1_len = v1_string.len();

        assert!(
            v2_len < v1_len,
            "CPPAIR2 ({v2_len} chars) must be shorter than CPPAIR1 ({v1_len} chars)"
        );
    }

    #[test]
    fn cppair2_fingerprint_field_is_43_chars() {
        // 32 raw bytes base64url-no-pad = ceil(32*4/3) = 43 chars (no '=' padding).
        let encoded = sample_v2().encode();
        let body = encoded
            .strip_prefix("CPPAIR2.")
            .expect("must start with CPPAIR2.");
        let fp_field = body.split('.').next().expect("must have fields");
        assert_eq!(
            fp_field.len(),
            43,
            "CPPAIR2 fingerprint field must be 43 chars (32-byte base64url), got {} chars: {fp_field}",
            fp_field.len()
        );
    }

    #[test]
    fn cppair2_device_id_field_is_22_chars() {
        // 16 UUID bytes base64url-no-pad = ceil(16*4/3) = 22 chars.
        let encoded = sample_v2().encode();
        let body = encoded
            .strip_prefix("CPPAIR2.")
            .expect("must start with CPPAIR2.");
        let fields: Vec<&str> = body.splitn(PAIRING_QR_FIELD_COUNT + 1, '.').collect();
        assert!(
            fields.len() >= PAIRING_QR_FIELD_COUNT,
            "must have ≥5 fields"
        );
        let device_id_field = fields[2];
        assert_eq!(
            device_id_field.len(),
            22,
            "CPPAIR2 device_id field must be 22 chars (16-byte base64url), got {} chars: {device_id_field}",
            device_id_field.len()
        );
    }

    #[test]
    fn cppair2_decode_rejects_bad_fp_length() {
        // Build a CPPAIR2 string with a bad fingerprint (wrong byte count after b64 decode).
        let payload = sample_v2();
        let encoded = payload.encode();
        let body = encoded
            .strip_prefix("CPPAIR2.")
            .expect("must start with CPPAIR2.");
        let mut fields: Vec<String> = body
            .splitn(PAIRING_QR_FIELD_COUNT + 1, '.')
            .map(|s| s.to_string())
            .collect();
        // Replace fingerprint field with b64url of 16 bytes (wrong: expected 32)
        fields[0] = b64().encode([0xffu8; 16]);
        let tampered = format!("CPPAIR2.{}", fields.join("."));
        let err = decode_err(&tampered);
        assert!(
            matches!(err, PairingQrError::FingerprintLength(_)),
            "wrong fp byte count must yield FingerprintLength, got: {err:?}"
        );
    }

    #[test]
    fn cppair2_decode_rejects_bad_device_id_length() {
        // Build a CPPAIR2 string with a bad device_id (wrong byte count after b64 decode).
        let payload = sample_v2();
        let encoded = payload.encode();
        let body = encoded
            .strip_prefix("CPPAIR2.")
            .expect("must start with CPPAIR2.");
        let fields: Vec<&str> = body.splitn(PAIRING_QR_FIELD_COUNT + 1, '.').collect();
        assert!(fields.len() >= PAIRING_QR_FIELD_COUNT);
        // Replace device_id field with b64url of 8 bytes (wrong: expected 16)
        let bad_id = b64().encode([0xaau8; 8]);
        let tampered = format!(
            "CPPAIR2.{}.{}.{}.{}.{}",
            fields[0], fields[1], bad_id, fields[3], fields[4]
        );
        let err = decode_err(&tampered);
        assert!(
            matches!(err, PairingQrError::DeviceIdLength(_)),
            "wrong device_id byte count must yield DeviceIdLength, got: {err:?}"
        );
    }

    #[test]
    fn cppair1_legacy_decode_still_works() {
        // A hardcoded CPPAIR1 string must still decode successfully (backward compat).
        // Build a v1 string manually to avoid calling encode() (which now emits v2).
        let fp = "aabbccdd";
        let token_b64 = b64().encode([5u8; PAIRING_TOKEN_LEN]);
        let device_id = "11112222-3333-4444-5555-666677778888";
        let name_b64 = b64().encode(b"My Phone");
        let v1 = format!("CPPAIR1.{fp}.{token_b64}.{device_id}.{name_b64}.192.168.1.1:1234");
        let decoded = decode(&v1);
        assert_eq!(decoded.fingerprint, fp);
        assert_eq!(decoded.device_id, device_id);
        assert_eq!(decoded.device_name, "My Phone");
        assert_eq!(decoded.addr_hint, "192.168.1.1:1234");
        assert!(decoded.provisioning.is_none());
    }

    // NOTE: CPPAIR1 does not support provisioning — the raw IPv4 addr_hint
    // (e.g. "192.168.1.1:1234") contains dots that collide with the field delimiter,
    // making a 6th field ambiguous. Provisioning is CPPAIR2-only.

    #[test]
    fn cppair2_fingerprint_recovered_as_lowercase_hex() {
        // After a CPPAIR2 round-trip, fingerprint must come back as lowercase hex
        // (same form as keys.rs fingerprint(), which callers downstream expect).
        let original = sample_v2();
        let encoded = original.encode();
        let decoded = decode(&encoded);
        // Must be lowercase hex (no colons), same as input
        assert_eq!(
            decoded.fingerprint,
            "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899"
        );
        // Must not contain colons (CPPAIR2 uses bare hex on output)
        assert!(!decoded.fingerprint.contains(':'));
    }

    #[test]
    fn cppair2_colon_hex_fingerprint_roundtrips_without_colons() {
        // If the caller encodes with a colon-separated fingerprint,
        // CPPAIR2 decode returns it as bare hex (colons stripped during b64 encoding).
        let payload = PairingPayload {
            fingerprint: "aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99".to_string(),
            token: token([1u8; PAIRING_TOKEN_LEN]),
            device_id: "11112222-3333-4444-5555-666677778888".to_string(),
            device_name: "Test".to_string(),
            addr_hint: String::new(),
            provisioning: None,
        };
        let encoded = payload.encode();
        let decoded = decode(&encoded);
        // Colons removed, bare hex
        assert_eq!(
            decoded.fingerprint,
            "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899"
        );
    }

    // ── QrProvisioning tests ─────────────────────────────────────────────────

    #[test]
    fn provisioning_roundtrip_all_fields() {
        let prov = QrProvisioning {
            relay_url: Some("https://relay.example.com".to_string()),
            supabase_url: Some("https://abcd.supabase.co".to_string()),
            supabase_anon_key: Some("eyJhbGciOiJIUzI1NiJ9.anon".to_string()),
        };
        let encoded = prov.encode();
        let decoded = QrProvisioning::decode(&encoded)
            .expect("decode must succeed for a valid provisioning field");
        assert_eq!(
            decoded.relay_url.as_deref(),
            Some("https://relay.example.com")
        );
        assert_eq!(
            decoded.supabase_url.as_deref(),
            Some("https://abcd.supabase.co")
        );
        assert_eq!(
            decoded.supabase_anon_key.as_deref(),
            Some("eyJhbGciOiJIUzI1NiJ9.anon")
        );
    }

    #[test]
    fn provisioning_roundtrip_partial_fields() {
        let prov = QrProvisioning {
            relay_url: Some("https://relay.example.com".to_string()),
            supabase_url: None,
            supabase_anon_key: None,
        };
        let encoded = prov.encode();
        let decoded = QrProvisioning::decode(&encoded).expect("decode must succeed");
        assert_eq!(
            decoded.relay_url.as_deref(),
            Some("https://relay.example.com")
        );
        assert!(decoded.supabase_url.is_none());
        assert!(decoded.supabase_anon_key.is_none());
    }

    #[test]
    fn provisioning_is_empty_when_all_none() {
        let prov = QrProvisioning::default();
        assert!(prov.is_empty());
    }

    #[test]
    fn provisioning_is_empty_when_all_blank() {
        let prov = QrProvisioning {
            relay_url: Some(String::new()),
            supabase_url: Some(String::new()),
            supabase_anon_key: Some(String::new()),
        };
        assert!(prov.is_empty());
    }

    #[test]
    fn payload_with_provisioning_roundtrips() {
        let original = sample_with_provisioning();
        let encoded = original.encode();
        // Must have 6 dot-separated fields in the body after the magic.
        let parts: Vec<&str> = encoded.splitn(2, '.').collect();
        // All body fields (fp, tok, id, name, addr_b64, prov_b64) are base64url
        // (no dots), so a plain split gives the accurate field count.
        let field_count = parts[1].split('.').count();
        assert_eq!(
            field_count, 6,
            "payload with provisioning must have 6 body fields"
        );

        let decoded = decode(&encoded);
        assert_eq!(decoded.fingerprint, original.fingerprint);
        assert!(decoded.token == original.token);
        assert_eq!(decoded.addr_hint, original.addr_hint);

        let prov = decoded.provisioning.expect("provisioning must be decoded");
        assert_eq!(prov.relay_url.as_deref(), Some("https://relay.example.com"));
        assert_eq!(
            prov.supabase_url.as_deref(),
            Some("https://abcd.supabase.co")
        );
        assert_eq!(
            prov.supabase_anon_key.as_deref(),
            Some("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.anon")
        );
    }

    #[test]
    fn payload_without_provisioning_has_5_fields() {
        let original = sample_v2();
        let encoded = original.encode();
        let parts: Vec<&str> = encoded.splitn(2, '.').collect();
        // All 5 body fields are base64url (no dots) — plain split is accurate.
        let field_count = parts[1].split('.').count();
        assert_eq!(
            field_count, 5,
            "payload without provisioning must have 5 body fields"
        );
    }

    #[test]
    fn malformed_provisioning_field_does_not_break_decode() {
        // Build a valid 5-field payload, then append a corrupt 6th field.
        // decode() must still succeed (with provisioning = None) — the
        // provisioning field is advisory and must never break pairing.
        let base = sample_v2().encode();
        let corrupt = format!("{base}.!!!notbase64!!!");
        let decoded = decode(&corrupt);
        // Core pairing fields must be intact.
        assert_eq!(decoded.fingerprint, sample_v2().fingerprint);
        assert_eq!(decoded.addr_hint, sample_v2().addr_hint);
        // Provisioning is silently None (corrupt field ignored).
        assert!(decoded.provisioning.is_none());
    }

    #[test]
    fn deeplink_with_provisioning_roundtrips() {
        let original = sample_with_provisioning();
        let wrapped = original.encode_deeplink();
        let stripped = strip_deeplink(&wrapped);
        let decoded = decode(&stripped);
        let prov = decoded
            .provisioning
            .expect("provisioning must survive deep-link round-trip");
        assert_eq!(prov.relay_url.as_deref(), Some("https://relay.example.com"));
    }

    /// #11 — QR pairing JSON schema parity: GOLDEN JSON test.
    ///
    /// Source of truth: `QrProvisioning::encode` in
    ///   `crates/copypaste-core/src/crypto/pairing_qr/payload.rs`
    ///
    /// The provisioning 6th field carries compact JSON
    ///   `{"ru":<relay_url>,"su":<supabase_url>,"sk":<supabase_anon_key>}`
    /// base64url-encoded (no padding). This test pins the EXACT field names and
    /// JSON structure so a rename in `QrProvisioning` immediately breaks this test.
    ///
    /// If this test fails after renaming fields, update the Android JVM test
    /// `QrProvisioningParityTest.kt` to match the new field names / JSON shape.
    ///
    /// The companion Android JVM test lives at:
    ///   android/app/src/test/java/com/copypaste/android/QrProvisioningParityTest.kt
    #[test]
    fn qr_provisioning_json_golden_schema() {
        // Canonical test vector — same values used in the Android companion test.
        let prov = QrProvisioning {
            relay_url: Some("https://relay.example.com".to_string()),
            supabase_url: Some("https://abcd.supabase.co".to_string()),
            supabase_anon_key: Some("anon-key-123".to_string()),
        };

        // `encode()` produces base64url(JSON). Decode it back to verify the
        // JSON field names are EXACTLY "ru", "su", "sk" — the names the Android
        // PairProvisioning.kt parser looks up.
        let encoded_b64 = prov.encode();
        let json_bytes = b64().decode(&encoded_b64)
            .expect("provisioning encodes to valid base64url");
        let json = std::str::from_utf8(&json_bytes)
            .expect("provisioning JSON is valid UTF-8");

        // Authoritative golden JSON string. The Android companion test
        // QrProvisioningParityTest.kt uses the SAME string to verify its parser.
        // Field order is insertion order: ru, su, sk.
        let expected_json =
            r#"{"ru":"https://relay.example.com","su":"https://abcd.supabase.co","sk":"anon-key-123"}"#;

        assert_eq!(
            json, expected_json,
            "QrProvisioning JSON golden schema mismatch — \
             field names/order must stay ru/su/sk; \
             update QrProvisioningParityTest.kt (Android) if this changes"
        );

        // Cross-check: decode() must round-trip from the produced JSON.
        let decoded = QrProvisioning::decode(&encoded_b64)
            .expect("QrProvisioning::decode must accept its own output");
        assert_eq!(
            decoded.relay_url.as_deref(),
            Some("https://relay.example.com")
        );
        assert_eq!(
            decoded.supabase_url.as_deref(),
            Some("https://abcd.supabase.co")
        );
        assert_eq!(
            decoded.supabase_anon_key.as_deref(),
            Some("anon-key-123")
        );
    }
}
