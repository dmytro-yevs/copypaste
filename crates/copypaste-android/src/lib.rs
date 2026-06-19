#![allow(clippy::empty_line_after_doc_comments)] // uniffi-generated scaffolding triggers this lint

uniffi::include_scaffolding!("copypaste_android");

pub mod p2p_listener;
pub mod pairing;
pub mod panic_boundary;
pub mod version;
pub use p2p_listener::{P2pListenerHandle, PeerSessionKey};
pub use pairing::{DiscoveredPeer, PairStatus};
pub use panic_boundary::PanicError;
pub use version::{
    check_compatibility, core_version, uniffi_abi_version, VersionError, UNIFFI_ABI_VERSION,
};

use copypaste_core::{
    build_item_aad, build_item_aad_v2, decrypt_from_cloud, decrypt_item_with_aad, derive_sync_key,
    detect, encrypt_for_cloud, encrypt_item_with_aad, is_sensitive_for_autowipe, SyncKeyError,
    AAD_SCHEMA_VERSION, AAD_SCHEMA_VERSION_V4, ITEM_KEY_VERSION_CURRENT, NONCE_SIZE,
};
// PG-16 (89ve): text-kind classification re-exported so Kotlin can call it
// instead of re-implementing the classifier in TextKind.kt.
use copypaste_core::text_kind::classify_text;
// Brings `Engine::encode` into scope for `relay_public_key_b64` (STANDARD base64).
use base64::Engine as _;
// SHA-256 for DB_BY_PATH cache key derivation (P1-8): hashing the raw 32-byte
// DB key so raw key material is not retained on the heap as a HashMap key.
use sha2::{Digest as _, Sha256};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use zeroize::Zeroizing;

// When using UDL-based scaffolding, uniffi::Error and uniffi::Record proc-macro
// derives conflict with the generated scaffolding. Only thiserror is needed here.
#[derive(Debug, thiserror::Error)]
pub enum CopypasteError {
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed: {reason}")]
    DecryptionFailed { reason: String },
    #[error("Database error: {reason}")]
    DatabaseError { reason: String },
    #[error("Invalid key length: expected 32")]
    InvalidKeyLength,
    /// P2P pairing / transport failure surfaced from `copypaste_p2p`
    /// (`TransportError`): TLS, socket, framing, or PAKE handshake errors —
    /// including a wrong pairing password or a channel-binding MitM abort. Also
    /// raised for a malformed `addr_hint` that cannot be parsed into a
    /// `SocketAddr`. The `reason` carries the underlying error's display form.
    #[error("P2P pairing failed: {reason}")]
    P2pError { reason: String },
    /// v0.3 (OI-7): a Rust panic was caught at the FFI boundary by
    /// [`panic_boundary::catch_result`]. Carries the panic message so Kotlin
    /// can log/surface it instead of seeing a JVM-killing abort.
    ///
    /// NOTE: the field is named `reason` (not `message`) on purpose — a UniFFI
    /// flat-error variant field named `message` collides with the Kotlin
    /// `Throwable.message` supertype property and produces "conflicting
    /// declarations" / missing-`override` codegen errors. See the generated
    /// `CopypasteException` binding.
    #[error("Panicked: {reason}")]
    Panicked { reason: String },
}

impl From<PanicError> for CopypasteError {
    fn from(p: PanicError) -> Self {
        match p {
            PanicError::Panicked(reason) => CopypasteError::Panicked { reason },
        }
    }
}

pub struct EncryptedBlob {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

/// Encrypt `bytes` with `key` (XChaCha20-Poly1305), binding `item_id` and
/// `key_version` into the AEAD AAD.
///
/// | `key_version` | AAD format                           |
/// |---------------|--------------------------------------|
/// | 1             | `build_item_aad(item_id, 3)`         |
/// | 2             | `build_item_aad_v2(item_id, 4, 2)`   |
/// | other         | `Err(EncryptionFailed)`              |
///
/// Kotlin callers MUST persist `key_version` alongside the ciphertext and pass
/// it back to `decrypt_text` verbatim — a mismatch will fail decryption.
/// New items should always use `key_version = 2` (matches the daemon's
/// `ITEM_KEY_VERSION_CURRENT`). Legacy stored items encrypted with v1 must
/// continue to round-trip with `key_version = 1`.
pub fn encrypt_text(
    item_id: String,
    bytes: &[u8],
    key: &[u8],
    key_version: u8,
) -> Result<EncryptedBlob, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        // Mirror the dispatch table in decrypt_item_by_version (copypaste-core).
        let aad = match key_version {
            1 => build_item_aad(&item_id, AAD_SCHEMA_VERSION),
            2 => build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, u32::from(key_version)),
            _ => return Err(CopypasteError::EncryptionFailed),
        };
        let (nonce, ciphertext) = encrypt_item_with_aad(bytes, &key_arr, &aad)
            .map_err(|_| CopypasteError::EncryptionFailed)?;
        Ok(EncryptedBlob {
            nonce: nonce.to_vec(),
            ciphertext,
        })
    })
}

/// Decrypt `ciphertext` encrypted by `encrypt_text`, dispatching on
/// `key_version` to select the correct AAD format.
///
/// | `key_version` | AAD format                           |
/// |---------------|--------------------------------------|
/// | 1             | `build_item_aad(item_id, 3)`         |
/// | 2             | `build_item_aad_v2(item_id, 4, 2)`   |
/// | other         | `Err(DecryptionFailed)`              |
///
/// `item_id` and `key_version` MUST match the values used during
/// `encrypt_text` — a mismatch will cause an AEAD auth-tag failure.
pub fn decrypt_text(
    item_id: String,
    ciphertext: &[u8],
    nonce: &[u8],
    key: &[u8],
    key_version: u8,
) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let nonce_arr: [u8; NONCE_SIZE] =
            nonce
                .try_into()
                .map_err(|_| CopypasteError::DecryptionFailed {
                    reason: "wrong nonce length".into(),
                })?;
        // Mirror the dispatch table in decrypt_item_by_version (copypaste-core).
        let aad = match key_version {
            1 => build_item_aad(&item_id, AAD_SCHEMA_VERSION),
            2 => build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, u32::from(key_version)),
            v => {
                return Err(CopypasteError::DecryptionFailed {
                    reason: format!("unknown key_version: {v}"),
                })
            }
        };
        decrypt_item_with_aad(ciphertext, &nonce_arr, &key_arr, &aad).map_err(|e| {
            CopypasteError::DecryptionFailed {
                reason: e.to_string(),
            }
        })
    })
}

/// One encrypted local clipboard item handed to [`decrypt_text_batch`].
///
/// Mirrors the at-rest columns Kotlin reads from its local SQLite store:
/// the stable `item_id` (bound into the AEAD AAD), the `ciphertext` + `nonce`
/// blobs, and the `key_version` (1 or 2) that selects the AAD/key format.
#[derive(Debug)]
pub struct EncryptedItem {
    pub item_id: String,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub key_version: u8,
}

/// One successfully-decrypted item returned by [`decrypt_text_batch`], carrying
/// its `item_id` back so Kotlin can re-associate the plaintext with its row.
#[derive(Debug)]
pub struct DecryptedItem {
    pub item_id: String,
    pub plaintext: Vec<u8>,
}

/// Outcome of [`decrypt_text_batch`]: the decryptable items plus an aggregate
/// count of the rows skipped because they could not be decrypted.
#[derive(Debug)]
pub struct DecryptBatchResult {
    /// Items whose AEAD auth tag verified and decrypted cleanly.
    pub items: Vec<DecryptedItem>,
    /// Number of input items skipped because they failed to decrypt (wrong /
    /// rotated key, format drift, an unsupported `key_version`, or a malformed
    /// nonce). Kotlin logs this ONCE in aggregate instead of one error per row.
    pub skipped: u32,
}

/// Decrypt a batch of local clipboard items at startup/list time, **degrading
/// gracefully** when individual items cannot be decrypted (CopyPaste-00zz).
///
/// # Why this exists
///
/// Kotlin's startup load previously called [`decrypt_text`] once per row, and
/// every undecryptable legacy item (encrypted under a now-rotated key/format)
/// threw `DecryptionFailed`. After a key rotation / re-pair this fired hundreds
/// of times on a single launch (~629 observed) — flooding logcat and degrading
/// UX even though those rows are simply dead legacy ciphertext.
///
/// This batch entry point decrypts every item in one FFI call: each item that
/// fails AEAD verification (or carries an unsupported `key_version` / malformed
/// nonce) is **skipped, not thrown**, and counted in
/// [`DecryptBatchResult::skipped`]. Kotlin surfaces a single aggregate line
/// ("skipped N undecryptable legacy items") and renders only the decryptable
/// items, instead of catching one exception per row.
///
/// # Security
///
/// Graceful means *skip*, never *bypass*. A failed auth tag is never accepted
/// as plaintext — the item is dropped from `items`. The AAD binding of
/// `(item_id, schema_version, key_version)` is preserved verbatim (each item's
/// AAD is rebuilt here exactly as [`decrypt_text`] does), so this path cannot be
/// used to swap or replay ciphertext across items. `key` is zeroized on drop via
/// [`Zeroizing`].
///
/// Errors: `InvalidKeyLength` if `key` is not exactly 32 bytes. Per-item
/// decryption failures do NOT error — they are skipped and counted.
pub fn decrypt_text_batch(
    items: Vec<EncryptedItem>,
    key: &[u8],
) -> Result<DecryptBatchResult, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let mut decrypted = Vec::with_capacity(items.len());
        let mut skipped: u32 = 0;
        for item in &items {
            match try_decrypt_one(item, &key_arr) {
                Some(plaintext) => decrypted.push(DecryptedItem {
                    item_id: item.item_id.clone(),
                    plaintext,
                }),
                // Skip-and-count: a wrong/rotated key, format drift, malformed
                // nonce, or unsupported key_version is NOT surfaced as an error.
                None => skipped = skipped.saturating_add(1),
            }
        }
        Ok(DecryptBatchResult {
            items: decrypted,
            skipped,
        })
    })
}

/// Attempt to decrypt a single [`EncryptedItem`], returning `None` (rather than
/// erroring) on any failure so [`decrypt_text_batch`] can skip-and-count.
///
/// Rebuilds the AAD from the item's own `item_id` + `key_version` exactly as
/// [`decrypt_text`] does, keeping the AAD binding intact. The only `Some` path
/// is a fully-verified AEAD decrypt — a failed auth tag yields `None`, never
/// accepted plaintext.
fn try_decrypt_one(item: &EncryptedItem, key: &[u8; 32]) -> Option<Vec<u8>> {
    let nonce: [u8; NONCE_SIZE] = item.nonce.as_slice().try_into().ok()?;
    let aad = match item.key_version {
        1 => build_item_aad(&item.item_id, AAD_SCHEMA_VERSION),
        2 => build_item_aad_v2(
            &item.item_id,
            AAD_SCHEMA_VERSION_V4,
            u32::from(item.key_version),
        ),
        // Unknown key_version: undecryptable by definition — skip.
        _ => return None,
    };
    decrypt_item_with_aad(&item.ciphertext, &nonce, key, &aad).ok()
}

/// Returns `true` if `text` is sensitive at the HIGH-confidence threshold.
///
/// AB-6a (v0.6.1 threshold parity): this used to flag on `detect(&text).is_some()`
/// — i.e. ANY pattern match, including low-confidence heuristics (phone 0.55,
/// passport 0.55, email 0.60). macOS gates on confidence >= 0.70
/// (`is_sensitive_for_autowipe`), so the two platforms disagreed: mildly-sensitive
/// text that macOS keeps was dropped on Android. We now call the SAME core gate
/// (`is_sensitive_for_autowipe`, the >= 0.70 floor) so the sensitivity verdict is
/// byte-for-byte identical to the daemon's. The Kotlin store policy (store+mask
/// vs drop) is changed in a LATER wave — here we only align the threshold.
///
/// Wrapped in [`panic_boundary::catch`] because the detector runs regex/allocation
/// that could panic; an unwound panic across the JNI boundary aborts the JVM. This
/// export returns a plain `bool`, so a caught panic recovers to `false` (treat as
/// "not sensitive" rather than crash).
pub fn is_sensitive(text: String) -> bool {
    panic_boundary::catch(|| is_sensitive_for_autowipe(&text)).unwrap_or(false)
}

/// Returns the sensitive-kind label for `text`, or `None` if not sensitive.
///
/// PG-23 (l9z8) alignment: `sensitive_kind` now gates at the SAME >= 0.70
/// confidence floor as `is_sensitive_for_autowipe` / `is_sensitive`. Previously
/// it called `detect()` which fires on ANY pattern including low-confidence
/// heuristics (phone 0.55, passport 0.55, email 0.60, IBAN 0.65, SSN 0.65).
/// This produced a divergence where `sensitive_kind` returned `Some("Phone")`
/// while `is_sensitive` returned `false` for the same phone number, confusing
/// Kotlin callers that relied on `sensitive_kind.isNotNull()` as a sensitivity
/// signal.
///
/// The fix: only return a non-null kind for patterns whose confidence is >= 0.70
/// (the SAME autowipe floor). Low-confidence pattern hits that fall below the
/// floor are still available via `detect_sensitive_spans` / `is_sensitive_for_autowipe`
/// but `sensitive_kind` is now purely an informational label that agrees with
/// `is_sensitive`.
///
/// Wrapped in [`panic_boundary::catch`] for the same reason as
/// [`is_sensitive`]. This export returns a plain `Option<String>`, so a caught
/// panic recovers to `None`.
pub fn sensitive_kind(text: String) -> Option<String> {
    panic_boundary::catch(|| {
        // Only report a kind when the text also triggers the auto-wipe gate
        // (confidence >= 0.70). This keeps sensitive_kind and is_sensitive in
        // sync — Kotlin can safely use `sensitive_kind.isNotNull()` as a proxy
        // for is_sensitive.
        if !is_sensitive_for_autowipe(&text) {
            return None;
        }
        detect(&text).map(|k| format!("{:?}", k))
    })
    .unwrap_or(None)
}

// ---------------------------------------------------------------------------
// PG-3 (349q): sensitive_capture_decision — single source of truth for whether
// text is sensitive at capture time. Returns `SensitiveCaptureDecision` with
// three fields Kotlin needs to store+mask a sensitive item correctly:
//
//   is_sensitive   — true when confidence >= 0.70 (same as macOS daemon gate)
//   kind           — the SensitiveKind label, or None when not sensitive
//   expires_at_ms  — unix-ms expiry timestamp (now_unix_ms + ttl_secs * 1000),
//                    or None when sensitive_ttl_secs == 0 ("auto-wipe disabled")
//                    or when the text is not sensitive
//
// This replaces the split calls to is_sensitive / sensitive_kind / separate
// expires_at computation that ClipboardService.kt would otherwise need to
// coordinate. One FFI round-trip per capture is cheaper and keeps the logic
// in Rust where it belongs.
//
// SECURITY: the item_id AAD binding (in encrypt_text / decrypt_text) is
// unchanged — callers still pass item_id into the crypto functions. This
// function is PURE (no DB I/O, no file I/O).
//
// PG-4  (ojsh): sensitive_spans — core detector spans for Kotlin masking.
// PG-24 (5tnx): sensitive_expires_at_ms — per-item expires_at from core TTL.
// ---------------------------------------------------------------------------

/// Result of `sensitive_capture_decision` — single-round-trip sensitivity
/// verdict for one clipboard item at capture time.
///
/// Kotlin stores `is_sensitive` and `expires_at_ms` in the DB row and uses
/// `kind` for the badge label. If `is_sensitive` is false, `kind` and
/// `expires_at_ms` are always `None`.
///
/// `expires_at_ms` is `None` when:
///   - `is_sensitive` is false, OR
///   - `sensitive_ttl_secs` is 0 (the "auto-wipe disabled" sentinel).
pub struct SensitiveCaptureDecision {
    /// True when the text triggers the >= 0.70 confidence floor.
    pub is_sensitive: bool,
    /// The canonical sensitive-kind label (e.g. `"AwsKey"`, `"CreditCard"`),
    /// or `None` when the text is not sensitive.
    pub kind: Option<String>,
    /// Unix-millisecond expiry timestamp for this item, or `None` when
    /// auto-wipe is disabled (`sensitive_ttl_secs == 0`) or not sensitive.
    pub expires_at_ms: Option<i64>,
}

/// Compute the sensitivity verdict and auto-wipe expiry for one clipboard item
/// at capture time.
///
/// `now_unix_ms` is the current wall-clock time in Unix milliseconds. Kotlin
/// should pass `System.currentTimeMillis()`. `sensitive_ttl_secs` is from the
/// user-tunable config (defaults to 30 s; 0 = "auto-wipe disabled").
///
/// This is the CORRECT gate for Android capture (PG-3 / 349q). It uses the
/// SAME `is_sensitive_for_autowipe` (>= 0.70 confidence floor) as the macOS
/// daemon, so a phone number (confidence 0.55) is NOT flagged and NOT dropped
/// on Android. Previously ClipboardService.kt checked `is_sensitive` and
/// early-returned, dropping items that macOS keeps.
///
/// PURE — no DB I/O.
pub fn sensitive_capture_decision(
    text: String,
    now_unix_ms: i64,
    sensitive_ttl_secs: u64,
) -> SensitiveCaptureDecision {
    panic_boundary::catch(|| {
        let sensitive = is_sensitive_for_autowipe(&text);
        if !sensitive {
            return SensitiveCaptureDecision {
                is_sensitive: false,
                kind: None,
                expires_at_ms: None,
            };
        }
        let kind = detect(&text).map(|k| format!("{:?}", k));
        // sensitive_ttl_secs == 0 is the "never wipe" sentinel — no expiry.
        let expires_at_ms = if sensitive_ttl_secs == 0 {
            None
        } else {
            Some(now_unix_ms.saturating_add(sensitive_ttl_secs as i64 * 1000))
        };
        SensitiveCaptureDecision {
            is_sensitive: true,
            kind,
            expires_at_ms,
        }
    })
    .unwrap_or(SensitiveCaptureDecision {
        is_sensitive: false,
        kind: None,
        expires_at_ms: None,
    })
}

// ---------------------------------------------------------------------------
// PG-24 (5tnx): sensitive_expires_at_ms
//
// macOS daemon stamps `expires_at = now_ms + sensitive_ttl_local_secs * 1000`
// (daemon.rs:2183) at capture. Android ClipboardRepository.kt:1128-1177 only
// pruned by age in getItems(), leaving expired items alive in suspended apps.
//
// This FFI computes the per-item expiry timestamp from the SAME formula so
// Kotlin stores `expires_at` in the DB row and a WorkManager periodic job can
// sweep stale rows even when the app is suspended.
//
// Returns None when sensitive_ttl_secs == 0 ("auto-wipe disabled" sentinel).
// ---------------------------------------------------------------------------

/// Compute the per-item `expires_at` timestamp (Unix milliseconds) for a
/// sensitive clipboard item, matching the daemon's formula:
///
///   `expires_at = now_unix_ms + sensitive_ttl_secs * 1000`
///
/// Returns `None` when `sensitive_ttl_secs == 0` (the "auto-wipe disabled"
/// sentinel — Kotlin should not write `expires_at` for such items).
///
/// `now_unix_ms` is `System.currentTimeMillis()` from Kotlin.
/// `sensitive_ttl_secs` is the user-tunable `Config.sensitive_ttl_secs`
/// (default 30, from `default_config()`).
///
/// PURE — no DB I/O. Wrapped in `panic_boundary::catch` as a defensive
/// measure; the saturation math cannot panic in practice.
pub fn sensitive_expires_at_ms(now_unix_ms: i64, sensitive_ttl_secs: u64) -> Option<i64> {
    panic_boundary::catch(|| {
        if sensitive_ttl_secs == 0 {
            return None;
        }
        Some(now_unix_ms.saturating_add(sensitive_ttl_secs as i64 * 1000))
    })
    .unwrap_or(None)
}

// ---------------------------------------------------------------------------
// PG-4 (ojsh): detect_sensitive_spans — sensitive byte spans for Kotlin masking
//
// macOS daemon ipc.rs:4460-4487 calls `SensitiveDetector::detect_normalised` and
// maps byte→char offsets for the `sensitive_spans` JSON array used by
// HistoryView.tsx to bullet-mask embedded credentials. Android had no equivalent,
// so a card/IBAN buried in longer non-sensitive text showed unmasked.
//
// This FFI returns the same char-offset spans so Kotlin can mask sub-string
// sensitive matches in the history list. PURE — no DB I/O.
//
// NOTE: spans are over the NFKC-NORMALISED string, not the original. Kotlin
// must use `SensitiveSpan.start/end` as character indices into the normalised
// string returned alongside the spans (or re-normalise the same text before
// masking). Normalization rarely changes the string (only Unicode bypass tricks
// trigger it), so callers can usually index into the original text directly —
// but correctness requires the normalised form.
// ---------------------------------------------------------------------------

/// One matched sensitive span (char-offset, NOT byte-offset).
///
/// `start` and `end` are Unicode scalar-value indices into the NFKC-normalised
/// form of the input text. Kotlin masks `text[start..<end]` with bullet chars.
///
/// NOTE ON NORMALIZATION: `copypaste_core::sensitive::nfkc_normalize` is
/// idempotent on ASCII and almost all practical clipboard text. The only time
/// the normalised string differs from the original is when the text contains
/// full-width Unicode digits/letters (the NFKC form collapses them to ASCII).
/// In that case Kotlin should normalise the text before rendering spans.
pub struct SensitiveSpan {
    /// Start character index (inclusive) in the NFKC-normalised text.
    pub start: u32,
    /// End character index (exclusive) in the NFKC-normalised text.
    pub end: u32,
    /// Confidence score of this match (0.0 – 1.0).
    pub confidence: f32,
    /// Pattern name (e.g. `"aws_access_key"`, `"credit_card"`, `"jwt"`).
    pub pattern_name: String,
}

/// Detect sensitive spans in `text` and return their char offsets for masking.
///
/// Uses `SensitiveDetector::detect_normalised` (the SAME detector as the macOS
/// daemon's `sensitive_spans` IPC response) to find all pattern matches,
/// including low-confidence hits (phone 0.55, IBAN 0.65) — the masking
/// decision is intentionally broader than the auto-wipe gate. Kotlin masks
/// ALL returned spans regardless of confidence (any credential visible in the
/// history list should be obscured).
///
/// The returned spans are char-offset indices into the NFKC-normalised
/// rendering of `text`. For ASCII text (virtually all practical clipboard
/// content) the normalised form is byte-for-byte identical to the original, so
/// Kotlin can index directly. For unusual Unicode input Kotlin should run
/// `text.normalize(Form.NFKC)` before applying the offsets.
///
/// Returns an empty `Vec` when no sensitive patterns are found. Wrapped in
/// `panic_boundary::catch` — the detector runs regex/allocation that could
/// panic; a caught panic returns an empty span list (safe: no masking applied).
pub fn detect_sensitive_spans(text: String) -> Vec<SensitiveSpan> {
    panic_boundary::catch(|| {
        use copypaste_core::sensitive::nfkc_normalize;
        let normalised = nfkc_normalize(&text);
        let detector = copypaste_core::SensitiveDetector::new();
        detector
            .detect_normalised(&normalised)
            .into_iter()
            .map(|m| {
                let start = byte_to_char_offset_android(&normalised, m.matched_range.start);
                let end = byte_to_char_offset_android(&normalised, m.matched_range.end);
                SensitiveSpan {
                    start,
                    end,
                    confidence: m.confidence,
                    pattern_name: m.pattern_name.to_string(),
                }
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Convert a byte offset in `s` to a char (Unicode scalar value) offset.
///
/// Mirrors the daemon's `byte_to_char_offset` helper (ipc.rs) used to generate
/// the `sensitive_spans` JSON array. A byte offset equal to `s.len()` maps to
/// the char count (one-past-the-end). An out-of-bounds byte offset saturates to
/// the char count. The result is capped at `u32::MAX` for the FFI type; in
/// practice no clipboard item approaches 4 billion chars.
fn byte_to_char_offset_android(s: &str, byte_offset: usize) -> u32 {
    // Count the number of chars whose byte offset is strictly less than
    // `byte_offset`. This matches the daemon's `byte_to_char_offset` helper
    // that iterates `char_indices` and counts chars up to the target byte.
    let count = s
        .char_indices()
        .take_while(|(bi, _)| *bi < byte_offset)
        .count();
    count.min(u32::MAX as usize) as u32
}

// ---------------------------------------------------------------------------
// Cloud sync crypto — cross-device SyncKey (Argon2id-derived) + schema v5
//
// These FFI functions expose the SAME crypto used by the macOS daemon's
// cloud.rs so Android can push/pull from the same Supabase table with
// identical encrypted payloads.
//
// Key facts (MUST match cloud.rs):
//   - KDF: Argon2id, 19 MiB / 2 passes / 1 lane, fixed domain salt
//   - AEAD: XChaCha20-Poly1305, 24-byte random nonce prepended to ciphertext
//   - AAD: "{item_id}|5"  (CLOUD_AAD_SCHEMA_VERSION = 5)
//   - Blob wire format: base64(nonce[24] || ciphertext_with_tag)
// ---------------------------------------------------------------------------

/// Minimum accepted passphrase length for cloud-sync key derivation.
///
/// Argon2id accepts any length including empty, but an empty or trivially-short
/// passphrase would produce a weak key that an attacker could brute-force even
/// against a memory-hard KDF. Matches the macOS daemon's UI-side enforcement
/// so both platforms reject the same bad input with an informative error.
const MIN_PASSPHRASE_LEN: usize = 8;

/// Derive a 32-byte sync key from `passphrase` using Argon2id.
///
/// Returns the raw 32-byte key material. The caller (Kotlin) should treat
/// these bytes as a short-lived secret: derive once at passphrase entry,
/// use, then zero the array. Do NOT persist to disk or SharedPreferences.
///
/// # SECURITY NOTE — returned `Vec<u8>` crosses the FFI boundary unzeroized.
/// UniFFI copies the bytes into a Kotlin `ByteArray`; the Kotlin layer MUST
/// zero that array after use. This is a load-bearing contract: failure to do
/// so leaves raw key material on the JVM heap until GC.
///
/// Errors:
///   - `DecryptionFailed { reason }` — passphrase is shorter than
///     `MIN_PASSPHRASE_LEN` bytes; `reason` carries the human-readable cause
///     so the user (and logs) learn why, matching the macOS surface.
///   - `EncryptionFailed` — Argon2 parameter or runtime failure (should not
///     occur with the hardcoded constants; surfaces as a non-panic error).
pub fn derive_cloud_sync_key(passphrase: String) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        // Guard on char count (Unicode scalar values), not byte length, to match
        // copypaste_core::derive_sync_key which uses passphrase.chars().count().
        // A byte-length guard would silently pass a 2-emoji passphrase (which is
        // ≥8 bytes) while core rejects it as PassphraseTooShort.
        let char_count = passphrase.chars().count();
        if char_count < MIN_PASSPHRASE_LEN {
            return Err(CopypasteError::DecryptionFailed {
                reason: format!(
                    "passphrase too short: must be at least {MIN_PASSPHRASE_LEN} characters \
                     (got {char_count})",
                ),
            });
        }
        let key = derive_sync_key(&passphrase).map_err(|e| match e {
            // Propagate any Argon2 runtime message rather than discarding it.
            SyncKeyError::Argon2Params(msg) | SyncKeyError::Argon2Hash(msg) => {
                CopypasteError::DecryptionFailed { reason: msg }
            }
            // Core pre-checked length above, but handle PassphraseTooShort
            // explicitly so the reason is never swallowed into EncryptionFailed.
            SyncKeyError::PassphraseTooShort(n) => CopypasteError::DecryptionFailed {
                reason: format!(
                    "passphrase too short: must be at least {MIN_PASSPHRASE_LEN} characters \
                     (got {n})",
                ),
            },
            // These encryption/decryption variants should not arise from key
            // derivation alone; surface them with a reason string.
            SyncKeyError::EncryptFailed(msg) => CopypasteError::DecryptionFailed {
                reason: format!("cloud encrypt failed during key derivation: {msg}"),
            },
            SyncKeyError::DecryptFailed => CopypasteError::DecryptionFailed {
                reason: "cloud decrypt failed during key derivation".into(),
            },
            SyncKeyError::BlobTooShort(n) => CopypasteError::DecryptionFailed {
                reason: format!("blob too short during key derivation: {n} bytes"),
            },
        })?;
        Ok(key.as_bytes().to_vec())
    })
}

/// Encrypt `plaintext` for cloud storage.
///
/// `sync_key_bytes` MUST be the 32 bytes returned by `derive_cloud_sync_key`.
/// `item_id` is the item's UUID string — it is bound into the AEAD AAD so
/// substituting the blob into a different item slot fails authentication.
///
/// Returns base64(nonce[24] || ciphertext_with_tag), matching exactly what
/// the macOS daemon POSTs as `payload_ct`.
///
/// Errors: `EncryptionFailed` on AEAD failure, `InvalidKeyLength` if
/// `sync_key_bytes` is not exactly 32 bytes.
pub fn cloud_encrypt(
    item_id: String,
    plaintext: &[u8],
    sync_key_bytes: &[u8],
) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            sync_key_bytes
                .try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let sync_key = copypaste_core::SyncKey::from_bytes(*key_arr);
        let blob = encrypt_for_cloud(&sync_key, &item_id, plaintext)
            .map_err(|_| CopypasteError::EncryptionFailed)?;
        Ok(blob)
    })
}

