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
//! The encoded payload is a single ASCII line, safe to embed in any QR code:
//!
//! ```text
//! CPPAIR1.<fp_hex>.<token_b64url>.<device_id>.<name_b64url>.<host:port>
//! ```
//!
//! * `CPPAIR1` — magic + version (`1`). Bumping the trailing digit is a hard
//!   version change; decoders reject any other value.
//! * `fp_hex` — the displaying device's cert fingerprint in the user-facing
//!   lowercase colon-hex form (`xx:xx:...`) the daemon pairing surface accepts.
//! * `token_b64url` — the 32-byte pairing token, base64url **without** padding.
//! * `device_id` — the displaying device's UUID string.
//! * `name_b64url` — the human-readable device name, base64url (so `.` and
//!   non-ASCII in names cannot break field splitting).
//! * `host:port` — optional discovery hint (`host` may be empty → mDNS only).
//!
//! Fields are `.`-separated. `fp_hex` is `.`-free hex, `token_b64url` and
//! `name_b64url` use the URL-safe base64 alphabet (no `.`), `device_id` is a
//! `.`-free UUID, and `host:port` is the final field — so `.` is an
//! unambiguous separator. The format is deliberately delimiter-based (not
//! JSON/CBOR) to keep the QR small: every byte saved lowers the QR version and
//! improves scan reliability.

use base64::Engine as _;
use rand::rngs::OsRng;
use rand::RngCore;
use subtle::ConstantTimeEq;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Magic prefix + version digit for the encoded QR pairing payload.
///
/// The trailing `1` is the format version. Decoders MUST reject any other
/// version rather than attempting a best-effort parse — this prevents a
/// downgrade attack that strips the token field.
pub const PAIRING_QR_MAGIC: &str = "CPPAIR1";

/// Number of `.`-separated fields in the v1 payload after the magic prefix.
const PAIRING_QR_FIELD_COUNT: usize = 5;

/// Length in bytes of the pairing token.
///
/// 32 bytes = 256 bits of entropy, drawn from the OS CSPRNG. This is the
/// shared secret fed into the PAKE handshake in place of the legacy
/// 6-character typed password, so it is dramatically stronger while remaining
/// short enough to fit comfortably in a QR code.
pub const PAIRING_TOKEN_LEN: usize = 32;

/// base64url engine (URL-safe alphabet, no padding) used for binary fields.
fn b64() -> base64::engine::general_purpose::GeneralPurpose {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
}

// ─────────────────────────────────────────────────────────────────────────────
// PairingToken
// ─────────────────────────────────────────────────────────────────────────────

/// A short-lived, high-entropy secret transported by the QR code and fed into
/// the PAKE handshake as the shared "password".
///
/// # Security
/// * `ZeroizeOnDrop` scrubs the bytes when dropped, bounding the in-memory
///   lifetime of the secret.
/// * Does NOT implement `Debug` / `Display` / `Clone` to avoid accidental
///   logging or silent duplication.
/// * Equality is constant-time via [`ConstantTimeEq`] ([`PartialEq`] impl).
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct PairingToken([u8; PAIRING_TOKEN_LEN]);

impl PairingToken {
    /// Generate a fresh 256-bit pairing token from the OS CSPRNG.
    pub fn generate() -> Self {
        let mut bytes = [0u8; PAIRING_TOKEN_LEN];
        // OsRng is infallible on all supported targets; a failure here means
        // the OS entropy source is broken, which is unrecoverable regardless.
        OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// Borrow the raw token bytes.
    pub fn as_bytes(&self) -> &[u8; PAIRING_TOKEN_LEN] {
        &self.0
    }

    /// Construct a token from exactly [`PAIRING_TOKEN_LEN`] bytes.
    ///
    /// # Errors
    /// Returns [`PairingQrError::TokenLength`] if `bytes` is the wrong length.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PairingQrError> {
        let arr: [u8; PAIRING_TOKEN_LEN] = bytes
            .try_into()
            .map_err(|_| PairingQrError::TokenLength(bytes.len()))?;
        Ok(Self(arr))
    }

