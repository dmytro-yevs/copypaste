//! Wire-encoding helpers for the QR pairing payload.
//!
//! This module contains all the encoding/decoding utilities that are NOT
//! cryptographically sensitive: percent-encode/decode, deeplink wrapping,
//! hex↔base64url conversion helpers for fingerprints and UUIDs, and the
//! hand-rolled minimal JSON builder/parser used by [`super::QrProvisioning`].

use super::{b64, PAIRING_DEEPLINK_PREFIX};
use base64::Engine as _;

// ─────────────────────────────────────────────────────────────────────────────
// Deep-link helpers
// ─────────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
// Percent-encode / decode (RFC 3986)
// ─────────────────────────────────────────────────────────────────────────────

/// Minimal RFC 3986 percent-encoding for the `p` query-component value.
///
/// Encodes everything that is not an unreserved character (`A-Z a-z 0-9 - _ . ~`).
pub(super) fn percent_encode_component(input: &str) -> String {
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

// ─────────────────────────────────────────────────────────────────────────────
// Fingerprint / UUID helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Lowercase a fingerprint while preserving its colon grouping (used for CPPAIR1).
///
/// The colon-hex `XX:XX:...` form is the user-facing identifier the daemon's
/// `is_valid_fingerprint` accepts and that `canonical_fingerprint` later strips
/// for the mTLS verifier. Preserving it here keeps the v1 QR payload compatible
/// with the existing pairing surface without a separate translation step.
pub(super) fn normalize_fingerprint(fp: &str) -> String {
    fp.to_ascii_lowercase()
}

/// Encode a hex or colon-hex fingerprint as base64url (no padding) for CPPAIR2.
///
/// Strips colons, hex-decodes the remaining bytes, then base64url-encodes.
/// A valid SHA-256 fingerprint (32 bytes = 64 hex chars) yields 43 base64url chars.
/// If decoding fails (e.g. non-hex chars other than `:`) we return the b64url of
/// whatever bytes were decoded — the downstream decoder will reject the wrong length.
pub(super) fn fp_hex_to_b64url(fp: &str) -> String {
    // Strip colons to normalise both "aabbcc..." and "aa:bb:cc:..." forms.
    let hex_only: String = fp.chars().filter(|&c| c != ':').collect();
    match hex::decode(hex_only.to_ascii_lowercase()) {
        Ok(bytes) => b64().encode(&bytes),
        // Non-hex input: encode the raw UTF-8 bytes so the string is non-empty;
        // decode_v2 will reject it with FingerprintLength.
        Err(_) => b64().encode(fp.as_bytes()),
    }
}

/// Encode a UUID string as base64url (no padding) for CPPAIR2.
///
/// Parses the UUID string (with or without hyphens) to its 16 raw bytes, then
/// base64url-encodes them (22 chars). If parsing fails the raw UTF-8 bytes are
/// encoded instead — decode_v2 will reject the wrong length.
pub(super) fn uuid_str_to_b64url(uuid: &str) -> String {
    // Strip hyphens and hex-decode the 32 remaining hex chars → 16 bytes.
    let hex_only: String = uuid.chars().filter(|&c| c != '-').collect();
    match hex::decode(hex_only) {
        Ok(bytes) => b64().encode(&bytes),
        // Non-UUID input: encode raw UTF-8; decode_v2 will reject the wrong length.
        Err(_) => b64().encode(uuid.as_bytes()),
    }
}

/// Format 16 UUID bytes as a standard hyphenated UUID string (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`).
pub(super) fn uuid_bytes_to_str(bytes: &[u8; super::UUID_BYTE_LEN]) -> String {
    // Standard UUID layout: 4-2-2-2-6 groups, all lowercase hex.
    format!(
        "{}-{}-{}-{}-{}",
        hex::encode(&bytes[0..4]),
        hex::encode(&bytes[4..6]),
        hex::encode(&bytes[6..8]),
        hex::encode(&bytes[8..10]),
        hex::encode(&bytes[10..16]),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Minimal hand-rolled JSON builder / parser
// ─────────────────────────────────────────────────────────────────────────────

/// Build a minimal JSON-escaped string literal: `"value"`.
///
/// Escapes `"` and `\` only — sufficient for URLs and JWTs which never contain
/// control characters. Kept dependency-free.
pub(super) fn json_str(s: &str) -> String {
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
pub(super) fn extract_json_string(json: &str, key: &str) -> Option<String> {
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
