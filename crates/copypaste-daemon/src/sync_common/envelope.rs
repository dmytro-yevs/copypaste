//! Cloud file-identity envelope (BUG C1) + wire ciphertext decoding.
//!
//! Split out of the former flat `sync_common.rs` (ADR-017, CopyPaste-vp63.7)
//! — moved verbatim, no behavior change.
//!
//! Cloud / relay sync re-wraps a file's raw bytes under the sync key, but the
//! wire schema carries only `content_type` — NOT the file's name/MIME. To
//! preserve file identity end-to-end WITHOUT a schema change, we prepend a small
//! self-describing header to the file bytes *before* `encrypt_for_cloud`, so
//! name+MIME live INSIDE the encrypted plaintext (the relay/cloud only ever sees
//! opaque ciphertext).
//!
//! Wire format (all multi-byte integers big-endian):
//!   [1 byte  version = CLOUD_FILE_HEADER_VERSION]
//!   [2 bytes name_len][name_len bytes UTF-8 file name]
//!   [2 bytes mime_len][mime_len bytes UTF-8 MIME type]
//!   [file bytes ...]
//!
//! Back-compat: a file uploaded by an OLD daemon has no header. On download we
//! validate the version byte and both length fields against the buffer; if any
//! check fails we treat the ENTIRE plaintext as raw file bytes with the legacy
//! name="file" / mime="application/octet-stream" (the pre-fix behaviour).

use copypaste_core::ClipboardItem;

/// Version byte for the cloud file-identity header. Bump only with a matching
/// decoder branch.
///
/// `pub(crate)` (CopyPaste-vp63.7): visible to sibling `rebuild.rs` (via
/// [`decode_cloud_file_payload`]'s legacy fallback constants) AND to
/// `cloud::bytea_e2e`'s fake-PostgREST test harness, which is outside the
/// `sync_common` subtree and so needs crate-wide reach.
pub(crate) const CLOUD_FILE_HEADER_VERSION: u8 = 1;

/// Legacy fallback file name for headerless (old-daemon) file payloads.
pub(crate) const CLOUD_FILE_LEGACY_NAME: &str = "file";

/// Legacy fallback MIME for headerless (old-daemon) file payloads.
pub(crate) const CLOUD_FILE_LEGACY_MIME: &str = "application/octet-stream";

/// Prepend the cloud file-identity header to `file_bytes`.
///
/// `name`/`mime` longer than `u16::MAX` bytes are truncated on a UTF-8 char
/// boundary — these come from a captured file path / sniffed MIME and are in
/// practice far shorter, so the cap only guards the 2-byte length field.
pub(crate) fn encode_cloud_file_payload(name: &str, mime: &str, file_bytes: &[u8]) -> Vec<u8> {
    let name_b = truncate_utf8(name, u16::MAX as usize).as_bytes();
    let mime_b = truncate_utf8(mime, u16::MAX as usize).as_bytes();
    let mut out = Vec::with_capacity(1 + 2 + name_b.len() + 2 + mime_b.len() + file_bytes.len());
    out.push(CLOUD_FILE_HEADER_VERSION);
    // Lengths fit u16 by construction (truncate_utf8 bounds them).
    out.extend_from_slice(&(name_b.len() as u16).to_be_bytes());
    out.extend_from_slice(name_b);
    out.extend_from_slice(&(mime_b.len() as u16).to_be_bytes());
    out.extend_from_slice(mime_b);
    out.extend_from_slice(file_bytes);
    out
}

