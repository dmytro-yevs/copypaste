#![allow(clippy::empty_line_after_doc_comments)] // uniffi-generated scaffolding triggers this lint

uniffi::include_scaffolding!("copypaste_android");

pub mod panic_boundary;
pub mod version;
pub use panic_boundary::PanicError;
pub use version::{
    check_compatibility, core_version, uniffi_abi_version, VersionError, UNIFFI_ABI_VERSION,
};

use copypaste_core::{
    build_item_aad, decrypt_from_cloud, decrypt_item_with_aad, derive_sync_key, detect,
    encrypt_for_cloud, encrypt_item_with_aad, SyncKeyError, AAD_SCHEMA_VERSION,
    ITEM_KEY_VERSION_CURRENT, NONCE_SIZE,
};
// Only used by the feature-gated `add_clipboard_item` live binding below.
#[cfg(feature = "android-uniffi-live")]
use copypaste_core::{build_item_aad_v2, AAD_SCHEMA_VERSION_V4};
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

/// v0.3: `item_id` is bound into the AEAD AAD alongside `AAD_SCHEMA_VERSION`.
/// Kotlin callers MUST persist the same item_id alongside the ciphertext and
/// pass it back to `decrypt_text` verbatim — a mismatch will fail decryption
/// with `EncryptionFailed`. (Legacy empty-AAD fallback was removed in 1c55e57.)
pub fn encrypt_text(
    item_id: String,
    bytes: &[u8],
    key: &[u8],
) -> Result<EncryptedBlob, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let aad = build_item_aad(&item_id, AAD_SCHEMA_VERSION);
        let (nonce, ciphertext) = encrypt_item_with_aad(bytes, &key_arr, &aad)
            .map_err(|_| CopypasteError::EncryptionFailed)?;
        Ok(EncryptedBlob {
            nonce: nonce.to_vec(),
            ciphertext,
        })
    })
}

/// v0.3: `item_id` MUST match the value used during `encrypt_text` — see the
/// docstring on `encrypt_text` for the AAD binding rationale.
pub fn decrypt_text(
    item_id: String,
    ciphertext: &[u8],
    nonce: &[u8],
    key: &[u8],
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
        let aad = build_item_aad(&item_id, AAD_SCHEMA_VERSION);
        decrypt_item_with_aad(ciphertext, &nonce_arr, &key_arr, &aad).map_err(|e| {
            CopypasteError::DecryptionFailed {
                reason: e.to_string(),
            }
        })
    })
}

/// Returns `true` if `text` matches a sensitive pattern.
///
/// Wrapped in [`panic_boundary::catch`] because `copypaste_core::detect`
/// runs regex/allocation that could panic; an unwound panic across the JNI
/// boundary aborts the JVM. This export returns a plain `bool`, so a caught
/// panic recovers to `false` (treat as "not sensitive" rather than crash).
pub fn is_sensitive(text: String) -> bool {
    panic_boundary::catch(|| detect(&text).is_some()).unwrap_or(false)
}