/// Decrypt a cloud blob produced by `cloud_encrypt` (or the macOS daemon).
///
/// `sync_key_bytes` MUST be the same 32 bytes used during encryption.
/// `item_id` MUST match the value bound into the AAD at encrypt time.
/// `blob` is the raw bytes from base64-decoding the `payload_ct` column.
///
/// Returns the plaintext bytes on success.
///
/// Errors: `DecryptionFailed` if key, item_id, or ciphertext do not match;
/// `InvalidKeyLength` if `sync_key_bytes` is not 32 bytes.
pub fn cloud_decrypt(
    item_id: String,
    blob: &[u8],
    sync_key_bytes: &[u8],
) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            sync_key_bytes
                .try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let sync_key = copypaste_core::SyncKey::from_bytes(*key_arr);
        decrypt_from_cloud(&sync_key, &item_id, blob).map_err(|e| {
            CopypasteError::DecryptionFailed {
                reason: e.to_string(),
            }
        })
    })
}

// ---------------------------------------------------------------------------
// Shared-account relay inbox derivation (R3b — relay-as-database sync path)
//
// The relay sync path uses a SINGLE inbox per account that every device
// co-registers, pushes to, and subscribes to. Both the inbox `device_id` and
// the registration `public_key_b64` are derived DETERMINISTICALLY from the
// shared sync key so Android shares the macOS daemon's inbox without any
// coordination through the relay. These wrappers expose the EXACT core
// functions (`derive_relay_inbox_id` / `derive_relay_public_key`) so the value
// is byte-identical to the daemon's — Kotlin must NEVER re-derive in-app.
//
// SECURITY: the inbox id is SECRET-derived (anyone who learns it can read/write
// the account's still-E2E-encrypted inbox). Kotlin MUST NOT log it or the
// public key. See crates/copypaste-core/src/relay.rs.
// ---------------------------------------------------------------------------

/// Derive the deterministic shared relay inbox `device_id` from the account's
/// 32-byte sync key (the bytes returned by `derive_cloud_sync_key`).
///
/// Returns a canonical lowercase hyphenated UUID string, byte-identical to the
/// macOS daemon's `copypaste_core::derive_relay_inbox_id`, so Android registers
/// and subscribes to the SAME inbox the daemon uses.
///
/// Errors: `InvalidKeyLength` if `sync_key` is not exactly 32 bytes.
///
/// # Security
/// The returned id is derived from secret key material; Kotlin MUST NOT log it.
pub fn relay_inbox_id(sync_key: &[u8]) -> Result<String, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            sync_key
                .try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        Ok(copypaste_core::derive_relay_inbox_id(&key_arr))
    })
}

/// Derive the relay registration `public_key_b64` from the account's 32-byte
/// sync key.
///
/// Returns `base64(derive_relay_public_key(sync_key))` using the STANDARD
/// alphabet, byte-identical to what the macOS daemon presents at registration
/// (`base64::engine::general_purpose::STANDARD.encode(pubkey)`), so all of the
/// account's devices co-register with a consistent value.
///
/// Errors: `InvalidKeyLength` if `sync_key` is not exactly 32 bytes.
///
/// # Security
/// Derived from secret key material; Kotlin MUST NOT log it.
pub fn relay_public_key_b64(sync_key: &[u8]) -> Result<String, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            sync_key
                .try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let pubkey = copypaste_core::derive_relay_public_key(&key_arr);
        Ok(base64::engine::general_purpose::STANDARD.encode(pubkey))
    })
}

// ---------------------------------------------------------------------------
// PG-2 (kmcr): Relay registration Proof-of-Possession (PoP) over Android FFI
//
// The macOS daemon sends HMAC-SHA256(sync_key, "relay-registration-pop-v1:" +
// device_id) at relay registration (relay.rs). Android was missing this export,
// so Kotlin could not compute the PoP and registration silently skipped it.
// This export delegates directly to `copypaste_core::derive_relay_registration_pop`
// — no crypto reimplementation. The result MUST be base64-encoded on the wire.
//
// SECURITY: derived from secret key material; Kotlin MUST NOT log the result.
// ---------------------------------------------------------------------------

/// Compute the relay registration Proof-of-Possession (PoP) for a device.
///
/// Returns `HMAC-SHA256(key=sync_key, msg="relay-registration-pop-v1:" + device_id)`
/// as 32 raw bytes. The caller (Kotlin) MUST base64-encode them for the wire
/// (`pop_b64`) and MUST NOT log the result.
///
/// `sync_key` MUST be the 32 bytes returned by `derive_cloud_sync_key`.
/// `device_id` is the relay inbox id (`relay_inbox_id`), which is also the
/// `device_id` field sent at registration. Using a different value here will
/// produce a PoP that the relay rejects.
///
/// # Security
/// Derived from secret key material; do not log.
pub fn relay_registration_pop(
    sync_key: &[u8],
    device_id: String,
) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            sync_key
                .try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let pop = copypaste_core::derive_relay_registration_pop(&key_arr, &device_id);
        Ok(pop.to_vec())
    })
}

// ---------------------------------------------------------------------------
// PG-16 (89ve): Content-type (TextKind) classifier over Android FFI
//
// Android TextKind.kt re-implemented copypaste-core/src/text_kind.rs, causing
// silent classification drift (e.g. `{;` vs `contains(;)&&contains({)` for Code
// detection). This export delegates to `copypaste_core::text_kind::classify_text`
// so Kotlin can call the SINGLE canonical classifier rather than maintaining a
// parallel one. The Kotlin call-site swap in TextKind.kt is a SEPARATE agent
// step (GRADLE-REQUIRED).
//
// Returns the stable uppercase label (e.g. "TEXT", "URL", "CODE") that matches
// `TextKind::label()` in the Rust source.
// ---------------------------------------------------------------------------

/// Classify a text clipboard payload and return its stable uppercase kind label.
///
/// Delegates to `copypaste_core::text_kind::classify_text`, which is the SINGLE
/// canonical classifier both macOS and (after the Kotlin call-site migration)
/// Android will share. This eliminates the silent drift between TextKind.kt's
/// re-implementation and the core logic.
///
/// Returns one of: `"TEXT"`, `"URL"`, `"EMAIL"`, `"PHONE"`, `"COLOR"`, `"JSON"`,
/// `"CODE"`, `"NUMBER"`, `"PATH"`.
///
/// Wrapped in `panic_boundary::catch` — the classifier runs regex/allocation
/// that could panic; a caught panic returns `"TEXT"` (safest fallback: no
/// misclassification, just no decoration chip).
pub fn classify_text_kind(text: String) -> String {
    panic_boundary::catch(|| classify_text(&text).label().to_string())
        .unwrap_or_else(|_| "TEXT".to_string())
}

// ---------------------------------------------------------------------------
// PG-35 (08r1): Private mode FFI — Rust as the source of truth on Android
//
// macOS private mode is daemon-backed (AtomicBool in IpcHandler, persisted to
// disk by `persist_private_mode`). Android was SharedPrefs-only (Settings.kt:795)
// with no Rust path. The architecture note says SharedPrefs is "architecturally
// fine (no daemon)" but the capture path (ClipboardService.kt:887) must check
// the setting before recording any clip — if that check goes through SharedPrefs
// alone, a Rust code path that captures a clip bypasses the guard.
//
// This FFI exposes a Rust-side `AtomicBool` as the authoritative in-process flag.
// Kotlin MUST:
//   1. At startup: call `set_private_mode(prefs.getBoolean("private_mode", false))`
//      to seed the Rust flag from the persisted SharedPrefs value.
//   2. On every user toggle: call `set_private_mode(enabled)` AND persist to
//      SharedPrefs (Rust does not persist; Android has no daemon/disk store here).
//   3. Before capturing any clip: call `get_private_mode()` on the Rust side so
//      any Rust-side capture path honours the same flag.
//
// SECURITY: private mode suppresses capture of sensitive content. The flag MUST
// be seeded from SharedPrefs before the ClipboardService starts accepting clips.
// ---------------------------------------------------------------------------

/// Process-global private-mode flag.
///
/// `true` = private mode ON (suppress clipboard capture).
/// Initialised to `false` (capture on) at process start. Kotlin seeds it at
/// startup from SharedPrefs and keeps it in sync on every toggle.
static PRIVATE_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Set the private-mode flag. Kotlin MUST call this:
///   - At service startup, seeded from SharedPrefs.
///   - On every user toggle (then also persist to SharedPrefs).
///
/// When `enabled` is `true`, clipboard capture MUST be suppressed by the
/// ClipboardService (check `get_private_mode()` before every capture).
///
/// Wrapped in `panic_boundary::catch` — cannot panic in practice; defensive.
pub fn set_private_mode(enabled: bool) {
    panic_boundary::catch(|| {
        PRIVATE_MODE.store(enabled, std::sync::atomic::Ordering::Relaxed);
    })
    .ok(); // void on panic: flag keeps its previous value rather than crashing JVM
}

/// Read the current private-mode flag. Returns `true` when private mode is ON.
///
/// Kotlin MUST check this on the Rust side before passing any clipboard content
/// to a Rust capture path so Rust-initiated captures honour the same toggle as
/// the SharedPrefs check in ClipboardService.kt:887.
pub fn get_private_mode() -> bool {
    panic_boundary::catch(|| PRIVATE_MODE.load(std::sync::atomic::Ordering::Relaxed))
        .unwrap_or(false) // conservative: default to "no private mode" on impossible panic
}

// ---------------------------------------------------------------------------
// PG-12 (8qcm): Revoke peer + sync-key rotation over Android FFI
//
// macOS exposes `revoke_and_rotate` (ipc.rs:4882): revoke a peer's DB row +
// rotate the cloud sync key under a new passphrase in one atomic step. Android
// DevicesActivity.kt:577 only calls `revoke_device_audit` (DB revoke) — it never
// rotates the sync key. That means the revoked peer still holds the old key and
// can continue decrypting any blobs in the shared relay/cloud inbox.
//
// This FFI adds `revoke_device_and_rotate_key` which:
//   1. Derives the new sync key from the provided passphrase (FAIL FAST if bad).
//   2. Calls `revoke_device_audit` for the audit-table write (db side-effect).
//   3. Returns the new 32-byte derived sync key so Kotlin can:
//      a. Store it in AndroidKeystore (replacing the old key).
//      b. Re-encrypt any locally-cached blobs that must survive (optional, same
//         as macOS wave: remaining devices re-provision).
//      c. Re-derive the relay inbox id + PoP for re-registration under the new key.
//
// SECURITY INVARIANTS (load-bearing — do NOT relax):
//   - Key derivation MUST fail before any revocation mutation so a bad passphrase
//     does not leave the DB in a half-revoked state.
//   - The returned key bytes MUST be stored in AndroidKeystore by Kotlin. The
//     ByteArray MUST be zeroed after persisting (identical contract to
//     `derive_cloud_sync_key`).
//   - Kotlin MUST also call `update_p2p_listener_peers` / `sync_with_peer` with
//     the revoked fingerprint in `revoked_fingerprints` so the mTLS denylist is
//     updated at the transport layer.
//
// RUNTIME VERIFICATION REQUIRED before trusting in production: the full
// round-trip (revoke + re-register with new key + confirm old key rejected) can
// only be tested with a live relay and the Android Gradle build. Flag this as
// GRADLE-REQUIRED for integration test coverage.
// ---------------------------------------------------------------------------

/// Revoke a peer and rotate the cloud sync key to a new passphrase (live build).
///
/// # Steps (in order — FAIL FAST before any mutation)
///
/// 1. Derive `new_sync_key = Argon2id(new_passphrase)`. Returns
///    `DecryptionFailed` if the passphrase is too short or derivation fails —
///    this happens BEFORE any DB write so no revocation occurs on a bad passphrase.
/// 2. Write the revocation audit row via `revoke_device` (DB I/O). Returns
///    `DatabaseError` on failure.
/// 3. Return `new_sync_key` (32 raw bytes) so Kotlin can store it in
///    AndroidKeystore and re-register with the relay under the new key.
///
/// # SECURITY NOTE
/// The returned `Vec<u8>` crosses the FFI boundary unzeroized. UniFFI copies it
/// into a Kotlin `ByteArray`. The Kotlin layer MUST zero that array after
/// persisting the key to AndroidKeystore — this is a load-bearing contract.
/// Kotlin MUST also remove the peer from its P2P roster and call
/// `update_p2p_listener_peers` with the revoked fingerprint in the denylist.
///
/// # GRADLE-REQUIRED
/// Full end-to-end verification (relay re-registration under new key, old-key
/// rejection) requires a live relay and can only be tested via the Android
/// Gradle/instrumented-test pipeline — not host `cargo check`.
#[cfg(feature = "android-uniffi-live")]
pub fn revoke_device_and_rotate_key(
    db_path: String,
    key: &[u8],
    fingerprint: String,
    name: String,
    new_passphrase: String,
) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        // STEP 1: Derive the new key FIRST so a bad passphrase fails before any
        // revocation mutation (mirrors ipc.rs:4910-4918 "Derive the new key FIRST").
        let new_key = derive_new_sync_key_from_passphrase(&new_passphrase)?;

        // STEP 2: Revoke the peer audit row. `key` is the 32-byte device storage
        // key (distinct from the cloud sync key being rotated).
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        with_cached_db(&db_path, &key_arr, |db| {
            copypaste_core::revoke_device(db.conn(), &fingerprint, &name).map_err(|e| {
                CopypasteError::DatabaseError {
                    reason: e.to_string(),
                }
            })
        })?;

        // STEP 3: Return the new key bytes. Kotlin stores them in AndroidKeystore.
        // SECURITY: ByteArray crosses FFI unzeroized — Kotlin MUST zero after storing.
        Ok(new_key.as_bytes().to_vec())
    })
}

/// Stub (feature off): derives and returns the new key WITHOUT the DB revocation
/// write. Kotlin gets the new key bytes so the rotation path can be exercised
/// even without the live DB; the DB revocation must be done separately by the
/// Kotlin layer via `revoke_device_audit` when the live feature is not compiled in.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn revoke_device_and_rotate_key(
    _db_path: String,
    key: &[u8],
    _fingerprint: String,
    _name: String,
    new_passphrase: String,
) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        // Validate the DB key shape (mirrors the live path's key check).
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        // Derive + return the new key; no DB I/O in stub mode.
        let new_key = derive_new_sync_key_from_passphrase(&new_passphrase)?;
        Ok(new_key.as_bytes().to_vec())
    })
}

/// Rotate the cloud sync key to a new passphrase WITHOUT revoking a peer.
///
/// Use this when the user changes their sync passphrase independently of a
/// revocation event. Mirrors the macOS `rotate_sync_key` IPC handler path
/// (ipc.rs:5099-5105) but without the revocation audit write.
///
/// Returns the new 32-byte derived sync key. Kotlin MUST store it in
/// AndroidKeystore and zero the ByteArray after persisting.
///
/// # GRADLE-REQUIRED
/// Full verification requires a live relay — see `revoke_device_and_rotate_key`.
pub fn rotate_sync_key(new_passphrase: String) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let new_key = derive_new_sync_key_from_passphrase(&new_passphrase)?;
        Ok(new_key.as_bytes().to_vec())
    })
}

/// Internal helper: validate a passphrase length and derive a new SyncKey.
///
/// Shared by `revoke_device_and_rotate_key` and `rotate_sync_key` so the
/// validation/error-mapping path is byte-for-byte identical on both call sites —
/// the same pattern `derive_cloud_sync_key` uses. Mirrors the macOS
/// `ipc.rs:4910-4918` "derive FIRST so a bad passphrase fails before mutation".
fn derive_new_sync_key_from_passphrase(
    passphrase: &str,
) -> Result<copypaste_core::SyncKey, CopypasteError> {
    let char_count = passphrase.chars().count();
    if char_count < MIN_PASSPHRASE_LEN {
        return Err(CopypasteError::DecryptionFailed {
            reason: format!(
                "new passphrase too short: must be at least {MIN_PASSPHRASE_LEN} characters \
                 (got {char_count})",
            ),
        });
    }
    derive_sync_key(passphrase).map_err(|e| match e {
        SyncKeyError::PassphraseTooShort(n) => CopypasteError::DecryptionFailed {
            reason: format!(
                "new passphrase too short: must be at least {MIN_PASSPHRASE_LEN} characters \
                 (got {n})",
            ),
        },
        SyncKeyError::Argon2Params(msg) | SyncKeyError::Argon2Hash(msg) => {
            CopypasteError::DecryptionFailed { reason: msg }
        }
        SyncKeyError::EncryptFailed(msg) => CopypasteError::DecryptionFailed {
            reason: format!("key derivation encrypt step failed: {msg}"),
        },
        SyncKeyError::DecryptFailed => CopypasteError::DecryptionFailed {
            reason: "key derivation decrypt step failed".into(),
        },
        SyncKeyError::BlobTooShort(n) => CopypasteError::DecryptionFailed {
            reason: format!("key derivation blob too short: {n} bytes"),
        },
    })
}

// ── QR device pairing ───────────────────────────────────────────────────────
//
// The QR code is purely a transport for the existing PAKE pairing material.
// `pake_password` is the base64url rendering of the single-use token; it is fed
// into the existing password-authenticated pairing flow in place of the
// manually-typed code, preserving every property of that handshake.

/// FFI result of [`build_pairing_qr`].
pub struct PairingQrPayload {
    pub qr: String,
    pub pake_password: String,
}

/// FFI result of [`parse_pairing_qr`].
pub struct ScannedPairing {
    pub fingerprint: String,
    pub device_id: String,
    pub device_name: String,
    pub addr_hint: String,
    pub pake_password: String,
}

/// Build a QR pairing payload (display side). Generates a fresh single-use
/// token internally and returns both the encoded QR string and the PAKE
/// password derived from that token.
pub fn build_pairing_qr(
    fingerprint: String,
    device_id: String,
    device_name: String,
    addr_hint: String,
) -> Result<PairingQrPayload, CopypasteError> {
    panic_boundary::catch_result(|| {
        let payload =
            copypaste_core::PairingPayload::new(fingerprint, device_id, device_name, addr_hint)
                // P2pError is semantically correct here: QR payload generation is
                // pairing infrastructure (token generation / encoding), not a
                // decryption step.  DecryptionFailed was a copy-paste mistake from
                // parse_pairing_qr (the scan side) and is misleading to Kotlin
                // callers trying to distinguish pairing vs. crypto failures.
                .map_err(|e| CopypasteError::P2pError {
                    reason: e.to_string(),
                })?;
        let pake_password = payload.token.to_pake_password();
        let qr = payload.encode();
        Ok(PairingQrPayload { qr, pake_password })
    })
}

/// Parse a scanned QR payload (scan side). Returns the peer pairing material,
/// including the PAKE password to drive the initiator handshake.
///
/// A malformed or unsupported-version payload yields
/// [`CopypasteError::DecryptionFailed`] (reused as the generic parse error so
/// no new FFI error variant / ABI break is needed).
pub fn parse_pairing_qr(payload: String) -> Result<ScannedPairing, CopypasteError> {
    panic_boundary::catch_result(|| {
        let parsed = copypaste_core::PairingPayload::decode(&payload).map_err(|e| {
            CopypasteError::DecryptionFailed {
                reason: e.to_string(),
            }
        })?;
        let pake_password = parsed.token.to_pake_password();
        Ok(ScannedPairing {
            fingerprint: parsed.fingerprint,
            device_id: parsed.device_id,
            device_name: parsed.device_name,
            addr_hint: parsed.addr_hint,
            pake_password,
        })
    })
}

// ---------------------------------------------------------------------------
// P2P pairing FFI — drive the EXISTING copypaste-p2p stack from Android.
//
// Android does NOT reimplement P2P. These wrappers expose the same mTLS cert
// generation and bootstrap PAKE pairing the macOS daemon uses, so the
// fingerprints Android generates/pins are bit-for-bit what the desktop side
// expects. The synchronous UniFFI surface blocks on a long-lived multi-thread
// tokio runtime (the bootstrap handshake drives concurrent TLS read/write).
// ---------------------------------------------------------------------------

/// Process-wide tokio runtime backing the blocking P2P FFI wrappers.
///
/// A single multi-thread runtime is created lazily on first pairing call and
/// reused for the life of the process. Multi-thread is required: the bootstrap
/// handshake interleaves framed TLS reads and writes that would deadlock on a
/// current-thread runtime under `block_on`.
///
/// `OnceLock` only lets us store a fully-initialised value, so we store a
/// `Result` (via an `Option`) to propagate build failures to callers instead
/// of panicking across the FFI boundary. The `Option` is always `Some` after
/// the first call; `None` is unreachable in practice but handled for
/// soundness.
static RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();

/// Return a reference to the shared multi-thread runtime, or an error if it
/// could not be built. Never panics — callers surface the error as
/// `CopypasteError::P2pError` so the JVM is not killed.
fn runtime() -> Result<&'static tokio::runtime::Runtime, CopypasteError> {
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("failed to build tokio runtime for P2P FFI: {e}"))
        })
        .as_ref()
        .map_err(|e| CopypasteError::P2pError { reason: e.clone() })
}

/// FFI result of [`generate_device_cert`]: a fresh self-signed mTLS identity.
///
/// `fingerprint` is `hex(SHA-256(cert_der))` — the SAME value the macOS side
/// pins. Kotlin must persist `cert_der` + `key_der` securely (key_der is
/// secret) and advertise `fingerprint` / `device_id` in the pairing QR.
///
/// # SECURITY NOTE — `key_der` crosses the FFI boundary unzeroized.
/// UniFFI copies it into a Kotlin `ByteArray`. The Kotlin layer MUST zero that
/// array and any copies after use (store in AndroidKeystore; never log/persist
/// the raw bytes). This is a load-bearing contract: failing to do so leaves
/// private key material on the JVM heap until GC.
pub struct DeviceCert {
    pub device_id: String,
    pub fingerprint: String,
    pub cert_der: Vec<u8>,
    pub key_der: Vec<u8>,
}

/// FFI result of [`bootstrap_pair_initiator`]: the outcome of one PAKE pairing.
///
/// `peer_fingerprint` is the responder's pinned cert fingerprint; `session_key`
/// is the 32-byte PAKE+channel-bound key both ends derived.
///
/// # SECURITY NOTE — `session_key` crosses the FFI boundary unzeroized.
/// UniFFI copies it into a Kotlin `ByteArray`. The Kotlin layer MUST zero that
/// array after deriving the content sync key from it — it is a load-bearing
/// contract that must not be skipped, otherwise raw PAKE key material lingers
/// on the JVM heap until GC.
#[derive(Debug)]
pub struct BootstrapResult {
    pub peer_fingerprint: String,
    pub peer_sync_addr: String,
    pub session_key: Vec<u8>,
    /// Sync-account provisioning the PEER advertised over the authenticated
    /// bootstrap tunnel ("QR fully provisions all sync"). `None` when the peer
    /// advertised nothing or is a legacy build. Kotlin persists these later
    /// (Supabase URL/anon key + the derived cloud sync key) so scanning a
    /// configured PC also sets up cloud sync, not just P2P. See
    /// [`SyncProvisioning`].
    pub peer_provisioning: Option<SyncProvisioning>,
    /// HB-1b (ABI 14): the PEER's device metadata, learned in-band over the
    /// authenticated bootstrap tunnel and sourced from `BootstrapPairing.peer_*`.
    /// All `None` when the peer is a legacy build or advertised nothing. Kotlin
    /// persists these on the `PairedPeer` so Wave 3 renders a device card at
    /// parity with macOS. `peer_public_ip` is informational metadata only — never
    /// used for authentication or trust decisions.
    pub peer_model: Option<String>,
    pub peer_os: Option<String>,
    pub peer_app_version: Option<String>,
    pub peer_local_ip: Option<String>,
    pub peer_public_ip: Option<String>,
    /// ABI 17 (CopyPaste-3k6m): the PEER's stable device UUID (from its
    /// `generate_device_cert` / `PeerMeta.device_id`), learned in-band over the
    /// authenticated bootstrap tunnel. `None` for legacy peers that do not
    /// advertise this field. Kotlin persists it as `PairedPeer.peerDeviceId` so
    /// `OriginDeviceFilter` can resolve clipboard item names by UUID instead of
    /// falling back to the TLS cert fingerprint.
    pub peer_device_id: Option<String>,
}

/// FFI mirror of [`copypaste_p2p::bootstrap::SyncProvisioning`].
///
/// Carries the sync-account setup exchanged in-band over the authenticated
/// bootstrap tunnel. The URLs and anon key are non-secret; `derived_sync_key`
/// is the 32-byte DERIVED cloud sync key (NOT the passphrase) and is secret.
///
/// # SECURITY NOTE — `derived_sync_key` crosses the FFI boundary unzeroized.
/// UniFFI copies it into a Kotlin `ByteArray`. The Kotlin layer MUST zero that
/// array after persisting the key (store in AndroidKeystore; never log it) —
/// a load-bearing contract, otherwise raw key material lingers on the JVM heap.
#[derive(Clone)]
pub struct SyncProvisioning {
    pub supabase_url: Option<String>,
    pub supabase_anon_key: Option<String>,
    pub relay_url: Option<String>,
    pub derived_sync_key: Option<Vec<u8>>,
}

impl std::fmt::Debug for SyncProvisioning {
    /// NEVER prints the secret `derived_sync_key` bytes — only a redacted length
    /// marker. The URLs/anon key are non-secret and shown verbatim. Required so
    /// `BootstrapResult`'s `#[derive(Debug)]` does not leak the key.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncProvisioning")
            .field("supabase_url", &self.supabase_url)
            .field("supabase_anon_key", &self.supabase_anon_key)
            .field("relay_url", &self.relay_url)
            .field(
                "derived_sync_key",
                &self
                    .derived_sync_key
                    .as_ref()
                    .map(|k| format!("<{} bytes redacted>", k.len())),
            )
            .finish()
    }
}

