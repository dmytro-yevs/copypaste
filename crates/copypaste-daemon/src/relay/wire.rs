//! Relay wire-payload framing (CopyPaste-crh3.69).
//!
//! The relay is a pure opaque passthrough: it stores and echoes back exactly one
//! per-item field of interest — `content_b64` — and never inspects it. All
//! cross-device metadata (item_id, lamport_ts, delete/pin state, LWW tie-break
//! keys) therefore has to travel *inside* `content_b64`.
//!
//! # The double-base64 bug
//!
//! The original V1 framing was
//!
//! ```text
//! content_b64 = base64( JSON{ item_id, lamport_ts, ct_b64: base64(ciphertext), ... } )
//! ```
//!
//! i.e. the ciphertext was base64-encoded into `ct_b64`, embedded in a JSON
//! object, and then the **whole JSON object was base64-encoded again**. Because
//! the JSON is dominated by the already-base64 `ct_b64` field, the outer base64
//! inflates the wire payload by ~33 % over the necessary single encoding.
//!
//! # V2 framing (single base64)
//!
//! ```text
//! content_b64 = base64( 0x01 || u32_le(meta_len) || meta_json || raw_ciphertext )
//! ```
//!
//! The ciphertext is carried **raw** as the frame tail, so it is base64-encoded
//! exactly once (by the single outer base64). The small metadata JSON
//! (`item_id`, `lamport_ts`, `deleted`, `pinned`, `pin_order`, `wall_time`,
//! `origin_device_id` — the same fields V1 carried, minus `ct_b64`) is
//! length-prefixed ahead of it.
//!
//! # Backward compatibility (version-gated dual decode)
//!
//! The leading decoded byte is the wire version discriminator:
//!
//! * `0x7B` (`{`) → **V1**: the bytes are the legacy `JSON(RelayEnvelope)`.
//! * [`RELAY_WIRE_V2`] (`0x01`) → **V2**: the length-prefixed frame above.
//!
//! `0x01` can never be the first byte of a JSON object/array/string/number, so
//! the two formats are unambiguous. On **receive** a daemon decodes BOTH formats
//! (so in-flight inbox items written by older daemons still ingest); on **send**
//! a daemon emits V2 only.

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use super::types::RelayEnvelope;
use crate::sync_common::decode_payload_ct;

/// Wire version discriminator for the single-base64 frame.
///
/// Chosen as `0x01` because it is not a valid leading byte of any JSON value, so
/// it can never collide with a legacy `base64(JSON(..))` payload (whose first
/// decoded byte is always `{` = `0x7B`).
pub(super) const RELAY_WIRE_V2: u8 = 0x01;

/// Metadata carried in the V2 frame header. Identical serde field names to
/// [`RelayEnvelope`] **minus** `ct_b64` (the ciphertext is now the raw frame
/// tail rather than a base64 field). All non-identity fields default so a future
/// reader stays lenient.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct RelayWireMeta {
    pub(super) item_id: String,
    pub(super) lamport_ts: i64,
    #[serde(default)]
    pub(super) deleted: bool,
    #[serde(default)]
    pub(super) pinned: bool,
    #[serde(default)]
    pub(super) pin_order: Option<f64>,
    #[serde(default)]
    pub(super) wall_time: i64,
    #[serde(default)]
    pub(super) origin_device_id: String,
}

/// A wire-version-agnostic decoded relay payload: metadata plus the RAW
/// ciphertext bytes (empty for a tombstone). Both V1 and V2 funnel into this so
/// the ingest path is wire-format-independent.
pub(super) struct DecodedRelayPayload {
    pub(super) item_id: String,
    pub(super) lamport_ts: i64,
    pub(super) deleted: bool,
    pub(super) pinned: bool,
    pub(super) pin_order: Option<f64>,
    pub(super) wall_time: i64,
    pub(super) origin_device_id: String,
    /// Raw cloud-encrypted ciphertext (the `encrypt_for_cloud` blob). Empty for
    /// a tombstone envelope.
    pub(super) ct: Vec<u8>,
}

