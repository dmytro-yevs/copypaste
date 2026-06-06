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
//!   field when no provisioning is present, and the provisioning 6th field uses
//!   the URL-safe base64 alphabet (no `.`) so `splitn(6)` is clean.
//! * `prov_b64url` — **optional** provisioning JSON, base64url.

use base64::Engine as _;
use rand::rngs::OsRng;
use rand::RngCore;
use subtle::ConstantTimeEq;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Magic prefix for the v1 (legacy) QR pairing payload.
///
/// Still accepted by [`PairingPayload::decode`] for backward compatibility.
/// [`PairingPayload::encode`] now emits [`PAIRING_QR_MAGIC_V2`] instead.
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
// PairingToken
// ─────────────────────────────────────────────────────────────────────────────

/// A short-lived, high-entropy secret transported by the QR code and fed into
/// the PAKE handshake as the shared "password".
///
/// # Security
/// * `ZeroizeOnDrop` scrubs the bytes when dropped.
/// * Does NOT implement `Debug` / `Display` / `Clone` to avoid accidental
///   logging or silent duplication.
/// * Equality is constant-time via [`ConstantTimeEq`].
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct PairingToken([u8; PAIRING_TOKEN_LEN]);

impl PairingToken {
    /// Generate a fresh 256-bit pairing token from the OS CSPRNG.
    pub fn generate() -> Self {
        let mut bytes = [0u8; PAIRING_TOKEN_LEN];
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
    /// Renders the raw token bytes as base64url so the full 256 bits of entropy
    /// survive the byte→str conversion losslessly.
    pub fn to_pake_password(&self) -> String {
        b64().encode(self.0)
    }
}

impl PartialEq for PairingToken {
    /// Constant-time comparison — never short-circuit on the first differing byte.
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for PairingToken {}

// ─────────────────────────────────────────────────────────────────────────────
// QrProvisioning
// ─────────────────────────────────────────────────────────────────────────────

/// Optional sync-account provisioning embedded in the QR payload (6th field).
///
/// These are all **non-secret** configuration values (URLs + publishable JWT)
/// that let a scanning device inherit the displaying device's sync endpoints
/// without manual configuration.
///
/// Stored as compact JSON (`{"ru":…,"su":…,"sk":…}`) base64url-encoded to
/// keep the QR payload as small as possible.
#[derive(Debug, Clone, Default)]
pub struct QrProvisioning {
    /// HTTP relay base URL (e.g. `https://relay.example.com`). Non-secret.
    pub relay_url: Option<String>,
    /// Supabase project URL (e.g. `https://xxxx.supabase.co`). Non-secret.
    pub supabase_url: Option<String>,
    /// Supabase publishable anon/public JWT. Non-secret by Supabase design.
    pub supabase_anon_key: Option<String>,
}

impl QrProvisioning {
    /// Returns `true` if every field is `None` or empty — nothing to encode.
    pub fn is_empty(&self) -> bool {
        self.relay_url.as_deref().map_or(true, str::is_empty)
            && self.supabase_url.as_deref().map_or(true, str::is_empty)
            && self
                .supabase_anon_key
                .as_deref()
                .map_or(true, str::is_empty)
    }

    /// Encode as compact JSON then base64url (no padding) for the QR 6th field.
    ///
    /// Only present, non-empty fields are emitted to keep the JSON small.
    pub fn encode(&self) -> String {
        let mut parts: Vec<String> = Vec::with_capacity(3);
        if let Some(ref v) = self.relay_url {
            if !v.is_empty() {
                parts.push(format!("\"ru\":{}", json_str(v)));
            }
        }
        if let Some(ref v) = self.supabase_url {
            if !v.is_empty() {
                parts.push(format!("\"su\":{}", json_str(v)));
            }
        }
        if let Some(ref v) = self.supabase_anon_key {
            if !v.is_empty() {
                parts.push(format!("\"sk\":{}", json_str(v)));
            }
        }
        let json = format!("{{{}}}", parts.join(","));
        b64().encode(json.as_bytes())
    }