    /// Encode the token as the PAKE "password" string.
    ///
    /// The PAKE API (`copypaste_p2p::pake::PakeInitiator::new`) takes a
    /// `&str`. We render the raw token bytes as base64url so the full 256 bits
    /// of entropy survive the byte→str conversion losslessly (the bytes are
    /// not valid UTF-8 in general). Both devices derive the identical string
    /// from the identical token, so the PAKE converges.
    pub fn to_pake_password(&self) -> String {
        b64().encode(self.0)
    }
}

impl PartialEq for PairingToken {
    /// Constant-time comparison — never short-circuit on the first differing
    /// byte (timing side-channel resistance, per project crypto conventions).
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for PairingToken {}

// ─────────────────────────────────────────────────────────────────────────────
// PairingPayload
// ─────────────────────────────────────────────────────────────────────────────

/// The fully-decoded contents of a QR pairing code.
///
/// Produced by [`PairingPayload::encode`] (on the displaying device) and
/// recovered by [`PairingPayload::decode`] (on the scanning device). Holds the
/// material the scanner needs to (a) pin the right mTLS peer and (b) drive the
/// PAKE handshake as initiator.
///
/// The [`token`](Self::token) field is the only secret; everything else is
/// public pairing metadata. The struct does not implement `Clone` because the
/// token is non-`Clone` by design.
pub struct PairingPayload {
    /// Displaying device's cert fingerprint in the user-facing lowercase
    /// colon-hex form (`xx:xx:...`). The daemon validates this form and strips
    /// the colons itself before comparing against the mTLS verifier's canonical
    /// (colon-free) fingerprint.
    pub fingerprint: String,
    /// Single-use, TTL-bounded pairing secret fed into the PAKE handshake.
    pub token: PairingToken,
    /// Displaying device's UUID (used as the peer identifier on the scanner).
    pub device_id: String,
    /// Human-readable device name shown to the scanning user.
    pub device_name: String,
    /// Optional discovery hint `host:port`. Empty when discovery is mDNS-only.
    pub addr_hint: String,
}

impl PairingPayload {
    /// Build a payload for the displaying device, generating a fresh token.
    ///
    /// The caller supplies the stable identity fields; the token is generated
    /// here so it is fresh on every QR render (the daemon binds it to a
    /// TTL-limited PAKE session).
    ///
    /// # Errors
    /// Returns [`PairingQrError::EmptyFingerprint`] if `fingerprint` is empty.
    pub fn new(
        fingerprint: impl Into<String>,
        device_id: impl Into<String>,
        device_name: impl Into<String>,
        addr_hint: impl Into<String>,
    ) -> Result<Self, PairingQrError> {
        let fingerprint = fingerprint.into();
        if fingerprint.is_empty() {
            return Err(PairingQrError::EmptyFingerprint);
        }
        Ok(Self {
            fingerprint,
            token: PairingToken::generate(),
            device_id: device_id.into(),
            device_name: device_name.into(),
            addr_hint: addr_hint.into(),
        })
    }

    /// Serialise to the single-line QR string described in the module docs.
    ///
    /// The fingerprint is lowercased but its colons are preserved: the daemon
    /// pairing surface (`is_valid_fingerprint`) expects the user-facing
    /// `XX:XX:...` colon-hex form and canonicalises (strips colons) itself
    /// downstream. Hex digits and `:` never contain the `.` field separator, so
    /// the fingerprint remains a single unambiguous field.
    pub fn encode(&self) -> String {
        let fp = normalize_fingerprint(&self.fingerprint);
        let token_b64 = b64().encode(self.token.0);
        let name_b64 = b64().encode(self.device_name.as_bytes());
        // device_id (UUID) and addr_hint (host:port) are passed through.
        // addr_hint is the final field, so any ':' it contains is harmless.
        format!(
            "{magic}.{fp}.{token_b64}.{device_id}.{name_b64}.{addr_hint}",
            magic = PAIRING_QR_MAGIC,
            fp = fp,
            token_b64 = token_b64,
            device_id = self.device_id,
            name_b64 = name_b64,
            addr_hint = self.addr_hint,
        )
    }