impl From<copypaste_p2p::bootstrap::SyncProvisioning> for SyncProvisioning {
    fn from(p: copypaste_p2p::bootstrap::SyncProvisioning) -> Self {
        SyncProvisioning {
            supabase_url: p.supabase_url,
            supabase_anon_key: p.supabase_anon_key,
            relay_url: p.relay_url,
            derived_sync_key: p.derived_sync_key,
        }
    }
}

impl From<SyncProvisioning> for copypaste_p2p::bootstrap::SyncProvisioning {
    fn from(p: SyncProvisioning) -> Self {
        copypaste_p2p::bootstrap::SyncProvisioning {
            supabase_url: p.supabase_url,
            supabase_anon_key: p.supabase_anon_key,
            relay_url: p.relay_url,
            derived_sync_key: p.derived_sync_key,
        }
    }
}

/// Generate a fresh self-signed ECDSA P-256 mTLS certificate for this device,
/// reusing `copypaste_p2p::SelfSignedCert` (the exact mechanism the daemon and
/// P2P transport use). A random `device_id` (UUID) is generated and used as the
/// cert CN; the returned `fingerprint` is `fingerprint_of(cert_der)`.
///
/// Errors: [`CopypasteError::P2pError`] if rcgen certificate generation fails.
pub fn generate_device_cert() -> Result<DeviceCert, CopypasteError> {
    panic_boundary::catch_result(|| {
        let device_id = uuid::Uuid::new_v4().to_string();
        let cert = copypaste_p2p::SelfSignedCert::generate(&device_id).map_err(|e| {
            CopypasteError::P2pError {
                reason: e.to_string(),
            }
        })?;
        let fingerprint = copypaste_p2p::fingerprint_of(&cert.cert_der);
        Ok(DeviceCert {
            device_id,
            fingerprint,
            cert_der: cert.cert_der,
            key_der: cert.key_der,
        })
    })
}

/// Run the initiator side of bootstrap PAKE pairing against a responder at
/// `addr_hint` (a `host:port` string), driving `copypaste_p2p::bootstrap::
/// run_initiator` on the shared runtime.
///
/// `cert_der`/`key_der` are this device's mTLS identity (from
/// [`generate_device_cert`]). `pake_password` is the PAKE password derived from
/// the scanned QR token. `sync_addr` is this device's own P2P sync-listener
/// `host:port`, sent in-band so the peer can persist it.
///
/// Errors: [`CopypasteError::P2pError`] for a malformed `addr_hint`, or any
/// `TransportError` (TLS / socket / framing / PAKE failure, wrong password, or
/// a channel-binding MitM abort).
#[allow(clippy::too_many_arguments)] // FFI contract: identity + addr + 5 meta fields.
pub fn bootstrap_pair_initiator(
    addr_hint: String,
    cert_der: &[u8],
    key_der: &[u8],
    pake_password: String,
    sync_addr: String,
    // "QR fully provisions all sync": optional provisioning THIS device sends to
    // the responder. An Android device scanning a configured PC passes `None`
    // (it has nothing to offer yet); the received provisioning comes back in the
    // result's `peer_provisioning`.
    local_provisioning: Option<SyncProvisioning>,
    // HB-1a (ABI 14): THIS device's own metadata, gathered in Kotlin
    // (`Build.MODEL`, "Android <release>", BuildConfig.VERSION_NAME, device name,
    // LAN IP) and sent in-band so the peer's device card shows real Android info
    // instead of a bare entry. `public_ip` is intentionally not collected here.
    device_name: Option<String>,
    device_model: Option<String>,
    os_version: Option<String>,
    app_version: Option<String>,
    local_ip: Option<String>,
) -> Result<BootstrapResult, CopypasteError> {
    panic_boundary::catch_result(|| {
        let addr: std::net::SocketAddr =
            addr_hint
                .parse()
                .map_err(|e: std::net::AddrParseError| CopypasteError::P2pError {
                    reason: format!("invalid addr_hint '{addr_hint}': {e}"),
                })?;

        let pairing = runtime()?
            .block_on(copypaste_p2p::bootstrap::run_initiator(
                addr,
                cert_der.to_vec(),
                key_der.to_vec(),
                &pake_password,
                &sync_addr,
                // HB-1a: build a real PeerMeta from the Kotlin-gathered fields so
                // the responder records this Android device's name/model/OS/app/IP
                // (was `PeerMeta::default()` — all None — before ABI 14).
                &build_android_peer_meta(
                    device_name,
                    device_model,
                    os_version,
                    app_version,
                    local_ip,
                ),
                local_provisioning.map(Into::into),
            ))
            .map_err(|e| CopypasteError::P2pError {
                reason: e.to_string(),
            })?;

        Ok(bootstrap_result_from_pairing(pairing))
    })
}

/// HB-1a (ABI 14): assemble a `copypaste_p2p::bootstrap::PeerMeta` from the
/// optional device-metadata fields Kotlin gathers and passes across the FFI.
/// `public_ip` is left `None` — Android does not run STUN here. Used by every
/// Android pairing path (initiator, discovery initiator, standing responder) so
/// the peer always sees real Android device info instead of `PeerMeta::default()`.
fn build_android_peer_meta(
    device_name: Option<String>,
    device_model: Option<String>,
    os_version: Option<String>,
    app_version: Option<String>,
    local_ip: Option<String>,
) -> copypaste_p2p::bootstrap::PeerMeta {
    copypaste_p2p::bootstrap::PeerMeta {
        model: device_model,
        os_version,
        app_version,
        local_ip,
        device_name,
        // Android does not collect its own public IP during pairing; the peer's
        // public_ip still flows back to us via `BootstrapResult.peer_public_ip`.
        public_ip: None,
        device_id: None,
    }
}

/// HB-1b (ABI 14): map a completed `BootstrapPairing` into the FFI
/// [`BootstrapResult`], carrying the PEER's `peer_*` metadata through so Kotlin
/// can persist + render it. Shared by the QR-initiator path (the discovery paths
/// build a [`pairing::ConfirmedPairing`] instead).
fn bootstrap_result_from_pairing(
    pairing: copypaste_p2p::bootstrap::BootstrapPairing,
) -> BootstrapResult {
    BootstrapResult {
        peer_fingerprint: pairing.peer_fingerprint,
        peer_sync_addr: pairing.peer_sync_addr,
        session_key: pairing.session_key.as_bytes().to_vec(),
        peer_provisioning: pairing.peer_provisioning.map(Into::into),
        peer_model: pairing.peer_model,
        peer_os: pairing.peer_os,
        peer_app_version: pairing.peer_app_version,
        peer_local_ip: pairing.peer_local_ip,
        peer_public_ip: pairing.peer_public_ip,
        peer_device_id: pairing.peer_device_id,
    }
}

/// HB-1b (ABI 14): map a completed `BootstrapPairing` into the discovery-path
/// [`pairing::ConfirmedPairing`], carrying the PEER's `peer_*` metadata through
/// so the polled [`pairing::PairStatus`] surfaces it to Kotlin on `confirmed`.
/// Shared by both discovery paths (standing responder + `pair_with_discovered`).
fn confirmed_pairing_from(
    p: copypaste_p2p::bootstrap::BootstrapPairing,
) -> pairing::ConfirmedPairing {
    pairing::ConfirmedPairing {
        peer_fingerprint: p.peer_fingerprint,
        peer_sync_addr: p.peer_sync_addr,
        session_key: p.session_key.as_bytes().to_vec(),
        peer_provisioning: p.peer_provisioning.map(Into::into),
        peer_model: p.peer_model,
        peer_os: p.peer_os,
        peer_app_version: p.peer_app_version,
        peer_local_ip: p.peer_local_ip,
        peer_public_ip: p.peer_public_ip,
        peer_device_id: p.peer_device_id,
    }
}

// ---------------------------------------------------------------------------
// P2P clipboard sync FFI — run ONE sync session with an already-paired peer.
//
// Android does NOT reimplement the sync protocol. This drives the SAME
// transport-agnostic `copypaste_sync::SyncEngine::run_session` the desktop
// daemon's engine uses, over the SAME `copypaste_p2p` mTLS transport. Items
// are re-keyed under a shared content key derived from the PAKE session key
// EXACTLY as the macOS daemon's `SyncCrypto` does, so what the peer sends
// decrypts to readable plaintext here (and vice-versa).
// ---------------------------------------------------------------------------

/// Fixed, non-secret domain-separation salt for the P2P content sync key.
///
/// **MUST stay byte-for-byte identical to the macOS daemon's constant.**
/// Canonical location: `crates/copypaste-daemon/src/ipc.rs`, constant
/// `PEER_SYNC_KEY_SALT` (search for `copypaste/p2p/content-sync-key/v1`).
/// Both sides derive the shared XChaCha20-Poly1305 content key from the same
/// PAKE `SessionKey` via `SessionKey::derive_xchacha_key(P2P_SYNC_KEY_SALT)`,
/// so a mismatch here makes every synced item undecryptable on the peer.
///
/// If this value ever needs to change, update BOTH locations in lockstep and
/// bump the P2P protocol version. A shared-crate constant is the correct long-
/// term fix but requires a workspace restructure (out of scope for this patch).
const P2P_SYNC_KEY_SALT: &[u8] = b"copypaste/p2p/content-sync-key/v1";

/// Compile-time assertion that `P2P_SYNC_KEY_SALT` is non-empty.
/// This catches accidental truncation to `b""` during a merge conflict.
const _: () = assert!(
    !P2P_SYNC_KEY_SALT.is_empty(),
    "P2P_SYNC_KEY_SALT must not be empty — check daemon ipc.rs for the canonical value",
);

/// `key_version` stamped on outbound `WireItem`s during P2P sync.
///
/// Must match `ITEM_KEY_VERSION_CURRENT` in `copypaste-core` (currently 2).
/// `WireItem::key_version` is `u8`; the cast is lossless because
/// `ITEM_KEY_VERSION_CURRENT` is a small positive constant.
/// Using this named constant instead of the literal `2` makes accidental drift
/// visible at the use site and during code review.
const P2P_WIRE_KEY_VERSION: u8 = ITEM_KEY_VERSION_CURRENT as u8;

/// A local clipboard item (plaintext) offered to a peer during one sync session.
///
/// `item_id` is the STABLE cross-device identity minted ONCE at capture and
/// reused on every push/sync — the daemon keys merge/dedup/LWW on it, so it
/// must NOT change between sends of the same logical clip. `id` is the local
/// row id (may differ per device). If `item_id` is empty (transitional rows
/// captured before this field existed) the send path falls back to `id`.
#[derive(Debug)]
pub struct LocalItem {
    pub id: String,
    pub item_id: String,
    pub wall_time_ms: i64,
    pub content_type: String,
    pub plaintext: Vec<u8>,
    /// Original filename for file items (e.g. `"report.pdf"`). `None` for text/image items.
    /// Added in ABI 8 to mirror `SyncedItem::file_name` on the outbound side.
    pub file_name: Option<String>,
    /// MIME type for file items (e.g. `"application/pdf"`). `None` for text/image items.
    /// Added in ABI 8 to enable Android→macOS file metadata forwarding.
    pub mime: Option<String>,
    /// ABI 14: soft-delete tombstone flag. When `true` the Rust send path produces a
    /// `WireItem` with `deleted = true` (and empty `content`) so the macOS daemon
    /// applies a tombstone for this `item_id` via LWW. `plaintext` MUST be empty
    /// for tombstones — no decryption is attempted.
    pub deleted: bool,
    /// ABI 14: pin state of this item on the Android device. Carried on the wire so
    /// pin/unpin propagates to macOS and other peers.
    pub pinned: bool,
    /// ABI 14: explicit sort order among pinned items (`None` when not pinned or no
    /// explicit order has been set). Propagates drag-to-reorder across devices.
    pub pin_order: Option<f64>,
}

/// An item received from the peer during sync, decrypted back to plaintext.
///
/// `item_id` is the peer's STABLE cross-device identity for this clip. Kotlin
/// MUST persist it on the stored row and reuse it on any later re-sync so the
/// same logical item is never re-minted (which would resurface as a duplicate).
///
/// `file_name` and `mime` are populated for `content_type == "file"` items only
/// (sourced from the new `WireItem::file_name` / `WireItem::mime` fields added in
/// task #21b). Both are `None` for text/image items.
///
/// ABI 14: `deleted` is `true` when the peer soft-deleted this item. Kotlin MUST
/// write/refresh a local tombstone for this `item_id` via LWW instead of storing
/// visible content. `pinned` and `pin_order` carry the originating device's pin
/// state; Kotlin applies them to the stored row.
#[derive(Debug)]
pub struct SyncedItem {
    pub id: String,
    pub item_id: String,
    pub content_type: String,
    pub plaintext: Vec<u8>,
    pub wall_time_ms: i64,
    /// Original filename for file items (e.g. `"report.pdf"`). `None` for non-file types.
    pub file_name: Option<String>,
    /// MIME type for file items (e.g. `"application/pdf"`). `None` for non-file types.
    pub mime: Option<String>,
    /// ABI 14: true when the originating device soft-deleted this item.
    pub deleted: bool,
    /// ABI 14: pin state on the originating device.
    pub pinned: bool,
    /// ABI 14: explicit pin sort order on the originating device (`None` when unpinned).
    pub pin_order: Option<f64>,
}

/// Outcome of one completed P2P sync session.
#[derive(Debug)]
pub struct P2pSyncResult {
    pub items_received: u64,
    pub items_sent: u64,
    pub items: Vec<SyncedItem>,
    /// Count of inbound text frames skipped because they carried a
    /// `content_nonce` (i.e. a legacy / non-rekeyed peer that hasn't migrated
    /// to the sync-key-wrapped cloud-blob shape). Such frames cannot be
    /// decrypted with the shared sync key, so they are dropped — but, unlike
    /// before, the drop is now both logged and counted here so a build-skew
    /// peer no longer makes items vanish silently. See the
    /// "decrypt 7/7 build-skew" investigation.
    pub items_skipped_legacy: u32,
    /// HB-7a (ABI 14): inbound frames whose shared-key `decrypt_from_cloud`
    /// FAILED (wrong key / corrupt blob / tampered tag). Previously a silent
    /// `continue` — now counted so "received N stored 0" reveals a decrypt
    /// problem rather than vanishing items.
    pub items_skipped_decrypt_fail: u32,
    /// HB-7a (ABI 14): inbound frames whose `content_type` is none of
    /// text/image/file (unknown to this build). Previously a silent `continue`.
    pub items_skipped_unknown_type: u32,
    /// HB-7a (ABI 14): inbound frames of a known type that carried NO `content`
    /// blob to decrypt. Previously a silent `continue`.
    pub items_skipped_missing_blob: u32,
    /// Gap C (mutual unpair): `true` when the peer sent a
    /// `ControlMsg::Unpair` frame on this connection — i.e. the peer has
    /// removed this device from its pairing list. The fingerprint is the
    /// mTLS-authenticated peer, so this can only ever signal an unpair of THIS
    /// peer. Kotlin MUST delete the local pairing record for `peer_fingerprint`
    /// (and stop syncing with it) when this is set. Defaults to `false`.
    pub peer_unpaired: bool,
}

/// Derive the shared content [`SyncKey`](copypaste_core::SyncKey) from a 32-byte
/// PAKE session key, matching the macOS daemon's derivation exactly.
fn shared_sync_key_from_session(
    session_key: &[u8],
) -> Result<copypaste_core::SyncKey, CopypasteError> {
    let arr: [u8; 32] = session_key
        .try_into()
        .map_err(|_| CopypasteError::InvalidKeyLength)?;
    // SessionKey is a thin wrapper over [u8; 32]; the field is public.
    let session = copypaste_p2p::pake::SessionKey(arr);
    let content_key = session.derive_xchacha_key(P2P_SYNC_KEY_SALT);
    Ok(copypaste_core::SyncKey::from_bytes(*content_key))
}

/// Canonicalize a cert fingerprint for denylist comparison: lowercase and
/// strip any `:` separators so a colon-grouped hex string (`AB:CD:…`) matches a
/// bare-hex denylist entry (`abcd…`) and vice versa.
fn canonicalize_fingerprint(fp: &str) -> String {
    fp.chars()
        .filter(|c| *c != ':')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Returns `true` if `fingerprint` is present in `revoked` after canonicalizing
/// both sides. This is the security predicate enforced at the top of
/// [`sync_with_peer`]; it is unit-tested directly so the refusal can be
/// verified without a live socket.
fn is_fingerprint_revoked(fingerprint: &str, revoked: &[String]) -> bool {
    let target = canonicalize_fingerprint(fingerprint);
    revoked
        .iter()
        .any(|r| canonicalize_fingerprint(r) == target)
}

// ── AppConfig over UniFFI (W6 — single source of truth shared with macOS) ────
//
// `Config` mirrors the USER-TUNABLE subset of `copypaste_core::AppConfig`.
// Android keeps its SharedPreferences store but seeds defaults from
// `default_config()` and routes every write through `clamp_config()`, so the
// floors/ceilings/defaults match the macOS daemon exactly (triage B2/B6/B7).
//
// A few fields are Android-only display/runtime knobs with NO `AppConfig`
// counterpart (`mask_sensitive_content`, `p2p_enabled`, `image_max_height`).
// They are carried verbatim through the mappers (no clamp) so the round-trip is
// lossless; the canonical AppConfig fields are the ones actually clamped.

/// Canonical user-tunable configuration shared with the macOS daemon. Mirrors
/// `copypaste_core::AppConfig`'s user-tunable subset; see the UDL `Config`
/// dictionary for per-field clamp ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub max_text_size_bytes: u64,
    pub max_image_size_bytes: u64,
    pub max_file_size_bytes: u64,
    pub storage_quota_bytes: u64,
    pub sensitive_ttl_secs: u64,
    pub poll_interval_ms: u64,
    pub sound_on_copy: bool,
    pub notify_on_copy: bool,
    /// Android-only display knob (no `AppConfig` field). Preserved verbatim.
    pub mask_sensitive_content: bool,
    pub sync_on_wifi_only: bool,
    /// Android-only runtime toggle (no `AppConfig` field). Preserved verbatim.
    pub p2p_enabled: bool,
    pub image_quality: u32,
    /// Android-only display knob (no `AppConfig` field). Preserved verbatim.
    pub image_max_height: u32,
    pub collect_public_ip: bool,
    pub paste_as_plain_text: bool,
    /// Bundle ids / package names excluded from clipboard capture. Maps directly
    /// to `AppConfig::excluded_app_bundle_ids` (round-trips losslessly through
    /// the mappers — `clamp_values()` does not touch this list). Lets the Android
    /// settings UI render + edit the excluded-apps list at parity with macOS.
    pub excluded_app_bundle_ids: Vec<String>,
}

/// Default for the Android-only `mask_sensitive_content` knob. Mirrors the
/// macOS UI default of masking detected secrets in the history list.
const DEFAULT_MASK_SENSITIVE_CONTENT: bool = true;
/// Default for the Android-only `p2p_enabled` runtime toggle. macOS now defaults
/// P2P ON (the daemon is launched with `COPYPASTE_P2P=1` by the app), so a fresh
/// Android install mirrors that "on by default" behaviour for cross-platform
/// parity — scanning the pairing QR yields P2P sync without flipping a toggle.
const DEFAULT_P2P_ENABLED: bool = true;
/// Default for the Android-only `image_max_height` display knob (px). Matches
/// the Maccy-style preview cap used by the history list.
const DEFAULT_IMAGE_MAX_HEIGHT: u32 = 680;

/// Map a `copypaste_core::AppConfig` onto the FFI `Config` dictionary. The
/// Android-only knobs (no AppConfig field) take the supplied carry-through
/// values so the round-trip in `clamp_config` is lossless.
fn config_from_appconfig(
    ac: &copypaste_core::AppConfig,
    mask_sensitive_content: bool,
    p2p_enabled: bool,
    image_max_height: u32,
) -> Config {
    Config {
        max_text_size_bytes: ac.max_text_size_bytes,
        max_image_size_bytes: ac.max_image_size_bytes,
        max_file_size_bytes: ac.max_file_size_bytes,
        storage_quota_bytes: ac.storage_quota_bytes,
        sensitive_ttl_secs: ac.sensitive_ttl_secs,
        poll_interval_ms: ac.poll_interval_ms,
        sound_on_copy: ac.sound_on_copy,
        notify_on_copy: ac.notify_on_copy,
        mask_sensitive_content,
        sync_on_wifi_only: ac.sync_on_wifi_only,
        p2p_enabled,
        // `image_quality` is `u8` in AppConfig (1..=100); widen to `u32` for
        // the FFI dict. Always in range, so the cast is lossless.
        image_quality: ac.image_quality as u32,
        image_max_height,
        collect_public_ip: ac.collect_public_ip,
        paste_as_plain_text: ac.paste_as_plain_text,
        excluded_app_bundle_ids: ac.excluded_app_bundle_ids.clone(),
    }
}

/// Overlay a `Config`'s AppConfig-backed fields onto `AppConfig::default()`.
/// Fields with no AppConfig counterpart are ignored here (they are clamped/kept
/// by the caller). `image_quality` is narrowed back to `u8` with a clamp so an
/// out-of-range FFI value cannot wrap.
fn appconfig_from_config(cfg: &Config) -> copypaste_core::AppConfig {
    copypaste_core::AppConfig {
        max_text_size_bytes: cfg.max_text_size_bytes,
        max_image_size_bytes: cfg.max_image_size_bytes,
        max_file_size_bytes: cfg.max_file_size_bytes,
        storage_quota_bytes: cfg.storage_quota_bytes,
        sensitive_ttl_secs: cfg.sensitive_ttl_secs,
        poll_interval_ms: cfg.poll_interval_ms,
        sound_on_copy: cfg.sound_on_copy,
        notify_on_copy: cfg.notify_on_copy,
        sync_on_wifi_only: cfg.sync_on_wifi_only,
        // Narrow u32 → u8 safely: clamp into the valid quality range first so a
        // hostile/garbage value can never wrap on the `as u8` cast.
        image_quality: cfg.image_quality.clamp(1, 100) as u8,
        collect_public_ip: cfg.collect_public_ip,
        paste_as_plain_text: cfg.paste_as_plain_text,
        excluded_app_bundle_ids: cfg.excluded_app_bundle_ids.clone(),
        ..copypaste_core::AppConfig::default()
    }
}

/// Canonical default configuration: `AppConfig::default()` mapped to `Config`,
/// plus the Android-only knob defaults. PURE — performs no I/O.
pub fn default_config() -> Config {
    // Pure mapping over `AppConfig::default()` — cannot panic in practice; the
    // `catch` is defensive (panics must never cross the JNI boundary). The
    // fallback recomputes the same value so it can never diverge.
    panic_boundary::catch(build_default_config).unwrap_or_else(|_| build_default_config())
}

fn build_default_config() -> Config {
    config_from_appconfig(
        &copypaste_core::AppConfig::default(),
        DEFAULT_MASK_SENSITIVE_CONTENT,
        DEFAULT_P2P_ENABLED,
        DEFAULT_IMAGE_MAX_HEIGHT,
    )
}

/// Clamp a `Config` to the SAME floors/ceilings the macOS daemon enforces, by
/// mapping it onto `AppConfig`, running `AppConfig::clamp_values()`, and mapping
/// back. Android-only knobs (`mask_sensitive_content`, `p2p_enabled`,
/// `image_max_height`) are carried through verbatim. PURE — performs no I/O.
pub fn clamp_config(cfg: Config) -> Config {
    // Pure arithmetic clamp — cannot panic in practice; the `catch` is
    // defensive. On the impossible panic path we return the caller's input
    // unchanged (better than fabricating a value), since clamping is a
    // best-effort tightening, never a correctness invariant the caller relies
    // on for safety.
    let fallback = cfg.clone();
    panic_boundary::catch(move || {
        let mut ac = appconfig_from_config(&cfg);
        ac.clamp_values();
        config_from_appconfig(
            &ac,
            cfg.mask_sensitive_content,
            cfg.p2p_enabled,
            cfg.image_max_height,
        )
    })
    .unwrap_or(fallback)
}

// ── Device-management parity (W7 — revoke / audit) ───────────────────────────
//
// `RevokedPeer` mirrors `copypaste_core::RevokedDevice`. The audit table lives
// in the SQLCipher `copypaste.db` under the same key as the rest of the Android
// store, so these calls are feature-gated exactly like `add_clipboard_item` /
// `get_history_count`: with `android-uniffi-live` off they are pure stubs.

/// One revoked-device audit row (mirror of `copypaste_core::RevokedDevice`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevokedPeer {
    pub fingerprint: String,
    pub name: String,
    pub revoked_at: i64,
}

/// Record a manual peer revocation in the local `revoked_devices` audit table
/// (and remove the matching `devices` row), returning the `revoked_at`
/// timestamp. Live build: writes through `with_cached_db` →
/// `copypaste_core::revoke_device`.
#[cfg(feature = "android-uniffi-live")]
pub fn revoke_device_audit(
    db_path: String,
    key: &[u8],
    fingerprint: String,
    name: String,
) -> Result<u64, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        with_cached_db(&db_path, &key_arr, |db| {
            copypaste_core::revoke_device(db.conn(), &fingerprint, &name).map_err(|e| {
                CopypasteError::DatabaseError {
                    reason: e.to_string(),
                }
            })
        })
    })
}

/// Stub revoke (feature off): no DB I/O; returns a current unix-seconds
/// timestamp so the Kotlin caller's UI can still echo a revoke time. The peer
/// removal from the roster and the denylist enforcement happen Kotlin-side.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn revoke_device_audit(
    _db_path: String,
    key: &[u8],
    _fingerprint: String,
    _name: String,
) -> Result<u64, CopypasteError> {
    panic_boundary::catch_result(|| {
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Ok(now)
    })
}

/// List the fingerprints of all revoked devices, newest first (live build).
#[cfg(feature = "android-uniffi-live")]
pub fn list_revoked_fingerprints(
    db_path: String,
    key: &[u8],
) -> Result<Vec<String>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        with_cached_db(&db_path, &key_arr, |db| {
            let rows = copypaste_core::list_revoked_devices(db.conn()).map_err(|e| {
                CopypasteError::DatabaseError {
                    reason: e.to_string(),
                }
            })?;
            Ok(rows.into_iter().map(|r| r.fingerprint).collect())
        })
    })
}

/// Stub (feature off): no revoked devices to report.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn list_revoked_fingerprints(
    _db_path: String,
    key: &[u8],
) -> Result<Vec<String>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok(Vec::new())
    })
}

/// Richer revoked-device listing (fingerprint + name + revoked_at), newest
/// first (live build).
#[cfg(feature = "android-uniffi-live")]
pub fn list_revoked_peers(db_path: String, key: &[u8]) -> Result<Vec<RevokedPeer>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        with_cached_db(&db_path, &key_arr, |db| {
            let rows = copypaste_core::list_revoked_devices(db.conn()).map_err(|e| {
                CopypasteError::DatabaseError {
                    reason: e.to_string(),
                }
            })?;
            Ok(rows
                .into_iter()
                .map(|r| RevokedPeer {
                    fingerprint: r.fingerprint,
                    name: r.name,
                    revoked_at: r.revoked_at,
                })
                .collect())
        })
    })
}

/// Stub (feature off): no revoked devices to report.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn list_revoked_peers(
    _db_path: String,
    key: &[u8],
) -> Result<Vec<RevokedPeer>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok(Vec::new())
    })
}

