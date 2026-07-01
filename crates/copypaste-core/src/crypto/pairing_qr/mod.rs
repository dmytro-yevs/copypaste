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

#[cfg(test)]
mod tests;

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
