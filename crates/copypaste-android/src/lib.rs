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
    encrypt_for_cloud, encrypt_item_with_aad, AAD_SCHEMA_VERSION, NONCE_SIZE,
};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

// When using UDL-based scaffolding, uniffi::Error and uniffi::Record proc-macro
// derives conflict with the generated scaffolding. Only thiserror is needed here.
#[derive(Debug, thiserror::Error)]
pub enum CopypasteError {
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed: {message}")]
    DecryptionFailed { message: String },
    #[error("Database error: {message}")]
    DatabaseError { message: String },
    #[error("Invalid key length: expected 32")]
    InvalidKeyLength,
    /// P2P pairing / transport failure surfaced from `copypaste_p2p`
    /// (`TransportError`): TLS, socket, framing, or PAKE handshake errors —
    /// including a wrong pairing password or a channel-binding MitM abort. Also
    /// raised for a malformed `addr_hint` that cannot be parsed into a
    /// `SocketAddr`. The `message` carries the underlying error's display form.
    #[error("P2P pairing failed: {message}")]
    P2pError { message: String },
    /// v0.3 (OI-7): a Rust panic was caught at the FFI boundary by
    /// [`panic_boundary::catch_result`]. Carries the panic message so Kotlin
    /// can log/surface it instead of seeing a JVM-killing abort.
    #[error("Panicked: {message}")]
    Panicked { message: String },
}

impl From<PanicError> for CopypasteError {
    fn from(p: PanicError) -> Self {
        match p {
            PanicError::Panicked(message) => CopypasteError::Panicked { message },
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
        let key_arr: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
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
        let key_arr: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        let nonce_arr: [u8; NONCE_SIZE] =
            nonce
                .try_into()
                .map_err(|_| CopypasteError::DecryptionFailed {
                    message: "wrong nonce length".into(),
                })?;
        let aad = build_item_aad(&item_id, AAD_SCHEMA_VERSION);
        decrypt_item_with_aad(ciphertext, &nonce_arr, &key_arr, &aad).map_err(|e| {
            CopypasteError::DecryptionFailed {
                message: e.to_string(),
            }
        })
    })
}

pub fn is_sensitive(text: String) -> bool {
    detect(&text).is_some()
}