/// Encode a V2 frame: `base64( 0x01 || u32_le(meta_len) || meta_json || ct )`.
///
/// `ct` is the RAW cloud ciphertext (NOT base64); pass an empty slice for a
/// tombstone. Returns `None` only if the metadata fails to serialize (infallible
/// in practice) or is implausibly large (> `u32::MAX` bytes).
pub(super) fn encode_v2(meta: &RelayWireMeta, ct: &[u8]) -> Option<String> {
    let meta_json = serde_json::to_vec(meta).ok()?;
    let meta_len = u32::try_from(meta_json.len()).ok()?;
    let mut frame = Vec::with_capacity(1 + 4 + meta_json.len() + ct.len());
    frame.push(RELAY_WIRE_V2);
    frame.extend_from_slice(&meta_len.to_le_bytes());
    frame.extend_from_slice(&meta_json);
    frame.extend_from_slice(ct);
    Some(base64::engine::general_purpose::STANDARD.encode(&frame))
}

/// Decode a relay `content_b64` of EITHER wire version into a
/// [`DecodedRelayPayload`]. Returns `Err` with a short reason on malformed
/// input (the caller logs + skips the row).
pub(super) fn decode_payload(content_b64: &str) -> Result<DecodedRelayPayload, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(content_b64)
        .map_err(|e| format!("content_b64 base64 decode: {e}"))?;
    match bytes.first() {
        None => Err("empty content_b64 frame".to_string()),
        // V2: length-prefixed binary frame.
        Some(&RELAY_WIRE_V2) => decode_v2_frame(&bytes),
        // V1 (legacy): the decoded bytes are `JSON(RelayEnvelope)` (starts with `{`).
        Some(&b'{') => decode_v1_envelope(&bytes),
        Some(other) => Err(format!("unknown relay wire version byte 0x{other:02x}")),
    }
}

/// Decode a V2 length-prefixed frame body (the already-base64-decoded bytes,
/// including the leading [`RELAY_WIRE_V2`] marker).
fn decode_v2_frame(bytes: &[u8]) -> Result<DecodedRelayPayload, String> {
    // [0]=marker, [1..5]=u32_le meta_len, [5..5+meta_len]=meta_json, rest=ct.
    if bytes.len() < 5 {
        return Err("v2 frame too short for length prefix".to_string());
    }
    let meta_len = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
    let meta_start = 5usize;
    let meta_end = meta_start
        .checked_add(meta_len)
        .ok_or_else(|| "v2 meta length overflow".to_string())?;
    if bytes.len() < meta_end {
        return Err("v2 frame truncated before end of metadata".to_string());
    }
    let meta: RelayWireMeta = serde_json::from_slice(&bytes[meta_start..meta_end])
        .map_err(|e| format!("v2 meta json parse: {e}"))?;
    let ct = bytes[meta_end..].to_vec();
    Ok(DecodedRelayPayload {
        item_id: meta.item_id,
        lamport_ts: meta.lamport_ts,
        deleted: meta.deleted,
        pinned: meta.pinned,
        pin_order: meta.pin_order,
        wall_time: meta.wall_time,
        origin_device_id: meta.origin_device_id,
        ct,
    })
}

