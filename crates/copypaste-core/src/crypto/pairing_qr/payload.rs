//! QR payload types: [`QrProvisioning`] and [`PairingPayload`].
//!
//! [`QrProvisioning`] carries optional, non-secret sync configuration (relay /
//! Supabase URLs) embedded in the QR's 6th field.
//!
//! [`PairingPayload`] is the fully-decoded contents of a QR pairing code,
//! produced by [`PairingPayload::encode`] on the displaying device and recovered
//! by [`PairingPayload::decode`] on the scanning device.

use base64::Engine as _;

use super::{
    b64,
    token::PairingToken,
    wire::{
        extract_json_string, fp_hex_to_b64url, json_str, normalize_fingerprint,
        percent_encode_component, uuid_bytes_to_str, uuid_str_to_b64url,
    },
    FP_BYTE_LEN, PAIRING_DEEPLINK_PREFIX, PAIRING_QR_FIELD_COUNT, PAIRING_QR_MAGIC,
    PAIRING_QR_MAGIC_V2, UUID_BYTE_LEN,
};
use crate::crypto::pairing_qr::PairingQrError;

// ─────────────────────────────────────────────────────────────────────────────
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
        self.relay_url.as_deref().is_none_or(str::is_empty)
            && self.supabase_url.as_deref().is_none_or(str::is_empty)
            && self.supabase_anon_key.as_deref().is_none_or(str::is_empty)
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
    /// when the generating device is configured for cloud/relay sync and wants
    /// the scanning device to inherit those settings automatically.
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
    /// When [`Self::provisioning`] is present and non-empty, appends a 6th field:
    /// the provisioning JSON base64url-encoded.
    ///
    /// `addr_hint` is base64url-encoded so that IPv4 dots (e.g. `192.168.1.5`)
    /// cannot collide with the `.` field delimiter and corrupt a 6-field split.
    /// This is a v2-only encoding — `decode_v2` base64url-decodes `field[4]`.
    ///
    /// The fingerprint (hex or colon-hex) is stripped of colons and hex-decoded to
    /// 32 raw bytes, then base64url-encoded (43 chars). If the fingerprint is not
    /// valid 32-byte hex the field is passed through as base64url of whatever bytes
    /// hex-decode produces — decode will reject it with [`PairingQrError::FingerprintLength`].
    /// The device_id UUID is parsed to its 16 raw bytes and base64url-encoded (22 chars).
    /// If the UUID cannot be parsed the bytes field is left empty and decode will reject it.
    pub fn encode(&self) -> String {
        let fp_b64 = fp_hex_to_b64url(&self.fingerprint);
        let token_b64 = b64().encode(self.token.inner());
        let device_id_b64 = uuid_str_to_b64url(&self.device_id);
        let name_b64 = b64().encode(self.device_name.as_bytes());
        // base64url-encode addr_hint to avoid dot collisions: an IPv4 address
        // like "192.168.1.5:54321" contains dots that would otherwise corrupt
        // the splitn(6) when a provisioning 6th field is present.
        let addr_b64 = b64().encode(self.addr_hint.as_bytes());
        let base = format!(
            "{magic}.{fp_b64}.{token_b64}.{device_id_b64}.{name_b64}.{addr_b64}",
            magic = PAIRING_QR_MAGIC_V2,
            fp_b64 = fp_b64,
            token_b64 = token_b64,
            device_id_b64 = device_id_b64,
            name_b64 = name_b64,
            addr_b64 = addr_b64,
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
    /// Accepts an optional 6th `.`-separated field (CPPAIR2 only) carrying
    /// provisioning JSON (base64url). Old payloads with exactly 5 fields are
    /// decoded with `provisioning: None` — no backward-compat break.
    ///
    /// # Errors
    /// * [`PairingQrError::BadMagic`] — missing or unrecognised magic+version prefix.
    /// * [`PairingQrError::FieldCount`] — wrong number of `.`-separated fields.
    /// * [`PairingQrError::Base64`] — a base64url field failed to decode.
    /// * [`PairingQrError::Utf8`] — the device-name field was not valid UTF-8.
    /// * [`PairingQrError::TokenLength`] — the token was not exactly 32 bytes.
    /// * [`PairingQrError::FingerprintLength`] — the fingerprint b64 field did not
    ///   decode to exactly `FP_BYTE_LEN` bytes (CPPAIR2 only).
    /// * [`PairingQrError::DeviceIdLength`] — the device_id b64 field did not
    ///   decode to exactly `UUID_BYTE_LEN` bytes (CPPAIR2 only).
    /// * [`PairingQrError::AddrHintDecode`] — addr_hint b64url decode failed (CPPAIR2 only).
    /// * [`PairingQrError::EmptyFingerprint`] — the fingerprint field was empty.
    pub fn decode(input: &str) -> Result<Self, PairingQrError> {
        let trimmed = input.trim();

        // Split on the first '.' to extract the magic prefix.
        let (magic, body) = trimmed.split_once('.').ok_or(PairingQrError::BadMagic)?;

        // Branch on the version; reject unknown versions (anti-downgrade guard).
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

        // v1 does not carry provisioning: addr_hint is a raw host:port and its
        // IPv4 dots collide with the field delimiter, making a 6th field ambiguous.
        // Provisioning is CPPAIR2-only (where addr_hint is base64url-encoded).
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