pub fn sensitive_kind(text: String) -> Option<String> {
    detect(&text).map(|k| format!("{:?}", k))
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

/// Derive a 32-byte sync key from `passphrase` using Argon2id.
///
/// Returns the raw 32-byte key material. The caller (Kotlin) should treat
/// these bytes as a short-lived secret: derive once at passphrase entry,
/// use, then zero the array. Do NOT persist to disk or SharedPreferences.
///
/// Errors:
///   - `EncryptionFailed` — Argon2 parameter or runtime failure (should not
///     occur with the hardcoded constants; surfaces as non-panic error).
pub fn derive_cloud_sync_key(passphrase: String) -> Result<Vec<u8>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key = derive_sync_key(&passphrase).map_err(|_| CopypasteError::EncryptionFailed)?;
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
        let key_arr: [u8; 32] = sync_key_bytes
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        let sync_key = copypaste_core::SyncKey::from_bytes(key_arr);
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
        let key_arr: [u8; 32] = sync_key_bytes
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        let sync_key = copypaste_core::SyncKey::from_bytes(key_arr);
        decrypt_from_cloud(&sync_key, &item_id, blob).map_err(|e| {
            CopypasteError::DecryptionFailed {
                message: e.to_string(),
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
                .map_err(|e| CopypasteError::DecryptionFailed {
                    message: e.to_string(),
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
                message: e.to_string(),
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
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime for P2P FFI")
    })
}

/// FFI result of [`generate_device_cert`]: a fresh self-signed mTLS identity.
///
/// `fingerprint` is `hex(SHA-256(cert_der))` — the SAME value the macOS side
/// pins. Kotlin must persist `cert_der` + `key_der` securely (key_der is
/// secret) and advertise `fingerprint` / `device_id` in the pairing QR.
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
                message: e.to_string(),
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
                    message: format!("invalid addr_hint '{addr_hint}': {e}"),
                })?;

        let pairing = runtime()
            .block_on(copypaste_p2p::bootstrap::run_initiator(
                addr,
                cert_der.to_vec(),
                key_der.to_vec(),
                &pake_password,
                &sync_addr,
            ))
            .map_err(|e| CopypasteError::P2pError {
                message: e.to_string(),
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
/// MUST stay byte-for-byte identical to the macOS daemon's
/// `ipc::Handler::derive_peer_sync_key_b64` constant — both sides derive the
/// shared XChaCha20-Poly1305 content key from the same PAKE `SessionKey` via
/// `SessionKey::derive_xchacha_key(P2P_SYNC_KEY_SALT)`, so a mismatch here
/// would make every synced item undecryptable on the peer.
const P2P_SYNC_KEY_SALT: &[u8] = b"copypaste/p2p/content-sync-key/v1";

/// A local clipboard item (plaintext) offered to a peer during one sync session.
#[derive(Debug)]
pub struct LocalItem {
    pub id: String,
    pub wall_time_ms: i64,
    pub content_type: String,
    pub plaintext: Vec<u8>,
}

/// An item received from the peer during sync, decrypted back to plaintext.
#[derive(Debug)]
pub struct SyncedItem {
    pub id: String,
    pub content_type: String,
    pub plaintext: Vec<u8>,
    pub wall_time_ms: i64,
}

/// Outcome of one completed P2P sync session.
#[derive(Debug)]
pub struct P2pSyncResult {
    pub items_received: u64,
    pub items_sent: u64,
    pub items: Vec<SyncedItem>,
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

/// Run ONE clipboard sync session against an already-paired peer over mTLS.
///
/// See the UDL docstring for the full contract. The flow mirrors the desktop
/// daemon's sync path:
///   1. derive the shared content key from `session_key`;
///   2. wrap each `LocalItem`'s plaintext under that key (`encrypt_for_cloud`),
///      producing the SAME on-wire form the daemon's `rekey_outbound` emits
///      (self-framed cloud blob in `content`, `content_nonce = None`);
///   3. connect to `peer_addr` with `peer_fingerprint` allow-listed, then run
///      `SyncEngine::run_session` over the raw TLS stream;
///   4. unwrap each received item with the shared key (`decrypt_from_cloud`)
///      back to plaintext.
///
/// Errors: [`CopypasteError::P2pError`] for a malformed `peer_addr`, a
/// connect/TLS failure, or a sync-protocol error; [`CopypasteError::InvalidKeyLength`]
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
        use copypaste_p2p::transport::{PairedPeers, PeerTransport};
        use copypaste_sync::engine::SyncEngine;

        let addr: std::net::SocketAddr =
            peer_addr
                .parse()
                .map_err(|e: std::net::AddrParseError| CopypasteError::P2pError {
                    message: format!("invalid peer_addr '{peer_addr}': {e}"),
                })?;

        let shared = shared_sync_key_from_session(&session_key)?;

        // Build the local item set in the SAME sync-key-wrapped wire form the
        // daemon's `rekey_outbound` produces: the cloud blob (self-framed, its
        // own 24-byte nonce prefix) goes in `content`, and `content_nonce` is
        // cleared so the peer recognises it as sync-key-wrapped.
        let mut wrapped: Vec<copypaste_core::ClipboardItem> = Vec::with_capacity(local_items.len());
        for it in &local_items {
            // Only text items are re-keyed (image chunks use a separate scheme).
            if it.content_type != "text" {
                continue;
            }
            let mut item = copypaste_core::ClipboardItem::new_text(Vec::new(), Vec::new(), 0);
            if !it.id.is_empty() {
                item.id = it.id.clone();
            }
            item.wall_time = it.wall_time_ms;
            item.lamport_ts = it.wall_time_ms;
            let blob = encrypt_for_cloud(&shared, &item.item_id, &it.plaintext)
                .map_err(|_| CopypasteError::EncryptionFailed)?;
            item.content = Some(blob);
            item.content_nonce = None;
            wrapped.push(item);
        }

        // Connect over mTLS with the peer fingerprint allow-listed, then run one
        // session over the RAW TLS stream (`run_session` does its own framing).
        let device_id = uuid::Uuid::new_v4().to_string();
        let peers = PairedPeers::new();
        peers.add(peer_fingerprint.clone(), "android-peer");
        let transport = PeerTransport::from_cert(cert_der, key_der, peers);

        let (result, to_upsert) = runtime()
            .block_on(async {
                let framed = transport.connect(addr, &peer_fingerprint).await?;
                // The sync engine drives its own length-prefixed JSON framing on
                // a raw byte stream, so peel off the LengthDelimitedCodec.
                let mut stream = framed.into_inner();
                let mut engine = SyncEngine::new(device_id.clone());
                engine
                    .run_session(&mut stream, &wrapped)
                    .await
                    .map_err(|e| {
                        copypaste_p2p::transport::TransportError::Io(std::io::Error::other(
                            e.to_string(),
                        ))
                    })
            })
            .map_err(
                |e: copypaste_p2p::transport::TransportError| CopypasteError::P2pError {
                    message: e.to_string(),
                },
            )?;

        // Unwrap every received item back to plaintext using the shared key.
        let mut items: Vec<SyncedItem> = Vec::with_capacity(to_upsert.len());
        for ci in &to_upsert {
            // A sync-key-wrapped text item carries `content` (the cloud blob) and
            // no `content_nonce`. Skip anything that doesn't fit that shape.
            if ci.content_type != "text" {
                continue;
            }
            let Some(blob) = ci.content.as_ref() else {
                continue;
            };
            if ci.content_nonce.is_some() {
                continue;
            }
            match decrypt_from_cloud(&shared, &ci.item_id, blob) {
                Ok(plaintext) => items.push(SyncedItem {
                    id: ci.id.clone(),
                    content_type: ci.content_type.clone(),
                    plaintext,
                    wall_time_ms: ci.wall_time,
                }),
                Err(_) => {
                    // A blob we cannot decrypt under the shared key is not a hard
                    // failure for the session — skip it but keep the rest.
                    continue;
                }
            }
        }

        Ok(P2pSyncResult {
            items_received: result.items_received as u64,
            items_sent: result.items_sent as u64,
            items,
        })
    })
}

// Database handle table. OnceLock is stable on Rust 1.70+ (our MSRV is 1.75).
static DB_HANDLES: OnceLock<Mutex<HashMap<u64, copypaste_core::Database>>> = OnceLock::new();
static NEXT_HANDLE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn db_handles() -> &'static Mutex<HashMap<u64, copypaste_core::Database>> {
    DB_HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Open (or create) an encrypted SQLite database at `path` using the 32-byte `key`.
/// Returns an opaque handle for subsequent calls.
pub fn open_database(path: String, key: &[u8]) -> Result<u64, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        let db =
            copypaste_core::Database::open(std::path::Path::new(&path), &key_arr).map_err(|e| {
                CopypasteError::DatabaseError {
                    message: e.to_string(),
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
        let key_arr: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;

        // Skip sensitive content (caller-visible: empty string return).
        if detect(&text).is_some() {
            return Ok(String::new());
        }

        let db = copypaste_core::Database::open(std::path::Path::new(&db_path), &key_arr).map_err(
            |e| CopypasteError::DatabaseError {
                message: e.to_string(),
            },
        )?;

        // v0.3: pre-generate item_id so the AAD baked into the ciphertext matches
        // the value persisted in the row — decryption later rebuilds the AAD from
        // the stored item_id (AAD_SCHEMA_VERSION = 3). Legacy empty-AAD fallback
        // was removed in 1c55e57.
        let item_id = uuid::Uuid::new_v4().to_string();
        let aad = build_item_aad(&item_id, AAD_SCHEMA_VERSION);
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

        copypaste_core::insert_item(&db, &item).map_err(|e| CopypasteError::DatabaseError {
            message: e.to_string(),
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
        let key_arr: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        let db = copypaste_core::Database::open(std::path::Path::new(&db_path), &key_arr).map_err(
            |e| CopypasteError::DatabaseError {
                message: e.to_string(),
            },
        )?;
        let n = copypaste_core::count_items(&db).map_err(|e| CopypasteError::DatabaseError {
            message: e.to_string(),
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
            Err(CopypasteError::Panicked { message }) => {
                assert!(
                    message.contains("synthetic panic inside FFI body"),
                    "expected panic message in error, got: {message}"
                );
            }
            other => panic!("expected CopypasteError::Panicked, got {other:?}"),
        }
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
        let enc_key = derive_cloud_sync_key("correct".into()).expect("derive enc");
        let dec_key = derive_cloud_sync_key("wrong".into()).expect("derive dec");
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
                responder.run(&pw, resp_sync_addr).await
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

    /// End-to-end loopback sync: stand up a real mTLS peer endpoint that holds
    /// ONE known clipboard item (wrapped under the SAME shared content key the
    /// FFI derives from the session key), then call `sync_with_peer` against it.
    ///
    /// Proves the full FFI path: derive shared key → mTLS connect (fingerprint
    /// pinned) → `SyncEngine::run_session` over the raw TLS stream → unwrap the
    /// received cloud blob back to the ORIGINAL plaintext. Asserts the FFI
    /// returns that item as correct plaintext and `items_received >= 1`.
    #[test]
    fn sync_with_peer_receives_item_from_loopback_peer() {
        use copypaste_p2p::pake::SessionKey;
        use copypaste_p2p::transport::{PairedPeers, PeerTransport};
        use copypaste_sync::engine::SyncEngine;
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

        // The one known item the peer holds, wrapped under the shared key exactly
        // as the daemon's `rekey_outbound` does (cloud blob, no item nonce).
        let known_item_id = uuid::Uuid::new_v4().to_string();
        let known_plaintext = b"hello from the loopback peer".to_vec();
        let known_blob = encrypt_for_cloud(&shared, &known_item_id, &known_plaintext)
            .expect("peer wraps its item under the shared key");
        let mut peer_item = copypaste_core::ClipboardItem::new_text(Vec::new(), Vec::new(), 5);
        peer_item.item_id = known_item_id.clone();
        peer_item.content = Some(known_blob);
        peer_item.content_nonce = None;

        // Peer runs on its OWN runtime in a background OS thread so the main test
        // thread is free of an ambient runtime for the synchronous FFI call.
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

                let (_addr, _fp, framed) = transport.accept(&listener).await.expect("accept");
                let mut stream = framed.into_inner();
                let mut engine = SyncEngine::new("loopback-peer");
                let items = [peer_item];
                engine
                    .run_session(&mut stream, &items)
                    .await
                    .expect("peer session")
            })
        });

        let port = port_rx.recv().expect("peer port");
        let addr = format!("127.0.0.1:{port}");

        // The FFI under test: connect, sync, unwrap received items to plaintext.
        let result = sync_with_peer(
            addr,
            peer_fp,
            session_key.to_vec(),
            init_cert.cert_der.clone(),
            init_cert.key_der.clone(),
            Vec::new(), // Android offers nothing this round; it only receives.
        )
        .expect("FFI sync_with_peer must succeed over loopback");

        assert!(
            result.items_received >= 1,
            "must receive at least the peer's one item, got {}",
            result.items_received
        );
        let got = result
            .items
            .iter()
            .find(|i| i.plaintext == known_plaintext)
            .expect("the peer's item must come back decrypted to its plaintext");
        assert_eq!(got.content_type, "text");
        assert_eq!(got.plaintext, known_plaintext);

        // Drain the peer thread so its session result is checked too.
        let peer_result = peer_thread.join().expect("peer thread join");
        assert_eq!(
            peer_result.0.items_sent, 1,
            "peer must have sent its one item to the FFI initiator"
        );
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
}