/// Decode a legacy V1 `JSON(RelayEnvelope)` body (already base64-decoded bytes).
fn decode_v1_envelope(bytes: &[u8]) -> Result<DecodedRelayPayload, String> {
    let env: RelayEnvelope =
        serde_json::from_slice(bytes).map_err(|e| format!("v1 envelope json parse: {e}"))?;
    // ct_b64 is empty for a tombstone; decode_payload_ct accepts both bare
    // base64 and `\x`-hex (the same helper the pre-fix path used).
    let ct = if env.ct_b64.is_empty() {
        Vec::new()
    } else {
        decode_payload_ct(&env.ct_b64).map_err(|e| format!("v1 ct decode: {e}"))?
    };
    Ok(DecodedRelayPayload {
        item_id: env.item_id,
        lamport_ts: env.lamport_ts,
        deleted: env.deleted,
        pinned: env.pinned,
        pin_order: env.pin_order,
        wall_time: env.wall_time,
        origin_device_id: env.origin_device_id,
        ct,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_meta() -> RelayWireMeta {
        RelayWireMeta {
            item_id: "item-abc-123".to_string(),
            lamport_ts: 42,
            deleted: false,
            pinned: true,
            pin_order: Some(3.5),
            wall_time: 1700000000123,
            origin_device_id: "dev-origin".to_string(),
        }
    }

    /// Build the legacy V1 wire string for the SAME logical item, so size and
    /// backward-compat can be compared against V2.
    fn legacy_v1_wire(meta: &RelayWireMeta, ct: &[u8]) -> String {
        let ct_b64 = base64::engine::general_purpose::STANDARD.encode(ct);
        let env = RelayEnvelope {
            item_id: meta.item_id.clone(),
            lamport_ts: meta.lamport_ts,
            ct_b64,
            deleted: meta.deleted,
            pinned: meta.pinned,
            pin_order: meta.pin_order,
            wall_time: meta.wall_time,
            origin_device_id: meta.origin_device_id.clone(),
        };
        base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&env).unwrap())
    }

    /// V2 round-trips: encode → decode recovers every metadata field and the
    /// exact raw ciphertext bytes.
    #[test]
    fn v2_round_trips() {
        let meta = sample_meta();
        let ct: Vec<u8> = (0..512u32).map(|i| (i % 256) as u8).collect();
        let wire = encode_v2(&meta, &ct).expect("encode v2");

        // First decoded byte is the V2 marker, NOT a JSON brace.
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&wire)
            .unwrap();
        assert_eq!(raw[0], RELAY_WIRE_V2);
        assert_ne!(raw[0], b'{');

        let got = decode_payload(&wire).expect("decode v2");
        assert_eq!(got.item_id, meta.item_id);
        assert_eq!(got.lamport_ts, meta.lamport_ts);
        assert_eq!(got.deleted, meta.deleted);
        assert_eq!(got.pinned, meta.pinned);
        assert_eq!(got.pin_order, meta.pin_order);
        assert_eq!(got.wall_time, meta.wall_time);
        assert_eq!(got.origin_device_id, meta.origin_device_id);
        assert_eq!(got.ct, ct);
    }

    /// Backward compat: a legacy V1 (double-base64) payload still decodes via the
    /// version-gated path and recovers the same fields + ciphertext.
    #[test]
    fn v1_legacy_still_decodes() {
        let meta = sample_meta();
        let ct: Vec<u8> = (0..300u32).map(|i| (i % 256) as u8).collect();
        let legacy = legacy_v1_wire(&meta, &ct);

        // The legacy frame's first decoded byte is a JSON brace.
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&legacy)
            .unwrap();
        assert_eq!(raw[0], b'{');

        let got = decode_payload(&legacy).expect("decode legacy v1");
        assert_eq!(got.item_id, meta.item_id);
        assert_eq!(got.lamport_ts, meta.lamport_ts);
        assert_eq!(got.pinned, meta.pinned);
        assert_eq!(got.pin_order, meta.pin_order);
        assert_eq!(got.wall_time, meta.wall_time);
        assert_eq!(got.origin_device_id, meta.origin_device_id);
        assert_eq!(got.ct, ct);
    }

    /// A V2 tombstone (empty ct) round-trips with deleted=true and empty ct.
    #[test]
    fn v2_tombstone_round_trips() {
        let meta = RelayWireMeta {
            item_id: "tomb-1".to_string(),
            lamport_ts: 9,
            deleted: true,
            pinned: false,
            pin_order: None,
            wall_time: 123,
            origin_device_id: "dev-x".to_string(),
        };
        let wire = encode_v2(&meta, &[]).expect("encode tombstone");
        let got = decode_payload(&wire).expect("decode tombstone");
        assert!(got.deleted);
        assert!(got.ct.is_empty());
        assert_eq!(got.item_id, "tomb-1");
    }

    /// Golden wire-size: for a representative ciphertext the V2 payload is
    /// materially smaller than the legacy double-base64 payload — the legacy
    /// payload is ≥ 1.3× the V2 size (the ~33 % bloat the bug report cites).
    #[test]
    fn v2_is_smaller_than_legacy_double_base64() {
        let meta = sample_meta();
        // 4 KiB ciphertext — large enough that the ct term dominates the small
        // metadata, so the ratio approaches the asymptotic 4/3 (~33 %).
        let ct: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();

        let v2 = encode_v2(&meta, &ct).expect("encode v2");
        let v1 = legacy_v1_wire(&meta, &ct);

        assert!(
            v2.len() < v1.len(),
            "v2 ({}) must be smaller than legacy v1 ({})",
            v2.len(),
            v1.len()
        );
        let ratio = v1.len() as f64 / v2.len() as f64;
        assert!(
            ratio >= 1.3,
            "legacy must be ≥1.3× the v2 size (got {ratio:.3}: v1={}, v2={})",
            v1.len(),
            v2.len()
        );
    }

    /// An unknown leading version byte is rejected (not silently misparsed).
    #[test]
    fn unknown_version_byte_rejected() {
        let wire = base64::engine::general_purpose::STANDARD.encode([0xFEu8, 1, 2, 3]);
        assert!(decode_payload(&wire).is_err());
    }
}