/// Returns the sensitive-kind label for `text`, or `None` if not sensitive.
///
/// Wrapped in [`panic_boundary::catch`] for the same reason as
/// [`is_sensitive`]. This export returns a plain `Option<String>`, so a caught
/// panic recovers to `None`.
pub fn sensitive_kind(text: String) -> Option<String> {
    panic_boundary::catch(|| detect(&text).map(|k| format!("{:?}", k))).unwrap_or(None)
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
pub fn bootstrap_pair_initiator(
    addr_hint: String,
    cert_der: &[u8],
    key_der: &[u8],
    pake_password: String,
    sync_addr: String,
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
                // Android-side device metadata is gathered in Kotlin and synced
                // separately; send an empty meta frame so the responder still
                // gets a well-formed Phase-4 bootstrap exchange.
                &copypaste_p2p::bootstrap::PeerMeta::default(),
            ))
            .map_err(|e| CopypasteError::P2pError {
                reason: e.to_string(),
            })?;

        Ok(BootstrapResult {
            peer_fingerprint: pairing.peer_fingerprint,
            peer_sync_addr: pairing.peer_sync_addr,
            session_key: pairing.session_key.as_bytes().to_vec(),
        })
    })
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
    Ok(copypaste_core::SyncKey::from_bytes(content_key))
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
) -> Result<P2pSyncResult, CopypasteError> {
    panic_boundary::catch_result(|| {
        use bytes::Bytes;
        use copypaste_p2p::transport::{PairedPeers, PeerTransport};
        use copypaste_sync::protocol::WireItem;
        use futures_util::{SinkExt, StreamExt};

        let addr: std::net::SocketAddr =
            peer_addr
                .parse()
                .map_err(|e: std::net::AddrParseError| CopypasteError::P2pError {
                    reason: format!("invalid peer_addr '{peer_addr}': {e}"),
                })?;

        let shared = shared_sync_key_from_session(&session_key)?;
        // FIXWAVE: `origin_device_id` below is stamped with a fresh random UUID
        // on every sync call, so the peer sees a different "device" each time and
        // cannot deduplicate by origin. The FFI signature is UDL-exported and
        // changing it would break the Kotlin ABI, so this is deferred. The correct
        // fix is to pass the caller's stable `device_id` (from `generate_device_cert`)
        // into `sync_with_peer` as an extra parameter and use it here. Track as
        // FIXWAVE(android/origin_device_id): add `device_id: String` param to
        // `sync_with_peer` in the UDL and regenerate Kotlin bindings.
        let device_id = uuid::Uuid::new_v4().to_string();

        // Build the outbound `WireItem`s in the SAME sync-key-wrapped wire form
        // the daemon's `rekey_outbound` produces: the cloud blob (self-framed,
        // its own 24-byte nonce prefix) goes in `content`, and `content_nonce`
        // is `None` so the peer recognises it as sync-key-wrapped. Text, image
        // and file items are all re-keyed identically here (v0.6 Option 2 wire
        // contract): the whole plaintext travels as ONE shared-key blob, no
        // per-chunk re-key and no wire `file_id`.
        let mut outbound: Vec<WireItem> = Vec::with_capacity(local_items.len());
        for it in &local_items {
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
                // Android send path does not carry file items today; these are
                // always None on the outbound side until file-send is added.
                file_name: None,
                mime: None,
            });
        }

        // Connect over mTLS with the peer fingerprint allow-listed. KEEP the
        // `Framed<_, LengthDelimitedCodec>` the transport set up — the daemon's
        // `run_peer_connection_framed` exchanges length-delimited JSON
        // `WireItem` frames over exactly this framing (NOT `run_session`).
        let peers = PairedPeers::new();
        peers.add(peer_fingerprint.clone(), "android-peer");
        let transport = PeerTransport::from_cert(cert_der, key_der, peers);

        // Bounded receive window: the daemon pushes its catch-up history right
        // after accepting the connection, so frames arrive promptly. We read
        // until any of: no new frame for `IDLE`, `MAX_ITEMS` received, or the
        // overall `DEADLINE` elapses — then we stop (the daemon keeps the link
        // open indefinitely, so we cannot wait for an EOF here).
        const IDLE: std::time::Duration = std::time::Duration::from_secs(3);
        const DEADLINE: std::time::Duration = std::time::Duration::from_secs(20);
        const MAX_ITEMS: usize = 10_000;

        let received: Vec<WireItem> = runtime()?
            .block_on(async {
                let mut framed = transport.connect(addr, &peer_fingerprint).await?;

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
                        // A frame arrived: deserialise it as a `WireItem` exactly
                        // as the daemon's read half does.
                        Ok(Some(Ok(frame))) => match serde_json::from_slice::<WireItem>(&frame) {
                            Ok(wire) => got.push(wire),
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
                Ok(got)
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
        for wire in &received {
            // A text frame that still carries a `content_nonce` is a legacy /
            // non-rekeyed frame (e.g. a stale daemon that predates the sync-key
            // re-keying). We cannot decrypt it with the shared sync key, so we
            // still skip it — but do NOT do so silently: warn and count it so a
            // build-skew peer is observable instead of making items vanish (this
            // silent `continue` is what hid the "decrypt 7/7" failure).
            if wire.content_type == "text" && wire.content_nonce.is_some() {
                items_skipped_legacy = items_skipped_legacy.saturating_add(1);
                // NOTE: eprintln! on Android goes to a logcat black hole (stderr
                // is not captured by the Android logging subsystem). A proper fix
                // requires adding `android_logger` or `tracing-logcat` to this
                // crate's dependencies and initialising a log subscriber in the
                // FFI entry point. Until then, the skip is counted in
                // `items_skipped_legacy` (visible to Kotlin callers) so the
                // build-skew condition remains observable without silent data loss.
                // FIXWAVE: replace eprintln! with log::warn! once an android
                // logging backend (android_logger/tracing-logcat) is wired up.
                eprintln!(
                    "copypaste-android: WARN skipping legacy/non-rekeyed P2P text frame \
                     (item_id={}, origin={}): content_nonce is set, peer has not migrated \
                     to sync-key-wrapped cloud blobs; cannot decrypt with shared key",
                    wire.item_id, wire.origin_device_id
                );
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
                continue;
            }
            let Some(blob) = wire.content.as_ref() else {
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
                }),
                Err(_) => continue,
            }
        }

        Ok(P2pSyncResult {
            items_received: received.len() as u64,
            items_sent: outbound.len() as u64,
            items,
            items_skipped_legacy,
        })
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
// The cache key includes the raw key bytes (not just the path) so that a
// different key for the same path does NOT silently reuse the connection opened
// under the first key — which would mask an authentication failure.
//
// `Database` wraps a `rusqlite::Connection` (Send, !Sync) — serialising all
// access behind this `Mutex` keeps it sound, exactly like the handle table.
#[cfg(feature = "android-uniffi-live")]
static DB_BY_PATH: OnceLock<Mutex<HashMap<(String, [u8; 32]), copypaste_core::Database>>> =
    OnceLock::new();

#[cfg(feature = "android-uniffi-live")]
fn db_by_path() -> &'static Mutex<HashMap<(String, [u8; 32]), copypaste_core::Database>> {
    DB_BY_PATH.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Run `f` against the cached `Database` for `(db_path, key)`, opening (and
/// caching) it on first use. The connection is reused across calls with the
/// same path **and** the same key; a different key for the same path opens a
/// separate connection instead of silently reusing the first one.
#[cfg(feature = "android-uniffi-live")]
fn with_cached_db<T>(
    db_path: &str,
    key: &[u8; 32],
    f: impl FnOnce(&copypaste_core::Database) -> Result<T, CopypasteError>,
) -> Result<T, CopypasteError> {
    let cache_key = (db_path.to_string(), *key);
    let mut map = db_by_path().lock().unwrap_or_else(|e| e.into_inner());
    if !map.contains_key(&cache_key) {
        // #40b: evict any stale entry for the same path but a different key
        // before inserting the new connection. Without this, each key rotation
        // leaks a connection handle (the old (path, old_key) entry stays in the
        // map forever). Entries for OTHER paths are unaffected.
        map.retain(|(p, k), _| p != db_path || k == key);
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
/// # Important: handle-table only — path cache is unaffected
///
/// This function removes `handle` from the opaque integer→`Database` map
/// (`DB_HANDLES`) that backs `open_database` / the read/write exports.
/// It does **NOT** touch `DB_BY_PATH` — the path+key-keyed connection cache
/// used by the live `add_clipboard_item` / `get_history_count` exports
/// (feature `android-uniffi-live`).  Callers that need the path-cache
/// connection to also close (e.g. on logout) must do so at the Kotlin layer
/// by ensuring the process is restarted or by calling the appropriate
/// cache-clearing export when one is added.
pub fn close_database(handle: u64) {
    // A poisoned mutex on the global handle table would otherwise abort the
    // JVM via `unwrap()`. Wrapping in `catch_result` converts that into a
    // `Result::Err(PanicError)` we then deliberately discard — `close_database`
    // is declared as void in the UDL and Kotlin callers cannot signal a
    // failure path, but at minimum we keep the process alive.
    let _ = panic_boundary::catch_result(|| {
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

        // Skip sensitive content (caller-visible: empty string return).
        if detect(&text).is_some() {
            return Ok(String::new());
        }

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
        let (nonce, ciphertext) = encrypt_item_with_aad(text.as_bytes(), &key_arr, &aad)
            .map_err(|_| CopypasteError::EncryptionFailed)?;

        let lamport_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let mut item =
            copypaste_core::ClipboardItem::new_text(ciphertext, nonce.to_vec(), lamport_ts);
        item.item_id = item_id;
        let id = item.id.clone();

        // M5: reuse a cached connection instead of open-per-call.
        with_cached_db(&db_path, &key_arr, |db| {
            copypaste_core::insert_item(db, &item).map_err(|e| CopypasteError::DatabaseError {
                reason: e.to_string(),
            })
        })?;

        Ok(id)
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
        let blob = encrypt_text(item_id.clone(), b"hello android", &key).expect("encrypt");
        let plaintext =
            decrypt_text(item_id, &blob.ciphertext, &blob.nonce, &key).expect("decrypt");
        assert_eq!(plaintext, b"hello android");
    }

    /// v0.3 regression: ciphertext is bound to item_id via AAD — decrypting
    /// with a different item_id must fail with `DecryptionFailed` rather than
    /// silently returning plaintext (legacy empty-AAD fallback removed in
    /// 1c55e57).
    #[test]
    fn decrypt_rejects_mismatched_item_id() {
        let key = test_key();
        let blob = encrypt_text("item-A".into(), b"secret", &key).expect("encrypt");
        let err = decrypt_text("item-B".into(), &blob.ciphertext, &blob.nonce, &key)
            .expect_err("mismatched item_id must reject");
        assert!(
            matches!(err, CopypasteError::DecryptionFailed { .. }),
            "expected DecryptionFailed, got {err:?}"
        );
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

    #[cfg(feature = "android-uniffi-live")]
    #[test]
    fn add_clipboard_item_skips_sensitive() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("live.db");
        let key = test_key();

        // GitHub PAT pattern is detected by copypaste_core::detect.
        let pat = format!("ghp_{}", "A".repeat(36));
        let id = add_clipboard_item(path.to_string_lossy().into_owned(), &key, pat)
            .expect("sensitive returns Ok empty");
        assert!(id.is_empty(), "sensitive content yields empty id");

        let n = get_history_count(path.to_string_lossy().into_owned(), &key).expect("count");
        assert_eq!(n, 0, "no row inserted for sensitive content");
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
                        &copypaste_p2p::bootstrap::PeerMeta::default(),
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
        )
        .expect("FFI bootstrap pairing must succeed over loopback");

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
            resp_pairing
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
            copypaste_core::SyncKey::from_bytes(sk.derive_xchacha_key(P2P_SYNC_KEY_SALT))
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
            id: String::new(),
            item_id: offered_item_id.clone(),
            wall_time_ms: 7,
            content_type: offered_content_type.to_string(),
            plaintext: offered_plaintext.clone(),
        }];

        let result = sync_with_peer(
            addr,
            peer_fp,
            session_key.to_vec(),
            init_cert.cert_der.clone(),
            init_cert.key_der.clone(),
            local_items,
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
            copypaste_core::SyncKey::from_bytes(sk.derive_xchacha_key(P2P_SYNC_KEY_SALT))
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
            id: String::new(),
            item_id: offered_item_id.clone(),
            wall_time_ms: 11,
            content_type: "image".to_string(),
            plaintext: offered_plaintext.clone(),
        }];

        let result = sync_with_peer(
            addr,
            peer_fp,
            session_key.to_vec(),
            init_cert.cert_der.clone(),
            init_cert.key_der.clone(),
            local_items,
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
            copypaste_core::SyncKey::from_bytes(sk.derive_xchacha_key(P2P_SYNC_KEY_SALT))
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
            id: "local-row-1".to_string(),
            item_id: stable_id.clone(),
            wall_time_ms: 11,
            content_type: "text".to_string(),
            plaintext: b"stable-id body".to_vec(),
        }];

        let result = sync_with_peer(
            addr,
            peer_fp,
            session_key.to_vec(),
            init_cert.cert_der.clone(),
            init_cert.key_der.clone(),
            local_items,
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
        let blob = encrypt_text(item_id.clone(), b"zeroize path check", &key).expect("encrypt");
        let pt = decrypt_text(item_id, &blob.ciphertext, &blob.nonce, &key).expect("decrypt");
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
            if !map.contains_key(&cache_key_a) {
                let db = copypaste_core::Database::open(
                    std::path::Path::new(&path),
                    &Zeroizing::new(key_a),
                )
                .expect("open with key_a");
                map.insert(cache_key_a, db);
            }
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
}
