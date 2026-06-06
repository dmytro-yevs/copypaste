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
//! CPPAIR1.<fp_hex>.<token_b64url>.<device_id>.<name_b64url>.<host:port>[.<prov_b64url>]
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
//! * `prov_b64url` — **optional** sync-provisioning JSON, base64url **without**
//!   padding. Present when the generating device has at least one of relay URL,
//!   Supabase URL, or Supabase anon key configured. The JSON keys are compact
//!   single-letter abbreviations to minimise QR density: `ru` = relay_url,
//!   `su` = supabase_url, `sk` = supabase_anon_key. All values are optional
//!   strings. Old decoders that stop at 5 fields simply ignore the 6th, so there
//!   is no forward-compat break for the 5-field body.
//!
//! Fields are `.`-separated. `fp_hex` is `.`-free hex, `token_b64url` and
//! `name_b64url` use the URL-safe base64 alphabet (no `.`), `device_id` is a
//! `.`-free UUID, and `host:port` is the 5th field — so `.` is an
//! unambiguous separator. The format is deliberately delimiter-based (not
//! JSON/CBOR in the first 5 fields) to keep the QR small: every byte saved
//! lowers the QR version and improves scan reliability.

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

/// Number of `.`-separated fields in the mandatory part of the payload body
/// (after the magic prefix). A 6th optional field carries provisioning JSON.
const PAIRING_QR_FIELD_COUNT: usize = 5;

/// Deep-link URI prefix wrapping the bare [`PAIRING_QR_MAGIC`] payload so that
/// external QR scanners (e.g. Google Lens, the iOS/Android camera app) treat the
/// QR as an actionable link and offer "open in app" instead of plain text.
///
/// The wrapped form is `cppair://pair?p=<percent-encoded CPPAIR1...>`. The bare
/// payload is the value of the `p` query parameter. This is purely a *transport*
/// envelope around the unchanged [`PairingPayload::encode`] wire format — the
/// `AndroidManifest.xml` intent-filter (`scheme="cppair" host="pair"`) and
/// `PairActivity.handleDeepLinkIntent` already extract `?p=` on the receiver side.
///
/// Keep this in sync with the Android `PairActivity` / `CopypasteBindings`
/// constants and the manifest intent-filter.
pub const PAIRING_DEEPLINK_PREFIX: &str = "cppair://pair?p=";

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

// ──────────────────────────���──────────────────────────────────────────────────
// QrProvisioning
// ─────────────────────────────────────────────────────────────────────────────