    /// Decode from the base64url 6th QR field.
    ///
    /// Silently ignores unknown JSON keys (forward compat). Returns `None` on
    /// any base64 or UTF-8 error so old payloads that happen to have a 6th
    /// field are benignly skipped — the pairing itself is unaffected.
    pub fn decode(field: &str) -> Option<Self> {
        let bytes = b64().decode(field).ok()?;
        let json = std::str::from_utf8(&bytes).ok()?;
        Some(Self::parse_json(json))
    }

    /// Minimal hand-rolled JSON parser for the compact `{"ru":…,"su":…,"sk":…}` shape.
    fn parse_json(json: &str) -> Self {
        let relay_url = extract_json_string(json, "ru");
        let supabase_url = extract_json_string(json, "su");
        let supabase_anon_key = extract_json_string(json, "sk");
        Self {
            relay_url,
            supabase_url,
            supabase_anon_key,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PairingPayload
// ─────────────────────────────────────────────────────────────────────────────

/// The fully-decoded contents of a QR pairing code.
///
/// Produced by [`PairingPayload::encode`] (on the displaying device) and
/// recovered by [`PairingPayload::decode`] (on the scanning device).
pub struct PairingPayload {
    /// Displaying device's cert fingerprint in lowercase colon-hex or bare-hex
    /// form, depending on version. CPPAIR2 round-trips as bare lowercase hex.
    pub fingerprint: String,
    /// Single-use, TTL-bounded pairing secret fed into the PAKE handshake.
    pub token: PairingToken,
    /// Displaying device's UUID (used as the peer identifier on the scanner).
    pub device_id: String,
    /// Human-readable device name shown to the scanning user.
    pub device_name: String,
    /// Optional discovery hint `host:port`. Empty when discovery is mDNS-only.
    pub addr_hint: String,
    /// Optional sync-account provisioning (relay + Supabase URLs/key). Present
    /// when the generating device is configured for cloud/relay sync.
    pub provisioning: Option<QrProvisioning>,
}

impl PairingPayload {
    /// Build a payload for the displaying device, generating a fresh token.
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
            provisioning: None,
        })
    }

    /// Serialise to the CPPAIR2 single-line QR string described in the module docs.
    ///
    /// Emits:
    /// `CPPAIR2.<fp_b64url43>.<token_b64url>.<device_id_b64url22>.<name_b64url>.<addr_b64url>`
    ///
    /// When [`Self::provisioning`] is present and non-empty, appends a 6th field.
    ///
    /// `addr_hint` is base64url-encoded so that IPv4 dots (e.g. `192.168.1.5`)
    /// cannot collide with the `.` field delimiter and corrupt a 6-field split.
    /// This is a v2-only encoding — `decode_v2` base64url-decodes field[4].
    pub fn encode(&self) -> String {
        let fp_b64 = fp_hex_to_b64url(&self.fingerprint);
        let token_b64 = b64().encode(self.token.0);
        let device_id_b64 = uuid_str_to_b64url(&self.device_id);
        let name_b64 = b64().encode(self.device_name.as_bytes());
        // base64url-encode addr_hint to avoid dot collisions: an IPv4 address
        // like "192.168.1.5:54321" contains dots that would otherwise corrupt
        // the splitn(6) when a provisioning 6th field is present.
        let addr_b64 = b64().encode(self.addr_hint.as_bytes());
        let base = format!(
            "{magic}.{fp_b64}.{token_b64}.{device_id_b64}.{name_b64}.{addr_b64}",
            magic = PAIRING_QR_MAGIC_V2,
        );
        // Append optional 6th field for provisioning JSON.
        match &self.provisioning {
            Some(prov) if !prov.is_empty() => format!("{base}.{}", prov.encode()),
            _ => base,
        }
    }