    /// Parse a scanned QR string back into a [`PairingPayload`].
    ///
    /// # Errors
    /// * [`PairingQrError::BadMagic`] — missing/unknown magic+version prefix
    ///   (this is the anti-downgrade guard: a payload without the exact
    ///   `CPPAIR1` prefix is rejected, never best-effort parsed).
    /// * [`PairingQrError::FieldCount`] — wrong number of `.`-separated fields.
    /// * [`PairingQrError::Base64`] — a base64url field failed to decode.
    /// * [`PairingQrError::Utf8`] — the device-name field was not valid UTF-8.
    /// * [`PairingQrError::TokenLength`] — the token was not exactly 32 bytes.
    /// * [`PairingQrError::EmptyFingerprint`] — the fingerprint field was empty.
    pub fn decode(input: &str) -> Result<Self, PairingQrError> {
        let trimmed = input.trim();

        // Anti-downgrade: require the exact magic+version prefix. Split on the
        // first '.' to separate the magic from the body so the magic check is
        // independent of the body's field count.
        let (magic, body) = trimmed.split_once('.').ok_or(PairingQrError::BadMagic)?;
        if magic != PAIRING_QR_MAGIC {
            return Err(PairingQrError::BadMagic);
        }

        // The body has exactly PAIRING_QR_FIELD_COUNT fields. addr_hint is the
        // last field and may itself contain ':' (host:port) but not '.', so
        // splitn keeps it intact even when host:port is empty.
        let parts: Vec<&str> = body.splitn(PAIRING_QR_FIELD_COUNT, '.').collect();
        if parts.len() != PAIRING_QR_FIELD_COUNT {
            return Err(PairingQrError::FieldCount(parts.len()));
        }

        let fingerprint = normalize_fingerprint(parts[0]);
        if fingerprint.is_empty() {
            return Err(PairingQrError::EmptyFingerprint);
        }

        let token_bytes = b64()
            .decode(parts[1])
            .map_err(|e| PairingQrError::Base64(format!("token: {e}")))?;
        let token = PairingToken::from_bytes(&token_bytes)?;

        let device_id = parts[2].to_string();

        let name_bytes = b64()
            .decode(parts[3])
            .map_err(|e| PairingQrError::Base64(format!("name: {e}")))?;
        let device_name = String::from_utf8(name_bytes)
            .map_err(|e| PairingQrError::Utf8(format!("name: {e}")))?;

        let addr_hint = parts[4].to_string();

        Ok(Self {
            fingerprint,
            token,
            device_id,
            device_name,
            addr_hint,
        })
    }
}

/// Lowercase a fingerprint while preserving its colon grouping.
///
/// The colon-hex `XX:XX:...` form is the user-facing identifier the daemon's
/// `is_valid_fingerprint` accepts and that `canonical_fingerprint` later strips
/// for the mTLS verifier. Preserving it here keeps the QR payload compatible
/// with the existing pairing surface without a separate translation step.
fn normalize_fingerprint(fp: &str) -> String {
    fp.to_ascii_lowercase()
}

// ─────────────────────────────────────────────────────────────────────────────
// Errors
// ─────────────────────────────────────────────────────────────────────────────

/// Errors from QR pairing payload encode/decode.
#[derive(Debug, Error)]
pub enum PairingQrError {
    /// The magic + version prefix was missing or not exactly `CPPAIR1`.
    /// Rejecting here (rather than guessing) is the anti-downgrade guard.
    #[error("missing or unsupported pairing QR magic/version (expected {PAIRING_QR_MAGIC})")]
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
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// `from_bytes` test helper that panics with a clear message instead of
    /// `.unwrap()` (which would require `PairingToken: Debug`, deliberately not
    /// implemented to keep the secret out of logs).
    fn token(bytes: [u8; PAIRING_TOKEN_LEN]) -> PairingToken {
        match PairingToken::from_bytes(&bytes) {
            Ok(t) => t,
            Err(e) => panic!("from_bytes must succeed for {PAIRING_TOKEN_LEN} bytes: {e}"),
        }
    }

    fn sample() -> PairingPayload {
        PairingPayload {
            fingerprint: "aabbccddeeff00112233445566778899".to_string(),
            token: token([7u8; PAIRING_TOKEN_LEN]),
            device_id: "11112222-3333-4444-5555-666677778888".to_string(),
            device_name: "Dmytro's MacBook".to_string(),
            addr_hint: "192.168.1.5:54321".to_string(),
        }
    }