/// Run ONE clipboard sync exchange against an already-paired peer over mTLS.
///
/// **Wire protocol — matches the daemon, NOT `SyncEngine::run_session`.** The
/// macOS daemon's per-connection pump (`p2p.rs::run_peer_connection_framed`)
/// does NOT run the HELLO/HAVE/WANT/ITEMS/DONE handshake on a paired link. It
/// KEEPS the `Framed<_, LengthDelimitedCodec>` and exchanges each item as one
/// length-delimited frame carrying a JSON-serialised
/// [`copypaste_sync::protocol::WireItem`]. Right after a connection is
/// accepted it PUSHES its catch-up history (re-keyed under the shared sync
/// key) into the peer as these framed `WireItem`s. A previous version of this
/// FFI peeled the codec and ran `run_session`, so it spoke a different wire
/// protocol than the daemon and live sync failed with "frame too large".
///
/// This function therefore mirrors the daemon's framed pump exactly:
///   1. derive the shared content key from `session_key`;
///   2. connect to `peer_addr` with `peer_fingerprint` allow-listed, KEEPING
///      the length-delimited framing the transport set up;
///   3. SEND each text [`LocalItem`], re-keyed under the shared key
///      (`encrypt_for_cloud`) into the SAME on-wire `WireItem` shape the
///      daemon's `rekey_outbound` emits (self-framed cloud blob in `content`,
///      `content_nonce = None`), as one JSON frame each;
///   4. READ incoming `WireItem` frames (the daemon's catch-up push) until a
///      short idle timeout elapses with no new frame, an item cap is hit, or
///      an overall deadline passes, decrypting each with the shared key
///      (`decrypt_from_cloud`) back to plaintext.
///
/// Errors: [`CopypasteError::P2pError`] for a malformed `peer_addr`, a
/// connect/TLS failure, or a framing/transport error; [`CopypasteError::InvalidKeyLength`]
/// if `session_key` is not 32 bytes.
pub fn sync_with_peer(
    peer_addr: String,
    peer_fingerprint: String,
    session_key: Vec<u8>,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    local_items: Vec<LocalItem>,
    revoked_fingerprints: Vec<String>,
    device_id: String,
) -> Result<P2pSyncResult, CopypasteError> {
    panic_boundary::catch_result(|| {
        use bytes::Bytes;
        use copypaste_p2p::transport::{PairedPeers, PeerTransport};
        use copypaste_sync::protocol::{ControlMsg, PeerFrame, WireItem};
        use futures_util::{SinkExt, StreamExt};

        // SECURITY (load-bearing): refuse to dial a revoked peer at the TRUST
        // layer, BEFORE building `PairedPeers` or opening any socket. This is
        // the Android analog of the daemon's live-allowlist eviction
        // (transport.rs `PairedPeers::remove`): even if a stale roster entry or
        // a queued sync still references this fingerprint, revocation wins.
        // Canonicalize both sides (lowercase, strip ':') so a fingerprint
        // stored colon-separated still matches a bare-hex denylist entry.
        if is_fingerprint_revoked(&peer_fingerprint, &revoked_fingerprints) {
            return Err(CopypasteError::P2pError {
                reason: format!("peer {peer_fingerprint} is revoked"),
            });
        }

        let addr: std::net::SocketAddr =
            peer_addr
                .parse()
                .map_err(|e: std::net::AddrParseError| CopypasteError::P2pError {
                    reason: format!("invalid peer_addr '{peer_addr}': {e}"),
                })?;

        let shared = shared_sync_key_from_session(&session_key)?;
        // Stable per-device origin identity (from `generate_device_cert`,
        // threaded by the caller). Stamped on every outbound `WireItem` so the
        // peer can deduplicate by origin across sync calls. Empty `device_id`
        // (transitional callers) falls back to a fresh UUID to preserve the
        // pre-existing behaviour rather than emitting a blank origin.
        let device_id = if device_id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            device_id
        };

        // Build the outbound `WireItem`s in the SAME sync-key-wrapped wire form
        // the daemon's `rekey_outbound` produces: the cloud blob (self-framed,
        // its own 24-byte nonce prefix) goes in `content`, and `content_nonce`
        // is `None` so the peer recognises it as sync-key-wrapped. Text, image
        // and file items are all re-keyed identically here (v0.6 Option 2 wire
        // contract): the whole plaintext travels as ONE shared-key blob, no
        // per-chunk re-key and no wire `file_id`.
        let mut outbound: Vec<WireItem> = Vec::with_capacity(local_items.len());
        for it in &local_items {
            // Tombstones (deleted=true): emit a WireItem with no content blob so
            // the peer applies the delete via LWW without needing to decrypt anything.
            // The content_type is preserved (typed tombstone) so the peer can route
            // it correctly; plaintext MUST be empty — skip the encrypt step entirely.
            if it.deleted {
                let item_id = if it.item_id.is_empty() {
                    it.id.clone()
                } else {
                    it.item_id.clone()
                };
                let id = if it.id.is_empty() {
                    item_id.clone()
                } else {
                    it.id.clone()
                };
                outbound.push(WireItem {
                    id,
                    item_id,
                    content_type: it.content_type.clone(),
                    content: None,
                    content_nonce: None,
                    blob_ref: None,
                    is_sensitive: false,
                    lamport_ts: it.wall_time_ms,
                    wall_time: it.wall_time_ms,
                    expires_at: None,
                    app_bundle_id: None,
                    origin_device_id: device_id.clone(),
                    key_version: P2P_WIRE_KEY_VERSION,
                    file_name: None,
                    mime: None,
                    deleted: true,
                    pinned: it.pinned,
                    pin_order: it.pin_order,
                });
                continue;
            }
            // Determine the canonical wire content type for this item, or skip
            // it if the type is one we don't sync. Defense-in-depth: callers
            // (the Android Kotlin layer) normalize to the canonical "text"
            // token, but tolerate MIME-style "text/plain" and any "text/*" here
            // so a stored content type never silently drops an item from the
            // send path. Image/file items (Android→macOS symmetry) are carried
            // with their content type preserved.
            let wire_content_type =
                if it.content_type == "text" || it.content_type.starts_with("text/") {
                    "text".to_string()
                } else if it.content_type == "image" || it.content_type.starts_with("image/") {
                    it.content_type.clone()
                } else if it.content_type == "file" {
                    "file".to_string()
                } else {
                    continue;
                };
            // STABLE identity: reuse the caller's `item_id` (minted ONCE at
            // capture and persisted on the row) on every send so the daemon
            // dedups/LWW-merges this clip instead of seeing a new item each
            // push. Only fall back to `id` for transitional rows that predate
            // the `item_id` field; never mint a fresh `Uuid` here (that was the
            // duplicates bug). The cloud blob's AAD is bound to this SAME id.
            let item_id = if it.item_id.is_empty() {
                it.id.clone()
            } else {
                it.item_id.clone()
            };
            let id = if it.id.is_empty() {
                item_id.clone()
            } else {
                it.id.clone()
            };
            let blob = encrypt_for_cloud(&shared, &item_id, &it.plaintext)
                .map_err(|_| CopypasteError::EncryptionFailed)?;
            outbound.push(WireItem {
                id,
                item_id,
                content_type: wire_content_type,
                content: Some(blob),
                // `None` is the daemon's "sync-key-wrapped" unwrap marker.
                content_nonce: None,
                blob_ref: None,
                is_sensitive: false,
                lamport_ts: it.wall_time_ms,
                wall_time: it.wall_time_ms,
                expires_at: None,
                app_bundle_id: None,
                origin_device_id: device_id.clone(),
                // Sync-key-wrapped blobs are version-independent on the wire;
                // the daemon stamps the same default for re-keyed items.
                key_version: P2P_WIRE_KEY_VERSION,
                // For file items, forward the caller-supplied file_name and mime
                // so the macOS daemon can reconstruct the original filename and
                // MIME type on receive (rewrap_inbound_blob already handles them).
                // Text and image items never set these fields.
                file_name: it.file_name.clone(),
                mime: it.mime.clone(),
                // Propagate caller-supplied pin state so pin/unpin/reorder
                // operations travel to the peer alongside content.
                deleted: false,
                pinned: it.pinned,
                pin_order: it.pin_order,
            });
        }

        // Connect over mTLS with the peer fingerprint allow-listed. KEEP the
        // `Framed<_, LengthDelimitedCodec>` the transport set up — the daemon's
        // `run_peer_connection_framed` exchanges length-delimited JSON
        // `WireItem` frames over exactly this framing (NOT `run_session`).
        let peers = PairedPeers::new();
        peers.add(peer_fingerprint.clone(), "android-peer");
        // Gap C: keep a clone of the live allowlist BEFORE it moves into the
        // transport. `PairedPeers` is interior-mutable (shared `Arc<RwLock<…>>`),
        // so removing the peer from this clone on an inbound `ControlMsg::Unpair`
        // also drops it from the transport's verifier for the rest of this call.
        let peers_handle = peers.clone();
        let transport = PeerTransport::from_cert(cert_der, key_der, peers);

        // Bounded receive window: the daemon pushes its catch-up history right
        // after accepting the connection, so frames arrive promptly. We read
        // until any of: no new frame for `IDLE`, `MAX_ITEMS` received, or the
        // overall `DEADLINE` elapses — then we stop (the daemon keeps the link
        // open indefinitely, so we cannot wait for an EOF here).
        const IDLE: std::time::Duration = std::time::Duration::from_secs(3);
        const DEADLINE: std::time::Duration = std::time::Duration::from_secs(20);
        const MAX_ITEMS: usize = 10_000;

        let (received, peer_unpaired): (Vec<WireItem>, bool) = runtime()?
            .block_on(async {
                let mut framed = transport.connect(addr, &peer_fingerprint).await?;
                // Gap C: set when the peer sends a `ControlMsg::Unpair` frame on
                // this connection. The peer is the mTLS-authenticated party (its
                // cert fingerprint was verified by `transport.connect`), so the
                // signal can only unpair THIS peer — never another device.
                let mut unpaired = false;

                // Send this device's items first, mirroring the daemon's
                // outbound write half (`serde_json::to_vec(&WireItem)` → frame).
                for item in &outbound {
                    match serde_json::to_vec(item) {
                        Ok(payload) => framed.send(Bytes::from(payload)).await?,
                        Err(e) => {
                            return Err(copypaste_p2p::transport::TransportError::Io(
                                std::io::Error::other(format!("serialise outbound WireItem: {e}")),
                            ));
                        }
                    }
                }

                // Read incoming frames within the bounded window.
                let mut got: Vec<WireItem> = Vec::new();
                let deadline = tokio::time::Instant::now() + DEADLINE;
                loop {
                    if got.len() >= MAX_ITEMS {
                        break;
                    }
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    let idle = IDLE.min(remaining);
                    match tokio::time::timeout(idle, framed.next()).await {
                        // A frame arrived: deserialise it as a `PeerFrame` exactly
                        // as the daemon's read half does. `PeerFrame` is
                        // `#[serde(untagged)]` with `Data(WireItem)` first, so a
                        // normal item still parses as `Data`; a control frame
                        // (`{"control":"unpair"}`) parses as `Control`.
                        Ok(Some(Ok(frame))) => match serde_json::from_slice::<PeerFrame>(&frame) {
                            Ok(PeerFrame::Data(wire)) => got.push(wire),
                            Ok(PeerFrame::Control(ControlMsg::Unpair)) => {
                                // Gap C: the peer unpaired us. Drop it from the
                                // live allowlist (defence-in-depth for the rest of
                                // this session) and stop reading — the connection
                                // is done. Surface the flag so Kotlin can delete
                                // the local pairing record.
                                peers_handle.remove(&peer_fingerprint);
                                unpaired = true;
                                break;
                            }
                            Ok(PeerFrame::Control(_)) => {
                                // Other control frames (e.g. the Ping/Pong RTT
                                // probes added in CopyPaste-ql7) are not handled on
                                // this Android catch-up read path — ignore and keep
                                // reading. An Android RTT reply is deferred (8dd).
                            }
                            Err(_e) => {
                                // A frame we cannot parse is not fatal — skip it
                                // and keep reading (matches the daemon, which
                                // logs and continues on a deserialise error).
                            }
                        },
                        // Frame-level read error or clean EOF: stop reading and
                        // keep what we already collected. The daemon's read half
                        // (`run_peer_connection_framed`) likewise just drops the
                        // connection on a frame error / EOF rather than failing
                        // the exchange — and the peer dropping its end yields a
                        // non-graceful TLS EOF here, which is expected, not fatal.
                        Ok(Some(Err(_e))) => break,
                        Ok(None) => break,
                        // Idle timeout with no new frame: the catch-up push is
                        // drained, so the receive window is complete.
                        Err(_elapsed) => break,
                    }
                }
                Ok::<(Vec<WireItem>, bool), copypaste_p2p::transport::TransportError>((
                    got, unpaired,
                ))
            })
            .map_err(
                |e: copypaste_p2p::transport::TransportError| CopypasteError::P2pError {
                    reason: e.to_string(),
                },
            )?;

        // Unwrap every received item back to plaintext using the shared key. A
        // sync-key-wrapped text/image/file item carries `content` (the cloud
        // blob) and no `content_nonce`; skip anything that doesn't fit that
        // shape, and skip (rather than fail) a blob we cannot decrypt. Images
        // and files travel under the SAME wrapped shape as text (v0.6 Option 2
        // wire contract): the whole plaintext is ONE shared-key blob, recovered
        // with `decrypt_from_cloud` exactly like text.
        let mut items: Vec<SyncedItem> = Vec::with_capacity(received.len());
        let mut items_skipped_legacy: u32 = 0;
        // HB-7a (ABI 14): per-reason drop counters surfaced to Kotlin so a
        // "received N stored 0" pairing status can show WHY frames dropped.
        let mut items_skipped_decrypt_fail: u32 = 0;
        let mut items_skipped_unknown_type: u32 = 0;
        let mut items_skipped_missing_blob: u32 = 0;
        for wire in &received {
            // A text frame that still carries a `content_nonce` is a legacy /
            // non-rekeyed frame (e.g. a stale daemon that predates the sync-key
            // re-keying). We cannot decrypt it with the shared sync key, so we
            // still skip it — but do NOT do so silently: warn and count it so a
            // build-skew peer is observable instead of making items vanish (this
            // silent `continue` is what hid the "decrypt 7/7" failure).
            if wire.content_type == "text" && wire.content_nonce.is_some() {
                items_skipped_legacy = items_skipped_legacy.saturating_add(1);
                // P2-2ffx: replaced eprintln! (→ logcat black hole on Android)
                // with tracing::debug! which flows through whatever tracing
                // subscriber is initialised in the FFI entry point (or is a
                // no-op when none is set — still better than lost stderr output).
                tracing::debug!(
                    item_id = %wire.item_id,
                    origin = %wire.origin_device_id,
                    "copypaste-android: skipping legacy/non-rekeyed P2P text frame: \
                     content_nonce is set, peer has not migrated to sync-key-wrapped \
                     cloud blobs; cannot decrypt with shared key"
                );
                continue;
            }
            // ABI 14: tombstone frame — the peer soft-deleted this item. Surface
            // it to Kotlin as a SyncedItem with deleted=true and empty plaintext
            // so Kotlin can apply/refresh the local tombstone via LWW without
            // attempting a decrypt. Skip the content-type and blob checks below.
            if wire.deleted {
                items.push(SyncedItem {
                    id: wire.id.clone(),
                    item_id: wire.item_id.clone(),
                    content_type: wire.content_type.clone(),
                    plaintext: Vec::new(),
                    wall_time_ms: wire.wall_time,
                    file_name: None,
                    mime: None,
                    deleted: true,
                    pinned: wire.pinned,
                    pin_order: wire.pin_order,
                });
                continue;
            }
            // Accept text, image and file frames. Every accepted type uses the
            // identical sync-key-wrapped shape (`content` present, `content_nonce`
            // None), so the decrypt path below is shared. Any other content type
            // is unknown to this build and is skipped.
            let is_text = wire.content_type == "text" || wire.content_type.starts_with("text/");
            let is_image = wire.content_type == "image" || wire.content_type.starts_with("image/");
            let is_file = wire.content_type == "file";
            if !(is_text || is_image || is_file) {
                items_skipped_unknown_type = items_skipped_unknown_type.saturating_add(1);
                continue;
            }
            let Some(blob) = wire.content.as_ref() else {
                items_skipped_missing_blob = items_skipped_missing_blob.saturating_add(1);
                continue;
            };
            match decrypt_from_cloud(&shared, &wire.item_id, blob) {
                Ok(plaintext) => items.push(SyncedItem {
                    id: wire.id.clone(),
                    // Carry the peer's STABLE item_id through so Kotlin can
                    // persist it and reuse it on any later re-sync.
                    item_id: wire.item_id.clone(),
                    content_type: wire.content_type.clone(),
                    plaintext,
                    wall_time_ms: wire.wall_time,
                    // Carry filename + mime for file items (populated by the
                    // macOS sender's `rekey_blob_outbound` via #21b wire fields).
                    // Both are None for text/image items — that is correct.
                    file_name: wire.file_name.clone(),
                    mime: wire.mime.clone(),
                    // ABI 14: propagate pin state from the wire.
                    deleted: false,
                    pinned: wire.pinned,
                    pin_order: wire.pin_order,
                }),
                Err(_) => {
                    items_skipped_decrypt_fail = items_skipped_decrypt_fail.saturating_add(1);
                    continue;
                }
            }
        }

        Ok(P2pSyncResult {
            items_received: received.len() as u64,
            items_sent: outbound.len() as u64,
            items,
            items_skipped_legacy,
            items_skipped_decrypt_fail,
            items_skipped_unknown_type,
            items_skipped_missing_blob,
            peer_unpaired,
        })
    })
}

// ── Inbound P2P listener FFI (ABI 11) ─────────────────────────────────────────
//
// These four functions expose the persistent inbound mTLS accept loop in
// `p2p_listener.rs` so macOS can INITIATE a P2P session to Android (today
// Android only dials out). The listener is a long-lived task driven by a
// process-global registry; the FFI returns a `u64` handle (UDL has no interface
// objects). Each wrapper is in a `panic_boundary::catch_result` so a Rust panic
// is surfaced as `CopypasteError::Panicked` instead of killing the JVM.

/// Bind `0.0.0.0:listen_port`, register an inbound mTLS listener, and spawn its
/// accept loop on the shared runtime. Returns IMMEDIATELY with the registry
/// handle and the OS-assigned bound port (pass `listen_port == 0` to let the
/// kernel choose; the real port comes back in `actual_port`).
///
/// `cert_der`/`key_der` are this device's mTLS identity (`generate_device_cert`).
/// `allowed_fingerprints` is the pinned allowlist — ONLY these complete the TLS
/// handshake (pinning IS the authenticator). `revoked_fingerprints` is the
/// denylist re-checked AT ACCEPT before any catch-up/frame (a revoked peer never
/// gets the history push). `session_keys` carries each peer's 32-byte PAKE
/// session key so a frame from peer A is decrypted with A's key (never a global
/// key). `local_items` is the catch-up history pushed once per accepted
/// connection; `device_id` stamps the origin on outbound frames.
///
/// # SECURITY NOTE — `key_der` and each `session_key` cross the FFI boundary
/// unzeroized. The Kotlin layer MUST zero those `ByteArray`s after the call and
/// never log them.
#[allow(clippy::too_many_arguments)] // mirrors `sync_with_peer`'s FFI shape.
pub fn start_p2p_listener(
    listen_port: u16,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    allowed_fingerprints: Vec<String>,
    revoked_fingerprints: Vec<String>,
    session_keys: Vec<PeerSessionKey>,
    local_items: Vec<LocalItem>,
    device_id: String,
) -> Result<P2pListenerHandle, CopypasteError> {
    panic_boundary::catch_result(|| {
        p2p_listener::start(
            runtime()?,
            listen_port,
            cert_der,
            key_der,
            allowed_fingerprints,
            revoked_fingerprints,
            session_keys,
            local_items,
            device_id,
        )
    })
}

/// Atomically drain every item the listener has decrypted from inbound frames
/// since the last poll. Kotlin stores these via the SAME paths the dialer uses
/// (`SyncedItem` → LWW store), so the dial/listen overlap dedups. Returns an
/// empty list for an unknown/stopped `listener_id`.
pub fn poll_p2p_listener(listener_id: u64) -> Result<Vec<SyncedItem>, CopypasteError> {
    panic_boundary::catch_result(|| p2p_listener::poll(listener_id))
}

/// Live roster/denylist/session-key refresh without restarting the listener.
/// Removes any no-longer-allowed or revoked fingerprint from the pinned
/// allowlist immediately (rejected at the next TLS handshake) and replaces the
/// denylist + per-peer session keys. No-op for an unknown `listener_id`.
pub fn update_p2p_listener_peers(
    listener_id: u64,
    allowed: Vec<String>,
    revoked: Vec<String>,
    session_keys: Vec<PeerSessionKey>,
) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        p2p_listener::update_peers(listener_id, allowed, revoked, session_keys)
    })
}

/// Cancel and deregister the listener. Idempotent: a second call (or an unknown
/// id) is a no-op. Fires the cancel token so the accept loop and its
/// per-connection tasks exit and the listener socket is dropped.
pub fn stop_p2p_listener(listener_id: u64) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| p2p_listener::stop(listener_id))
}

// ── Discovery + SAS pairing (ABI 12 — Android parity for LAN discovery) ──────
//
// The Android analog of the macOS daemon's discovery-pairing path. Drives the
// SAME `copypaste_p2p` discovery (mDNS browse/advertise) + bootstrap PAKE stack
// the desktop uses, wired to a POLLED state machine (UniFFI cannot pass an async
// Rust callback). Kotlin starts discovery once, polls `list_discovered`, calls
// `pair_with_discovered` to initiate, polls `pair_get_sas` for the SAS, then
// confirms/aborts. The standing responder bound on `bport` makes the Android
// device pairable FROM macOS. See `pairing.rs` for the full security contract.

/// Start LAN discovery + the standing SAS-pairing responder. Idempotent: a
/// second call tears down and replaces the previous discovery/responder tasks
/// (restart-in-place after a roster / port change).
///
/// Advertises this device over mDNS with the v2 `bport` TXT key (so macOS peers
/// can dial it for SAS pairing) and browses for peers. ALSO binds a standing
/// `BootstrapResponder` on `bport` that accepts inbound discovery-pair
/// connections and runs `run_with_confirm` wired to the SAME coordinator with
/// the `Responder` role — this is what makes Android pairable FROM macOS.
///
/// `cert_der`/`key_der` are this device's mTLS identity (`generate_device_cert`);
/// `sync_port` is the P2P sync-listener port advertised in mDNS; `bport` is the
/// fixed TCP port the standing bootstrap responder binds (advertised so
/// initiators know where to dial). `key_der` is secret — the caller must zero
/// the ByteArray after the call and never log it.
///
/// Errors: [`CopypasteError::P2pError`] if the discovery registration, mDNS
/// daemon, or the standing responder bind fails.
#[allow(clippy::too_many_arguments)] // FFI contract: identity + ports + names.
pub fn start_discovery(
    device_id: String,
    device_name: String,
    sync_port: u16,
    bport: u16,
    cert_der: &[u8],
    key_der: &[u8],
    // HB-1a (ABI 14): THIS device's own metadata, threaded into the standing
    // responder loop so a macOS-INITIATED discovery pair records real Android
    // device info (was `PeerMeta::default()`). `device_name` is already a param;
    // the standing responder reuses it for `PeerMeta.device_name`.
    device_model: Option<String>,
    os_version: Option<String>,
    app_version: Option<String>,
    local_ip: Option<String>,
) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        let rt = runtime()?;
        let cert_der = cert_der.to_vec();
        let key_der = key_der.to_vec();
        // Assemble the responder's PeerMeta once; reuse `device_name` (already a
        // param) for the friendly name field.
        let own_meta = build_android_peer_meta(
            Some(device_name.clone()),
            device_model,
            os_version,
            app_version,
            local_ip,
        );

        // Build + register the discovery service (advertise with bport so we are
        // a v2 peer macOS can pair with) and start its browse task.
        let discovery = std::sync::Arc::new(copypaste_p2p::discovery::DiscoveryService::new());
        discovery
            .register_with_bport(sync_port, device_id.clone(), device_name.clone(), bport)
            .map_err(|e| pairing::p2p_err(format!("discovery register failed: {e}")))?;
        let discovery_for_start = std::sync::Arc::clone(&discovery);
        let browse_task = rt.spawn(async move {
            // `start` returns a JoinHandle for the internal browse loop; await it
            // so this task lives as long as discovery is running. A start error
            // just ends the task (discovery simply yields no peers).
            if let Ok(handle) = discovery_for_start.start().await {
                let _ = handle.await;
            }
        });

        // Spawn the standing bootstrap responder: re-bind `bport`, accept ONE
        // inbound discovery-pair connection per iteration, run `run_with_confirm`
        // wired to the SAME coordinator with the Responder role.
        let responder_task = rt.spawn(standing_responder_loop(bport, cert_der, key_der, own_meta));

        pairing::global().install(discovery, browse_task, responder_task);
        Ok(())
    })
}

/// The standing-responder accept loop (Responder role). Re-binds `bport` for
/// each inbound pairing attempt and runs the confirm-gated responder handshake
/// wired to the global coordinator. Never logs key/SAS bytes.
async fn standing_responder_loop(
    bport: u16,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    // HB-1a (ABI 14): this device's own metadata, advertised to the macOS
    // initiator on every accepted pairing (was `PeerMeta::default()`).
    own_meta: copypaste_p2p::bootstrap::PeerMeta,
) {
    use copypaste_p2p::bootstrap::BootstrapResponder;

    let coordinator = std::sync::Arc::clone(&pairing::global().coordinator);
    loop {
        // Bind the fixed bport afresh each iteration. A *listening* socket that
        // is dropped (not connected) never enters TIME_WAIT, so re-binding the
        // same port succeeds immediately (mirrors the macOS standing responder).
        let responder =
            match BootstrapResponder::bind_on(bport, cert_der.clone(), key_der.clone()).await {
                Ok(r) => r,
                Err(_e) => {
                    // Bind failed (port busy / transient). Back off briefly and retry
                    // so a momentary conflict does not permanently disable inbound
                    // pairing. Never log the error verbatim (no secrets, but keep it
                    // quiet — this loop is hot).
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    continue;
                }
            };

        // Only accept an inbound pairing when idle (single active pairing). If a
        // pairing is already in flight, drop this responder and loop; the next
        // bind happens once the previous one finishes.
        if !coordinator.try_begin(pairing::PairingRole::Responder) {
            drop(responder);
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            continue;
        }

        let confirm_coord = std::sync::Arc::clone(&coordinator);
        // The discovery path uses a FIXED well-known PAKE password
        // (`DISCOVERY_PAIRING_PASSWORD`): opaque-ke is asymmetric, so both ends
        // must register the IDENTICAL password or the handshake fails at frame 7
        // before any SAS is derived. Authentication is ENTIRELY via the SAS
        // compare (see `pairing::DISCOVERY_PAIRING_PASSWORD` docs + plan
        // §"SAS design rationale"). The responder advertises no sync_addr here
        // (Android learns the peer's address from the inbound frames / discovery).
        let result = responder
            .run_with_confirm(
                pairing::DISCOVERY_PAIRING_PASSWORD,
                "",
                // HB-1a: advertise this Android device's real metadata.
                &own_meta,
                None,
                move |sas: &str| {
                    let coord = std::sync::Arc::clone(&confirm_coord);
                    let sas = sas.to_string();
                    async move {
                        // Park on the user's decision, bounded by the SAS window.
                        let rx = coord.enter_awaiting_sas(sas, pairing::PairingRole::Responder);
                        match tokio::time::timeout(pairing::SAS_CONFIRM_TIMEOUT, rx).await {
                            Ok(Ok(accept)) => accept,
                            // Timeout or sender dropped (abort) → reject.
                            _ => false,
                        }
                    }
                },
            )
            .await;

        match result {
            Ok(p) => {
                coordinator.finish(pairing::PairingState::Confirmed(confirmed_pairing_from(p)))
            }
            Err(_e) => {
                // A confirm-rejected SAS, a timeout, an abort, or a network/PAKE
                // failure all land here. Only move out of an active state — if
                // `pair_abort` already set Aborted, leave it. Keys drop/zeroize
                // (nothing persisted). Distinguish timeout vs reject is not
                // observable from the Err alone, so report Aborted unless the
                // coordinator already recorded a terminal state.
                if coordinator.snapshot().is_active() {
                    coordinator.finish(pairing::PairingState::Aborted);
                }
            }
        }
    }
}