/// Truncate `s` to at most `max` bytes on a UTF-8 char boundary.
fn truncate_utf8(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Parse a cloud file payload into `(file_bytes, name, mime)`.
///
/// Returns the embedded name/MIME when a valid header is present; otherwise
/// (old-daemon payload, or any malformed/overrunning header) treats the WHOLE
/// buffer as raw file bytes with the legacy name/MIME — never panics.
///
/// `pub(crate)` (CopyPaste-vp63.7): consumed by sibling `rebuild.rs` AND by
/// `cloud::bytea_e2e`'s test harness (outside the `sync_common` subtree).
pub(crate) fn decode_cloud_file_payload(payload: &[u8]) -> (Vec<u8>, String, String) {
    let legacy = || {
        (
            payload.to_vec(),
            CLOUD_FILE_LEGACY_NAME.to_string(),
            CLOUD_FILE_LEGACY_MIME.to_string(),
        )
    };
    // Smallest valid header: version + 2 zero-len fields = 5 bytes.
    if payload.len() < 5 || payload[0] != CLOUD_FILE_HEADER_VERSION {
        return legacy();
    }
    let mut pos = 1usize;
    let read_field = |buf: &[u8], pos: &mut usize| -> Option<String> {
        if *pos + 2 > buf.len() {
            return None;
        }
        let len = u16::from_be_bytes([buf[*pos], buf[*pos + 1]]) as usize;
        *pos += 2;
        if *pos + len > buf.len() {
            return None;
        }
        let s = std::str::from_utf8(&buf[*pos..*pos + len])
            .ok()?
            .to_string();
        *pos += len;
        Some(s)
    };
    let name = match read_field(payload, &mut pos) {
        Some(s) => s,
        None => return legacy(),
    };
    let mime = match read_field(payload, &mut pos) {
        Some(s) => s,
        None => return legacy(),
    };
    (payload[pos..].to_vec(), name, mime)
}

/// Read a file item's `(file_name, mime)` from its local `blob_ref` meta JSON.
///
/// Mirrors the source the P2P / IPC paths use (`parse_file_meta`). Falls back to
/// the legacy name/MIME if the meta is missing or unparseable so a malformed row
/// still uploads (just without identity) rather than being dropped.
fn file_identity_from_item(item: &ClipboardItem) -> (String, String) {
    match item.blob_ref.as_deref() {
        Some(meta_json) => match crate::ipc::parse_file_meta(meta_json) {
            Ok(meta) => (meta.filename, meta.mime),
            Err(e) => {
                tracing::warn!(
                    "sync: file id={} blob_ref meta unparseable ({e}); \
                     uploading with legacy name/mime",
                    item.id
                );
                (
                    CLOUD_FILE_LEGACY_NAME.to_string(),
                    CLOUD_FILE_LEGACY_MIME.to_string(),
                )
            }
        },
        None => (
            CLOUD_FILE_LEGACY_NAME.to_string(),
            CLOUD_FILE_LEGACY_MIME.to_string(),
        ),
    }
}

/// Wrap a decrypted plaintext for cloud upload.
///
/// For `content_type == "file"` this prepends the [`encode_cloud_file_payload`]
/// header (name+MIME read from the item's local `blob_ref`). For every other
/// type the plaintext is returned unchanged.
pub(crate) fn wrap_cloud_upload_plaintext(item: &ClipboardItem, plaintext: Vec<u8>) -> Vec<u8> {
    if item.content_type == "file" {
        let (name, mime) = file_identity_from_item(item);
        encode_cloud_file_payload(&name, &mime, &plaintext)
    } else {
        plaintext
    }
}

/// Wrap a decrypted plaintext for cloud upload and enforce the sync ceiling on
/// the WRAPPED bytes (the exact bytes that get encrypted and shipped).
///
/// Returns `Err` (caller logs a `warn!` and skips the item) when the wrapped
/// payload exceeds the ceiling — never panics, never silently drops.
pub(crate) fn wrap_and_check_cloud_upload_plaintext(
    item: &ClipboardItem,
    plaintext: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let wrapped = wrap_cloud_upload_plaintext(item, plaintext);
    let ceiling = crate::sync_orch::SYNC_MAX_BLOB_BYTES;
    if wrapped.len() > ceiling {
        return Err(format!(
            "wrapped blob {} bytes exceeds cloud sync ceiling {ceiling}",
            wrapped.len()
        ));
    }
    Ok(wrapped)
}

/// Decode a `payload_ct` value into the raw ciphertext blob (nonce||ciphertext).
///
/// PostgREST renders `bytea` in hex output form (`\x<hex>`); we also accept a
/// bare base64 string (the relay envelope's `ct_b64`, and pre-fix Supabase rows).
pub(crate) fn decode_payload_ct(payload_ct: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    if let Some(hexpart) = payload_ct.strip_prefix("\\x") {
        return hex::decode(hexpart).map_err(|e| format!("hex decode: {e}"));
    }
    base64::engine::general_purpose::STANDARD
        .decode(payload_ct)
        .map_err(|e| format!("base64 decode: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #10 — Cloud-file payload header parity: GOLDEN BYTES test.
    ///
    /// Source of truth: `encode_cloud_file_payload` in THIS file
    /// (sync_common/envelope.rs, formerly sync_common.rs).
    /// Wire format (all multi-byte integers big-endian):
    ///   [1 byte  version = 1]
    ///   [2 bytes name_len][name_len bytes UTF-8 file name]
    ///   [2 bytes mime_len][mime_len bytes UTF-8 MIME type]
    ///   [file bytes ...]
    ///
    /// If this test breaks, update the Android JVM test
    /// `CloudFilePayloadParityTest.kt` to match the new layout.
    ///
    /// The companion Android JVM test lives at:
    ///   android/app/src/test/java/com/copypaste/android/CloudFilePayloadParityTest.kt
    #[test]
    fn cloud_file_payload_golden_bytes() {
        // Canonical test vector — must be byte-for-byte identical to the
        // Android SyncManager.encodeCloudFilePayload result for the same inputs.
        let name = "hello.txt"; // 9 UTF-8 bytes
        let mime = "text/plain"; // 10 UTF-8 bytes
        let body = b"BODY"; // 4 bytes

        let encoded = encode_cloud_file_payload(name, mime, body);

        // Build expected bytes by hand from the documented wire format:
        //  [0x01]              — version byte = 1
        //  [0x00, 0x09]        — name_len = 9 (big-endian u16)
        //  "hello.txt" (9 B)
        //  [0x00, 0x0A]        — mime_len = 10 (big-endian u16)
        //  "text/plain" (10 B)
        //  "BODY" (4 B)
        let mut expected: Vec<u8> = vec![
            // version
            CLOUD_FILE_HEADER_VERSION,
            // name_len = 9
            0x00,
            0x09,
        ];
        expected.extend_from_slice(b"hello.txt");
        expected.extend_from_slice(&[
            // mime_len = 10
            0x00, 0x0A,
        ]);
        expected.extend_from_slice(b"text/plain");
        expected.extend_from_slice(b"BODY");

        assert_eq!(
            encoded, expected,
            "encode_cloud_file_payload golden bytes mismatch — \
             if this changed, update CloudFilePayloadParityTest.kt (Android) too"
        );

        // Cross-check: decode must round-trip.
        let (decoded_body, decoded_name, decoded_mime) = decode_cloud_file_payload(&encoded);
        assert_eq!(decoded_body, body);
        assert_eq!(decoded_name, name);
        assert_eq!(decoded_mime, mime);
    }

    #[test]
    fn file_envelope_roundtrip() {
        let name = "report.pdf";
        let mime = "application/pdf";
        let file_bytes = b"%PDF-1.7 fake body".to_vec();
        let wrapped = encode_cloud_file_payload(name, mime, &file_bytes);
        assert_eq!(wrapped[0], CLOUD_FILE_HEADER_VERSION);
        let (rb, rn, rm) = decode_cloud_file_payload(&wrapped);
        assert_eq!(rb, file_bytes);
        assert_eq!(rn, name);
        assert_eq!(rm, mime);
    }

    #[test]
    fn file_envelope_empty_fields() {
        let file_bytes = b"raw".to_vec();
        let wrapped = encode_cloud_file_payload("", "", &file_bytes);
        let (rb, rn, rm) = decode_cloud_file_payload(&wrapped);
        assert_eq!(rb, file_bytes);
        assert_eq!(rn, "");
        assert_eq!(rm, "");
    }

    #[test]
    fn headerless_payload_falls_back_to_legacy() {
        let raw = b"not a header at all, just bytes".to_vec();
        let (bytes, name, mime) = decode_cloud_file_payload(&raw);
        assert_eq!(bytes, raw);
        assert_eq!(name, CLOUD_FILE_LEGACY_NAME);
        assert_eq!(mime, CLOUD_FILE_LEGACY_MIME);
    }

    #[test]
    fn malformed_header_falls_back_to_legacy() {
        // version byte present but name_len overruns the buffer.
        let malformed = vec![CLOUD_FILE_HEADER_VERSION, 0xFF, 0xFF, 0x00];
        let (bytes, name, mime) = decode_cloud_file_payload(&malformed);
        assert_eq!(bytes, malformed);
        assert_eq!(name, CLOUD_FILE_LEGACY_NAME);
        assert_eq!(mime, CLOUD_FILE_LEGACY_MIME);
    }

    #[test]
    fn decode_payload_ct_accepts_hex_and_base64() {
        use base64::Engine as _;
        let blob = vec![0xde, 0xad, 0xbe, 0xef];
        // PostgREST hex form
        let hexform = format!("\\x{}", hex::encode(&blob));
        assert_eq!(decode_payload_ct(&hexform).unwrap(), blob);
        // bare base64 form (relay envelope ct_b64)
        let b64 = base64::engine::general_purpose::STANDARD.encode(&blob);
        assert_eq!(decode_payload_ct(&b64).unwrap(), blob);
    }
}