    #[test]
    fn token_generate_is_correct_length_and_random() {
        let a = PairingToken::generate();
        let b = PairingToken::generate();
        assert_eq!(a.as_bytes().len(), PAIRING_TOKEN_LEN);
        // Two fresh tokens must (overwhelmingly likely) differ.
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
        // 32 bytes base64url-no-pad = 43 chars, well above the 6-char PAKE
        // minimum the daemon enforces.
        assert!(p1.len() >= 6);
    }

    /// Decode helper that panics with a clear message instead of `.unwrap()`
    /// (which would require `PairingPayload: Debug`).
    fn decode(s: &str) -> PairingPayload {
        match PairingPayload::decode(s) {
            Ok(p) => p,
            Err(e) => panic!("decode must succeed: {e}"),
        }
    }

    /// Returns the error from a decode that is expected to fail. Avoids
    /// `.unwrap_err()`, which would require the Ok type (`PairingPayload`) to
    /// be `Debug`.
    fn decode_err(s: &str) -> PairingQrError {
        match PairingPayload::decode(s) {
            Ok(_) => panic!("decode was expected to fail but succeeded"),
            Err(e) => e,
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let original = sample();
        let encoded = original.encode();
        let decoded = decode(&encoded);

        assert_eq!(decoded.fingerprint, original.fingerprint);
        assert!(decoded.token == original.token);
        assert_eq!(decoded.device_id, original.device_id);
        assert_eq!(decoded.device_name, original.device_name);
        assert_eq!(decoded.addr_hint, original.addr_hint);
    }

    #[test]
    fn encoded_starts_with_magic() {
        let encoded = sample().encode();
        assert!(encoded.starts_with("CPPAIR1."));
    }

    #[test]
    fn decode_rejects_bad_magic() {
        let encoded = sample().encode().replacen("CPPAIR1", "CPPAIR0", 1);
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
        // Magic present but too few fields.
        let err = decode_err("CPPAIR1.aabb.tok");
        assert!(matches!(err, PairingQrError::FieldCount(_)));
    }

    #[test]
    fn decode_rejects_short_token() {
        // Build a payload then swap the token field for a too-short base64url.
        let original = sample();
        let encoded = original.encode();
        let parts: Vec<&str> = encoded.splitn(2, '.').collect();
        let body: Vec<&str> = parts[1].splitn(PAIRING_QR_FIELD_COUNT, '.').collect();
        let short_token = b64().encode([0u8; 8]);
        let tampered = format!(
            "{}.{}.{}.{}.{}.{}",
            PAIRING_QR_MAGIC, body[0], short_token, body[2], body[3], body[4]
        );
        let err = decode_err(&tampered);
        assert!(matches!(err, PairingQrError::TokenLength(8)));
    }

    #[test]
    fn fingerprint_is_lowercased_but_colons_preserved() {
        // The daemon's `is_valid_fingerprint` expects the colon-hex form, so
        // the QR must preserve colons (only case is normalised).
        let payload = PairingPayload {
            fingerprint: "AA:BB:CC:DD".to_string(),
            token: token([0u8; PAIRING_TOKEN_LEN]),
            device_id: "id".to_string(),
            device_name: "n".to_string(),
            addr_hint: String::new(),
        };
        let decoded = decode(&payload.encode());
        assert_eq!(decoded.fingerprint, "aa:bb:cc:dd");
    }

    #[test]
    fn empty_addr_hint_roundtrips() {
        let payload = PairingPayload {
            fingerprint: "deadbeef".to_string(),
            token: token([3u8; PAIRING_TOKEN_LEN]),
            device_id: "dev".to_string(),
            device_name: "Phone".to_string(),
            addr_hint: String::new(),
        };
        let decoded = decode(&payload.encode());
        assert_eq!(decoded.addr_hint, "");
    }

    #[test]
    fn device_name_with_special_chars_roundtrips() {
        let payload = PairingPayload {
            fingerprint: "cafe".to_string(),
            token: token([9u8; PAIRING_TOKEN_LEN]),
            device_id: "dev".to_string(),
            // Dots, colons and unicode would break naive splitting; base64url
            // of the name field protects against that.
            device_name: "A.B:C — café".to_string(),
            addr_hint: "host:1234".to_string(),
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
}