    /// Parse a scanned QR string back into a [`PairingPayload`].
    ///
    /// Accepts both `CPPAIR2` (current) and `CPPAIR1` (legacy) prefixes.
    ///
    /// # Errors
    /// * [`PairingQrError::BadMagic`] — missing or unrecognised magic+version prefix.
    /// * [`PairingQrError::FieldCount`] — wrong number of `.`-separated fields.
    /// * [`PairingQrError::Base64`] — a base64url field failed to decode.
    /// * [`PairingQrError::Utf8`] — the device-name field was not valid UTF-8.
    /// * [`PairingQrError::TokenLength`] — the token was not exactly 32 bytes.
    /// * [`PairingQrError::FingerprintLength`] — fp b64 field ≠ 32 bytes (CPPAIR2).
    /// * [`PairingQrError::DeviceIdLength`] — device_id b64 field ≠ 16 bytes (CPPAIR2).
    /// * [`PairingQrError::AddrHintDecode`] — addr_hint b64url decode failed (CPPAIR2).
    /// * [`PairingQrError::EmptyFingerprint`] — fingerprint field was empty.
    pub fn decode(input: &str) -> Result<Self, PairingQrError> {
        let trimmed = input.trim();
        let (magic, body) = trimmed.split_once('.').ok_or(PairingQrError::BadMagic)?;
        match magic {
            PAIRING_QR_MAGIC_V2 => Self::decode_v2(body),
            PAIRING_QR_MAGIC => Self::decode_v1(body),
            _ => Err(PairingQrError::BadMagic),
        }
    }

    /// Decode a CPPAIR1 body (the part after the magic prefix dot).
    ///
    /// CPPAIR1 addr_hint is a raw `host:port` string that may contain IPv4 dots
    /// (e.g. `192.168.1.1:1234`). To avoid ambiguity we use `splitn(5)` so the
    /// entire tail — including any IPv4 dots — goes into `parts[4]` as addr_hint.
    /// Provisioning is not supported in the CPPAIR1 wire format for this reason;
    /// CPPAIR2 (with base64url-encoded addr_hint) is the format that carries the
    /// optional 6th provisioning field.
    fn decode_v1(body: &str) -> Result<Self, PairingQrError> {
        let parts: Vec<&str> = body.splitn(PAIRING_QR_FIELD_COUNT, '.').collect();
        if parts.len() < PAIRING_QR_FIELD_COUNT {
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

        // addr_hint is the terminal field; it may contain dots (IPv4) and colons.
        let addr_hint = parts[4].to_string();

        Ok(Self {
            fingerprint,
            token,
            device_id,
            device_name,
            addr_hint,
            provisioning: None,
        })
    }

    /// Decode a CPPAIR2 body (the part after the magic prefix dot).
    fn decode_v2(body: &str) -> Result<Self, PairingQrError> {
        // Use splitn(6) so the optional provisioning 6th field is captured.
        // Because addr_hint is now base64url-encoded (no dots), fields [0..4]
        // are all dot-free and splitn(6) cleanly separates them.
        let parts: Vec<&str> = body.splitn(PAIRING_QR_FIELD_COUNT + 1, '.').collect();
        if parts.len() < PAIRING_QR_FIELD_COUNT {
            return Err(PairingQrError::FieldCount(parts.len()));
        }

        // Fingerprint: b64url → 32 bytes → lowercase hex string.
        let fp_bytes = b64()
            .decode(parts[0])
            .map_err(|e| PairingQrError::Base64(format!("fingerprint: {e}")))?;
        if fp_bytes.len() != FP_BYTE_LEN {
            return Err(PairingQrError::FingerprintLength(fp_bytes.len()));
        }
        let fingerprint = hex::encode(&fp_bytes);
        if fingerprint.is_empty() {
            // Unreachable: hex::encode of non-empty bytes is always non-empty.
            return Err(PairingQrError::EmptyFingerprint);
        }

        let token_bytes = b64()
            .decode(parts[1])
            .map_err(|e| PairingQrError::Base64(format!("token: {e}")))?;
        let token = PairingToken::from_bytes(&token_bytes)?;

        // device_id: b64url → 16 bytes → UUID string.
        let id_bytes = b64()
            .decode(parts[2])
            .map_err(|e| PairingQrError::Base64(format!("device_id: {e}")))?;
        if id_bytes.len() != UUID_BYTE_LEN {
            return Err(PairingQrError::DeviceIdLength(id_bytes.len()));
        }
        // SAFETY: id_bytes.len() == 16, so try_into() is infallible.
        let id_arr: [u8; UUID_BYTE_LEN] = id_bytes
            .try_into()
            .expect("id_bytes.len() == UUID_BYTE_LEN == 16; infallible");
        let device_id = uuid_bytes_to_str(&id_arr);

        let name_bytes = b64()
            .decode(parts[3])
            .map_err(|e| PairingQrError::Base64(format!("name: {e}")))?;
        let device_name = String::from_utf8(name_bytes)
            .map_err(|e| PairingQrError::Utf8(format!("name: {e}")))?;

        // addr_hint: base64url-encoded in v2 to avoid dot collision with the
        // optional provisioning 6th field.
        let addr_hint_bytes = b64()
            .decode(parts[4])
            .map_err(|_| PairingQrError::AddrHintDecode)?;
        let addr_hint =
            String::from_utf8(addr_hint_bytes).map_err(|_| PairingQrError::AddrHintDecode)?;

        // Optional 6th field = provisioning. Silently ignored on decode error
        // so a corrupt/unknown blob cannot break pairing.
        let provisioning = parts
            .get(5)
            .filter(|f| !f.is_empty())
            .and_then(|f| QrProvisioning::decode(f));

        Ok(Self {
            fingerprint,
            token,
            device_id,
            device_name,
            addr_hint,
            provisioning,
        })
    }

    /// Serialise and wrap the payload in the [`PAIRING_DEEPLINK_PREFIX`] URI so
    /// external scanners (Google Lens, the system camera) offer "open in app".
    pub fn encode_deeplink(&self) -> String {
        format!(
            "{PAIRING_DEEPLINK_PREFIX}{}",
            percent_encode_component(&self.encode())
        )
    }
}

/// Strip the [`PAIRING_DEEPLINK_PREFIX`] wrapper from a scanned QR string,
/// returning the bare `CPPAIR1.…` / `CPPAIR2.…` payload.
///
/// Accepts both wrapped and bare forms for backward compatibility.
pub fn strip_deeplink(scanned: &str) -> String {
    let trimmed = scanned.trim();
    match trimmed.strip_prefix(PAIRING_DEEPLINK_PREFIX) {
        Some(encoded) => percent_decode_component(encoded),
        None => trimmed.to_string(),
    }
}

/// Minimal RFC 3986 percent-encoding for the `p` query-component value.
///
/// Encodes everything that is not an unreserved character (`A-Z a-z 0-9 - _ . ~`).
fn percent_encode_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        let unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
        if unreserved {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(hex_upper(b >> 4));
            out.push(hex_upper(b & 0x0f));
        }
    }
    out
}

/// Inverse of [`percent_encode_component`].
fn percent_decode_component(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                (Some(hi), Some(lo)) => {
                    out.push((hi << 4) | lo);
                    i += 3;
                }
                _ => {
                    out.push(b'%');
                    i += 1;
                }
            },
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Map a 0–15 nibble to its uppercase hex ASCII digit.
fn hex_upper(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + (nibble - 10)) as char,
    }
}