/// Stop LAN discovery + the standing responder. Idempotent. Aborts the browse,
/// responder, and any in-flight initiator task and drops the discovery service
/// (releasing the mDNS socket). Any in-flight confirmation is aborted.
pub fn stop_discovery() -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        pairing::global().stop();
        Ok(())
    })
}

/// Snapshot the peers currently discovered on the LAN. Despite its legacy name
/// (frozen for ABI 14), `paired_fingerprints` now carries the caller's set of
/// already-paired IP HOSTS (a peer's `local_ip` / sync-address host) — NOT cert
/// fingerprints.
///
/// HB-4: the mDNS `device_id` is a random UUID, not a cert fingerprint, so the
/// old fingerprint-compare against `device_id` NEVER matched and paired devices
/// kept showing "Pair". We now mark a peer `paired` when ANY of its resolved
/// `ip_addrs` is in the caller-supplied set. Returns an empty list when
/// discovery is not running.
pub fn list_discovered(
    paired_fingerprints: Vec<String>,
) -> Result<Vec<DiscoveredPeer>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let Some(discovery) = pairing::global().discovery() else {
            return Ok(Vec::new());
        };
        // Param name is frozen at ABI 14; the values are paired IP hosts.
        let paired_ips: std::collections::HashSet<String> = paired_fingerprints
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect();
        let peers = discovery
            .peers()
            .into_iter()
            .map(|p| {
                let is_paired = p
                    .ip_addrs
                    .iter()
                    .any(|ip| paired_ips.contains(&ip.to_string()));
                DiscoveredPeer::from_peer_info(p, is_paired)
            })
            .collect();
        Ok(peers)
    })
}

/// Begin pairing (Initiator role) with a discovered peer. Resolves the peer's
/// `bport` + IPv4-first address from discovery, claims the coordinator, and
/// SPAWNS the bootstrap initiator on the shared runtime (does NOT block). Kotlin
/// then polls `pair_get_sas` for the SAS and calls `pair_confirm_sas`.
///
/// `cert_der`/`key_der` are this device's mTLS identity; `sync_addr` is this
/// device's own P2P sync-listener `host:port` (sent in-band); `local_provisioning`
/// is the OPTIONAL sync-account setup this device offers (typically `null` on
/// Android). Errors: [`CopypasteError::P2pError`] if the peer is unknown, lacks a
/// `bport` (v1 peer), advertises no address, or a pairing is already in flight.
#[allow(clippy::too_many_arguments)] // FFI contract: peer id + identity + 5 meta fields.
pub fn pair_with_discovered(
    device_id: String,
    cert_der: &[u8],
    key_der: &[u8],
    sync_addr: String,
    local_provisioning: Option<SyncProvisioning>,
    // HB-1a (ABI 14): THIS device's own metadata, advertised to the discovered
    // peer during the initiator handshake (was `PeerMeta::default()`).
    device_name: Option<String>,
    device_model: Option<String>,
    os_version: Option<String>,
    app_version: Option<String>,
    local_ip: Option<String>,
) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        let rt = runtime()?;
        let global = pairing::global();

        let Some(discovery) = global.discovery() else {
            return Err(pairing::p2p_err("discovery is not running"));
        };
        let peer = discovery
            .resolve_peer(&device_id)
            .ok_or_else(|| pairing::p2p_err(format!("peer {device_id} not found in discovery")))?;
        if peer.bport.is_none() {
            return Err(pairing::p2p_err(
                "peer is a v1 build (no bport) and cannot SAS-pair",
            ));
        }
        let addr = pairing::ipv4_first_addr(&peer)
            .ok_or_else(|| pairing::p2p_err("peer advertised no routable address"))?;

        // Claim the machine (single active pairing). The standing responder uses
        // the same coordinator, so this also refuses while an inbound pairing is
        // in flight.
        if !global
            .coordinator
            .try_begin(pairing::PairingRole::Initiator)
        {
            return Err(pairing::p2p_err("a pairing is already in flight"));
        }

        let coordinator = std::sync::Arc::clone(&global.coordinator);
        let cert_der = cert_der.to_vec();
        let key_der = key_der.to_vec();
        let provisioning = local_provisioning.map(Into::into);
        // HB-1a: build this device's PeerMeta before moving into the task.
        let own_meta =
            build_android_peer_meta(device_name, device_model, os_version, app_version, local_ip);

        let task = rt.spawn(async move {
            use copypaste_p2p::bootstrap::run_initiator_with_confirm;
            let confirm_coord = std::sync::Arc::clone(&coordinator);
            let result = run_initiator_with_confirm(
                addr,
                cert_der,
                key_der,
                // Discovery path: fixed well-known PAKE password; the SAS is the
                // real authenticator (see `pairing::DISCOVERY_PAIRING_PASSWORD`).
                pairing::DISCOVERY_PAIRING_PASSWORD,
                &sync_addr,
                // HB-1a: advertise this Android device's real metadata.
                &own_meta,
                provisioning,
                move |sas: &str| {
                    let coord = std::sync::Arc::clone(&confirm_coord);
                    let sas = sas.to_string();
                    async move {
                        let rx = coord.enter_awaiting_sas(sas, pairing::PairingRole::Initiator);
                        match tokio::time::timeout(pairing::SAS_CONFIRM_TIMEOUT, rx).await {
                            Ok(Ok(accept)) => accept,
                            _ => false,
                        }
                    }
                },
            )
            .await;

            match result {
                Ok(p) => {
                    coordinator.finish(pairing::PairingState::Confirmed(confirmed_pairing_from(p)))
                }
                Err(_e) => {
                    // Reject/timeout/abort/network failure: keys drop/zeroize,
                    // nothing persisted. Only move out of an active state so an
                    // explicit `pair_abort` (Aborted) is not clobbered.
                    if coordinator.snapshot().is_active() {
                        coordinator.finish(pairing::PairingState::Aborted);
                    }
                }
            }
        });
        global.set_initiator_task(task);
        Ok(())
    })
}

/// Poll the current pairing status. While active, `sas` + `role` are populated;
/// the `peer_*` outputs (incl. the 32-byte `session_key`) are populated ONLY
/// when `state == "confirmed"`. Kotlin persists those then calls `pair_reset`.
/// The `session_key` is secret — zero the ByteArray after KEK-wrapping it.
pub fn pair_get_sas() -> Result<PairStatus, CopypasteError> {
    panic_boundary::catch_result(|| {
        let state = pairing::global().coordinator.snapshot();
        Ok(PairStatus::from_state(&state))
    })
}

/// Deliver the local user's accept(`true`)/reject(`false`) SAS decision into the
/// in-flight handshake. A reject drops/zeroizes the session key (nothing
/// persisted). No-op (returns Ok) when no pairing is awaiting confirmation.
pub fn pair_confirm_sas(accept: bool) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        pairing::global().coordinator.deliver_decision(accept);
        Ok(())
    })
}

/// Abort the in-flight pairing: cancel the initiator task, drop the confirmation
/// channel (the handshake's confirm await resolves to a rejection → keys
/// drop/zeroize), and move the machine to `aborted`. Idempotent.
pub fn pair_abort() -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        let global = pairing::global();
        global.abort_initiator();
        global.coordinator.abort();
        Ok(())
    })
}

/// Reset the pairing machine to `idle` (call after observing a terminal state so
/// a fresh pairing may begin). Also aborts any lingering initiator task.
pub fn pair_reset() -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        let global = pairing::global();
        global.abort_initiator();
        global.coordinator.reset();
        Ok(())
    })
}

// Database handle table. OnceLock is stable on Rust 1.70+ (our MSRV is 1.75).
static DB_HANDLES: OnceLock<Mutex<HashMap<u64, copypaste_core::Database>>> = OnceLock::new();
static NEXT_HANDLE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn db_handles() -> &'static Mutex<HashMap<u64, copypaste_core::Database>> {
    DB_HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}

// M5: path+key-keyed cache of open `Database` connections for the *live* FFI
// calls (`add_clipboard_item` / `get_history_count`). Previously each of those
// calls did `Database::open(...)` — a full SQLCipher open (PRAGMA key + key
// derivation + WAL setup) — and dropped the connection at function exit, i.e.
// one open+close per clipboard event. We now open once per `(db_path, key)`
// pair and reuse the connection for the life of the process.
//
// P1-8 fix: the cache key carries a SHA-256 HASH of the 32-byte DB key, NOT
// the raw key bytes. This prevents the key material from surviving on the heap
// as a HashMap key for the lifetime of the process. The hash still provides the
// "different key for the same path → fresh connection" discrimination because
// two distinct keys produce distinct hashes. The raw key is derived ephemerally
// inside `with_cached_db` via `Zeroizing<[u8;32]>` and never stored.
//
// `Database` wraps a `rusqlite::Connection` (Send, !Sync) — serialising all
// access behind this `Mutex` keeps it sound, exactly like the handle table.
// Path+key-hash keyed map of open `Database` connections. Aliased to keep the
// `static`/accessor signatures readable (and to satisfy clippy::type_complexity
// under newer toolchains). Keyed on (db_path, sha256(key)[..32]) so a different
// key for the same path opens a fresh connection rather than reusing one
// authenticated under another key, without retaining raw key material.
#[cfg(feature = "android-uniffi-live")]
type DbByPathMap = Mutex<HashMap<(String, [u8; 32]), copypaste_core::Database>>;

#[cfg(feature = "android-uniffi-live")]
static DB_BY_PATH: OnceLock<DbByPathMap> = OnceLock::new();

#[cfg(feature = "android-uniffi-live")]
fn db_by_path() -> &'static DbByPathMap {
    DB_BY_PATH.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Derive the cache key for `DB_BY_PATH` from a 32-byte DB key.
///
/// P1-8: we store SHA-256(key) rather than the raw key bytes so that raw key
/// material is NOT retained on the heap as part of the HashMap key. Two
/// distinct keys still produce distinct hashes (collision resistance), so the
/// "different key → fresh connection" discrimination is preserved.
///
/// The result is stack-allocated ([u8; 32]) and never written to a static —
/// only the 32-byte hash digest lives in the map key.
///
/// Not gated on `android-uniffi-live` because `open_database` and the
/// `DB_HANDLE_TO_CACHE_KEY` side-map machinery always need it, regardless of
/// whether the live DB cache path is compiled in.
fn key_cache_hash(key: &[u8; 32]) -> [u8; 32] {
    // SHA-256 output is 32 bytes; convert the GenericArray to a plain array.
    Sha256::digest(key.as_ref()).into()
}

/// Side-map from open_database handle → (path, key_hash) so `close_database`
/// can evict the corresponding DB_BY_PATH entry (P1-8 use-after-close fix).
///
/// Populated by `open_database`; cleared by `close_database`. Only active for
/// the handle-table path; the `with_cached_db` path does not use handles.
// The type is self-describing given the field-name comments; aliasing would
// not improve readability. Allow is here because the overall file size crossed
// the clippy::type_complexity token-count threshold after the PG-* additions.
#[allow(clippy::type_complexity)]
static DB_HANDLE_TO_CACHE_KEY: OnceLock<Mutex<HashMap<u64, (String, [u8; 32])>>> = OnceLock::new();

fn db_handle_to_cache_key() -> &'static Mutex<HashMap<u64, (String, [u8; 32])>> {
    DB_HANDLE_TO_CACHE_KEY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Run `f` against the cached `Database` for `(db_path, key)`, opening (and
/// caching) it on first use. The connection is reused across calls with the
/// same path **and** the same key; a different key for the same path opens a
/// separate connection instead of silently reusing the first one.
///
/// P1-8: the map key carries SHA-256(key) rather than the raw key bytes.
#[cfg(feature = "android-uniffi-live")]
fn with_cached_db<T>(
    db_path: &str,
    key: &[u8; 32],
    f: impl FnOnce(&copypaste_core::Database) -> Result<T, CopypasteError>,
) -> Result<T, CopypasteError> {
    let key_hash = key_cache_hash(key); // 32-byte hash; raw key not stored
    let cache_key = (db_path.to_string(), key_hash);
    let mut map = db_by_path().lock().unwrap_or_else(|e| e.into_inner());
    if !map.contains_key(&cache_key) {
        // #40b: evict any stale entry for the same path but a different key
        // before inserting the new connection. Without this, each key rotation
        // leaks a connection handle (the old (path, old_key_hash) entry stays in
        // the map forever). Entries for OTHER paths are unaffected.
        map.retain(|(p, h), _| p != db_path || h == &key_hash);
        let db =
            copypaste_core::Database::open(std::path::Path::new(db_path), key).map_err(|e| {
                CopypasteError::DatabaseError {
                    reason: e.to_string(),
                }
            })?;
        map.insert(cache_key.clone(), db);
    }
    // Just inserted or already present — but use ok_or instead of expect so a
    // logic error here (e.g. if HashMap::insert was somehow rolled back by a
    // reallocation failure) surfaces as a DatabaseError rather than unwinding
    // across the JNI boundary and aborting the JVM.
    let db = map
        .get(&cache_key)
        .ok_or_else(|| CopypasteError::DatabaseError {
            reason: "cache miss after insert — this is a bug".into(),
        })?;
    f(db)
}

/// Open (or create) an encrypted SQLite database at `path` using the 32-byte `key`.
/// Returns an opaque handle for subsequent calls.
///
/// P1-8: records a `handle → (path, SHA-256(key))` entry in
/// `DB_HANDLE_TO_CACHE_KEY` so `close_database` can also evict the
/// corresponding `DB_BY_PATH` entry. The raw key bytes are never stored; only
/// the 32-byte hash is retained alongside the path.
pub fn open_database(path: String, key: &[u8]) -> Result<u64, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let db =
            copypaste_core::Database::open(std::path::Path::new(&path), &key_arr).map_err(|e| {
                CopypasteError::DatabaseError {
                    reason: e.to_string(),
                }
            })?;
        let handle = NEXT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // P1-8: record hash(key) for this handle so close_database can evict
        // the DB_BY_PATH entry without retaining raw key bytes.
        let key_hash = key_cache_hash(&key_arr);
        db_handle_to_cache_key()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(handle, (path.clone(), key_hash));
        // recover from mutex poison instead of panicking across FFI boundary
        db_handles()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(handle, db);
        Ok(handle)
    })
}

/// Release the handle-table entry for `handle`, allowing the `Database`
/// object to be dropped and its underlying SQLCipher connection closed.
///
/// P1-8 fix: also evicts the corresponding `DB_BY_PATH` entry (via the
/// `DB_HANDLE_TO_CACHE_KEY` side-map) so raw key material stored in the cache
/// (now a SHA-256 hash, not the raw bytes) is released, and the use-after-close
/// footgun is eliminated. If the handle was not opened via `open_database` (i.e.
/// it only ever went through `with_cached_db`) the side-map lookup is a no-op.
pub fn close_database(handle: u64) {
    // A poisoned mutex on the global handle table would otherwise abort the
    // JVM via `unwrap()`. Wrapping in `catch_result` converts that into a
    // `Result::Err(PanicError)` we then deliberately discard — `close_database`
    // is declared as void in the UDL and Kotlin callers cannot signal a
    // failure path, but at minimum we keep the process alive.
    let _ = panic_boundary::catch_result(|| {
        // P1-8: look up and remove the (path, key_hash) for this handle, then
        // evict the corresponding DB_BY_PATH entry so it is not kept alive
        // after the handle has been released.
        let _cache_key_opt = db_handle_to_cache_key()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&handle);
        #[cfg(feature = "android-uniffi-live")]
        if let Some(cache_key) = _cache_key_opt {
            db_by_path()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&cache_key);
        }
        db_handles()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&handle); // recover from mutex poison instead of panicking across FFI boundary
        Ok::<(), CopypasteError>(())
    });
}

// ---------------------------------------------------------------------------
// Live binding for Android end-to-end clipboard flow.
//
// Behaviour:
//   * Feature `android-uniffi-live` ON  → open DB at `db_path`, encrypt the
//     text via `copypaste_core::encrypt_item`, build a `ClipboardItem`, and
//     persist via `copypaste_core::insert_item`. Returns the new row id, or
//     an empty string if the text was flagged as sensitive.
//   * Feature OFF (default)            → no DB I/O. Validates the key shape
//     and returns an empty string so Kotlin callers treat it as "not stored
//     natively" and fall through to the SharedPreferences repository.
//
// `key` must be the 32-byte device key (derived from Android Keystore by the
// caller; that derivation lives in Kotlin).
// ---------------------------------------------------------------------------

/// Process-scoped monotonic Lamport counter shared by the two capture paths
/// (`add_clipboard_item` and `store_clipboard_item`).
///
/// Using `SystemTime::now()` here produced timestamps ≈1.7×10^12 that the macOS
/// daemon's `MAX_LAMPORT_SKEW` clamp would eventually reject, and — more critically
/// — mixing wall-clock numbers with the daemon's small logical counter (starting at
/// 1 and ticking once per write) broke LWW ordering: Android items appeared causally
/// far ahead of every Mac item, so Mac writes could never win conflicts.
///
/// This function is the legacy UniFFI capture path (the primary Kotlin path goes
/// through ClipboardRepository.storeItem which manages the persistent LamportClock
/// in SharedPreferences). A per-process atomic gives correct logical ordering within
/// this process; cross-session ordering is maintained by the daemon's `observe()` on
/// ingest.
#[cfg(feature = "android-uniffi-live")]
fn next_android_lamport_ts() -> i64 {
    static LAMPORT_COUNTER: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    LAMPORT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Internal shared implementation for `add_clipboard_item` and
/// `store_clipboard_item` (the new PG-3/349q path with explicit TTL).
///
/// `sensitive_ttl_secs`:
///   - `None`  → use the default (`SENSITIVE_TTL_SECS = 30 s`). This preserves
///               the pre-existing behaviour of `add_clipboard_item` callers that
///               have not yet been updated.
///   - `Some(0)` → auto-wipe disabled (no `expires_at`).
///   - `Some(n)` → stamp `expires_at = now_ms + n * 1000`.
#[cfg(feature = "android-uniffi-live")]
fn store_clipboard_item_inner(
    db_path: &str,
    key_arr: &Zeroizing<[u8; 32]>,
    text: String,
    sensitive_ttl_secs: Option<u64>,
) -> Result<String, CopypasteError> {
    // PG-3 (349q): determine sensitivity using the SAME gate as macOS daemon
    // (`is_sensitive_for_autowipe`, confidence >= 0.70). Do NOT use `detect()`
    // which fires on any pattern — that was the old bug that misaligned the
    // threshold with macOS and caused sensitive items to be dropped instead of
    // stored+marked.
    let is_sensitive = is_sensitive_for_autowipe(&text);

    // v0.3: pre-generate item_id so the AAD baked into the ciphertext matches
    // the value persisted in the row.
    //
    // IMPORTANT: use build_item_aad_v2(item_id, AAD_SCHEMA_VERSION_V4, 2) —
    // NOT the 2-arg build_item_aad(…, AAD_SCHEMA_VERSION=3). The item is
    // stamped with key_version=ITEM_KEY_VERSION_CURRENT=2 by ClipboardItem::new_text,
    // and the daemon decrypts key_version=2 rows with AAD "{item_id}|4|2"
    // (build_item_aad_v2). Using the 2-arg form ("{item_id}|3") causes an
    // auth-tag mismatch and makes every FFI-inserted item undecryptable on
    // the daemon side.
    let item_id = uuid::Uuid::new_v4().to_string();
    let aad = build_item_aad_v2(
        &item_id,
        AAD_SCHEMA_VERSION_V4,
        ITEM_KEY_VERSION_CURRENT as u32,
    );
    let (nonce, ciphertext) = encrypt_item_with_aad(text.as_bytes(), key_arr, &aad)
        .map_err(|_| CopypasteError::EncryptionFailed)?;

    let lamport_ts = next_android_lamport_ts();

    let mut item = copypaste_core::ClipboardItem::new_text(ciphertext, nonce.to_vec(), lamport_ts);
    item.item_id = item_id;

    // PG-3 (349q): stamp is_sensitive so the DB row, sync, and UI all agree.
    // macOS daemon.rs:2170 does the same: `item.is_sensitive = is_sensitive`.
    item.is_sensitive = is_sensitive;

    // PG-24 (5tnx): stamp expires_at for sensitive items, matching
    // daemon.rs:2183: `item.expires_at = Some(now_ms + ttl_secs * 1000)`.
    if is_sensitive {
        let ttl = sensitive_ttl_secs.unwrap_or(copypaste_core::config::SENSITIVE_TTL_SECS);
        if ttl > 0 {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                // Defensive: system clock behind epoch (impossible in practice).
                .unwrap_or(0);
            item.expires_at = Some(now_ms.saturating_add(ttl as i64 * 1000));
        }
        // ttl == 0 → "auto-wipe disabled" sentinel → leave expires_at = None.
    }

    let id = item.id.clone();

    // M5: reuse a cached connection instead of open-per-call.
    with_cached_db(db_path, key_arr, |db| {
        copypaste_core::insert_item(db, &item).map_err(|e| CopypasteError::DatabaseError {
            reason: e.to_string(),
        })
    })?;

    Ok(id)
}

/// Store a clipboard text item. Sensitive items are stored with
/// `is_sensitive = true` and `expires_at` stamped from `SENSITIVE_TTL_SECS`
/// (default 30 s). Returns the new row UUID.
///
/// PG-3 (349q): items are NO LONGER dropped on sensitivity — they are stored
/// encrypted with the `is_sensitive` flag set so the daemon, sync, and UI can
/// all handle them correctly. Use `store_clipboard_item` for the primary path
/// (it accepts an explicit `sensitive_ttl_secs`).
///
/// # GRADLE-REQUIRED
/// Kotlin's ClipboardService.kt must remove the early-return at lines 892-896
/// (text), 993 (image), 1169 (file) that dropped sensitive items before calling
/// this FFI, and instead call this function for ALL captures. The Rust side now
/// makes the store-or-drop decision.
#[cfg(feature = "android-uniffi-live")]
pub fn add_clipboard_item(
    db_path: String,
    key: &[u8],
    text: String,
) -> Result<String, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        // Use None → falls back to SENSITIVE_TTL_SECS default (30 s).
        store_clipboard_item_inner(&db_path, &key_arr, text, None)
    })
}

/// Store a clipboard text item with an EXPLICIT sensitive auto-wipe TTL.
///
/// Primary capture path for PG-3/349q-compliant Kotlin code. Sensitive items
/// are stored encrypted with `is_sensitive = true` and `expires_at` stamped
/// from `sensitive_ttl_secs` (pass 0 to disable auto-wipe for this item).
/// Returns the new row UUID.
///
/// `sensitive_ttl_secs` should be `Config.sensitive_ttl_secs` from the current
/// user config (call `default_config()` for the app default).
///
/// # GRADLE-REQUIRED
/// ClipboardService.kt must:
///   1. Replace `add_clipboard_item` calls with `store_clipboard_item` passing
///      the user's configured TTL.
///   2. Remove the sensitive early-returns (892-896, 993, 1169) that previously
///      dropped sensitive content — the Rust side now stores+marks it.
///   3. On the Kotlin side, check `is_sensitive` from `sensitive_capture_decision`
///      to decide whether to show a masked preview in the notification.
#[cfg(feature = "android-uniffi-live")]
pub fn store_clipboard_item(
    db_path: String,
    key: &[u8],
    text: String,
    sensitive_ttl_secs: u64,
) -> Result<String, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        store_clipboard_item_inner(&db_path, &key_arr, text, Some(sensitive_ttl_secs))
    })
}

#[cfg(not(feature = "android-uniffi-live"))]
pub fn add_clipboard_item(
    _db_path: String,
    key: &[u8],
    _text: String,
) -> Result<String, CopypasteError> {
    panic_boundary::catch_result(|| {
        // Validate key shape to mirror the live path's error surface.
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        // Return empty string so Kotlin callers treat this as "not stored
        // natively" and fall through to the SharedPreferences repository.
        // Previously returned "stub-uniffi-not-live" which was non-empty and
        // caused ClipboardService to skip the fallback store entirely (items lost).
        Ok(String::new())
    })
}

/// Stub `store_clipboard_item` (feature `android-uniffi-live` off): validates
/// the key shape and returns an empty string so Kotlin falls through to the
/// SharedPreferences repository. No DB I/O in stub mode.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn store_clipboard_item(
    _db_path: String,
    key: &[u8],
    _text: String,
    _sensitive_ttl_secs: u64,
) -> Result<String, CopypasteError> {
    panic_boundary::catch_result(|| {
        // Mirror the live path's key-shape error surface.
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok(String::new())
    })
}

#[cfg(feature = "android-uniffi-live")]
pub fn get_history_count(db_path: String, key: &[u8]) -> Result<u64, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        // M5: reuse a cached connection instead of open-per-call.
        let n = with_cached_db(&db_path, &key_arr, |db| {
            copypaste_core::count_items(db).map_err(|e| CopypasteError::DatabaseError {
                reason: e.to_string(),
            })
        })?;
        Ok(n.max(0) as u64)
    })
}

#[cfg(not(feature = "android-uniffi-live"))]
pub fn get_history_count(_db_path: String, key: &[u8]) -> Result<u64, CopypasteError> {
    panic_boundary::catch_result(|| {
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok(0)
    })
}

// ---------------------------------------------------------------------------
// PG-17 (mxoq): FTS5 search over Android FFI
//
// Android ClipboardRepository.searchIds previously did a full-content O(N)
// decrypt scan. The daemon uses `search_items` (FTS5 indexed, O(log N)), so
// the two code paths produced different recall AND incurred vastly different
// CPU cost on large histories.
//
// This export delegates to `copypaste_core::search_items` — the SAME
// FTS5-backed function the daemon's `search` IPC handler uses. The FTS index
// (`clipboard_fts`) lives inside the encrypted SQLCipher database and is
// maintained by `upsert_fts` / `delete_fts` alongside every item write.
//
// SECURITY: only `item_id`, `content_type`, `lamport_ts`, `wall_time`, and
// `is_sensitive` are returned across the FFI boundary — no plaintext content,
// no nonces, no key material. The FTS plaintext index remains inside core /
// SQLCipher and never crosses the FFI.
//
// GRADLE-REQUIRED: the Kotlin call-site swap in ClipboardRepository (replacing
// the O(N) decrypt loop with `fts_search`) requires the Android Gradle build.
// ---------------------------------------------------------------------------

/// One item returned by [`fts_search`].
///
/// Only metadata fields are returned — no plaintext content, no nonces.
/// The FTS index stays inside core/SQLCipher. Kotlin uses `item_id` to look
/// up the stored ciphertext row it already holds locally for rendering or
/// decryption.
#[derive(Debug)]
pub struct SearchResultItem {
    /// The stable cross-device identity (maps to Kotlin's `item_id` column).
    pub item_id: String,
    /// One of "text", "image", "file".
    pub content_type: String,
    /// Lamport clock value — use for causal ordering if needed.
    pub lamport_ts: i64,
    /// Wall-clock capture time in milliseconds since Unix epoch.
    pub wall_time_ms: i64,
    /// Whether this item was flagged as sensitive at capture time.
    pub is_sensitive: bool,
}