/// Optional sync-account provisioning embedded in the QR payload (6th field).
///
/// These are all **non-secret** configuration values (URLs + publishable JWT)
/// that let a scanning device inherit the displaying device's sync endpoints
/// without manual configuration. The displaying device (e.g. a configured
/// macOS daemon) encodes them into the QR so the phone can configure relay and
/// Supabase sync automatically at scan time — before the P2P bootstrap tunnel
/// is established.
///
/// Security note: `supabase_anon_key` is a Supabase publishable JWT (the
/// "anon" role, intentionally public). It is NOT a secret and is explicitly
/// safe to embed in a QR code per Supabase's own documentation.
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
    /// Dependency-free: hand-builds the JSON string.
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
    ///
    /// Only extracts string values for known keys. Non-string values, unknown
    /// keys, and structural issues are silently ignored — downstream code
    /// treats missing fields as unconfigured and falls back to the existing
    /// settings, so malformed JSON cannot cause data loss or crashes.
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
    /// Optional sync-account provisioning (relay + Supabase URLs/key). Present
    /// when the generating device is configured for cloud/relay sync and wants
    /// the scanning device to inherit those settings automatically.
    pub provisioning: Option<QrProvisioning>,
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
            provisioning: None,
        })
    }

    /// Serialise to the single-line QR string described in the module docs.
    ///
    /// Emits the 5-field `CPPAIR1.<fp>.<token>.<id>.<name>.<addr_hint>` base
    /// format. When [`Self::provisioning`] is present and non-empty, appends a
    /// 6th field: the provisioning JSON base64url-encoded.
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
        let base = format!(
            "{magic}.{fp}.{token_b64}.{device_id}.{name_b64}.{addr_hint}",
            magic = PAIRING_QR_MAGIC,
            fp = fp,
            token_b64 = token_b64,
            device_id = self.device_id,
            name_b64 = name_b64,
            addr_hint = self.addr_hint,
        );
        // Append optional 6th field for provisioning JSON.
        match &self.provisioning {
            Some(prov) if !prov.is_empty() => format!("{base}.{}", prov.encode()),
            _ => base,
        }
    }

    /// Parse a scanned QR string back into a [`PairingPayload`].
    ///
    /// Accepts an optional 6th `.`-separated field carrying provisioning JSON
    /// (base64url). Old payloads with exactly 5 fields are decoded with
    /// `provisioning: None` — no backward-compat break.
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

        // Split into at most 6 parts to allow the optional provisioning field.
        // The addr_hint (5th field) contains ':' but not '.', so splitn(6)
        // cleanly isolates it even when the 6th provisioning field is absent.
        let parts: Vec<&str> = body.splitn(PAIRING_QR_FIELD_COUNT + 1, '.').collect();
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

        let addr_hint = parts[4].to_string();

        // Decode the optional 6th provisioning field. A decode error here is
        // silently ignored so that a malformed/unknown provisioning blob cannot
        // break pairing — the worst case is the scanner doesn't get the creds.
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
    ///
    /// The bare [`encode`](Self::encode) output is percent-encoded into the `p`
    /// query parameter. The receiver strips the wrapper with
    /// [`strip_deeplink`] (or, on Android, the manifest deep-link path) before
    /// calling [`decode`](Self::decode).
    pub fn encode_deeplink(&self) -> String {
        format!(
            "{PAIRING_DEEPLINK_PREFIX}{}",
            percent_encode_component(&self.encode())
        )
    }
}

/// Strip the [`PAIRING_DEEPLINK_PREFIX`] wrapper from a scanned QR string,
/// returning the bare `CPPAIR1.…` payload ready for [`PairingPayload::decode`].
///
/// Accepts both forms for backward compatibility:
/// * Wrapped: `cppair://pair?p=<percent-encoded CPPAIR1…>` → decoded `p` value.
/// * Bare: any string not starting with the prefix is returned unchanged
///   (trimmed), so legacy QR codes and in-app scans keep working.
///
/// This is intentionally tolerant: it only knows how to *unwrap* the envelope,
/// never to validate the inner payload — that remains [`PairingPayload::decode`]'s
/// job (which still rejects anything without the exact `CPPAIR1` magic).
pub fn strip_deeplink(scanned: &str) -> String {
    let trimmed = scanned.trim();
    match trimmed.strip_prefix(PAIRING_DEEPLINK_PREFIX) {
        Some(encoded) => percent_decode_component(encoded),
        None => trimmed.to_string(),
    }
}

/// Minimal RFC 3986 percent-encoding for the `p` query-component value.
///
/// We encode everything that is not an unreserved character (`A-Z a-z 0-9 - _ .
/// ~`). The bare payload uses `.` and the base64url alphabet (`A-Z a-z 0-9 - _`),
/// all of which are unreserved, so in practice only the rare `:` inside an
/// `addr_hint` (`host:port`) is escaped — but we encode defensively so any future
/// field change stays URL-safe. Kept dependency-free to avoid pulling a URL crate
/// into `copypaste-core`.
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