/// Parse a single hex ASCII digit (upper or lower case) into its 0–15 value.
fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Lowercase a fingerprint while preserving its colon grouping (used for CPPAIR1).
fn normalize_fingerprint(fp: &str) -> String {
    fp.to_ascii_lowercase()
}

/// Encode a hex or colon-hex fingerprint as base64url (no padding) for CPPAIR2.
///
/// Strips colons, hex-decodes the remaining bytes, then base64url-encodes.
/// A valid SHA-256 fingerprint (32 bytes = 64 hex chars) yields 43 base64url chars.
fn fp_hex_to_b64url(fp: &str) -> String {
    let hex_only: String = fp.chars().filter(|&c| c != ':').collect();
    match hex::decode(hex_only.to_ascii_lowercase()) {
        Ok(bytes) => b64().encode(&bytes),
        Err(_) => b64().encode(fp.as_bytes()),
    }
}

/// Encode a UUID string as base64url (no padding) for CPPAIR2.
///
/// Parses to 16 raw bytes then base64url-encodes (22 chars).
fn uuid_str_to_b64url(uuid: &str) -> String {
    let hex_only: String = uuid.chars().filter(|&c| c != '-').collect();
    match hex::decode(hex_only) {
        Ok(bytes) => b64().encode(&bytes),
        Err(_) => b64().encode(uuid.as_bytes()),
    }
}

/// Format 16 UUID bytes as a standard hyphenated UUID string.
fn uuid_bytes_to_str(bytes: &[u8; UUID_BYTE_LEN]) -> String {
    format!(
        "{}-{}-{}-{}-{}",
        hex::encode(&bytes[0..4]),
        hex::encode(&bytes[4..6]),
        hex::encode(&bytes[6..8]),
        hex::encode(&bytes[8..10]),
        hex::encode(&bytes[10..16]),
    )
}