/// Search the local FTS5 index for items matching `query`.
///
/// Delegates to `copypaste_core::search_items` — the SAME FTS5 search used by
/// the daemon's `search` IPC handler. Returns up to `limit` results ordered by
/// FTS5 rank (best match first). Returns an empty list when `query` is blank or
/// contains no valid tokens after sanitization.
///
/// # SECURITY
/// Only metadata fields are returned across the FFI boundary; the FTS index
/// and item content remain inside the encrypted SQLCipher database.
///
/// # GRADLE-REQUIRED
/// The Kotlin call-site swap in ClipboardRepository that replaces the O(N)
/// decrypt scan with this function requires the Android Gradle build.
#[cfg(feature = "android-uniffi-live")]
pub fn fts_search(
    db_path: String,
    key: &[u8],
    query: String,
    limit: u32,
) -> Result<Vec<SearchResultItem>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        with_cached_db(&db_path, &key_arr, |db| {
            let items = copypaste_core::search_items(db, &query, limit as usize).map_err(|e| {
                CopypasteError::DatabaseError {
                    reason: e.to_string(),
                }
            })?;
            Ok(items
                .into_iter()
                .map(|it| SearchResultItem {
                    item_id: it.item_id,
                    content_type: it.content_type,
                    lamport_ts: it.lamport_ts,
                    wall_time_ms: it.wall_time,
                    is_sensitive: it.is_sensitive,
                })
                .collect())
        })
    })
}

/// Stub (feature off): validates the key shape, then returns an empty list.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn fts_search(
    _db_path: String,
    key: &[u8],
    _query: String,
    _limit: u32,
) -> Result<Vec<SearchResultItem>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok(Vec::new())
    })
}

// ---------------------------------------------------------------------------
// PG-19 (o0t3): Lamport-ordered history page for Android
//
// Android ClipboardRepository sorted the unpinned history by `wallTimeMs`.
// The correct ordering is `lamport_ts DESC` (with wall_time and origin_device_id
// as deterministic tie-breaks) because the Lamport clock advances monotonically
// on every write and sync — it correctly reflects causal ordering after
// cross-device sync, whereas wall-clock ordering can diverge when device clocks
// differ.
//
// This export delegates to `copypaste_core::get_page_pinned_first_lamport`
// which applies the CRDT-correct ordering: pinned items first (by pin_order),
// then unpinned items by `lamport_ts DESC, wall_time DESC, origin_device_id ASC`.
//
// The lamport_ts value is also returned in `HistoryItem` so Kotlin can validate
// or further sort client-side if needed.
//
// GRADLE-REQUIRED: the Kotlin call-site swap in ClipboardRepository (replacing
// the wall-time ORDER BY with a call to this FFI function) requires the Android
// Gradle build.
// ---------------------------------------------------------------------------

/// One item in the history page returned by [`get_history_page`].
///
/// Only metadata fields are returned — no plaintext content, no nonces, no
/// key material. Kotlin uses `item_id` to look up the stored ciphertext row
/// it already holds for rendering or decryption.
#[derive(Debug)]
pub struct HistoryItem {
    /// The stable cross-device identity (maps to Kotlin's `item_id` column).
    pub item_id: String,
    /// One of "text", "image", "file".
    pub content_type: String,
    /// Lamport clock value. Primary sort key for unpinned items (descending).
    /// Exposed here so Kotlin can verify or further sort if needed.
    pub lamport_ts: i64,
    /// Wall-clock capture time in milliseconds since Unix epoch.
    pub wall_time_ms: i64,
    /// Whether this item was flagged as sensitive at capture time.
    pub is_sensitive: bool,
    /// Whether this item is pinned.
    pub pinned: bool,
    /// Explicit pin sort order for pinned items; `None` for unpinned items.
    pub pin_order: Option<f64>,
}

/// Return a page of clipboard history ordered by:
///   1. Pinned items first, sorted by `pin_order ASC` (then `pin_order IS NULL`).
///   2. Unpinned items sorted by `lamport_ts DESC, wall_time DESC, origin_device_id ASC`.
///
/// This is the CRDT-correct ordering for cross-device sync: the Lamport clock
/// advances on every write/merge so post-sync ordering matches causal history
/// rather than wall-clock skew between devices.
///
/// `offset` / `limit` work identically to `get_page`. The caller should
/// mirror the daemon's `MAX_PAGE` cap (typically 200) when choosing `limit`.
///
/// # GRADLE-REQUIRED
/// The Kotlin call-site swap in ClipboardRepository that replaces the wall-time
/// ORDER BY with this function requires the Android Gradle build.
#[cfg(feature = "android-uniffi-live")]
pub fn get_history_page(
    db_path: String,
    key: &[u8],
    limit: u32,
    offset: u32,
) -> Result<Vec<HistoryItem>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        with_cached_db(&db_path, &key_arr, |db| {
            let items =
                copypaste_core::get_page_pinned_first_lamport(db, limit as usize, offset as usize)
                    .map_err(|e| CopypasteError::DatabaseError {
                        reason: e.to_string(),
                    })?;
            Ok(items
                .into_iter()
                .map(|it| HistoryItem {
                    item_id: it.item_id,
                    content_type: it.content_type,
                    lamport_ts: it.lamport_ts,
                    wall_time_ms: it.wall_time,
                    is_sensitive: it.is_sensitive,
                    pinned: it.pinned,
                    pin_order: it.pin_order,
                })
                .collect())
        })
    })
}