/// Inverse of [`percent_encode_component`]. Decodes `%XX` escapes (and `+` as a
/// space, matching `application/x-www-form-urlencoded` query semantics) and
/// passes any malformed escape through verbatim so a decode never panics — the
/// downstream [`PairingPayload::decode`] will reject a corrupted payload.
fn percent_decode_component(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                match (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                    (Some(hi), Some(lo)) => {
                        out.push((hi << 4) | lo);
                        i += 3;
                    }
                    // Malformed escape: keep the literal '%' and continue.
                    _ => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
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
    // Lossy is safe: a non-UTF-8 result means a corrupted QR; decode() rejects it.
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

/// Lowercase a fingerprint while preserving its colon grouping.
///
/// The colon-hex `XX:XX:...` form is the user-facing identifier the daemon's
/// `is_valid_fingerprint` accepts and that `canonical_fingerprint` later strips
/// for the mTLS verifier. Preserving it here keeps the QR payload compatible
/// with the existing pairing surface without a separate translation step.
fn normalize_fingerprint(fp: &str) -> String {
    fp.to_ascii_lowercase()
}

/// Build a minimal JSON-escaped string literal: `"value"`.
///
/// Escapes `"` and `\` only — sufficient for URLs and JWTs which never contain
/// control characters. Kept dependency-free.
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
///
/// Only handles simple string values (no nesting). Returns `None` when the key
/// is absent or the value is not a quoted string. Non-ASCII and escaped
/// characters in the value are preserved verbatim (sufficient for URLs/JWTs).
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    // Look for `"key":"`
    let needle = format!("\"{}\":\"", key);
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    // Scan for the closing quote, handling `\"` escapes.
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
// ─────────────────────────────────────────────────��───────────────────────────

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
            provisioning: None,
        }
    }

    fn sample_with_provisioning() -> PairingPayload {
        PairingPayload {
            fingerprint: "aabbccddeeff00112233445566778899".to_string(),
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
        assert!(decoded.provisioning.is_none());
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
            provisioning: None,
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
            provisioning: None,
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
        let original = sample();
        let wrapped = original.encode_deeplink();
        assert!(
            wrapped.starts_with(PAIRING_DEEPLINK_PREFIX),
            "deep-link must carry the cppair://pair?p= prefix: {wrapped}"
        );
        // The bare CPPAIR1 magic must NOT appear before the prefix (i.e. it is
        // wrapped, not concatenated), so external scanners see a URI.
        assert!(wrapped.starts_with("cppair://pair?p="));

        let stripped = strip_deeplink(&wrapped);
        assert!(
            stripped.starts_with("CPPAIR1."),
            "stripping the wrapper must yield the bare payload: {stripped}"
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
        // Back-compat: a bare (unwrapped) CPPAIR1 string must be returned as-is.
        let bare = sample().encode();
        assert_eq!(strip_deeplink(&bare), bare);
        // Whitespace is trimmed.
        let padded = format!("  {bare}  ");
        assert_eq!(strip_deeplink(&padded), bare);
    }

    #[test]
    fn deeplink_escapes_addr_hint_colon() {
        // addr_hint host:port contains ':', which must be percent-escaped in the
        // URI and faithfully restored on strip.
        let original = sample(); // addr_hint = "192.168.1.5:54321"
        let wrapped = original.encode_deeplink();
        assert!(
            wrapped.contains("%3A"),
            "the ':' in host:port must be percent-encoded: {wrapped}"
        );
        let decoded = decode(&strip_deeplink(&wrapped));
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
        // Must have 6 dot-separated fields after the magic.
        let parts: Vec<&str> = encoded.splitn(2, '.').collect();
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
        let original = sample();
        let encoded = original.encode();
        let parts: Vec<&str> = encoded.splitn(2, '.').collect();
        let field_count = parts[1].split('.').count();
        assert_eq!(
            field_count, 5,
            "payload without provisioning must have 5 body fields"
        );
    }

    #[test]
    fn old_payload_without_6th_field_decodes_with_none_provisioning() {
        // Simulate an old payload: strip the 6th field if any, decode — must succeed
        // with provisioning = None (backward compat).
        let old_payload = sample().encode();
        assert!(
            !old_payload.contains("CPPAIR1.") || old_payload.split('.').count() == 6,
            "sample() must not have a provisioning field"
        );
        let decoded = decode(&old_payload);
        assert!(decoded.provisioning.is_none());
    }

    #[test]
    fn malformed_provisioning_field_does_not_break_decode() {
        // Build a valid 5-field payload, then append a corrupt 6th field.
        // decode() must still succeed (with provisioning = None) — the
        // provisioning field is advisory and must never break pairing.
        let base = sample().encode();
        let corrupt = format!("{base}.!!!notbase64!!!");
        let decoded = decode(&corrupt);
        // Core pairing fields must be intact.
        assert_eq!(decoded.fingerprint, sample().fingerprint);
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
}