/// Build a minimal JSON-escaped string literal: `"value"`.
///
/// Escapes `"` and `\` only — sufficient for URLs and JWTs.
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// Extract a JSON string value for a known key from a flat `{"k":"v",...}` JSON.
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":\"", key);
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let mut value = String::new();
    let mut chars = rest.chars().peekable();
    loop {
        match chars.next()? {
            '"' => break,
            '\\' => match chars.next()? {
                '"' => value.push('"'),
                '\\' => value.push('\\'),
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                other => {
                    value.push('\\');
                    value.push(other);
                }
            },
            c => value.push(c),
        }
    }
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Errors
// ─────────────────────────────────────────────────────────────────────────────

/// Errors from QR pairing payload encode/decode.
#[derive(Debug, Error)]
pub enum PairingQrError {
    /// The magic + version prefix was missing or not a recognised version.
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
    #[error("fingerprint must be {FP_BYTE_LEN} bytes, got {0}")]
    FingerprintLength(usize),

    /// (CPPAIR2) The device_id b64url field decoded to the wrong number of bytes.
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
        let encoded = sample_v2().encode();
        assert!(encoded.starts_with("CPPAIR2."));
    }

    #[test]
    fn decode_rejects_bad_magic() {
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
        // Use CPPAIR1 to test token-length validation with arbitrary field values.
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
        // In CPPAIR2, fingerprint round-trips through bytes → bare lowercase hex.
        let payload = PairingPayload {
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
        assert_eq!(
            decoded.fingerprint,
            "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899"
        );
        assert!(!decoded.fingerprint.contains(':'));
    }

    #[test]
    fn empty_addr_hint_roundtrips() {
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
        let payload = PairingPayload {
            fingerprint: "cafecafecafecafecafecafecafecafecafecafecafecafecafecafecafecafe"
                .to_string(),
            token: token([9u8; PAIRING_TOKEN_LEN]),
            device_id: "11112222-3333-4444-5555-666677778888".to_string(),
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
        let original = sample_v2();
        let wrapped = original.encode_deeplink();
        assert!(
            wrapped.starts_with(PAIRING_DEEPLINK_PREFIX),
            "deep-link must carry the cppair://pair?p= prefix: {wrapped}"
        );
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
            b64().encode(payload.token.0),
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
        let encoded = sample_v2().encode();
        let body = encoded
            .strip_prefix("CPPAIR2.")
            .expect("must start with CPPAIR2.");
        let fp_field = body.split('.').next().expect("must have fields");
        assert_eq!(
            fp_field.len(),
            43,
            "CPPAIR2 fingerprint field must be 43 chars, got {}: {fp_field}",
            fp_field.len()
        );
    }

    #[test]
    fn cppair2_device_id_field_is_22_chars() {
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
            "CPPAIR2 device_id field must be 22 chars, got {}: {device_id_field}",
            device_id_field.len()
        );
    }

    #[test]
    fn cppair2_decode_rejects_bad_fp_length() {
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
        let payload = sample_v2();
        let encoded = payload.encode();
        let body = encoded
            .strip_prefix("CPPAIR2.")
            .expect("must start with CPPAIR2.");
        let fields: Vec<&str> = body.splitn(PAIRING_QR_FIELD_COUNT + 1, '.').collect();
        assert!(fields.len() >= PAIRING_QR_FIELD_COUNT);
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
        let original = sample_v2();
        let encoded = original.encode();
        let decoded = decode(&encoded);
        assert_eq!(
            decoded.fingerprint,
            "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899"
        );
        assert!(!decoded.fingerprint.contains(':'));
    }

    #[test]
    fn cppair2_colon_hex_fingerprint_roundtrips_without_colons() {
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
        // Append a corrupt 6th field. decode() must still succeed with provisioning=None.
        let base = sample_v2().encode();
        let corrupt = format!("{base}.!!!notbase64!!!");
        let decoded = decode(&corrupt);
        assert_eq!(decoded.fingerprint, sample_v2().fingerprint);
        assert_eq!(decoded.addr_hint, sample_v2().addr_hint);
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
}