/// Stub (feature off): validates the key shape, then returns an empty list.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn get_history_page(
    _db_path: String,
    key: &[u8],
    _limit: u32,
    _offset: u32,
) -> Result<Vec<HistoryItem>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok(Vec::new())
    })
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> Vec<u8> {
        vec![7u8; 32]
    }

    #[test]
    fn encrypt_then_decrypt_roundtrips() {
        let key = test_key();
        let item_id = "test-android-item".to_string();
        // Use key_version=2 (current daemon default, ITEM_KEY_VERSION_CURRENT=2).
        let blob = encrypt_text(item_id.clone(), b"hello android", &key, 2).expect("encrypt");
        let plaintext =
            decrypt_text(item_id, &blob.ciphertext, &blob.nonce, &key, 2).expect("decrypt");
        assert_eq!(plaintext, b"hello android");
    }

    /// v0.3 regression: ciphertext is bound to item_id via AAD — decrypting
    /// with a different item_id must fail with `DecryptionFailed` rather than
    /// silently returning plaintext (legacy empty-AAD fallback removed in
    /// 1c55e57).
    #[test]
    fn decrypt_rejects_mismatched_item_id() {
        let key = test_key();
        let blob = encrypt_text("item-A".into(), b"secret", &key, 2).expect("encrypt");
        let err = decrypt_text("item-B".into(), &blob.ciphertext, &blob.nonce, &key, 2)
            .expect_err("mismatched item_id must reject");
        assert!(
            matches!(err, CopypasteError::DecryptionFailed { .. }),
            "expected DecryptionFailed, got {err:?}"
        );
    }

    /// CopyPaste-00zz: `decrypt_text_batch` must DEGRADE GRACEFULLY across a mix
    /// of decryptable and undecryptable (wrong-key) items: it returns ONLY the
    /// items that verify+decrypt and reports the rest in `skipped`, instead of
    /// throwing one `DecryptionFailed` per undecryptable legacy row (the ~629
    /// startup-flood bug). A failed auth tag is never accepted as plaintext.
    #[test]
    fn decrypt_text_batch_skips_undecryptable_and_counts_them() {
        let key = test_key();
        // Two decryptable items encrypted under `key`.
        let blob_a = encrypt_text("item-A".into(), b"alpha", &key, 2).expect("encrypt A");
        let blob_b = encrypt_text("item-B".into(), b"bravo", &key, 2).expect("encrypt B");
        // Three undecryptable legacy items: encrypted under a DIFFERENT key,
        // standing in for a rotated/old key whose auth tag fails under `key`.
        let stale_key = vec![0x99u8; 32];
        let bad_1 = encrypt_text("legacy-1".into(), b"x", &stale_key, 2).expect("encrypt bad1");
        let bad_2 = encrypt_text("legacy-2".into(), b"y", &stale_key, 2).expect("encrypt bad2");
        let bad_3 = encrypt_text("legacy-3".into(), b"z", &stale_key, 2).expect("encrypt bad3");

        let items = vec![
            EncryptedItem {
                item_id: "item-A".into(),
                ciphertext: blob_a.ciphertext,
                nonce: blob_a.nonce,
                key_version: 2,
            },
            EncryptedItem {
                item_id: "legacy-1".into(),
                ciphertext: bad_1.ciphertext,
                nonce: bad_1.nonce,
                key_version: 2,
            },
            EncryptedItem {
                item_id: "item-B".into(),
                ciphertext: blob_b.ciphertext,
                nonce: blob_b.nonce,
                key_version: 2,
            },
            EncryptedItem {
                item_id: "legacy-2".into(),
                ciphertext: bad_2.ciphertext,
                nonce: bad_2.nonce,
                key_version: 2,
            },
            EncryptedItem {
                item_id: "legacy-3".into(),
                ciphertext: bad_3.ciphertext,
                nonce: bad_3.nonce,
                key_version: 2,
            },
        ];

        let result = decrypt_text_batch(items, &key).expect("batch must not error");
        assert_eq!(
            result.skipped, 3,
            "the 3 wrong-key items must be skipped + counted, not thrown"
        );
        let mut got: Vec<(String, Vec<u8>)> = result
            .items
            .into_iter()
            .map(|d| (d.item_id, d.plaintext))
            .collect();
        got.sort();
        let mut want = vec![
            ("item-A".to_string(), b"alpha".to_vec()),
            ("item-B".to_string(), b"bravo".to_vec()),
        ];
        want.sort();
        assert_eq!(
            got, want,
            "only the decryptable items, with correct plaintext"
        );
    }

    /// CopyPaste-00zz: an unknown `key_version` is undecryptable by definition
    /// and must be skipped+counted (never panic, never accepted).
    #[test]
    fn decrypt_text_batch_skips_unknown_key_version() {
        let key = test_key();
        let blob = encrypt_text("item-A".into(), b"alpha", &key, 2).expect("encrypt");
        let items = vec![EncryptedItem {
            item_id: "item-A".into(),
            ciphertext: blob.ciphertext,
            nonce: blob.nonce,
            key_version: 7, // unsupported
        }];
        let result = decrypt_text_batch(items, &key).expect("batch must not error");
        assert!(result.items.is_empty());
        assert_eq!(result.skipped, 1);
    }

    /// v0.3 OI-7: a panic raised inside a wrapped UniFFI body must surface
    /// as `CopypasteError::Panicked` (via the `From<PanicError>` impl) rather
    /// than aborting the process.
    #[test]
    fn panic_boundary_converts_to_copypaste_panicked() {
        let result: Result<(), CopypasteError> = panic_boundary::catch_result(|| {
            panic!("synthetic panic inside FFI body");
        });
        match result {
            Err(CopypasteError::Panicked { reason }) => {
                assert!(
                    reason.contains("synthetic panic inside FFI body"),
                    "expected panic message in error, got: {reason}"
                );
            }
            other => panic!("expected CopypasteError::Panicked, got {other:?}"),
        }
    }

    /// CRASH FIX: `is_sensitive`/`sensitive_kind` are now wrapped in the
    /// panic-boundary helper so a panic inside `detect()` can't unwind across
    /// the JNI boundary and abort the JVM. The helper is testable from Rust:
    /// confirm normal inputs return the expected values through the wrapper.
    #[test]
    fn is_sensitive_and_kind_return_expected_through_panic_boundary() {
        // A GitHub PAT is detected by copypaste_core::detect.
        let pat = format!("ghp_{}", "A".repeat(36));
        assert!(is_sensitive(pat.clone()), "PAT must be flagged sensitive");
        assert!(
            sensitive_kind(pat).is_some(),
            "PAT must yield a sensitive kind label"
        );

        // Plain text is not sensitive.
        assert!(
            !is_sensitive("just a plain note".into()),
            "plain text must not be sensitive"
        );
        assert_eq!(
            sensitive_kind("just a plain note".into()),
            None,
            "plain text must yield no kind"
        );
    }

    /// AB-6a (ABI 14): `is_sensitive` now gates on the SAME >= 0.70 confidence
    /// floor macOS uses (`is_sensitive_for_autowipe`) instead of flagging on ANY
    /// `detect()` match. A low-confidence heuristic (a bare US phone number,
    /// confidence 0.55) must NOT be flagged on Android anymore — otherwise such
    /// items vanish at capture/sync-in while macOS keeps them. High-confidence
    /// credentials (a GitHub PAT) must still flag.
    #[test]
    fn is_sensitive_uses_high_confidence_threshold_parity() {
        // High-confidence credential: still flagged.
        let pat = format!("ghp_{}", "A".repeat(36));
        assert!(
            is_sensitive(pat),
            "high-confidence credential (PAT) must remain sensitive"
        );

        // Low-confidence heuristic (bare US phone, 0.55) is below the 0.70 floor:
        // detect() still matches it, but is_sensitive must now return false so the
        // verdict matches the macOS daemon's is_sensitive_for_autowipe gate.
        let phone = "555-123-4567";
        assert!(
            detect(phone).is_some(),
            "precondition: detector matches a US phone (low confidence)"
        );
        assert!(
            !is_sensitive(phone.into()),
            "low-confidence phone match must NOT be flagged (>= 0.70 parity with macOS)"
        );
        // Cross-check: agrees byte-for-byte with the core gate macOS uses.
        assert_eq!(
            is_sensitive(phone.into()),
            is_sensitive_for_autowipe(phone),
            "Android is_sensitive must equal the core >= 0.70 gate"
        );
    }

    #[test]
    fn add_clipboard_item_rejects_bad_key() {
        let err = add_clipboard_item("/tmp/copypaste-test.db".into(), &[0u8; 16], "x".into())
            .expect_err("16-byte key must error");
        assert!(matches!(err, CopypasteError::InvalidKeyLength));
    }

    #[cfg(not(feature = "android-uniffi-live"))]
    #[test]
    fn add_clipboard_item_returns_empty_when_feature_off() {
        let id =
            add_clipboard_item("/dev/null".into(), &test_key(), "hello".into()).expect("stub path");
        // Empty string signals "not stored natively" so Kotlin falls back to
        // SharedPreferences. A non-empty stub value would wrongly suppress the
        // fallback and silently discard every clipboard item.
        assert!(
            id.is_empty(),
            "stub path must return empty string, got {id:?}"
        );
        let n = get_history_count("/dev/null".into(), &test_key()).expect("stub count");
        assert_eq!(n, 0);
    }

    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn add_clipboard_item_persists_when_feature_on() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("live.db");
        let key = test_key();

        let id = add_clipboard_item(
            path.to_string_lossy().into_owned(),
            &key,
            "live android body".into(),
        )
        .expect("insert");
        assert!(!id.is_empty(), "real insert returns a uuid");

        let n = get_history_count(path.to_string_lossy().into_owned(), &key).expect("count");
        assert_eq!(n, 1);
    }

    /// M5: repeated inserts on the same db_path must reuse one cached
    /// connection (no open-per-call) and the count must accumulate correctly
    /// through that shared handle.
    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn live_calls_reuse_cached_connection() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("reuse.db").to_string_lossy().into_owned();
        let key = test_key();

        for i in 0..5 {
            let id = add_clipboard_item(path.clone(), &key, format!("item {i}")).expect("insert");
            assert!(!id.is_empty(), "real insert returns a uuid");
        }

        // Every call above (and this count) went through with_cached_db for the
        // same path, so the same Database connection serviced all of them.
        let n = get_history_count(path.clone(), &key).expect("count");
        assert_eq!(n, 5, "all 5 inserts visible through the reused connection");

        // The (path, key) pair is cached after first use.
        let key_arr: [u8; 32] = key.try_into().expect("test key is 32 bytes");
        assert!(
            db_by_path()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains_key(&(path.clone(), key_arr)),
            "db_(path,key) must be cached after first live call"
        );
    }

    /// PG-3 (349q): sensitive items must be STORED (is_sensitive=true) not dropped.
    ///
    /// The old test asserted `id.is_empty()` and `count == 0`.  After the 349q fix,
    /// sensitive items are encrypted, stored, and flagged — they must produce a
    /// non-empty row id and a count of 1, matching the macOS daemon's behaviour
    /// (daemon.rs:2170-2183).
    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn add_clipboard_item_stores_sensitive_with_flag() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("live.db");
        let key = test_key();

        // GitHub PAT (confidence >= 0.70) — is_sensitive_for_autowipe returns true.
        let pat = format!("ghp_{}", "A".repeat(36));
        let id = add_clipboard_item(path.to_string_lossy().into_owned(), &key, pat)
            .expect("sensitive item must be stored and return Ok");
        assert!(
            !id.is_empty(),
            "PG-3 (349q): sensitive item must produce a non-empty row id"
        );

        let n = get_history_count(path.to_string_lossy().into_owned(), &key).expect("count");
        assert_eq!(
            n, 1,
            "PG-3 (349q): exactly one row must be inserted for sensitive content"
        );
    }

    /// PG-3 (349q): store_clipboard_item stores with explicit TTL.
    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn store_clipboard_item_stores_sensitive_with_explicit_ttl() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("live-ttl.db");
        let key = test_key();

        // Anthropic key — high-confidence sensitive (>= 0.70).
        let ak = format!("sk-ant-api03-{}", "A".repeat(80));
        let id = store_clipboard_item(
            path.to_string_lossy().into_owned(),
            &key,
            ak,
            60, // 60-second TTL
        )
        .expect("store sensitive with explicit TTL");
        assert!(
            !id.is_empty(),
            "store_clipboard_item must return a non-empty row id for sensitive content"
        );

        let n = get_history_count(path.to_string_lossy().into_owned(), &key).expect("count");
        assert_eq!(n, 1, "exactly one row stored");
    }

    /// PG-3 (349q): store_clipboard_item with TTL=0 stores but no expires_at.
    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn store_clipboard_item_ttl_zero_disables_autowipe() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("live-no-ttl.db");
        let key = test_key();

        let aws = "AKIAIOSFODNN7EXAMPLE".to_string();
        let id = store_clipboard_item(path.to_string_lossy().into_owned(), &key, aws, 0)
            .expect("store with ttl=0 (no auto-wipe)");
        assert!(!id.is_empty(), "item stored even with ttl=0");

        let n = get_history_count(path.to_string_lossy().into_owned(), &key).expect("count");
        assert_eq!(n, 1, "exactly one row stored");
    }

    /// PG-23 (l9z8): sensitive_kind must agree with is_sensitive (>= 0.70 floor).
    #[test]
    fn sensitive_kind_aligned_with_is_sensitive_threshold() {
        // High-confidence case: AWS key (0.99) — both must return Some / true.
        let aws = "AKIAIOSFODNN7EXAMPLE".to_string();
        assert!(
            is_sensitive(aws.clone()),
            "is_sensitive must be true for AWS key"
        );
        assert!(
            sensitive_kind(aws).is_some(),
            "sensitive_kind must be Some for AWS key (>= 0.70)"
        );

        // Low-confidence case: phone_us (0.55) — both must return None / false.
        let phone = "(555) 867-5309".to_string();
        assert!(
            !is_sensitive(phone.clone()),
            "is_sensitive must be false for phone (0.55 < 0.70)"
        );
        assert!(
            sensitive_kind(phone).is_none(),
            "sensitive_kind must be None for phone (0.55 < 0.70) — PG-23 alignment"
        );
    }

    /// PG-24 (5tnx): sensitive_expires_at_ms must compute now + ttl*1000.
    #[test]
    fn sensitive_expires_at_ms_basic() {
        let now_ms: i64 = 1_700_000_000_000; // arbitrary fixed timestamp
        let ttl_secs: u64 = 30;
        let expires = sensitive_expires_at_ms(now_ms, ttl_secs);
        assert_eq!(
            expires,
            Some(now_ms + 30_000),
            "expires_at must equal now + ttl_secs * 1000"
        );
    }

    /// PG-24 (5tnx): ttl=0 → None (auto-wipe disabled sentinel).
    #[test]
    fn sensitive_expires_at_ms_zero_ttl_returns_none() {
        let expires = sensitive_expires_at_ms(1_700_000_000_000, 0);
        assert!(
            expires.is_none(),
            "ttl=0 ('auto-wipe disabled') must return None"
        );
    }

    /// PG-4 (ojsh): detect_sensitive_spans returns spans for embedded secrets.
    #[test]
    fn detect_sensitive_spans_finds_aws_key_in_text() {
        let text = "Here is my key: AKIAIOSFODNN7EXAMPLE please keep secret".to_string();
        let spans = detect_sensitive_spans(text.clone());
        assert!(
            !spans.is_empty(),
            "detect_sensitive_spans must find at least one span for embedded AWS key"
        );
        // The span must point at the actual key region.
        let aws_start = text.find("AKIAIOSFODNN7EXAMPLE").expect("key in text");
        let any_matches = spans
            .iter()
            .any(|s| s.start as usize <= aws_start && (s.end as usize) > aws_start);
        assert!(
            any_matches,
            "at least one span must cover the AWS key offset"
        );
    }

    /// PG-4 (ojsh): detect_sensitive_spans returns empty list for benign text.
    #[test]
    fn detect_sensitive_spans_empty_for_benign_text() {
        let text = "Hello, world! This is not sensitive.".to_string();
        let spans = detect_sensitive_spans(text);
        assert!(spans.is_empty(), "no spans expected for benign text");
    }

    /// PG-3 (349q): sensitive_capture_decision returns correct verdict.
    #[test]
    fn sensitive_capture_decision_sensitive_text() {
        let aws = "AKIAIOSFODNN7EXAMPLE".to_string();
        let now_ms: i64 = 1_700_000_000_000;
        let ttl_secs: u64 = 30;

        let d = sensitive_capture_decision(aws, now_ms, ttl_secs);
        assert!(d.is_sensitive, "AWS key must be flagged sensitive");
        assert!(d.kind.is_some(), "AWS key kind must be Some");
        assert_eq!(
            d.expires_at_ms,
            Some(now_ms + 30_000),
            "expires_at_ms must be now + ttl*1000"
        );
    }

    /// PG-3 (349q): sensitive_capture_decision for benign text.
    #[test]
    fn sensitive_capture_decision_benign_text() {
        let d = sensitive_capture_decision("Hello world".to_string(), 1_700_000_000_000, 30);
        assert!(!d.is_sensitive);
        assert!(d.kind.is_none());
        assert!(d.expires_at_ms.is_none());
    }

    /// PG-3 (349q): sensitive_capture_decision with ttl=0 → no expires_at.
    #[test]
    fn sensitive_capture_decision_ttl_zero_no_expiry() {
        let aws = "AKIAIOSFODNN7EXAMPLE".to_string();
        let d = sensitive_capture_decision(aws, 1_700_000_000_000, 0);
        assert!(d.is_sensitive, "still sensitive");
        assert!(
            d.expires_at_ms.is_none(),
            "ttl=0 must produce null expires_at (auto-wipe disabled)"
        );
    }

    // ── Cloud sync crypto tests ──────────────────────────────────────────────

    /// derive_cloud_sync_key must be deterministic: same passphrase → same bytes.
    #[test]
    fn derive_cloud_sync_key_is_deterministic() {
        let k1 = derive_cloud_sync_key("shared-passphrase".into()).expect("derive 1");
        let k2 = derive_cloud_sync_key("shared-passphrase".into()).expect("derive 2");
        assert_eq!(k1, k2, "same passphrase must produce identical key bytes");
        assert_eq!(k1.len(), 32, "key must be exactly 32 bytes");
    }

    /// Different passphrases must produce different keys.
    #[test]
    fn derive_cloud_sync_key_different_passphrases_differ() {
        let k1 = derive_cloud_sync_key("passphrase-alpha".into()).expect("derive 1");
        let k2 = derive_cloud_sync_key("passphrase-beta".into()).expect("derive 2");
        assert_ne!(k1, k2, "different passphrases must yield different keys");
    }

    /// cloud_encrypt + cloud_decrypt must round-trip the plaintext.
    #[test]
    fn cloud_encrypt_decrypt_roundtrip() {
        let key = derive_cloud_sync_key("round-trip-passphrase".into()).expect("derive");
        let item_id = "android-cloud-item-001".to_string();
        let plaintext = b"hello from android";

        let blob = cloud_encrypt(item_id.clone(), plaintext, &key).expect("encrypt");
        let recovered = cloud_decrypt(item_id, &blob, &key).expect("decrypt");
        assert_eq!(recovered, plaintext);
    }

    /// Wrong passphrase must cause DecryptionFailed.
    #[test]
    fn cloud_decrypt_wrong_passphrase_fails() {
        // Passphrases must be >= MIN_PASSPHRASE_LEN (8); derive_sync_key rejects
        // shorter ones with PassphraseTooShort (surfaced here as EncryptionFailed).
        let enc_key = derive_cloud_sync_key("correct-passphrase".into()).expect("derive enc");
        let dec_key = derive_cloud_sync_key("wrong-passphrase".into()).expect("derive dec");
        let blob = cloud_encrypt("item-x".into(), b"data", &enc_key).expect("encrypt");
        let result = cloud_decrypt("item-x".into(), &blob, &dec_key);
        assert!(
            matches!(result, Err(CopypasteError::DecryptionFailed { .. })),
            "wrong passphrase must produce DecryptionFailed, got {result:?}"
        );
    }

    /// Wrong item_id (AAD mismatch) must cause DecryptionFailed.
    #[test]
    fn cloud_decrypt_wrong_item_id_fails() {
        let key = derive_cloud_sync_key("aad-test".into()).expect("derive");
        let blob = cloud_encrypt("item-correct".into(), b"payload", &key).expect("encrypt");
        let result = cloud_decrypt("item-wrong".into(), &blob, &key);
        assert!(
            matches!(result, Err(CopypasteError::DecryptionFailed { .. })),
            "wrong item_id must produce DecryptionFailed, got {result:?}"
        );
    }

    /// cloud_encrypt with a non-32-byte key must return InvalidKeyLength.
    #[test]
    fn cloud_encrypt_invalid_key_length() {
        let result = cloud_encrypt("item-bad".into(), b"data", &[0u8; 16]);
        assert!(
            matches!(result, Err(CopypasteError::InvalidKeyLength)),
            "16-byte key must return InvalidKeyLength"
        );
    }

    /// cloud_decrypt with a non-32-byte key must return InvalidKeyLength.
    #[test]
    fn cloud_decrypt_invalid_key_length() {
        let result = cloud_decrypt("item-bad".into(), &[0u8; 50], &[0u8; 16]);
        assert!(
            matches!(result, Err(CopypasteError::InvalidKeyLength)),
            "16-byte key must return InvalidKeyLength"
        );
    }

    // ── P2P pairing FFI tests ────────────────────────────────────────────────

    /// `generate_device_cert` returns a non-empty cert/key and a fingerprint
    /// that matches `fingerprint_of(cert_der)` — i.e. the SAME value the peer
    /// pins. Two calls produce distinct identities.
    #[test]
    fn generate_device_cert_fingerprint_matches() {
        let c = generate_device_cert().expect("cert gen");
        assert!(!c.cert_der.is_empty(), "cert DER must not be empty");
        assert!(!c.key_der.is_empty(), "key DER must not be empty");
        assert!(!c.device_id.is_empty(), "device_id must not be empty");
        assert_eq!(
            c.fingerprint,
            copypaste_p2p::fingerprint_of(&c.cert_der),
            "FFI fingerprint must equal fingerprint_of(cert_der)"
        );

        let c2 = generate_device_cert().expect("cert gen 2");
        assert_ne!(c.fingerprint, c2.fingerprint, "each cert is unique");
    }

    /// End-to-end: spin up a real `BootstrapResponder` on a loopback port in a
    /// background thread (with its own runtime), then call the
    /// `bootstrap_pair_initiator` FFI wrapper against it. Proves the FFI path
    /// completes a real PAKE + RFC 5705 channel-binding handshake over TLS:
    /// it must return the responder's cert fingerprint and a 32-byte session
    /// key. The responder thread asserts both ends derived the same key.
    #[test]
    fn bootstrap_pair_initiator_pairs_over_loopback() {
        use copypaste_p2p::bootstrap::BootstrapResponder;
        use std::sync::mpsc;

        let responder_cert = generate_device_cert().expect("responder cert");
        let initiator_cert = generate_device_cert().expect("initiator cert");
        let responder_fp = responder_cert.fingerprint.clone();
        let initiator_fp = initiator_cert.fingerprint.clone();

        let password = "shared-qr-secret-abcdef";
        let resp_sync_addr = "127.0.0.1:7001";
        let init_sync_addr = "127.0.0.1:7002";

        // The responder runs on its OWN runtime in a background OS thread so the
        // main test thread is free of an ambient runtime and can call the
        // synchronous FFI wrapper (which itself does runtime().block_on(...)).
        let (port_tx, port_rx) = mpsc::channel::<u16>();
        let resp_cert_der = responder_cert.cert_der.clone();
        let resp_key_der = responder_cert.key_der.clone();
        let pw = password.to_string();
        let responder_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("responder runtime");
            rt.block_on(async move {
                let responder = BootstrapResponder::bind(resp_cert_der, resp_key_der)
                    .await
                    .expect("bind responder");
                let port = responder.local_addr().expect("local addr").port();
                port_tx.send(port).expect("send port");
                responder
                    .run(
                        &pw,
                        resp_sync_addr,
                        // HB-1b: responder advertises real PeerMeta so the FFI
                        // initiator receives it in BootstrapResult.peer_*.
                        &copypaste_p2p::bootstrap::PeerMeta {
                            model: Some("MacBook Pro".into()),
                            os_version: Some("macOS 15.5".into()),
                            app_version: Some("0.6.1".into()),
                            local_ip: Some("127.0.0.1".into()),
                            device_name: Some("Test Mac".into()),
                            public_ip: Some("203.0.113.9".into()),
                            device_id: None,
                        },
                        // Responder advertises provisioning so the FFI initiator
                        // receives it in `peer_provisioning`.
                        Some(copypaste_p2p::bootstrap::SyncProvisioning {
                            supabase_url: Some("https://proj.supabase.co".into()),
                            supabase_anon_key: Some("anon-key".into()),
                            relay_url: None,
                            derived_sync_key: Some(vec![3u8; 32]),
                        }),
                    )
                    .await
            })
        });

        let port = port_rx.recv().expect("responder port");
        let addr_hint = format!("127.0.0.1:{port}");

        let result = bootstrap_pair_initiator(
            addr_hint,
            &initiator_cert.cert_der,
            &initiator_cert.key_der,
            password.to_string(),
            init_sync_addr.to_string(),
            None,
            // HB-1a: this device's own metadata, sent in-band to the responder.
            Some("Pixel 8".into()),
            Some("Pixel 8".into()),
            Some("Android 15".into()),
            Some("0.6.1".into()),
            Some("127.0.0.1".into()),
        )
        .expect("FFI bootstrap pairing must succeed over loopback");

        // HB-1b: the FFI initiator received the responder's device metadata.
        assert_eq!(result.peer_model.as_deref(), Some("MacBook Pro"));
        assert_eq!(result.peer_os.as_deref(), Some("macOS 15.5"));
        assert_eq!(result.peer_app_version.as_deref(), Some("0.6.1"));
        assert_eq!(result.peer_local_ip.as_deref(), Some("127.0.0.1"));
        assert_eq!(result.peer_public_ip.as_deref(), Some("203.0.113.9"));

        // QR-provisions-all-sync: the FFI initiator received the responder's
        // advertised provisioning, including the derived sync key bytes.
        let prov = result
            .peer_provisioning
            .as_ref()
            .expect("initiator must receive peer provisioning");
        assert_eq!(
            prov.supabase_url.as_deref(),
            Some("https://proj.supabase.co")
        );
        assert_eq!(prov.derived_sync_key.as_deref(), Some(&[3u8; 32][..]));

        // The FFI wrapper learned the responder's REAL pinned cert fingerprint.
        assert_eq!(
            result.peer_fingerprint, responder_fp,
            "initiator must pin the responder's cert fingerprint"
        );
        assert_eq!(result.peer_sync_addr, resp_sync_addr);
        assert_eq!(
            result.session_key.len(),
            32,
            "PAKE session key must be 32 bytes"
        );

        // The responder side must have derived the same key and learned our fp.
        let resp_pairing = responder_thread
            .join()
            .expect("responder thread join")
            .expect("responder pairing");
        assert_eq!(resp_pairing.peer_fingerprint, initiator_fp);
        assert_eq!(
            resp_pairing.session_key.as_bytes().as_slice(),
            result.session_key.as_slice(),
            "both endpoints must derive the same PAKE session key via the FFI path"
        );
        // HB-1a: the responder (macOS side) learned this Android device's real
        // metadata that the FFI initiator sent in-band — was None before ABI 14.
        assert_eq!(resp_pairing.peer_model.as_deref(), Some("Pixel 8"));
        assert_eq!(resp_pairing.peer_os.as_deref(), Some("Android 15"));
        assert_eq!(resp_pairing.peer_app_version.as_deref(), Some("0.6.1"));
        assert_eq!(resp_pairing.peer_local_ip.as_deref(), Some("127.0.0.1"));
    }

    /// REGRESSION (live emulator↔macOS divergence): after a real network PAKE
    /// pairing the macOS daemon (PAKE **responder**) re-keys catch-up items under
    /// the content sync key it derives from its pairing result, and the Android
    /// FFI (PAKE **initiator**) must derive the IDENTICAL key from the
    /// `session_key` the FFI returns — otherwise `decrypt_from_cloud` rejects
    /// every pushed item (itemsReceived=N, items=[]), the exact symptom seen live.
    ///
    /// This drives the real `BootstrapResponder::run` + `bootstrap_pair_initiator`
    /// over a loopback TLS socket, then derives the content sync key two ways: the
    /// DAEMON way (`derive_peer_sync_key_b64`'s exact derivation from the
    /// responder's `BootstrapPairing.session_key`) and the ANDROID way
    /// (`shared_sync_key_from_session` from the initiator's returned
    /// `BootstrapResult.session_key`). It asserts the two `SyncKey`s are
    /// byte-equal AND that a blob the daemon would push (`encrypt_for_cloud` under
    /// the daemon key) decrypts under the Android key. A divergence in which key
    /// (raw vs channel-bound) each side feeds into derivation makes this fail.
    #[test]
    fn pairing_derives_matching_content_sync_key_daemon_and_ffi() {
        use copypaste_p2p::bootstrap::BootstrapResponder;
        use std::sync::mpsc;

        let responder_cert = generate_device_cert().expect("responder cert");
        let initiator_cert = generate_device_cert().expect("initiator cert");

        let password = "shared-qr-secret-rekey";
        let resp_sync_addr = "127.0.0.1:7101";
        let init_sync_addr = "127.0.0.1:7102";

        // Responder (== macOS daemon role) on its own runtime / OS thread so the
        // main thread can call the synchronous FFI initiator wrapper.
        let (port_tx, port_rx) = mpsc::channel::<u16>();
        let resp_cert_der = responder_cert.cert_der.clone();
        let resp_key_der = responder_cert.key_der.clone();
        let pw = password.to_string();
        let responder_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("responder runtime");
            rt.block_on(async move {
                let responder = BootstrapResponder::bind(resp_cert_der, resp_key_der)
                    .await
                    .expect("bind responder");
                let port = responder.local_addr().expect("local addr").port();
                port_tx.send(port).expect("send port");
                responder
                    .run(
                        &pw,
                        resp_sync_addr,
                        &copypaste_p2p::bootstrap::PeerMeta::default(),
                        None,
                    )
                    .await
            })
        });

        let port = port_rx.recv().expect("responder port");
        let addr_hint = format!("127.0.0.1:{port}");

        let init_result = bootstrap_pair_initiator(
            addr_hint,
            &initiator_cert.cert_der,
            &initiator_cert.key_der,
            password.to_string(),
            init_sync_addr.to_string(),
            None,
            // ABI 14 device-meta params — not exercised in this key-derivation test.
            None,
            None,
            None,
            None,
            None,
        )
        .expect("FFI bootstrap pairing must succeed over loopback");

        let resp_pairing = responder_thread
            .join()
            .expect("responder thread join")
            .expect("responder pairing");

        // DAEMON derivation: `derive_peer_sync_key_b64` persists
        // `session_key.derive_xchacha_key(P2P_SYNC_KEY_SALT)` into peers.json and
        // `SyncCrypto::shared_sync_key` reads it back through a lossless base64
        // round-trip, so the effective key is exactly these derived bytes from
        // the responder's pairing result.
        let daemon_key = copypaste_core::SyncKey::from_bytes(
            *resp_pairing
                .session_key
                .derive_xchacha_key(P2P_SYNC_KEY_SALT),
        );

        // ANDROID derivation: the exact FFI path from the session_key the FFI
        // returned to Kotlin.
        let android_key = shared_sync_key_from_session(&init_result.session_key)
            .expect("FFI derives content sync key from returned session_key");

        // The two derived content keys MUST be byte-equal, or every catch-up
        // item the daemon pushes fails to decrypt on Android.
        assert_eq!(
            daemon_key.as_bytes(),
            android_key.as_bytes(),
            "daemon (responder) and Android FFI (initiator) must derive the IDENTICAL content sync key"
        );

        // And concretely: a blob the daemon would push must decrypt under the
        // Android key (the live `itemsReceived=N, items=[]` symptom).
        let item_id = uuid::Uuid::new_v4().to_string();
        let plaintext = b"catch-up item from the macOS daemon".to_vec();
        let blob = encrypt_for_cloud(&daemon_key, &item_id, &plaintext)
            .expect("daemon wraps catch-up item under its content key");
        let recovered = decrypt_from_cloud(&android_key, &item_id, &blob)
            .expect("Android must decrypt the daemon's catch-up blob");
        assert_eq!(recovered, plaintext);
    }

    /// A malformed `addr_hint` must surface as `P2pError`, not a panic.
    #[test]
    fn bootstrap_pair_initiator_rejects_bad_addr() {
        let cert = generate_device_cert().expect("cert");
        let err = bootstrap_pair_initiator(
            "not-an-addr".into(),
            &cert.cert_der,
            &cert.key_der,
            "pw".into(),
            "127.0.0.1:7000".into(),
            None,
            // ABI 14 device-meta params — irrelevant to the addr-parse error path.
            None,
            None,
            None,
            None,
            None,
        )
        .expect_err("malformed addr_hint must error");
        assert!(
            matches!(err, CopypasteError::P2pError { .. }),
            "expected P2pError, got {err:?}"
        );
    }

    /// `sync_with_peer` rejects a non-32-byte session key before any network I/O.
    #[test]
    fn sync_with_peer_rejects_bad_session_key() {
        let cert = generate_device_cert().expect("cert");
        let err = sync_with_peer(
            "127.0.0.1:1".into(),
            "deadbeef".into(),
            vec![0u8; 16], // wrong length
            cert.cert_der.clone(),
            cert.key_der.clone(),
            Vec::new(),
            Vec::new(),
            "test-device".into(),
        )
        .expect_err("16-byte session key must error");
        assert!(
            matches!(err, CopypasteError::InvalidKeyLength),
            "expected InvalidKeyLength, got {err:?}"
        );
    }

    /// End-to-end loopback sync against a peer that speaks the DAEMON's framed
    /// wire protocol (NOT `run_session`) — i.e. the real protocol live macOS
    /// daemons use. The fake peer accepts the mTLS connection, then PUSHES one
    /// framed JSON `WireItem` (re-keyed under the shared key, `content_nonce =
    /// None`) exactly like the daemon's sync-on-connect catch-up push. It also
    /// reads any inbound frame the FFI sends, so this test exercises BOTH
    /// directions of the framed exchange.
    ///
    /// Proves the full FFI path: derive shared key → mTLS connect (fingerprint
    /// pinned) → keep the `LengthDelimitedCodec` framing → read the peer's
    /// framed `WireItem` → unwrap the cloud blob back to the ORIGINAL plaintext.
    /// Asserts the FFI returns that item as correct plaintext and the peer
    /// received the FFI's one offered item (the Android→macOS send path).
    #[test]
    fn sync_with_peer_receives_item_from_loopback_peer() {
        loopback_sync_roundtrip("text");
    }

    /// Regression for the Android→peer "ZERO items sent" bug: the Kotlin layer
    /// stores `content_type = "text/plain"` and historically passed that raw
    /// into `LocalItem`, but the send path only re-keyed items whose content
    /// type was exactly "text" — so every Android item was silently dropped
    /// (items_sent = 0). This drives the same loopback exchange with a
    /// `"text/plain"` offered item and asserts it IS sent and received by the
    /// peer. The earlier loopback test used "text", masking the real value.
    #[test]
    fn sync_with_peer_sends_text_plain_item_to_loopback_peer() {
        loopback_sync_roundtrip("text/plain");
    }

    /// Shared body for the loopback send/receive tests, parameterized by the
    /// content type the FFI offers, so we can prove both the canonical "text"
    /// token and the MIME-style "text/plain" value are accepted by the send
    /// path. `offered_content_type` is the value placed on the outbound
    /// `LocalItem.content_type`.
    fn loopback_sync_roundtrip(offered_content_type: &str) {
        use bytes::Bytes;
        use copypaste_p2p::pake::SessionKey;
        use copypaste_p2p::transport::{PairedPeers, PeerTransport};
        use copypaste_sync::protocol::WireItem;
        use futures_util::{SinkExt, StreamExt};
        use std::sync::mpsc;
        use tokio::net::TcpListener;

        // Both ends agree on a 32-byte PAKE session key (the bootstrap output).
        let session_key = [0x5Au8; 32];
        // The peer derives the SAME shared content key the FFI will derive.
        let shared = {
            let sk = SessionKey(session_key);
            copypaste_core::SyncKey::from_bytes(*sk.derive_xchacha_key(P2P_SYNC_KEY_SALT))
        };

        // Identities. The FFI (initiator/client) pins the peer's fingerprint;
        // the peer (server) pins the initiator's fingerprint.
        let peer_cert = generate_device_cert().expect("peer cert");
        let init_cert = generate_device_cert().expect("initiator cert");
        let peer_fp = peer_cert.fingerprint.clone();
        let init_fp = init_cert.fingerprint.clone();

        // The one known item the peer pushes, wrapped under the shared key
        // exactly as the daemon's `rekey_outbound` does (self-framed cloud blob
        // in `content`, `content_nonce = None`).
        let known_item_id = uuid::Uuid::new_v4().to_string();
        let known_plaintext = b"hello from the loopback peer".to_vec();
        let known_blob = encrypt_for_cloud(&shared, &known_item_id, &known_plaintext)
            .expect("peer wraps its item under the shared key");
        let peer_wire = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: known_item_id.clone(),
            item_id: known_item_id.clone(),
            content_type: "text".to_string(),
            content: Some(known_blob),
            content_nonce: None,
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 5,
            wall_time: 5,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "loopback-peer".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        };

        // Peer runs on its OWN runtime in a background OS thread so the main test
        // thread is free of an ambient runtime for the synchronous FFI call.
        // Returns the count of frames it received from the FFI initiator.
        let (port_tx, port_rx) = mpsc::channel::<u16>();
        let peer_cert_der = peer_cert.cert_der.clone();
        let peer_key_der = peer_cert.key_der.clone();
        let init_fp_for_peer = init_fp.clone();
        let peer_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("peer runtime");
            rt.block_on(async move {
                let peers = PairedPeers::new();
                peers.add(init_fp_for_peer, "android-initiator");
                let transport = PeerTransport::from_cert(peer_cert_der, peer_key_der, peers);

                let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
                port_tx
                    .send(listener.local_addr().expect("addr").port())
                    .expect("send port");

                // Accept and KEEP the length-delimited framing — this is the
                // daemon's `run_peer_connection` shape, not `run_session`.
                let (_addr, _fp, mut framed) = transport.accept(&listener).await.expect("accept");

                // PUSH the catch-up item as one framed JSON `WireItem`, exactly
                // like the daemon does right after a connection is established.
                let payload = serde_json::to_vec(&peer_wire).expect("serialise peer WireItem");
                framed
                    .send(Bytes::from(payload))
                    .await
                    .expect("peer push frame");

                // Read whatever the FFI sends back (its offered local items),
                // bounded by a short idle timeout so the peer task terminates.
                let mut received_from_ffi = 0u64;
                while let Ok(Some(Ok(frame))) =
                    tokio::time::timeout(std::time::Duration::from_secs(2), framed.next()).await
                {
                    if serde_json::from_slice::<WireItem>(&frame).is_ok() {
                        received_from_ffi += 1;
                    }
                }
                received_from_ffi
            })
        });

        let port = port_rx.recv().expect("peer port");
        let addr = format!("127.0.0.1:{port}");

        // The FFI under test offers ONE local item (exercising the send path)
        // and must receive the peer's pushed item decrypted to plaintext.
        let offered_plaintext = b"hello from android initiator".to_vec();
        let offered_item_id = uuid::Uuid::new_v4().to_string();
        let local_items = vec![LocalItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: String::new(),
            item_id: offered_item_id.clone(),
            wall_time_ms: 7,
            content_type: offered_content_type.to_string(),
            plaintext: offered_plaintext.clone(),
            file_name: None,
            mime: None,
        }];

        let result = sync_with_peer(
            addr,
            peer_fp,
            session_key.to_vec(),
            init_cert.cert_der.clone(),
            init_cert.key_der.clone(),
            local_items,
            Vec::new(),
            "test-device".into(),
        )
        .expect("FFI sync_with_peer must succeed over loopback");

        assert!(
            result.items_received >= 1,
            "must receive at least the peer's one item, got {}",
            result.items_received
        );
        assert_eq!(
            result.items_sent, 1,
            "FFI must report its one offered item as sent"
        );
        let got = result
            .items
            .iter()
            .find(|i| i.plaintext == known_plaintext)
            .expect("the peer's item must come back decrypted to its plaintext");
        assert_eq!(got.content_type, "text");
        assert_eq!(got.plaintext, known_plaintext);
        // The peer's STABLE item_id must be carried through to the SyncedItem so
        // Kotlin can persist it and avoid re-minting on a later re-sync.
        assert_eq!(
            got.item_id, known_item_id,
            "received SyncedItem must carry the peer's stable item_id"
        );

        // The peer must have received the FFI's one offered item (send path).
        let frames_peer_got = peer_thread.join().expect("peer thread join");
        assert_eq!(
            frames_peer_got, 1,
            "peer must have received the FFI initiator's one offered framed WireItem"
        );
    }

    /// v0.6 image/file sync (RECEIVE + outbound symmetry): an image frame
    /// arrives on the wire under the SAME sync-key-wrapped shape as text
    /// (`content` = `encrypt_for_cloud(shared, item_id, plaintext)`,
    /// `content_nonce = None`, `content_type = "image"`). The FFI must NOT drop
    /// it (the old `content_type != "text"` guard did), must decrypt it back to
    /// the raw image bytes, and must surface it as a `SyncedItem` whose
    /// `content_type` is preserved as "image". Symmetrically, an image
    /// `LocalItem` offered by Android must be re-keyed and sent to the peer.
    #[test]
    fn sync_with_peer_receives_image_frame_from_loopback_peer() {
        use bytes::Bytes;
        use copypaste_p2p::pake::SessionKey;
        use copypaste_p2p::transport::{PairedPeers, PeerTransport};
        use copypaste_sync::protocol::WireItem;
        use futures_util::{SinkExt, StreamExt};
        use std::sync::mpsc;
        use tokio::net::TcpListener;

        let session_key = [0x5Au8; 32];
        let shared = {
            let sk = SessionKey(session_key);
            copypaste_core::SyncKey::from_bytes(*sk.derive_xchacha_key(P2P_SYNC_KEY_SALT))
        };

        let peer_cert = generate_device_cert().expect("peer cert");
        let init_cert = generate_device_cert().expect("initiator cert");
        let peer_fp = peer_cert.fingerprint.clone();
        let init_fp = init_cert.fingerprint.clone();

        // The peer pushes ONE image item, wrapped under the shared key exactly
        // as the daemon's `rekey_outbound` does for images: the raw PNG bytes
        // are the plaintext, the self-framed cloud blob goes in `content`, and
        // `content_nonce` is `None`.
        let known_item_id = uuid::Uuid::new_v4().to_string();
        // A minimal "PNG-ish" byte payload (content is opaque to sync).
        let known_plaintext: Vec<u8> =
            vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
        let known_blob = encrypt_for_cloud(&shared, &known_item_id, &known_plaintext)
            .expect("peer wraps its image under the shared key");
        let peer_wire = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: known_item_id.clone(),
            item_id: known_item_id.clone(),
            content_type: "image".to_string(),
            content: Some(known_blob),
            content_nonce: None,
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 9,
            wall_time: 9,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "loopback-peer".to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        };

        let (port_tx, port_rx) = mpsc::channel::<u16>();
        let peer_cert_der = peer_cert.cert_der.clone();
        let peer_key_der = peer_cert.key_der.clone();
        let init_fp_for_peer = init_fp.clone();
        let peer_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("peer runtime");
            rt.block_on(async move {
                let peers = PairedPeers::new();
                peers.add(init_fp_for_peer, "android-initiator");
                let transport = PeerTransport::from_cert(peer_cert_der, peer_key_der, peers);

                let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
                port_tx
                    .send(listener.local_addr().expect("addr").port())
                    .expect("send port");

                let (_addr, _fp, mut framed) = transport.accept(&listener).await.expect("accept");

                let payload = serde_json::to_vec(&peer_wire).expect("serialise peer WireItem");
                framed
                    .send(Bytes::from(payload))
                    .await
                    .expect("peer push frame");

                // Capture the content_type of the frame the FFI offers back, so
                // we can prove the outbound image symmetry (Android → macOS).
                let mut received_content_types: Vec<String> = Vec::new();
                while let Ok(Some(Ok(frame))) =
                    tokio::time::timeout(std::time::Duration::from_secs(2), framed.next()).await
                {
                    if let Ok(w) = serde_json::from_slice::<WireItem>(&frame) {
                        received_content_types.push(w.content_type);
                    }
                }
                received_content_types
            })
        });

        let port = port_rx.recv().expect("peer port");
        let addr = format!("127.0.0.1:{port}");

        // The FFI offers ONE local IMAGE item (exercising the outbound path).
        let offered_plaintext: Vec<u8> = vec![0x89, b'P', b'N', b'G', 9, 8, 7];
        let offered_item_id = uuid::Uuid::new_v4().to_string();
        let local_items = vec![LocalItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: String::new(),
            item_id: offered_item_id.clone(),
            wall_time_ms: 11,
            content_type: "image".to_string(),
            plaintext: offered_plaintext.clone(),
            file_name: None,
            mime: None,
        }];

        let result = sync_with_peer(
            addr,
            peer_fp,
            session_key.to_vec(),
            init_cert.cert_der.clone(),
            init_cert.key_der.clone(),
            local_items,
            Vec::new(),
            "test-device".into(),
        )
        .expect("FFI sync_with_peer must succeed over loopback");

        assert_eq!(
            result.items_sent, 1,
            "FFI must offer its one local image item (outbound symmetry)"
        );
        let got = result
            .items
            .iter()
            .find(|i| i.plaintext == known_plaintext)
            .expect("the peer's image must come back decrypted to its plaintext");
        assert_eq!(
            got.content_type, "image",
            "received SyncedItem must preserve the image content type"
        );
        assert_eq!(got.item_id, known_item_id);

        let peer_content_types = peer_thread.join().expect("peer thread join");
        assert!(
            peer_content_types.iter().any(|ct| ct == "image"),
            "peer must have received the FFI initiator's offered image frame, got {peer_content_types:?}"
        );
    }

    /// STABLE-IDENTITY regression: `sync_with_peer` must put the caller's
    /// `LocalItem.item_id` onto the outbound `WireItem.item_id` verbatim (no
    /// fresh `Uuid::new_v4()` per send) — that re-minting was the desktop
    /// "every clip is a new item → duplicates / broken LWW" bug. The fake peer
    /// captures the `item_id` of the frame it receives and we assert it equals
    /// the stable id we offered. Also covers the empty-`item_id` transitional
    /// fallback to `id`.
    #[test]
    fn sync_with_peer_sends_stable_item_id() {
        use bytes::Bytes;
        use copypaste_p2p::pake::SessionKey;
        use copypaste_p2p::transport::{PairedPeers, PeerTransport};
        use copypaste_sync::protocol::WireItem;
        use futures_util::StreamExt;
        use std::sync::mpsc;
        use tokio::net::TcpListener;

        let session_key = [0x5Au8; 32];
        let _shared = {
            let sk = SessionKey(session_key);
            copypaste_core::SyncKey::from_bytes(*sk.derive_xchacha_key(P2P_SYNC_KEY_SALT))
        };

        let peer_cert = generate_device_cert().expect("peer cert");
        let init_cert = generate_device_cert().expect("initiator cert");
        let peer_fp = peer_cert.fingerprint.clone();
        let init_fp = init_cert.fingerprint.clone();

        // Channel carries the item_id of the FIRST frame the peer receives.
        let (id_tx, id_rx) = mpsc::channel::<String>();
        let (port_tx, port_rx) = mpsc::channel::<u16>();
        let peer_cert_der = peer_cert.cert_der.clone();
        let peer_key_der = peer_cert.key_der.clone();
        let init_fp_for_peer = init_fp.clone();
        let peer_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("peer runtime");
            rt.block_on(async move {
                let peers = PairedPeers::new();
                peers.add(init_fp_for_peer, "android-initiator");
                let transport = PeerTransport::from_cert(peer_cert_der, peer_key_der, peers);
                let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
                port_tx
                    .send(listener.local_addr().expect("addr").port())
                    .expect("send port");
                let (_addr, _fp, mut framed) = transport.accept(&listener).await.expect("accept");
                // Read the FFI's offered frame and report its item_id.
                if let Ok(Some(Ok(frame))) =
                    tokio::time::timeout(std::time::Duration::from_secs(3), framed.next()).await
                {
                    if let Ok(w) = serde_json::from_slice::<WireItem>(&frame) {
                        let _ = id_tx.send(w.item_id);
                    }
                }
                // Keep the buffer typed for clarity; nothing else to send.
                let _ = Bytes::new();
            })
        });

        let port = port_rx.recv().expect("peer port");
        let addr = format!("127.0.0.1:{port}");

        let stable_id = uuid::Uuid::new_v4().to_string();
        let local_items = vec![LocalItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: "local-row-1".to_string(),
            item_id: stable_id.clone(),
            wall_time_ms: 11,
            content_type: "text".to_string(),
            plaintext: b"stable-id body".to_vec(),
            file_name: None,
            mime: None,
        }];

        let result = sync_with_peer(
            addr,
            peer_fp,
            session_key.to_vec(),
            init_cert.cert_der.clone(),
            init_cert.key_der.clone(),
            local_items,
            Vec::new(),
            "test-device".into(),
        )
        .expect("FFI sync_with_peer must succeed over loopback");
        assert_eq!(result.items_sent, 1);

        let sent_item_id = id_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("peer must report the received frame's item_id");
        assert_eq!(
            sent_item_id, stable_id,
            "outbound WireItem.item_id must be the caller's stable item_id, not a fresh Uuid"
        );

        peer_thread.join().expect("peer thread join");
    }

    /// Defense-in-depth observability: a peer that pushes a text `WireItem`
    /// still carrying a `content_nonce` (a legacy / non-rekeyed frame, the exact
    /// build-skew shape that hid the "decrypt 7/7" failure) must NOT vanish
    /// silently. The FFI must skip it (it's undecryptable with the shared sync
    /// key) but COUNT it in `items_skipped_legacy` and exercise the warn path.
    /// Mirrors `sync_with_peer_receives_item_from_loopback_peer`.
    #[test]
    fn sync_with_peer_counts_skipped_legacy_frame() {
        use bytes::Bytes;
        use copypaste_p2p::transport::{PairedPeers, PeerTransport};
        use copypaste_sync::protocol::WireItem;
        use futures_util::{SinkExt, StreamExt};
        use std::sync::mpsc;
        use tokio::net::TcpListener;

        let session_key = [0x5Au8; 32];

        let peer_cert = generate_device_cert().expect("peer cert");
        let init_cert = generate_device_cert().expect("initiator cert");
        let peer_fp = peer_cert.fingerprint.clone();
        let init_fp = init_cert.fingerprint.clone();

        // A LEGACY text frame: `content_nonce = Some(...)`. This is the
        // non-rekeyed shape the FFI cannot decrypt and previously dropped with a
        // silent `continue`.
        let legacy_item_id = uuid::Uuid::new_v4().to_string();
        let legacy_wire = WireItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: legacy_item_id.clone(),
            item_id: legacy_item_id.clone(),
            content_type: "text".to_string(),
            content: Some(vec![1, 2, 3, 4]),
            content_nonce: Some(vec![9u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: 3,
            wall_time: 3,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: "legacy-peer".to_string(),
            key_version: 1,
            file_name: None,
            mime: None,
        };

        let (port_tx, port_rx) = mpsc::channel::<u16>();
        let peer_cert_der = peer_cert.cert_der.clone();
        let peer_key_der = peer_cert.key_der.clone();
        let init_fp_for_peer = init_fp.clone();
        let peer_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("peer runtime");
            rt.block_on(async move {
                let peers = PairedPeers::new();
                peers.add(init_fp_for_peer, "android-initiator");
                let transport = PeerTransport::from_cert(peer_cert_der, peer_key_der, peers);

                let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
                port_tx
                    .send(listener.local_addr().expect("addr").port())
                    .expect("send port");

                let (_addr, _fp, mut framed) = transport.accept(&listener).await.expect("accept");

                // PUSH the legacy frame, exactly as a stale daemon's catch-up
                // push would.
                let payload = serde_json::to_vec(&legacy_wire).expect("serialise legacy WireItem");
                framed
                    .send(Bytes::from(payload))
                    .await
                    .expect("peer push legacy frame");

                // Drain anything the FFI offers so the peer task terminates.
                while let Ok(Some(Ok(_frame))) =
                    tokio::time::timeout(std::time::Duration::from_secs(2), framed.next()).await
                {
                }
            })
        });

        let port = port_rx.recv().expect("peer port");
        let addr = format!("127.0.0.1:{port}");

        let result = sync_with_peer(
            addr,
            peer_fp,
            session_key.to_vec(),
            init_cert.cert_der.clone(),
            init_cert.key_der.clone(),
            Vec::new(),
            Vec::new(),
            "test-device".into(),
        )
        .expect("FFI sync_with_peer must succeed over loopback");

        // The legacy frame was received on the wire ...
        assert!(
            result.items_received >= 1,
            "must have received the legacy frame, got {}",
            result.items_received
        );
        // ... skipped (undecryptable) so it yields NO plaintext item ...
        assert!(
            result.items.is_empty(),
            "legacy non-rekeyed frame must not surface as a decrypted item"
        );
        // ... but is now COUNTED instead of vanishing silently.
        assert_eq!(
            result.items_skipped_legacy, 1,
            "the skipped legacy/non-rekeyed frame must be counted, got {}",
            result.items_skipped_legacy
        );

        peer_thread.join().expect("peer thread join");
    }

    /// Blob format: nonce[24] prepended, total length = 24 + plaintext + 16 (AEAD tag).
    #[test]
    fn cloud_encrypt_blob_format() {
        let key = derive_cloud_sync_key("format-test".into()).expect("derive");
        let plaintext = b"test blob format";
        let blob = cloud_encrypt("item-fmt".into(), plaintext, &key).expect("encrypt");
        assert_eq!(
            blob.len(),
            24 + plaintext.len() + 16,
            "blob must be nonce(24) + plaintext + tag(16)"
        );
    }

    // ── R3b: shared-account relay inbox derivation (ABI 13) ─────────────────

    /// The FFI `relay_inbox_id` MUST be byte-identical to the core function it
    /// wraps — that equality is the cross-device agreement property that lets
    /// Android share the macOS daemon's relay inbox.
    #[test]
    fn relay_inbox_id_matches_core() {
        let key = derive_cloud_sync_key("relay-inbox-match".into()).expect("derive");
        let key_arr: [u8; 32] = key.as_slice().try_into().expect("32-byte key");
        let ffi = relay_inbox_id(&key).expect("inbox id");
        assert_eq!(ffi, copypaste_core::derive_relay_inbox_id(&key_arr));
        // Deterministic across calls.
        assert_eq!(ffi, relay_inbox_id(&key).expect("inbox id again"));
    }

    /// The FFI `relay_public_key_b64` MUST equal STANDARD-base64 of the core
    /// `derive_relay_public_key`, matching the daemon's registration value.
    #[test]
    fn relay_public_key_b64_matches_core() {
        let key = derive_cloud_sync_key("relay-pubkey-match".into()).expect("derive");
        let key_arr: [u8; 32] = key.as_slice().try_into().expect("32-byte key");
        let ffi = relay_public_key_b64(&key).expect("pubkey b64");
        let expected = base64::engine::general_purpose::STANDARD
            .encode(copypaste_core::derive_relay_public_key(&key_arr));
        assert_eq!(ffi, expected);
    }

    /// Both relay derivations reject a non-32-byte key with `InvalidKeyLength`
    /// rather than panicking across the FFI boundary.
    #[test]
    fn relay_derivations_reject_wrong_key_length() {
        let short = vec![0u8; 16];
        assert!(matches!(
            relay_inbox_id(&short),
            Err(CopypasteError::InvalidKeyLength)
        ));
        assert!(matches!(
            relay_public_key_b64(&short),
            Err(CopypasteError::InvalidKeyLength)
        ));
    }

    // ── Part A: LocalItem file_name + mime outbound wiring (ABI 8) ──────────

    /// A `LocalItem` with `content_type == "file"` and populated `file_name` /
    /// `mime` fields MUST have those fields forwarded onto the outbound
    /// `WireItem` (Android→macOS file-send, ABI 8).
    ///
    /// The peer is a loopback listener that captures the first inbound frame and
    /// reports its `file_name` / `mime` back via a channel.
    #[test]
    fn sync_with_peer_sends_file_item_with_file_name_and_mime() {
        use copypaste_p2p::transport::{PairedPeers, PeerTransport};
        use copypaste_sync::protocol::WireItem;
        use futures_util::StreamExt;
        use std::sync::mpsc;
        use tokio::net::TcpListener;

        let session_key = [0x7Bu8; 32];

        let peer_cert = generate_device_cert().expect("peer cert");
        let init_cert = generate_device_cert().expect("initiator cert");
        let peer_fp = peer_cert.fingerprint.clone();
        let init_fp = init_cert.fingerprint.clone();

        // Side-channel: first recv = port, second recv = captured (file_name, mime).
        let (name_tx, name_rx) = mpsc::channel::<(Option<String>, Option<String>)>();

        let peer_cert_der = peer_cert.cert_der.clone();
        let peer_key_der = peer_cert.key_der.clone();
        let init_fp_for_peer = init_fp.clone();
        let name_tx_peer = name_tx.clone();
        let peer_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("peer runtime");
            rt.block_on(async move {
                let peers = PairedPeers::new();
                peers.add(init_fp_for_peer, "android-initiator");
                let transport = PeerTransport::from_cert(peer_cert_der, peer_key_der, peers);

                let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
                let port = listener.local_addr().expect("addr").port();
                // Port is signalled as (None, Some("<port>")).
                name_tx_peer
                    .send((None, Some(port.to_string())))
                    .expect("send port");

                let (_addr, _fp, mut framed) = transport.accept(&listener).await.expect("accept");

                // Drain inbound frames; capture the first "file" frame's metadata.
                let mut captured: Option<(Option<String>, Option<String>)> = None;
                while let Ok(Some(Ok(frame))) =
                    tokio::time::timeout(std::time::Duration::from_secs(5), framed.next()).await
                {
                    if let Ok(w) = serde_json::from_slice::<WireItem>(&frame) {
                        if w.content_type == "file" && captured.is_none() {
                            captured = Some((w.file_name, w.mime));
                        }
                    }
                }
                if let Some(pair) = captured {
                    name_tx_peer.send(pair).expect("send captured fields");
                }
            })
        });

        // First receive is the port signal.
        let (_, port_opt) = name_rx.recv().expect("recv port signal");
        let addr = format!("127.0.0.1:{}", port_opt.expect("port string"));

        let file_bytes: Vec<u8> = b"fake pdf content".to_vec();
        let file_item_id = uuid::Uuid::new_v4().to_string();
        let local_items = vec![LocalItem {
            deleted: false,
            pinned: false,
            pin_order: None,
            id: String::new(),
            item_id: file_item_id.clone(),
            wall_time_ms: 42,
            content_type: "file".to_string(),
            plaintext: file_bytes.clone(),
            file_name: Some("report.pdf".to_string()),
            mime: Some("application/pdf".to_string()),
        }];

        let result = sync_with_peer(
            addr,
            peer_fp,
            session_key.to_vec(),
            init_cert.cert_der.clone(),
            init_cert.key_der.clone(),
            local_items,
            Vec::new(),
            "test-device".into(),
        )
        .expect("FFI sync_with_peer must succeed over loopback");

        assert_eq!(
            result.items_sent, 1,
            "FFI must report its one offered file item as sent"
        );

        // Second receive is the captured (file_name, mime) from the peer.
        let (got_name, got_mime) = name_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("peer must report the file_name/mime it observed");

        assert_eq!(
            got_name,
            Some("report.pdf".to_string()),
            "outbound WireItem.file_name must carry the LocalItem.file_name"
        );
        assert_eq!(
            got_mime,
            Some("application/pdf".to_string()),
            "outbound WireItem.mime must carry the LocalItem.mime"
        );

        peer_thread.join().expect("peer thread join");
    }

    // ── Fix #3: derive_cloud_sync_key PassphraseTooShort surface ────────────

    /// An empty passphrase must surface `DecryptionFailed { reason }` that
    /// mentions the cause — not `EncryptionFailed` which discards all info.
    #[test]
    fn derive_cloud_sync_key_empty_passphrase_surfaces_reason() {
        let err = derive_cloud_sync_key(String::new())
            .expect_err("empty passphrase must return an error");
        match err {
            CopypasteError::DecryptionFailed { reason } => {
                assert!(
                    !reason.is_empty(),
                    "reason must carry a non-empty message about the cause"
                );
            }
            other => panic!("expected DecryptionFailed {{reason}}, got {other:?}"),
        }
    }

    // ── Fix #2: key-aware DB cache ───────────────────────────────────────────

    /// Opening the same db_path with TWO different keys must NOT silently reuse
    /// the connection keyed under the first key. The second call must either
    /// succeed with its own connection OR return an appropriate error — but it
    /// must never silently return the first key's connection.
    ///
    /// This test verifies the path-only cache bug is fixed by confirming that
    /// two distinct keys produce independent operations (here: we just check the
    /// stub path returns 0 items regardless, and the live path would open two
    /// separate connections).
    #[cfg(not(feature = "android-uniffi-live"))]
    #[test]
    fn different_keys_same_path_stub_returns_zero() {
        let key_a = vec![1u8; 32];
        let key_b = vec![2u8; 32];
        // Both calls on the same path but different keys must each succeed
        // independently on the stub path.
        let n_a = get_history_count("/dev/null".into(), &key_a).expect("count key_a");
        let n_b = get_history_count("/dev/null".into(), &key_b).expect("count key_b");
        assert_eq!(n_a, 0, "stub key_a must return 0");
        assert_eq!(n_b, 0, "stub key_b must return 0");
    }

    // ── Fix #1: stack key copies are zeroized (Zeroizing<[u8;32]>) ──────────

    /// The key material path through encrypt_text / decrypt_text uses a
    /// Zeroizing<[u8;32]> wrapper — verify the functions still work correctly
    /// end-to-end (Zeroizing is transparent to callers; this confirms no
    /// accidental deref breakage was introduced).
    #[test]
    fn zeroizing_key_does_not_break_encrypt_decrypt() {
        let key = test_key();
        let item_id = "zeroize-test".to_string();
        // Use key_version=2 (current daemon default).
        let blob = encrypt_text(item_id.clone(), b"zeroize path check", &key, 2).expect("encrypt");
        let pt = decrypt_text(item_id, &blob.ciphertext, &blob.nonce, &key, 2).expect("decrypt");
        assert_eq!(pt, b"zeroize path check");
    }

    /// cloud_encrypt / cloud_decrypt paths use Zeroizing<[u8;32]> — verify
    /// that end-to-end round-trip is still correct.
    #[test]
    fn zeroizing_key_does_not_break_cloud_encrypt_decrypt() {
        let key = derive_cloud_sync_key("zeroize-cloud-check".into()).expect("derive");
        let item_id = "zeroize-cloud-item".to_string();
        let plaintext = b"cloud zeroize path";
        let blob = cloud_encrypt(item_id.clone(), plaintext, &key).expect("encrypt");
        let recovered = cloud_decrypt(item_id, &blob, &key).expect("decrypt");
        assert_eq!(recovered, plaintext);
    }

    /// #40b DB_BY_PATH cache eviction: when `with_cached_db` is called with a
    /// key that differs from a previously-cached entry for the SAME path, the
    /// stale (path, old_key) entry must be evicted before the new one is
    /// inserted. Without the `retain` call the map would accumulate one entry
    /// per key rotation — a connection leak.
    ///
    /// The test uses a unique path prefix so it does not collide with the
    /// global `DB_BY_PATH` state set by sibling tests.
    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn db_by_path_evicts_stale_entries_on_key_rotation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("evict_test.db")
            .to_string_lossy()
            .into_owned();

        let key_a: [u8; 32] = [1u8; 32];
        let key_b: [u8; 32] = [2u8; 32];

        // Prime the cache with (path, key_a).
        {
            let mut map = db_by_path().lock().unwrap_or_else(|e| e.into_inner());
            let cache_key_a = (path.clone(), key_a);
            map.entry(cache_key_a).or_insert_with(|| {
                copypaste_core::Database::open(std::path::Path::new(&path), &Zeroizing::new(key_a))
                    .expect("open with key_a")
            });
        }

        // Verify key_a is in the cache.
        {
            let map = db_by_path().lock().unwrap_or_else(|e| e.into_inner());
            assert!(
                map.contains_key(&(path.clone(), key_a)),
                "key_a must be in cache after initial insert"
            );
        }

        // Now call with_cached_db with key_b for the SAME path. The retain
        // must evict the (path, key_a) entry. Without the fix both entries
        // would coexist (connection leak).
        let result = with_cached_db(&path, &key_b, |_db| Ok(()));
        // The open may fail because key_a already encrypted the file, but the
        // eviction test is about what's LEFT in the map — key_a must be gone.
        let _ = result; // tolerate open failure; we only test the cache state

        let map = db_by_path().lock().unwrap_or_else(|e| e.into_inner());
        assert!(
            !map.contains_key(&(path.clone(), key_a)),
            "stale (path, key_a) entry must be evicted after inserting (path, key_b)"
        );
    }

    // ── W6: AppConfig over UniFFI ────────────────────────────────────────────

    #[test]
    fn default_config_matches_appconfig_default_mapping() {
        let ac = copypaste_core::AppConfig::default();
        let cfg = default_config();

        // Every AppConfig-backed field must equal AppConfig::default().
        assert_eq!(cfg.max_text_size_bytes, ac.max_text_size_bytes);
        assert_eq!(cfg.max_image_size_bytes, ac.max_image_size_bytes);
        assert_eq!(cfg.max_file_size_bytes, ac.max_file_size_bytes);
        assert_eq!(cfg.storage_quota_bytes, ac.storage_quota_bytes);
        assert_eq!(cfg.sensitive_ttl_secs, ac.sensitive_ttl_secs);
        assert_eq!(cfg.poll_interval_ms, ac.poll_interval_ms);
        assert_eq!(cfg.sound_on_copy, ac.sound_on_copy);
        assert_eq!(cfg.notify_on_copy, ac.notify_on_copy);
        assert_eq!(cfg.sync_on_wifi_only, ac.sync_on_wifi_only);
        assert_eq!(cfg.image_quality, ac.image_quality as u32);
        assert_eq!(cfg.collect_public_ip, ac.collect_public_ip);
        assert_eq!(cfg.paste_as_plain_text, ac.paste_as_plain_text);

        // Android-only knobs take their documented defaults.
        assert_eq!(cfg.mask_sensitive_content, DEFAULT_MASK_SENSITIVE_CONTENT);
        assert_eq!(cfg.p2p_enabled, DEFAULT_P2P_ENABLED);
        assert_eq!(cfg.image_max_height, DEFAULT_IMAGE_MAX_HEIGHT);

        // ABI 12: the excluded-apps list mirrors AppConfig (empty by default).
        assert_eq!(cfg.excluded_app_bundle_ids, ac.excluded_app_bundle_ids);
        assert!(cfg.excluded_app_bundle_ids.is_empty());
    }

    #[test]
    fn clamp_config_round_trips_excluded_app_bundle_ids() {
        // ABI 12: the excluded-apps list maps directly to/from
        // AppConfig::excluded_app_bundle_ids and survives clamp unchanged
        // (clamp_values does not touch it). Order + contents preserved.
        let mut cfg = default_config();
        cfg.excluded_app_bundle_ids = vec![
            "com.apple.keychainaccess".to_string(),
            "org.keepassxc.keepassxc".to_string(),
            "1password.desktop".to_string(),
        ];
        let original = cfg.excluded_app_bundle_ids.clone();

        let clamped = clamp_config(cfg);

        assert_eq!(
            clamped.excluded_app_bundle_ids, original,
            "excluded_app_bundle_ids must round-trip through clamp verbatim"
        );

        // And the mapping onto AppConfig carries the same list.
        let ac = appconfig_from_config(&clamped);
        assert_eq!(ac.excluded_app_bundle_ids, original);
    }

    #[test]
    fn clamp_config_enforces_file_ceiling_and_quota_floor() {
        // A hostile config: file size 8 GiB (above the 100 MiB library hard
        // cap) and storage quota 200 bytes (the historical self-clearing-history
        // bug, below the 50 MiB floor). clamp_config must enforce the SAME
        // bounds as the macOS daemon's AppConfig::clamp_values.
        let mut cfg = default_config();
        cfg.max_file_size_bytes = 8 * 1024 * 1024 * 1024; // 8 GiB
        cfg.storage_quota_bytes = 200;

        let clamped = clamp_config(cfg);

        // File cap clamps DOWN to the 100 MiB library hard cap.
        assert_eq!(
            clamped.max_file_size_bytes,
            100 * 1024 * 1024,
            "max_file_size_bytes must clamp to the 100 MiB ceiling"
        );
        // Storage quota floors UP to MIN_STORAGE_QUOTA_BYTES (50 MiB).
        assert_eq!(
            clamped.storage_quota_bytes,
            50 * 1024 * 1024,
            "storage_quota_bytes must floor to MIN_STORAGE_QUOTA_BYTES"
        );
    }

    #[test]
    fn clamp_config_floors_size_caps_and_poll_interval() {
        let mut cfg = default_config();
        cfg.max_text_size_bytes = 1;
        cfg.max_image_size_bytes = 1;
        cfg.poll_interval_ms = 1; // below the 100 ms floor

        let clamped = clamp_config(cfg);

        assert_eq!(clamped.max_text_size_bytes, 64 * 1024); // MIN_TEXT_SIZE_BYTES
        assert_eq!(clamped.max_image_size_bytes, 1024 * 1024); // MIN_IMAGE_SIZE_BYTES
        assert_eq!(clamped.poll_interval_ms, 100); // POLL_INTERVAL_MIN_MS
    }

    #[test]
    fn clamp_config_preserves_android_only_knobs_verbatim() {
        // The Android-only knobs have no AppConfig counterpart and must survive
        // the round-trip unchanged (not reset to a default).
        let mut cfg = default_config();
        cfg.mask_sensitive_content = false;
        cfg.p2p_enabled = true;
        cfg.image_max_height = 1234;

        let clamped = clamp_config(cfg);

        assert!(!clamped.mask_sensitive_content);
        assert!(clamped.p2p_enabled);
        assert_eq!(clamped.image_max_height, 1234);
    }

    #[test]
    fn clamp_config_does_not_floor_sensitive_ttl_zero_sentinel() {
        // 0 = "auto-wipe disabled" sentinel; clamp must NOT lift it to 1.
        let mut cfg = default_config();
        cfg.sensitive_ttl_secs = 0;
        let clamped = clamp_config(cfg);
        assert_eq!(clamped.sensitive_ttl_secs, 0);
    }

    // ── W7: revoked-fingerprint denylist predicate ───────────────────────────

    #[test]
    fn is_fingerprint_revoked_matches_canonicalized_forms() {
        // Colon-grouped, mixed-case fingerprint vs a bare-hex lowercase denylist
        // entry (and vice versa) must match after canonicalization.
        let revoked = vec!["abcd1234ef".to_string()];
        assert!(is_fingerprint_revoked("AB:CD:12:34:EF", &revoked));
        assert!(is_fingerprint_revoked("abcd1234ef", &revoked));

        let revoked_colon = vec!["AB:CD:12:34:EF".to_string()];
        assert!(is_fingerprint_revoked("abcd1234ef", &revoked_colon));
    }

    #[test]
    fn is_fingerprint_revoked_rejects_non_member_and_empty() {
        let revoked = vec!["abcd1234ef".to_string()];
        assert!(!is_fingerprint_revoked("deadbeef", &revoked));
        assert!(!is_fingerprint_revoked("abcd1234ef", &[]));
    }

    #[test]
    fn sync_with_peer_refuses_revoked_peer_without_network() {
        // The revoke check runs at the TOP of sync_with_peer, before the addr
        // is parsed or any socket opens. A revoked fingerprint must therefore
        // return P2pError("… is revoked") even with a bogus address and empty
        // identity material — proving the refusal is enforced at the trust
        // layer, not merely in the UI.
        let err = sync_with_peer(
            "not-a-valid-addr".to_string(),
            "AB:CD:EF".to_string(),
            vec![0u8; 32],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec!["abcdef".to_string()], // denylist (canonicalizes to match)
            "device-1".to_string(),
        )
        .expect_err("revoked peer must be refused");
        match err {
            CopypasteError::P2pError { reason } => {
                assert!(
                    reason.contains("revoked"),
                    "expected a 'revoked' refusal, got: {reason}"
                );
            }
            other => panic!("expected P2pError, got {other:?}"),
        }
    }

    // ── P1-8: DB_BY_PATH cache eviction on close_database ────────────────────
    //
    // Verifies that:
    //   1. `key_cache_hash` produces a deterministic 32-byte value.
    //   2. After `close_database`, the `DB_HANDLE_TO_CACHE_KEY` side-map no
    //      longer holds the handle, confirming the eviction path ran.
    //
    // Note: DB_BY_PATH itself is only populated by `with_cached_db` (the
    // `android-uniffi-live` feature path, not exercised in host unit tests
    // because it requires a real SQLCipher file). This test verifies the
    // side-map eviction logic which is always compiled in.

    #[test]
    fn key_cache_hash_is_deterministic_and_not_identity() {
        let key: [u8; 32] = [0x42u8; 32];
        let h1 = key_cache_hash(&key);
        let h2 = key_cache_hash(&key);
        assert_eq!(h1, h2, "key_cache_hash must be deterministic");
        // Hash must differ from the raw key (i.e. not a no-op passthrough).
        assert_ne!(h1, key, "key_cache_hash must not be the identity function");
    }

    #[test]
    fn key_cache_hash_different_keys_produce_different_hashes() {
        let key_a: [u8; 32] = [0x01u8; 32];
        let key_b: [u8; 32] = [0x02u8; 32];
        assert_ne!(
            key_cache_hash(&key_a),
            key_cache_hash(&key_b),
            "distinct keys must produce distinct hashes"
        );
    }

    #[test]
    fn close_database_evicts_handle_to_cache_key_side_map() {
        // Simulate the side-map lifecycle without opening a real database:
        // manually insert a (handle → cache_key) entry (as open_database does)
        // then call close_database and assert the entry is gone.
        let fake_handle: u64 = 0xDEAD_BEEF_1234_5678;
        let fake_key: [u8; 32] = [0x99u8; 32];
        let fake_hash = key_cache_hash(&fake_key);
        let fake_path = "/tmp/test-eviction-db".to_string();

        // Directly insert into the side-map, mimicking open_database.
        db_handle_to_cache_key()
            .lock()
            .unwrap()
            .insert(fake_handle, (fake_path.clone(), fake_hash));

        assert!(
            db_handle_to_cache_key()
                .lock()
                .unwrap()
                .contains_key(&fake_handle),
            "side-map must contain the handle before close"
        );

        // close_database must evict it.
        close_database(fake_handle);

        assert!(
            !db_handle_to_cache_key()
                .lock()
                .unwrap()
                .contains_key(&fake_handle),
            "P1-8: close_database must evict the handle from DB_HANDLE_TO_CACHE_KEY \
             — raw key material (now a hash) must not survive after close"
        );
    }

    // ── PG-17 (mxoq): fts_search stub mode ───────────────────────────────────

    /// Without `android-uniffi-live`, `fts_search` returns an empty list (not an
    /// error) and validates the key length.
    #[test]
    fn fts_search_stub_returns_empty_on_valid_key() {
        let key = test_key();
        let result = fts_search("/tmp/stub.db".to_string(), &key, "hello".to_string(), 10)
            .expect("stub fts_search must not error");
        assert!(result.is_empty(), "stub must return an empty list");
    }

    /// `fts_search` must fail with `InvalidKeyLength` for a key shorter than 32
    /// bytes, exactly like every other DB-keyed function.
    #[test]
    fn fts_search_stub_rejects_short_key() {
        let short_key = vec![0u8; 16];
        let err = fts_search(
            "/tmp/stub.db".to_string(),
            &short_key,
            "hello".to_string(),
            10,
        )
        .expect_err("fts_search must reject a short key");
        assert!(
            matches!(err, CopypasteError::InvalidKeyLength),
            "expected InvalidKeyLength, got {err:?}"
        );
    }

    // ── PG-19 (o0t3): get_history_page stub mode ──────────────────────────────

    /// Without `android-uniffi-live`, `get_history_page` returns an empty list
    /// and validates the key length.
    #[test]
    fn get_history_page_stub_returns_empty_on_valid_key() {
        let key = test_key();
        let result = get_history_page("/tmp/stub.db".to_string(), &key, 50, 0)
            .expect("stub get_history_page must not error");
        assert!(result.is_empty(), "stub must return an empty list");
    }

    /// `get_history_page` must fail with `InvalidKeyLength` for a key shorter
    /// than 32 bytes.
    #[test]
    fn get_history_page_stub_rejects_short_key() {
        let short_key = vec![0u8; 8];
        let err = get_history_page("/tmp/stub.db".to_string(), &short_key, 50, 0)
            .expect_err("get_history_page must reject a short key");
        assert!(
            matches!(err, CopypasteError::InvalidKeyLength),
            "expected InvalidKeyLength, got {err:?}"
        );
    }
}
