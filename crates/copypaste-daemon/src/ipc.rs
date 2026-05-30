use crate::protocol::{
    Request, Response, CURRENT_PROTOCOL_VERSION, ERR_CODE_AUTH_FAILED, ERR_CODE_INTERNAL_ERROR,
    ERR_CODE_INVALID_ARGUMENT, ERR_CODE_IPC_NOT_READY, ERR_CODE_NOT_FOUND,
    MIN_SUPPORTED_PROTOCOL_VERSION,
};
use copypaste_core::{
    bump_item_recency, chunks_from_blob, count_items, decode_image, decrypt_item_by_version,
    delete_fts, delete_item, derive_v2, ensure_revoked_devices_table, fetch_text_preview,
    get_item_by_id, get_page, get_page_pinned_first, pin_item, revoke_device, revoke_devices,
    search_items, unpin_item, Database, EncryptError, SensitiveDetector,
};
#[cfg(feature = "cloud-sync")]
use copypaste_core::{derive_sync_key, SyncKey};
use copypaste_p2p::pake::{PakeInitiator, PakeResponder, PasswordFile};
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// Maximum size of a single IPC request line. Clients exceeding this receive
/// an error response and have their connection closed. Prevents OOM from a
/// malicious or buggy client sending an unbounded stream without newlines.
const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;

/// Server-side cap on paginated reads (`list`, `history_page`). A client
/// may request more, but the server silently clamps to this value. Protects
/// the daemon from accidental or malicious requests that would attempt to
/// materialize huge result sets in a single response.
const MAX_PAGE: usize = 1000;

/// Per-item ceiling on `import` payloads (decoded `content_bytes_b64` length).
/// Larger items are rejected with `invalid_argument` BEFORE storage so a
/// malformed or hostile export cannot exhaust memory / disk on the daemon.
/// 4 MiB matches the practical upper bound for clipboard text/image payloads
/// we round-trip today; bumping this requires re-evaluating SQLite blob limits.
const MAX_IMPORT_ITEM_BYTES: usize = 4 * 1024 * 1024;

/// Error code returned when an IPC method is called before the server's
/// backing state (database, etc.) has finished initializing. Clients should
/// back off and retry rather than treat this as a hard failure.
const ERR_IPC_NOT_READY: &str = "IPC_NOT_READY";

/// Persistent application configuration stored at
/// `dirs::config_dir()/copypaste/config.json`.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub p2p_enabled: bool,
    #[serde(default)]
    pub supabase_url: Option<String>,
    #[serde(default)]
    pub supabase_anon_key: Option<String>,
    /// GoTrue account email for the `authenticated` scope sign-in. Persisted
    /// (not env-only) so the documented `copypaste cloud setup` flow yields a
    /// daemon that authenticates and passes the `authenticated`-only RLS
    /// policies — anon-key-only requests are rejected by RLS and sync silently
    /// fails. Stored in the same `0600` `config.json` as `supabase_anon_key`.
    #[serde(default)]
    pub supabase_email: Option<String>,
    /// GoTrue account password. See [`Self::supabase_email`]. Never logged; the
    /// `Debug` derive is acceptable because the daemon does not debug-print the
    /// whole config (only individual non-secret fields are surfaced over IPC).
    #[serde(default)]
    pub supabase_password: Option<String>,
}

/// Strip account credentials from a serialised [`AppConfig`] before it leaves
/// the daemon over IPC. Removes `supabase_password` and `supabase_email` and
/// replaces each with a `*_set` boolean presence flag. The anon/public key is
/// left intact (it is a publishable key the UI prefills). No-op for non-object
/// values. See the `get_config` handler for the rationale.
fn redact_config_secrets(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    let password_set = obj
        .get("supabase_password")
        .map(|p| !p.is_null())
        .unwrap_or(false);
    let email_set = obj
        .get("supabase_email")
        .map(|e| !e.is_null())
        .unwrap_or(false);
    obj.remove("supabase_password");
    obj.remove("supabase_email");
    obj.insert(
        "supabase_password_set".into(),
        serde_json::Value::Bool(password_set),
    );
    obj.insert(
        "supabase_email_set".into(),
        serde_json::Value::Bool(email_set),
    );
}

fn config_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("copypaste").join("config.json"))
}

pub(crate) fn read_config() -> AppConfig {
    let Some(path) = config_path() else {
        return AppConfig::default();
    };
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return AppConfig::default(),
    };
    match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                "config parse failed at {}: {e}, using defaults",
                path.display()
            );
            AppConfig::default()
        }
    }
}

fn write_config(cfg: &AppConfig) -> anyhow::Result<()> {
    let path = config_path().ok_or_else(|| anyhow::anyhow!("cannot determine config dir"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        // Best-effort: tighten parent dir perms to user-only.
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    }
    let json = serde_json::to_string_pretty(cfg)?;
    std::fs::write(&path, json)?;
    // chmod 0600 — config may carry supabase keys; never world-readable.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// P2P helpers
// ---------------------------------------------------------------------------

/// Format raw bytes as colon-separated hex groups (XX:XX:...).
///
/// NOTE (W3.6 consolidation): there are three near-identical fingerprint
/// formatters across daemon/UI/CLI. Within the daemon, only this one and
/// [`crate::keychain::own_fingerprint`] exist, and their semantics differ:
///
/// - [`crate::keychain::own_fingerprint`] SHA-256-hashes its input, then formats
///   the first 16 bytes (15 colons) — the canonical *device* fingerprint.
/// - This helper formats whatever raw bytes it is handed (any length) — used
///   for the legacy `get_own_fingerprint` stub which already supplies a
///   pre-derived 32-byte payload (31 colons).
///
/// Switching the call site below to `own_fingerprint` would change the
/// IPC contract (length + content) and is therefore deferred to post-alpha
/// along with the cross-crate consolidation into `copypaste-core`.
/// Convert a byte offset into `s` to a char offset, clamping to a valid char
/// boundary so it never panics.
///
/// list_view (`history_page`) maps the sensitive detector's byte ranges to char
/// offsets for the UI. The detector reports ranges over the NFKC-normalised
/// string; if a `byte` lands past the end of `s` or mid-codepoint (which can
/// happen on width-changing normalisation or any offset/string mismatch),
/// slicing `s[..byte]` would panic with "byte index is not a char boundary".
/// We clamp `byte` to `s.len()`, then walk back to the nearest char boundary at
/// or below it, and count the chars up to there.
fn byte_to_char_offset(s: &str, byte: usize) -> usize {
    let mut idx = byte.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    s[..idx].chars().count()
}

fn format_fingerprint(bytes: &[u8]) -> String {
    let encoded = hex::encode(bytes);
    encoded
        .chars()
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(":")
}

/// Path to peers.json in the app config directory.
///
/// Honours the `COPYPASTE_CONFIG_DIR` override (used by the isolated integration
/// harness, and any deployment that relocates config) before falling back to the
/// platform `dirs::config_dir()`. In all cases the file lives under a
/// `copypaste/` subdirectory so the path is stable across the override and the
/// default.
pub(crate) fn peers_file_path() -> PathBuf {
    static FALLBACK_WARNED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    let base = std::env::var_os("COPYPASTE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(dirs::config_dir)
        .unwrap_or_else(|| {
            FALLBACK_WARNED.get_or_init(|| {
                tracing::warn!(
                    "neither COPYPASTE_CONFIG_DIR nor dirs::config_dir() available — \
                     falling back to CWD for peers.json. Set $XDG_CONFIG_HOME or $HOME \
                     to silence this warning."
                );
            });
            PathBuf::from(".")
        });
    base.join("copypaste").join("peers.json")
}

/// Return `true` when a colon-hex fingerprint is a placeholder/test value —
/// i.e. all groups are the same repeated byte (e.g. "aa:aa:aa:..." or
/// "bb:bb:bb:...").  Real device fingerprints are SHA-256 of a TLS cert DER
/// and will never consist of a single repeated byte.
///
/// Filters out test fixtures that accidentally ended up in `peers.json`
/// (fix FAKE-PEERS #31).
fn is_placeholder_fingerprint(fp: &str) -> bool {
    // Must have at least one colon to be a colon-hex fingerprint at all.
    if !fp.contains(':') {
        return false;
    }
    let groups: Vec<&str> = fp.split(':').collect();
    if groups.is_empty() {
        return false;
    }
    // All groups must be valid two-hex-digit bytes AND all identical.
    let all_valid = groups
        .iter()
        .all(|g| g.len() == 2 && g.chars().all(|c| c.is_ascii_hexdigit()));
    if !all_valid {
        return false;
    }
    groups.iter().all(|g| *g == groups[0])
}

/// Load peers list from peers.json; returns empty vec if file is absent.
///
/// Filters out any peer whose fingerprint is an all-same-repeated-byte
/// placeholder (fix FAKE-PEERS #31 — test fixtures must not leak into runtime).
fn load_peers() -> anyhow::Result<Vec<serde_json::Value>> {
    let path = peers_file_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = std::fs::read_to_string(&path)?;
    let peers: Vec<serde_json::Value> = serde_json::from_str(&data)?;
    // Strip placeholder fingerprints.  Log once so the admin knows the file
    // had stale test data; do NOT auto-delete peers.json (non-destructive).
    let filtered: Vec<serde_json::Value> = peers
        .into_iter()
        .filter(|p| {
            let fp = p
                .get("fingerprint")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if is_placeholder_fingerprint(fp) {
                tracing::warn!(
                    fingerprint = %fp,
                    "list_peers: skipping placeholder/test fingerprint in peers.json (all-same-byte)"
                );
                false
            } else {
                true
            }
        })
        .collect();
    Ok(filtered)
}

/// Persist peers list to peers.json, creating directories as needed.
fn save_peers(peers: &[serde_json::Value]) -> anyhow::Result<()> {
    let path = peers_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        // Best-effort: tighten parent dir perms to user-only.
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    }
    let data = serde_json::to_string_pretty(peers)?;
    std::fs::write(&path, data)?;
    // chmod 0600 — peer fingerprints are sensitive identifiers; never world-readable.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

/// Validate that a fingerprint string matches the XX:XX:... hex pattern.
fn is_valid_fingerprint(fp: &str) -> bool {
    let groups: Vec<&str> = fp.split(':').collect();
    if groups.is_empty() {
        return false;
    }
    groups
        .iter()
        .all(|g| g.len() == 2 && g.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Normalise a user-facing `XX:XX:...` colon-hex fingerprint to the canonical
/// lowercase, colon-free hex form used by the mTLS layer
/// ([`copypaste_p2p::cert::fingerprint_of`] → `hex::encode(SHA-256(cert_der))`).
///
/// The IPC pairing surface and `peers.json` carry the human-readable colon
/// form; [`PairedPeers::is_known`] compares against `fingerprint_of` output.
/// Both must agree or a paired peer is silently rejected at handshake time, so
/// the live-allowlist registration (fix/p2p-c-review #2) goes through this.
pub(crate) fn canonical_fingerprint(fp: &str) -> String {
    fp.replace(':', "").to_ascii_lowercase()
}

/// Render a colon-free hex fingerprint (the mTLS layer's canonical form,
/// `hex(SHA-256(cert_der))`) into the user-facing `XX:XX:...` colon-grouped
/// form the pairing surface expects.
///
/// This is the inverse of [`canonical_fingerprint`] for the grouping: it pairs
/// the hex digits and joins them with `:` so the value passes
/// [`is_valid_fingerprint`] and round-trips back to the same canonical bytes
/// the verifier ([`copypaste_p2p::cert::fingerprint_of`]) compares against.
/// Input is lowercased; any `:` already present is stripped first so the
/// function is idempotent. An odd-length input (never produced by
/// `fingerprint_of`) keeps its trailing nibble in the final group rather than
/// panicking.
pub(crate) fn display_fingerprint(fp: &str) -> String {
    let canonical = canonical_fingerprint(fp);
    let bytes = canonical.as_bytes();
    bytes
        .chunks(2)
        .map(|pair| std::str::from_utf8(pair).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(":")
}

/// Maximum lifetime of an in-progress PAKE session before it is evicted as
/// stale (fix/p2p-c-review #1 — DoS). The full 3-message handshake is two
/// user-driven IPC round-trips; 120 s is generous for a human typing a
/// pairing password on the second device while bounding how long a leaked /
/// abandoned session (crashed client) pins a `PakeInitiator`/`PakeResponder`
/// in memory.
const PAKE_SESSION_TTL: std::time::Duration = std::time::Duration::from_secs(120);

/// Hard cap on the number of simultaneously-live PAKE sessions (fix/p2p-c-review
/// #1 — DoS). Pairing is an interactive, one-at-a-time-per-user operation; a
/// healthy host never approaches this. The cap converts an unbounded-growth
/// memory-exhaustion vector into a bounded one: past the cap, new `initiate` /
/// `pair_accept_password` calls are rejected with a clear error rather than
/// allocating without limit.
const MAX_PAKE_SESSIONS: usize = 64;

/// In-progress PAKE handshake session stored between IPC round-trips.
///
/// Because IPC is request-response (single turn), the 3-message OPAQUE
/// handshake is split across two calls on each side:
///
/// - Initiator: `pair_peer_with_password {step:"initiate"}` → stores
///   `PakeSession::Initiator`; `pair_peer_with_password {step:"finish"}` →
///   consumes it.
/// - Responder: `pair_accept_password` → stores `PakeSession::Responder`;
///   `pair_accept_finish` → consumes it.
///
/// Sessions are keyed by a UUID `session_id` that is returned to the caller
/// and echoed back in the follow-up call. Each entry is timestamped
/// ([`StampedPakeSession`]) and bounded by [`PAKE_SESSION_TTL`] /
/// [`MAX_PAKE_SESSIONS`] — see [`IpcServer::insert_pake_session`].
enum PakeSession {
    /// Initiator waiting for the server's `CredentialResponse` (message2)
    /// to call `PakeInitiator::finish`. Boxed to equalise variant sizes and
    /// satisfy `clippy::large_enum_variant`.
    Initiator(Box<PakeInitiator>),
    /// Responder waiting for the client's `CredentialFinalization` (message3)
    /// to call `PakeResponder::finish`, plus the peer fingerprint needed to
    /// store the resulting `PasswordFile`.
    Responder {
        responder: Box<PakeResponder>,
        /// Persisted `PasswordFile` registered for this session's password.
        /// Needed to re-drive `PakeResponder::respond` — already computed in
        /// `pair_accept_password`, stored here so `pair_accept_finish` can
        /// persist it without re-registering.
        password_file: PasswordFile,
        /// Fingerprint of the initiating peer; stored in peers.json on success.
        peer_fingerprint: String,
    },
}

/// A [`PakeSession`] tagged with its creation time so stale sessions can be
/// evicted (fix/p2p-c-review #1 — DoS).
struct StampedPakeSession {
    session: PakeSession,
    created_at: std::time::Instant,
}

pub struct IpcServer {
    db: Arc<Mutex<Database>>,
    /// Shared private-mode flag. When true, the clipboard monitor skips recording.
    private_mode: Arc<AtomicBool>,
    /// Local symmetric encryption key (XChaCha20-Poly1305). Required by the
    /// `copy`/`paste` handlers so paste-back can decrypt the ciphertext
    /// stored in `clipboard_items.content` and write *plaintext* to
    /// NSPasteboard. Audit CRIT #1: previously the handler wrote raw
    /// ciphertext bytes back, so paste produced "content is not valid
    /// UTF-8" for text and garbage for images.
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    /// Device public-key bytes (X25519). Historically `get_own_fingerprint`
    /// derived its value from this via `keychain::own_fingerprint` (audit HIGH
    /// #6, superseding an unstable DefaultHasher scheme). CRITICAL-1: pairing
    /// now advertises the mTLS **cert** fingerprint (`cert_fingerprint`)
    /// instead, since the device-key fingerprint is never what the mTLS layer
    /// pins. The bytes are retained here — they remain part of the
    /// `IpcServer::new` contract and the device identity is still useful for
    /// future non-pairing surfaces.
    // Retained for API stability / future use; no current read path. The cert
    // fingerprint, not this device-key fingerprint, is what pairing advertises.
    #[allow(dead_code)]
    device_public_key: Arc<[u8; 32]>,
    /// Readiness gate. While `false`, all data-touching methods return
    /// `IPC_NOT_READY` instead of dispatching. Default `true` for production
    /// use (db is fully constructed before `IpcServer::new` is called); tests
    /// use [`IpcServer::new_with_ready`] to exercise the not-ready path.
    ready: Arc<AtomicBool>,
    /// DUP-ON-COPY fix: after `write_to_pasteboard` completes, record the new
    /// NSPasteboard `changeCount` here. The clipboard monitor reads this on
    /// the next tick and skips recording when it matches — preventing the
    /// daemon's own pasteboard writes from being captured as new clipboard events.
    /// Sentinel -1 means "no pending self-write".
    pub self_write_change_count: Arc<std::sync::atomic::AtomicI64>,
    /// In-progress PAKE sessions keyed by session_id UUID string.
    ///
    /// Each entry lives from the first IPC call (initiate / accept) until the
    /// matching finish call consumes it. Bounded against unbounded growth
    /// (fix/p2p-c-review #1 — DoS): entries older than [`PAKE_SESSION_TTL`]
    /// are evicted on every insert, and the live count is capped at
    /// [`MAX_PAKE_SESSIONS`]. See [`IpcServer::insert_pake_session`].
    pake_sessions: Arc<Mutex<HashMap<String, StampedPakeSession>>>,
    /// The single active QR-pairing token issued by `pair_generate_qr`, with
    /// its issue time for TTL eviction.
    ///
    /// QR pairing is the displaying-device-is-responder flow: this device
    /// generates a fresh token, renders it in the QR, and stores it here so the
    /// `pair_accept_qr` handler can re-derive the same PAKE password when the
    /// scanning device's `message1` arrives — without the user re-typing
    /// anything. Only one QR is active at a time (regenerating replaces it),
    /// matching the single-token pairing UX. Bounded by [`PAKE_SESSION_TTL`].
    /// `None` until the first `pair_generate_qr` call.
    pending_qr_token: Arc<Mutex<Option<(copypaste_core::PairingToken, std::time::Instant)>>>,
    /// Live P2P paired-peer allowlist, shared with the running mTLS transport
    /// (fix/p2p-c-review #2). When a PAKE handshake finishes, the newly-paired
    /// peer fingerprint is fed into this same instance via
    /// [`PairedPeers::rotate_peer`] so the accept loop immediately honours it
    /// (the S10 grace path is exercised). `None` when P2P is disabled — the
    /// PAKE handlers then only persist to `peers.json` (loaded on next start).
    p2p_peers: Option<copypaste_p2p::transport::PairedPeers>,
    /// Our live mTLS **certificate** fingerprint in user-facing colon-hex form,
    /// i.e. `display_fingerprint(hex(SHA-256(cert_der)))` for the exact same
    /// cert the running `PeerTransport` presents and that peers pin
    /// ([`copypaste_p2p::transport::PeerTransport::fingerprint`] /
    /// [`copypaste_p2p::cert::fingerprint_of`]).
    ///
    /// CRITICAL-1 fix: pairing (`pair_generate_qr`, `get_own_fingerprint`)
    /// MUST advertise this value — NOT the device-key fingerprint
    /// (`keychain::own_fingerprint`, SHA-256 of the X25519 public key), which
    /// the mTLS allowlist never compares against, so cert-pinning could never
    /// match and pairing could never authenticate.
    ///
    /// `None` when P2P is disabled (`COPYPASTE_P2P` unset): no transport runs,
    /// so there is no cert to advertise and the pairing handlers return a clear
    /// error rather than a fingerprint that cannot authenticate any channel.
    cert_fingerprint: Option<String>,
    /// Our self-signed mTLS certificate DER + key, used to TLS-wrap the
    /// unauthenticated bootstrap pairing channel (P2P Phase 1). This is a clone
    /// of the SAME cert `start_p2p`'s transport presents and whose fingerprint
    /// `cert_fingerprint` advertises, so the fingerprints a pairing peer learns
    /// over the bootstrap channel match the ones the pinned mTLS layer compares.
    ///
    /// `None` when P2P is disabled — the QR pairing handlers then fall back to
    /// the legacy IPC-relayed PAKE path (no network bootstrap channel).
    p2p_cert: Option<Arc<(Vec<u8>, Vec<u8>)>>,
    /// Optional mDNS discovery handle used by the initiator's QR-accept path to
    /// resolve the responder's `host:port` when the QR carries no `addr_hint`
    /// (best-effort fallback — loopback mDNS is unreliable, so `addr_hint` is
    /// the primary path). `None` when P2P discovery is not wired in.
    discovery: Option<Arc<copypaste_p2p::discovery::DiscoveryService>>,
    /// This daemon's own P2P sync-listener address (`host:port`), filled once
    /// `start_p2p` has bound its accept loop (the port is OS-assigned, so it is
    /// not known when `IpcServer` is constructed). The pairing handlers send
    /// this value in-band over the bootstrap channel so the peer can persist it
    /// for the Phase 3 outbound connector. A `std::sync::Mutex` (not tokio's) is
    /// used because the critical section is a trivial clone with no `.await`.
    /// Holds `None` until populated, or when P2P is disabled.
    p2p_sync_addr: Arc<std::sync::Mutex<Option<String>>>,
    /// Shared passphrase-derived cloud sync key (Argon2id, 32 bytes).
    ///
    /// `None` means the user has not yet configured a sync passphrase, so
    /// cloud upload/download is skipped. Set via `set_sync_passphrase`; shared
    /// with the cloud push/poll loops via `Arc<Mutex<Option<SyncKey>>>`.
    #[cfg(feature = "cloud-sync")]
    pub sync_key: Arc<Mutex<Option<SyncKey>>>,
    /// Monotonic timestamp (ms since UNIX epoch) of the last successful cloud
    /// sync round-trip. `0` means never synced. Shared with cloud loops so
    /// `get_sync_status` returns a live value.
    #[cfg(feature = "cloud-sync")]
    pub last_sync_ms: Arc<std::sync::atomic::AtomicI64>,
    /// Real GoTrue auth state, published by the cloud push/poll loops (BUG 2).
    /// `true` once `start_cloud` resolves a bearer, `false` on a bearer-resolution
    /// failure (`CloudError::AuthFailed`) or a failed 401-refresh. Read by
    /// `get_sync_status` so the UI reflects the actual signed-in state instead of
    /// the old hardcoded `signed_in = supabase_configured`.
    #[cfg(feature = "cloud-sync")]
    pub cloud_signed_in: Arc<AtomicBool>,
    /// Broadcast sender for newly-ingested clipboard items, shared with the
    /// clipboard monitor and the sync orchestrator (P2P Phase 3).
    ///
    /// Captured-by-polling items already flow through this channel from the
    /// monitor. The `import` IPC method historically inserted straight into the
    /// DB without notifying anyone, so imported items never reached the sync
    /// orchestrator and could not be pushed to a paired peer. Wiring the sender
    /// here lets `import` broadcast each inserted row so it syncs like a captured
    /// one. `None` when the daemon did not provide a sender (e.g. unit tests).
    new_item_tx: Option<tokio::sync::broadcast::Sender<copypaste_core::ClipboardItem>>,
    /// Degraded-startup reason, surfaced verbatim in the `status` response so
    /// the UI can render a recovery banner instead of treating an unreachable
    /// socket as a dead daemon.
    ///
    /// `None` in the normal case (DB opened, key available). `Some(reason)`
    /// when the daemon came up in degraded mode — e.g. the SQLCipher key could
    /// not be obtained from the Keychain (`keychain_locked`) so the existing
    /// encrypted DB could not be opened (`db_unavailable`). In degraded mode
    /// `ready` is `false`, so every DB-touching method already returns
    /// `IPC_NOT_READY`; this field tells the client *why* and that recovery is
    /// possible (re-grant Keychain access, then relaunch). See the
    /// [`DEGRADED_REASON_KEYCHAIN_LOCKED`] constant for the canonical value.
    degraded_reason: Option<String>,
}

/// Canonical `status.degraded_reason` value for the keychain-locked /
/// DB-unavailable degraded startup (the post-reinstall regression). The UI
/// keys its recovery banner off this exact string.
pub const DEGRADED_REASON_KEYCHAIN_LOCKED: &str = "keychain_locked";

/// Canonical `status.degraded_reason` value for the case where the SQLCipher
/// key WAS obtained but does NOT match the existing database (SQLITE_NOTADB /
/// `file is not a database`). Distinct from `keychain_locked` (key unreachable)
/// because the recovery story differs: the key is present but wrong — e.g. a
/// re-keyed device, a restored/foreign Keychain entry, or a fresh file-store
/// key minted over a DB encrypted by a pre-file-store (v0.5.1) Keychain key.
/// The UI shows a distinct banner so users are not told to "re-grant the
/// Keychain prompt" when that will not help.
pub const DEGRADED_REASON_DB_KEY_MISMATCH: &str = "db_key_mismatch";

impl IpcServer {
    pub fn new(
        db: Arc<Mutex<Database>>,
        private_mode: Arc<AtomicBool>,
        local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
        device_public_key: Arc<[u8; 32]>,
    ) -> Self {
        Self {
            db,
            private_mode,
            local_key,
            device_public_key,
            ready: Arc::new(AtomicBool::new(true)),
            pake_sessions: Arc::new(Mutex::new(HashMap::new())),
            pending_qr_token: Arc::new(Mutex::new(None)),
            p2p_peers: None,
            cert_fingerprint: None,
            p2p_cert: None,
            discovery: None,
            p2p_sync_addr: Arc::new(std::sync::Mutex::new(None)),
            self_write_change_count: Arc::new(std::sync::atomic::AtomicI64::new(-1)),
            #[cfg(feature = "cloud-sync")]
            sync_key: Arc::new(Mutex::new(None)),
            #[cfg(feature = "cloud-sync")]
            last_sync_ms: Arc::new(std::sync::atomic::AtomicI64::new(0)),
            #[cfg(feature = "cloud-sync")]
            cloud_signed_in: Arc::new(AtomicBool::new(false)),
            new_item_tx: None,
            degraded_reason: None,
        }
    }

    /// Mark this server as serving a degraded startup (e.g. keychain-locked /
    /// db-unavailable). The reason is echoed in the `status` response so the UI
    /// can show a recovery banner. Pair this with `new_with_ready(.., false)`
    /// so DB-touching methods return `IPC_NOT_READY`.
    pub fn with_degraded_reason(mut self, reason: impl Into<String>) -> Self {
        self.degraded_reason = Some(reason.into());
        self
    }

    /// Attach the live mTLS certificate fingerprint that pairing advertises.
    ///
    /// CRITICAL-1: this MUST be the fingerprint of the same cert the running
    /// `PeerTransport` presents (`display_fingerprint(transport.fingerprint())`)
    /// so a scanning/pairing peer pins a value the mTLS layer actually compares
    /// against. The daemon generates the cert once and hands the same cert to
    /// `start_p2p` and the colon-hex fingerprint here, guaranteeing they agree.
    pub fn with_cert_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.cert_fingerprint = Some(fingerprint.into());
        self
    }

    /// Attach the live P2P paired-peer allowlist (fix/p2p-c-review #2).
    ///
    /// The daemon shares the same `PairedPeers` instance with the running mTLS
    /// transport; supplying it here lets the PAKE finish handlers register a
    /// freshly-paired peer in-memory so the accept loop honours it without a
    /// daemon restart.
    pub fn with_p2p_peers(mut self, peers: copypaste_p2p::transport::PairedPeers) -> Self {
        self.p2p_peers = Some(peers);
        self
    }

    /// Attach the self-signed mTLS cert (DER) + key used to TLS-wrap the
    /// unauthenticated bootstrap pairing channel (P2P Phase 1).
    ///
    /// MUST be a clone of the exact cert `start_p2p`'s transport presents (and
    /// whose fingerprint `with_cert_fingerprint` advertises) so the fingerprints
    /// a peer learns over the bootstrap channel match what the pinned mTLS layer
    /// later compares.
    pub fn with_p2p_cert(mut self, cert_der: Vec<u8>, key_der: Vec<u8>) -> Self {
        self.p2p_cert = Some(Arc::new((cert_der, key_der)));
        self
    }

    /// Attach the mDNS discovery handle used as the QR-accept fallback when the
    /// QR carries no `addr_hint`.
    pub fn with_discovery(
        mut self,
        discovery: Arc<copypaste_p2p::discovery::DiscoveryService>,
    ) -> Self {
        self.discovery = Some(discovery);
        self
    }

    /// Return a handle to the shared slot holding this daemon's own P2P
    /// sync-listener address (`host:port`).
    ///
    /// The IPC server is constructed before `start_p2p` binds its accept loop,
    /// so the OS-assigned port is not known yet. The daemon calls
    /// [`set_p2p_sync_addr`](Self::set_p2p_sync_addr) (via this same Arc) once
    /// `start_p2p` returns the bound port; the pairing handlers then read it and
    /// send it in-band over the bootstrap channel. Returning the Arc lets the
    /// daemon populate the slot after the server has been moved into its task.
    pub fn p2p_sync_addr_slot(&self) -> Arc<std::sync::Mutex<Option<String>>> {
        Arc::clone(&self.p2p_sync_addr)
    }

    /// Populate the shared slot with this daemon's bound P2P sync-listener
    /// address. Convenience wrapper over [`p2p_sync_addr_slot`](Self::p2p_sync_addr_slot)
    /// for callers that still hold the server (e.g. tests).
    ///
    /// A poisoned mutex (a prior panic while holding the lock) is recovered
    /// rather than propagated — the slot holds only a non-secret address string,
    /// so reusing it after a panic is safe and keeps pairing functional.
    pub fn set_p2p_sync_addr(&self, addr: impl Into<String>) {
        let mut slot = self
            .p2p_sync_addr
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *slot = Some(addr.into());
    }

    /// Wire up shared cloud-sync state created by the daemon before spawning
    /// the IPC server and `start_cloud`.
    ///
    /// By calling this the daemon guarantees both surfaces see the **same**
    /// `Arc`s: a `set_sync_passphrase` IPC call writes to the same
    /// `sync_key` `Mutex` that the cloud push/poll loops read from, and the
    /// cloud loops write to the same `last_sync_ms` counter that
    /// `get_sync_status` reads.
    #[cfg(feature = "cloud-sync")]
    pub fn with_cloud_sync_state(
        mut self,
        sync_key: Arc<Mutex<Option<SyncKey>>>,
        last_sync_ms: Arc<std::sync::atomic::AtomicI64>,
        cloud_signed_in: Arc<AtomicBool>,
    ) -> Self {
        self.sync_key = sync_key;
        self.last_sync_ms = last_sync_ms;
        self.cloud_signed_in = cloud_signed_in;
        self
    }

    /// Attach the broadcast sender for newly-ingested clipboard items so the
    /// `import` IPC method can notify the sync orchestrator (P2P Phase 3).
    pub fn with_new_item_tx(
        mut self,
        tx: tokio::sync::broadcast::Sender<copypaste_core::ClipboardItem>,
    ) -> Self {
        self.new_item_tx = Some(tx);
        self
    }

    /// Construct with an explicit readiness flag. The returned handle can be
    /// flipped to `true` once initialization completes. Intended for tests
    /// and for callers that want to bind the socket before the database is
    /// fully open.
    #[allow(dead_code)]
    pub fn new_with_ready(
        db: Arc<Mutex<Database>>,
        private_mode: Arc<AtomicBool>,
        local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
        device_public_key: Arc<[u8; 32]>,
        ready: Arc<AtomicBool>,
    ) -> Self {
        Self {
            db,
            private_mode,
            local_key,
            device_public_key,
            ready,
            pake_sessions: Arc::new(Mutex::new(HashMap::new())),
            pending_qr_token: Arc::new(Mutex::new(None)),
            p2p_peers: None,
            cert_fingerprint: None,
            p2p_cert: None,
            discovery: None,
            p2p_sync_addr: Arc::new(std::sync::Mutex::new(None)),
            self_write_change_count: Arc::new(std::sync::atomic::AtomicI64::new(-1)),
            #[cfg(feature = "cloud-sync")]
            sync_key: Arc::new(Mutex::new(None)),
            #[cfg(feature = "cloud-sync")]
            last_sync_ms: Arc::new(std::sync::atomic::AtomicI64::new(0)),
            #[cfg(feature = "cloud-sync")]
            cloud_signed_in: Arc::new(AtomicBool::new(false)),
            new_item_tx: None,
            degraded_reason: None,
        }
    }

    /// Insert a PAKE session under `session_id`, first evicting stale and
    /// excess sessions (fix/p2p-c-review #1 — DoS).
    ///
    /// Eviction policy, applied on every insert:
    /// 1. Drop any session older than [`PAKE_SESSION_TTL`].
    /// 2. If still at/above [`MAX_PAKE_SESSIONS`], reject the new session with
    ///    `Err` so the caller can surface a clear error instead of growing the
    ///    map without bound.
    ///
    /// On success returns `Ok(())` with the timestamped session stored.
    async fn insert_pake_session(
        &self,
        session_id: String,
        session: PakeSession,
    ) -> Result<(), &'static str> {
        let now = std::time::Instant::now();
        let mut sessions = self.pake_sessions.lock().await;

        // 1. Evict stale sessions (TTL).
        sessions.retain(|_, s| now.duration_since(s.created_at) < PAKE_SESSION_TTL);

        // 2. Enforce the hard cap. Reuse of an existing id (should not happen —
        //    ids are fresh UUIDs) overwrites in place and does not grow the map.
        if !sessions.contains_key(&session_id) && sessions.len() >= MAX_PAKE_SESSIONS {
            tracing::warn!(
                live = sessions.len(),
                cap = MAX_PAKE_SESSIONS,
                "rejecting new PAKE session: live-session cap reached"
            );
            return Err("too many in-flight pairing sessions; try again shortly");
        }

        sessions.insert(
            session_id,
            StampedPakeSession {
                session,
                created_at: now,
            },
        );
        Ok(())
    }

    /// Register a freshly-paired peer in the live mTLS allowlist so the accept
    /// loop honours it immediately, with no daemon restart (fix/p2p-c-review #2).
    ///
    /// `peer_fingerprint` is the user-facing colon-hex form; it is normalised
    /// to the canonical lowercase, colon-free hex the transport compares
    /// against. We go through [`PairedPeers::rotate_peer`] (rather than `add`)
    /// so the S10 cert-rotation grace path is exercised on the same code path
    /// used for re-pairing; for a first-time pair `old == new`, which `rotate`
    /// treats as a plain add (no superseded entry — nothing to grace).
    ///
    /// No-op when P2P is disabled (`p2p_peers == None`): the PAKE handler has
    /// already persisted the peer to `peers.json`, which `start_p2p` loads on
    /// the next run.
    fn register_live_peer(&self, peer_fingerprint: &str) {
        if let Some(ref peers) = self.p2p_peers {
            let canonical = canonical_fingerprint(peer_fingerprint);
            peers.rotate_peer(&canonical, canonical.clone(), peer_fingerprint);
            tracing::info!(
                fingerprint = %peer_fingerprint,
                "registered paired peer in live P2P allowlist"
            );
        }
    }

    /// This daemon's own P2P sync-listener address (`host:port`), if `start_p2p`
    /// has bound it. Sent in-band over the bootstrap channel so the peer can
    /// persist it for the Phase 3 connector. Returns an empty string when the
    /// port is not yet known (P2P disabled or not yet bound) — the bootstrap
    /// wire tolerates an empty address frame.
    fn own_sync_addr(&self) -> String {
        self.p2p_sync_addr
            .lock()
            .map(|slot| slot.clone().unwrap_or_default())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone().unwrap_or_default())
    }

    /// Derive the base64-encoded shared content sync key for a peer from the
    /// PAKE [`SessionKey`](copypaste_p2p::pake::SessionKey).
    ///
    /// Uses `SessionKey::derive_xchacha_key` with a fixed domain-separation
    /// salt so the derivation is (a) deterministic — both paired devices hold
    /// the same `SessionKey` and therefore derive the IDENTICAL content key —
    /// and (b) domain-separated from any other use of the same session key
    /// (e.g. TLS channel binding). The resulting 32-byte key is the
    /// XChaCha20-Poly1305 key the sync orchestrator feeds to
    /// `encrypt_for_cloud` / `decrypt_from_cloud` for cross-device item payloads.
    fn derive_peer_sync_key_b64(session_key: &copypaste_p2p::pake::SessionKey) -> String {
        use base64::Engine as _;
        // Fixed, non-secret domain-separation salt for the P2P content sync key.
        const P2P_SYNC_KEY_SALT: &[u8] = b"copypaste/p2p/content-sync-key/v1";
        let key = session_key.derive_xchacha_key(P2P_SYNC_KEY_SALT);
        base64::engine::general_purpose::STANDARD.encode(key)
    }

    /// Durably persist a freshly-paired peer to `peers.json` (P2P Phase 2), in
    /// addition to the in-memory allowlist registration.
    ///
    /// `peer_fp_canonical` is the canonical (colon-free, lowercase) cert
    /// fingerprint the bootstrap channel reports; it is stored in the
    /// user-facing colon-hex form so the rest of the IPC peers surface
    /// (`list_peers`, revoke, etc.) and `load_persisted_peers_into` round-trip
    /// it consistently. `peer_sync_addr` is the peer's P2P sync-listener address
    /// learned in-band, stored so the Phase 3 connector can dial it directly
    /// (loopback mDNS filters 127.0.0.1 and is unreliable).
    ///
    /// Idempotent: if a record with the same fingerprint already exists it is
    /// replaced (address/name refreshed) rather than duplicated. Failures are
    /// logged and swallowed — pairing already succeeded in memory, and a persist
    /// failure must not turn a successful pair into an IPC error.
    ///
    /// A free function (not a `&self` method) so the detached bootstrap-responder
    /// task can call it after `self` has been moved/borrowed away.
    fn persist_paired_peer(
        peer_fp_canonical: &str,
        peer_sync_addr: &str,
        session_key: &copypaste_p2p::pake::SessionKey,
    ) {
        let display = display_fingerprint(peer_fp_canonical);
        let added_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let address = if peer_sync_addr.is_empty() {
            None
        } else {
            Some(peer_sync_addr.to_string())
        };

        // P2P Phase 3 (cross-device readability): derive the shared content sync
        // key from the PAKE session key. Both sides hold the SAME session key
        // after a successful handshake, so each persists the IDENTICAL bytes —
        // that is what lets a peer decrypt items this device sends (and vice
        // versa). The derived key is base64-encoded into peers.json (chmod 0600).
        let sync_key_b64 = Some(Self::derive_peer_sync_key_b64(session_key));

        let path = peers_file_path();
        let mut peers = crate::peers::load_peers(&path);
        // Drop any prior record for the same peer (canonical compare) so a
        // re-pair refreshes the address/name instead of duplicating the entry.
        peers.retain(|p| canonical_fingerprint(&p.fingerprint) != peer_fp_canonical);
        peers.push(crate::peers::PairedDevice {
            fingerprint: display,
            name: String::new(),
            added_at,
            address,
            sync_key_b64,
        });

        match crate::peers::save_peers(&path, &peers) {
            Ok(()) => tracing::info!(
                fingerprint = %peer_fp_canonical,
                addr = %peer_sync_addr,
                "persisted paired peer to peers.json"
            ),
            Err(e) => tracing::warn!(
                fingerprint = %peer_fp_canonical,
                "failed to persist paired peer to peers.json: {e}"
            ),
        }
    }

    /// Spawn the responder side of the P2P Phase 1 bootstrap PAKE handshake.
    ///
    /// The `responder` already owns the bound, TLS-wrapped ephemeral listener
    /// whose address was advertised in the QR's `addr_hint`. This accepts ONE
    /// inbound connection within the pairing window and runs the PAKE responder
    /// over the TLS stream. On success the peer's cert fingerprint (learned over
    /// the same channel) is registered in the live mTLS allowlist so subsequent
    /// pinned mTLS sessions are accepted without a daemon restart.
    ///
    /// Runs detached: pairing is driven by the scanning device dialling in, so
    /// there is nothing for the IPC caller to await here. PAKE failure (wrong
    /// token, MitM, timeout) only logs — no peer is registered.
    fn spawn_bootstrap_responder(
        &self,
        responder: copypaste_p2p::bootstrap::BootstrapResponder,
        password: String,
    ) {
        let peers = self.p2p_peers.clone();
        // Our own P2P sync-listener address, sent in-band so the initiator can
        // persist it; and used by nothing else here. Captured before the move.
        let own_sync_addr = self.own_sync_addr();
        tokio::spawn(async move {
            match responder.run(&password, &own_sync_addr).await {
                Ok(outcome) => {
                    tracing::info!(
                        peer_fingerprint = %outcome.peer_fingerprint,
                        peer_sync_addr = %outcome.peer_sync_addr,
                        "bootstrap PAKE responder completed over network channel"
                    );
                    // Register the freshly-paired peer in the live allowlist.
                    // The bootstrap channel reports the canonical (colon-free)
                    // hex fingerprint; `rotate_peer` upserts it as active.
                    if let Some(peers) = peers {
                        peers.rotate_peer(
                            &outcome.peer_fingerprint,
                            outcome.peer_fingerprint.clone(),
                            String::new(),
                        );
                    }
                    // P2P Phase 2: durably persist the peer (fingerprint +
                    // sync-listener address) so it survives a restart and the
                    // Phase 3 connector can dial it directly.
                    Self::persist_paired_peer(
                        &outcome.peer_fingerprint,
                        &outcome.peer_sync_addr,
                        &outcome.session_key,
                    );
                }
                Err(e) => {
                    tracing::warn!("bootstrap PAKE responder failed: {e}");
                }
            }
        });
    }

    /// Initiator side of the P2P Phase 1 network pairing flow.
    ///
    /// Decodes the scanned `qr`, derives the PAKE password from its token,
    /// resolves the responder's `host:port` (QR `addr_hint` primary; mDNS
    /// `resolve_peer` fallback), dials the unauthenticated bootstrap TLS channel,
    /// and runs the PAKE initiator over it. On success the responder's cert
    /// fingerprint is registered in the live mTLS allowlist.
    ///
    /// Returns the IPC `Response` directly (this is the whole handler for the
    /// network branch of `pair_accept_qr`).
    async fn pair_accept_qr_network(&self, req_id: String, qr: &str) -> Response {
        // We must have our own cert to present on the bootstrap channel so the
        // responder learns the fingerprint it will later pin.
        let cert = match self.p2p_cert.as_ref() {
            Some(c) => Arc::clone(c),
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    "P2P is disabled (set COPYPASTE_P2P=1): cannot accept a pairing QR \
                     over the network without an mTLS certificate",
                )
            }
        };

        let payload = match copypaste_core::PairingPayload::decode(qr) {
            Ok(p) => p,
            Err(e) => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    format!("failed to decode pairing QR: {e}"),
                )
            }
        };

        let password = payload.token.to_pake_password();

        // Resolve the responder's address: addr_hint is primary; fall back to
        // mDNS resolution by device_id when it is empty (best-effort — loopback
        // mDNS is unreliable, see discovery::resolve_peer).
        let addr = match self.resolve_pairing_addr(&payload) {
            Ok(addr) => addr,
            Err(msg) => return Response::err_with_code(req_id, ERR_CODE_INVALID_ARGUMENT, msg),
        };

        let (cert_der, key_der) = (cert.0.clone(), cert.1.clone());
        // Our own P2P sync-listener address, sent in-band so the responder can
        // persist it for its Phase 3 connector.
        let own_sync_addr = self.own_sync_addr();
        match copypaste_p2p::bootstrap::run_initiator(
            addr,
            cert_der,
            key_der,
            &password,
            &own_sync_addr,
        )
        .await
        {
            Ok(outcome) => {
                tracing::info!(
                    peer_fingerprint = %outcome.peer_fingerprint,
                    peer_sync_addr = %outcome.peer_sync_addr,
                    "bootstrap PAKE initiator completed over network channel"
                );
                if let Some(ref peers) = self.p2p_peers {
                    peers.rotate_peer(
                        &outcome.peer_fingerprint,
                        outcome.peer_fingerprint.clone(),
                        String::new(),
                    );
                }
                // P2P Phase 2: durably persist the peer (fingerprint + the
                // sync-listener address it advertised) for restart-survival and
                // the Phase 3 outbound connector.
                Self::persist_paired_peer(
                    &outcome.peer_fingerprint,
                    &outcome.peer_sync_addr,
                    &outcome.session_key,
                );
                Response::ok(
                    req_id,
                    serde_json::json!({
                        "ok": true,
                        "peer_fingerprint": outcome.peer_fingerprint,
                    }),
                )
            }
            Err(e) => Response::err_with_code(
                req_id,
                ERR_CODE_AUTH_FAILED,
                format!("network PAKE pairing failed: {e}"),
            ),
        }
    }

    /// Resolve the responder's socket address for the initiator bootstrap dial.
    ///
    /// Uses the QR `addr_hint` when present; otherwise falls back to mDNS
    /// `resolve_peer` keyed by the QR's `device_id`. Returns a human-readable
    /// error string when neither yields a usable address.
    fn resolve_pairing_addr(
        &self,
        payload: &copypaste_core::PairingPayload,
    ) -> Result<std::net::SocketAddr, String> {
        if !payload.addr_hint.is_empty() {
            return payload
                .addr_hint
                .parse::<std::net::SocketAddr>()
                .map_err(|e| format!("invalid addr_hint '{}': {e}", payload.addr_hint));
        }

        // mDNS fallback (best-effort).
        let discovery = self
            .discovery
            .as_ref()
            .ok_or_else(|| "QR has no addr_hint and mDNS discovery is unavailable".to_string())?;
        let peer = discovery
            .resolve_peer(&payload.device_id)
            .ok_or_else(|| "QR has no addr_hint and the peer was not found via mDNS".to_string())?;
        let ip = peer
            .ip_addrs
            .first()
            .ok_or_else(|| "mDNS-resolved peer has no IP address".to_string())?;
        Ok(std::net::SocketAddr::new(*ip, peer.port))
    }

    /// Returns true if a request to `method` requires the backing database.
    /// Methods that only touch in-memory state (status, get/set_private_mode,
    /// get_own_fingerprint, peer file ops, config file ops) are allowed
    /// before the DB is ready so the client can still introspect the daemon.
    fn requires_db(method: &str) -> bool {
        matches!(
            method,
            "list"
                | "delete"
                | "count"
                | "search"
                | "copy"
                | "paste"
                | "copy_item"
                | "delete_all"
                | "delete_item"
                | "stats"
                | "pin"
                | "pin_item"
                | "history_page"
                | "import"
                | "revoke_peer"
                | "revoke_all_peers"
        )
    }

    /// Run the IPC accept loop until `shutdown` is cancelled.
    ///
    /// D2: accepts a [`CancellationToken`] so the daemon can stop the server
    /// cleanly on SIGINT/SIGTERM instead of relying on task abort.
    pub async fn serve(
        self,
        socket_path: &std::path::Path,
        shutdown: CancellationToken,
    ) -> anyhow::Result<()> {
        // T4 (v0.3) — make sure the `revoked_devices` audit table exists
        // before any client can call `revoke_peer`. The DDL is purely
        // additive (`CREATE TABLE IF NOT EXISTS`) and does NOT bump the
        // SQLite `user_version`, keeping us out of the HKDF v2 worker's
        // schema-migration territory.
        {
            let db = self.db.lock().await;
            if let Err(e) = ensure_revoked_devices_table(db.conn()) {
                tracing::error!(
                    "failed to ensure revoked_devices table: {e} — \
                     revoke_peer requests will fail until this is fixed"
                );
            }
        }

        // Ensure parent directory exists and is user-only (0o700) so that the
        // socket cannot be reached by other local users even if the socket
        // mode itself were ever loosened.
        if let Some(parent) = socket_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
        }

        // Self-heal stale sockets. A previous daemon that crashed or was
        // killed (e.g. a v0.3.4 process replaced by a v0.4.0 upgrade) leaves
        // the on-disk socket file behind. A plain `bind` over an existing path
        // fails with `EADDRINUSE`, so the new daemon would never come up and
        // the UI would see "process alive but socket not reachable". We probe
        // the existing socket first: if NO live listener answers it, it is a
        // stale file we may safely remove and rebind. If a live listener DOES
        // answer, another healthy daemon already owns it — we must NOT steal
        // the socket out from under it, so we surface a hard error instead.
        let listener = bind_with_stale_cleanup(socket_path)?;

        // chmod 0600 — the IPC socket gives full control over the user's
        // clipboard history and peer database. It must not be world- or
        // group-connectable. Done immediately after bind, before accept loop.
        std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600))?;

        tracing::info!("IPC listening on {} (mode=0600)", socket_path.display());

        let server = Arc::new(self);
        // daemon-core L2: track in-flight per-connection tasks in a JoinSet so
        // they can be aborted on shutdown instead of being orphaned. Previously
        // each `tokio::spawn` was fire-and-forget: on `shutdown.cancelled()` the
        // accept loop returned while connection tasks kept running (benign today
        // since the process exits shortly after, but it leaked tasks that could
        // hold the DB Mutex past the documented drain point).
        let mut conns: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                // D2: stop accepting new connections on daemon-wide shutdown.
                _ = shutdown.cancelled() => {
                    tracing::info!("IPC server: shutdown signal received, stopping accept loop");
                    break;
                }
                // Reap finished connection tasks so the JoinSet does not grow
                // unbounded over the daemon's lifetime. `join_next` resolves to
                // `None` only when the set is empty, in which case this branch is
                // disabled by the `if` guard and never busy-loops.
                _ = conns.join_next(), if !conns.is_empty() => {}
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            let s = server.clone();
                            conns.spawn(async move {
                                if let Err(e) = s.handle_connection(stream).await {
                                    tracing::warn!("IPC connection error: {e}");
                                }
                            });
                        }
                        Err(e) => tracing::error!("accept error: {e}"),
                    }
                }
            }
        }
        // daemon-core L2: abort any still-running connection tasks. The daemon's
        // drain step (`_ipc_handle.await` in daemon.rs) then completes promptly
        // instead of waiting on a client that never closes its socket.
        conns.abort_all();
        while conns.join_next().await.is_some() {}
        Ok(())
    }

    #[tracing::instrument(skip_all, name = "ipc_connection")]
    async fn handle_connection(&self, stream: UnixStream) -> anyhow::Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut buf: Vec<u8> = Vec::with_capacity(4 * 1024);

        loop {
            buf.clear();
            // Bound the read: at most MAX_REQUEST_BYTES + 1 so we can distinguish
            // "exactly the limit" from "exceeded the limit".
            let mut limited = (&mut reader).take((MAX_REQUEST_BYTES as u64) + 1);
            let n = match limited.read_until(b'\n', &mut buf).await {
                Ok(n) => n,
                Err(e) => {
                    tracing::warn!("ipc read error: {e}");
                    return Ok(());
                }
            };

            // Clean EOF — client closed the socket without sending more data.
            if n == 0 {
                return Ok(());
            }

            // Oversized request: read more than MAX_REQUEST_BYTES without
            // finding a newline. Reject with an error response, then close.
            if n > MAX_REQUEST_BYTES {
                tracing::warn!(
                    "ipc request exceeded {MAX_REQUEST_BYTES} bytes (read {n}); rejecting and closing"
                );
                let resp = Response::err("0", "request too large");
                if let Ok(mut out) = serde_json::to_string(&resp) {
                    out.push('\n');
                    let _ = writer.write_all(out.as_bytes()).await;
                }
                return Ok(());
            }

            // Trim trailing \n (and any stray \r) before dispatch.
            while matches!(buf.last(), Some(b'\n' | b'\r')) {
                buf.pop();
            }

            // Empty line — skip silently (treat as keep-alive / no-op).
            if buf.is_empty() {
                continue;
            }

            let line = match std::str::from_utf8(&buf) {
                Ok(s) => s,
                Err(e) => {
                    let resp = Response::err("0", format!("invalid UTF-8: {e}"));
                    if let Ok(mut out) = serde_json::to_string(&resp) {
                        out.push('\n');
                        let _ = writer.write_all(out.as_bytes()).await;
                    }
                    continue;
                }
            };

            let resp = self.dispatch(line).await;
            let mut out = serde_json::to_string(&resp)?;
            out.push('\n');
            if let Err(e) = writer.write_all(out.as_bytes()).await {
                // Client disconnected mid-response — log and exit cleanly,
                // do not panic the spawned task.
                tracing::debug!("ipc write failed (client disconnected): {e}");
                return Ok(());
            }
        }
    }

    #[tracing::instrument(skip(self), fields(method), name = "ipc_dispatch")]
    async fn dispatch(&self, line: &str) -> Response {
        let req: Request = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => return Response::err("?", format!("parse error: {e}")),
        };

        tracing::Span::current().record("method", req.method.as_str());
        tracing::debug!(method = %req.method, id = %req.id, "IPC request");

        // Protocol-version gate (ADR-007) — reject before touching any
        // method-specific logic so clients get a deterministic upgrade signal.
        if req.protocol_version < MIN_SUPPORTED_PROTOCOL_VERSION
            || req.protocol_version > CURRENT_PROTOCOL_VERSION
        {
            tracing::warn!(
                method = %req.method,
                id = %req.id,
                client_version = req.protocol_version,
                supported = format!("{MIN_SUPPORTED_PROTOCOL_VERSION}..={CURRENT_PROTOCOL_VERSION}"),
                "rejecting request: unsupported protocol version"
            );
            return Response::err_with_code(
                req.id,
                ERR_CODE_INVALID_ARGUMENT,
                format!(
                    "unsupported protocol version {} (daemon supports {}..={})",
                    req.protocol_version, MIN_SUPPORTED_PROTOCOL_VERSION, CURRENT_PROTOCOL_VERSION
                ),
            );
        }

        // Readiness gate — reject DB-touching methods before init is done.
        if !self.ready.load(Ordering::Relaxed) && Self::requires_db(req.method.as_str()) {
            tracing::debug!(
                method = %req.method,
                id = %req.id,
                "rejecting DB-touching request: server not ready"
            );
            return Response::err_with_code(req.id, ERR_CODE_IPC_NOT_READY, ERR_IPC_NOT_READY);
        }

        match req.method.as_str() {
            "list" => {
                let raw_limit = req
                    .params
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(50) as usize;
                let limit = raw_limit.min(MAX_PAGE);
                let offset = req
                    .params
                    .get("offset")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                let db_arc = self.db.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let items = get_page(&db, limit, offset)?;
                    let total = count_items(&db).unwrap_or(0);
                    Ok::<_, anyhow::Error>((items, total))
                })
                .await;
                match join {
                    Ok(Ok((items, total))) => {
                        let json_items: Vec<_> = items
                            .iter()
                            .map(|item| {
                                serde_json::json!({
                                    "id": item.id,
                                    "content_type": item.content_type,
                                    "is_sensitive": item.is_sensitive,
                                    "wall_time": item.wall_time,
                                    "lamport_ts": item.lamport_ts,
                                })
                            })
                            .collect();
                        Response::ok(
                            req.id,
                            serde_json::json!({"items": json_items, "total": total}),
                        )
                    }
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "delete" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: id"),
                };
                if uuid::Uuid::parse_str(&id).is_err() {
                    return Response::err(req.id, "invalid param: id must be a valid UUID");
                }
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let del = delete_item(&db, &id_for_task);
                    // Best-effort FTS cleanup; surface as warning, not failure
                    let fts = delete_fts(&db, &id_for_task);
                    (del, fts)
                })
                .await;
                match join {
                    Ok((Ok(_), fts_res)) => {
                        if let Err(e) = fts_res {
                            tracing::warn!("fts delete failed for id={id}: {e}");
                        }
                        Response::ok(req.id, serde_json::Value::Null)
                    }
                    Ok((Err(e), _)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "count" => {
                let db_arc = self.db.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    count_items(&db)
                })
                .await;
                match join {
                    Ok(Ok(n)) => Response::ok(req.id, serde_json::json!({"count": n})),
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "search" => {
                let query = match req.params.get("query").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: query"),
                };
                // Clamp to MAX_PAGE like `list` / `history_page` so an oversized
                // `limit` cannot make `search_items` allocate/scan unbounded rows.
                let limit = (req
                    .params
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize)
                    .min(MAX_PAGE);

                let db_arc = self.db.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    search_items(&db, &query, limit)
                })
                .await;
                match join {
                    Ok(Ok(items)) => {
                        let json_items: Vec<_> = items
                            .iter()
                            .map(|item| {
                                serde_json::json!({
                                    "id": item.id,
                                    "content_type": item.content_type,
                                    "is_sensitive": item.is_sensitive,
                                    "wall_time": item.wall_time,
                                    "lamport_ts": item.lamport_ts,
                                })
                            })
                            .collect();
                        Response::ok(req.id, serde_json::json!({"items": json_items}))
                    }
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "copy" | "paste" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: id"),
                };
                if uuid::Uuid::parse_str(&id).is_err() {
                    return Response::err(req.id, "invalid param: id must be a valid UUID");
                }
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    // Resolve directly by primary key — paging + linear scan
                    // silently missed any item past position 1000 (data loss).
                    let item = get_item_by_id(&db, &id_for_task)?;
                    Ok::<_, anyhow::Error>(item)
                })
                .await;
                match join {
                    Ok(Ok(Some(item))) => match self.write_to_pasteboard(&item) {
                        Ok(()) => {
                            // C. PROMOTE-ON-COPY: bump wall_time/lamport so this
                            // item sorts to the top of history_page on the next
                            // request, matching Maccy-style recency ordering.
                            let db_arc2 = self.db.clone();
                            let item_id_bump = item.id.clone();
                            let _ = tokio::task::spawn_blocking(move || {
                                let db = db_arc2.blocking_lock();
                                let now_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_millis() as i64)
                                    .unwrap_or(0);
                                let _ = bump_item_recency(&db, &item_id_bump, now_ms, now_ms);
                            })
                            .await;
                            Response::ok(
                                req.id,
                                serde_json::json!({
                                    "id": item.id,
                                    "content_type": item.content_type,
                                    "written": true,
                                }),
                            )
                        }
                        Err(PasteboardError::DecryptFailed(msg)) => Response::err_with_code(
                            req.id,
                            ERR_CODE_AUTH_FAILED,
                            format!("paste decrypt failed: {msg}"),
                        ),
                        Err(PasteboardError::Other(msg)) => {
                            Response::err(req.id, format!("pasteboard write failed: {msg}"))
                        }
                    },
                    Ok(Ok(None)) => Response::err(req.id, format!("item not found: {id}")),
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "delete_all" => {
                let db_arc = self.db.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    // Atomically delete every row from both tables inside a
                    // single transaction so the history is never half-cleared
                    // and FTS never drifts from clipboard_items.
                    let conn = db.conn();
                    let tx = conn.unchecked_transaction()?;
                    let deleted = tx.execute("DELETE FROM clipboard_items", [])?;
                    // Best-effort FTS purge — a failure here rolls back the
                    // outer delete too so the two tables stay consistent.
                    tx.execute("DELETE FROM clipboard_fts", [])?;
                    tx.commit()?;
                    Ok::<_, rusqlite::Error>(deleted)
                })
                .await;
                match join {
                    Ok(Ok(deleted)) => {
                        Response::ok(req.id, serde_json::json!({"deleted": deleted}))
                    }
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "stats" => {
                let db_arc = self.db.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let total = copypaste_core::count_items(&db).unwrap_or(0);
                    // Count sensitive items via get_page scan (limited to first 1000)
                    let sample = copypaste_core::get_page(&db, 1000, 0).unwrap_or_default();
                    let sensitive_count = sample.iter().filter(|i| i.is_sensitive).count() as i64;
                    (total, sensitive_count)
                })
                .await;
                match join {
                    Ok((total, sensitive_count)) => Response::ok(
                        req.id,
                        serde_json::json!({
                            "total_items": total,
                            "sensitive_items": sensitive_count,
                            "version": "1"
                        }),
                    ),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "pin" => {
                // Pin an item (remove expiry so it's never auto-deleted)
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: id"),
                };
                if uuid::Uuid::parse_str(&id).is_err() {
                    return Response::err(req.id, "invalid param: id must be a valid UUID");
                }
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    copypaste_core::pin_item(&db, &id_for_task)
                })
                .await;
                match join {
                    Ok(Ok(())) => {
                        Response::ok(req.id, serde_json::json!({"pinned": true, "id": id}))
                    }
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            // T5.x — pin or unpin an item by id. Unlike the legacy `pin`
            // verb (pin-only), this takes an explicit `pinned: bool` so the
            // UI can toggle from a single callback. A `pinned=false` request
            // clears the pin flag (restoring normal TTL behaviour).
            "pin_item" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: id",
                        )
                    }
                };
                if uuid::Uuid::parse_str(&id).is_err() {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "invalid param: id must be a valid UUID",
                    );
                }
                let pinned = match req.params.get("pinned").and_then(|v| v.as_bool()) {
                    Some(b) => b,
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: pinned (bool)",
                        )
                    }
                };
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    if pinned {
                        pin_item(&db, &id_for_task)
                    } else {
                        unpin_item(&db, &id_for_task)
                    }
                })
                .await;
                match join {
                    Ok(Ok(())) => {
                        Response::ok(req.id, serde_json::json!({"pinned": pinned, "id": id}))
                    }
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            // T5.x — delete a single item by id. Mirrors the legacy `delete`
            // verb but uses the typed `invalid_argument` error code (the UI
            // branches on `error_code`) and returns a structured `{deleted,
            // id}` payload. FTS cleanup is best-effort (logged on failure).
            "delete_item" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: id",
                        )
                    }
                };
                if uuid::Uuid::parse_str(&id).is_err() {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "invalid param: id must be a valid UUID",
                    );
                }
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let del = delete_item(&db, &id_for_task);
                    let fts = delete_fts(&db, &id_for_task);
                    (del, fts)
                })
                .await;
                match join {
                    Ok((Ok(removed), fts_res)) => {
                        if let Err(e) = fts_res {
                            tracing::warn!("fts delete failed for id={id}: {e}");
                        }
                        // Report whether a row was actually removed so the
                        // response matches reality: `deleted: false` for an id
                        // that did not exist, instead of always claiming `true`.
                        Response::ok(
                            req.id,
                            serde_json::json!({"deleted": removed > 0, "id": id}),
                        )
                    }
                    Ok((Err(e), _)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            // T5.x — copy an item back to the system clipboard by id. Same
            // paste-back path as `copy`/`paste` (decrypt → NSPasteboard) but
            // surfaces typed `invalid_argument` / `not_found` error codes so
            // the UI can branch on `error_code` rather than parsing strings.
            "copy_item" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: id",
                        )
                    }
                };
                if uuid::Uuid::parse_str(&id).is_err() {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "invalid param: id must be a valid UUID",
                    );
                }
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    // Resolve the row directly by primary key. Previously this
                    // paged `get_page(1000, 0)` and linear-scanned, so any item
                    // beyond position 1000 silently returned `not_found`
                    // (data-loss for power users). `get_item_by_id` is a single
                    // indexed `SELECT ... WHERE id = ?1` with no window cap.
                    let item = get_item_by_id(&db, &id_for_task)?;
                    Ok::<_, anyhow::Error>(item)
                })
                .await;
                match join {
                    Ok(Ok(Some(item))) => match self.write_to_pasteboard(&item) {
                        Ok(()) => {
                            // C. PROMOTE-ON-COPY: bump wall_time/lamport so this
                            // item sorts to the top of history_page on the next
                            // request, matching Maccy-style recency ordering.
                            let db_arc2 = self.db.clone();
                            let item_id_bump = item.id.clone();
                            let _ = tokio::task::spawn_blocking(move || {
                                let db = db_arc2.blocking_lock();
                                let now_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_millis() as i64)
                                    .unwrap_or(0);
                                let _ = bump_item_recency(&db, &item_id_bump, now_ms, now_ms);
                            })
                            .await;
                            Response::ok(
                                req.id,
                                serde_json::json!({
                                    "id": item.id,
                                    "content_type": item.content_type,
                                    "written": true,
                                }),
                            )
                        }
                        Err(PasteboardError::DecryptFailed(msg)) => Response::err_with_code(
                            req.id,
                            ERR_CODE_AUTH_FAILED,
                            format!("paste decrypt failed: {msg}"),
                        ),
                        Err(PasteboardError::Other(msg)) => {
                            Response::err(req.id, format!("pasteboard write failed: {msg}"))
                        }
                    },
                    Ok(Ok(None)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_NOT_FOUND,
                        format!("item not found: {id}"),
                    ),
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            // A. get_item_image — decrypt and return an IMAGE item as a data URI.
            //
            // Params: {"id": "<uuid>"}
            // Success: {"data_uri": "data:<content_type>;base64,<b64>"}
            // Error: item not found, non-image content_type, or decrypt failure.
            //
            // Reuses the same chunk-decrypt path as write_to_pasteboard for images
            // (chunks_from_blob → decode_image → PNG bytes), then base64-encodes
            // the raw PNG bytes for the UI to render as a thumbnail without having
            // to hit the pasteboard.
            "get_item_image" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: id",
                        )
                    }
                };
                if uuid::Uuid::parse_str(&id).is_err() {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "invalid param: id must be a valid UUID",
                    );
                }
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let item = get_item_by_id(&db, &id_for_task)?;
                    Ok::<_, anyhow::Error>(item)
                })
                .await;
                match join {
                    Ok(Ok(Some(item))) => {
                        // Only IMAGE items are supported; content_type == "image"
                        // (legacy) or starts with "image/" (MIME-typed future rows).
                        let is_image =
                            item.content_type == "image" || item.content_type.starts_with("image/");
                        if !is_image {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                format!(
                                    "item {} is not an image (content_type: {})",
                                    id, item.content_type
                                ),
                            );
                        }
                        let content = match &item.content {
                            Some(b) => b.clone(),
                            None => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INTERNAL_ERROR,
                                    format!("image item {} has no content blob", id),
                                )
                            }
                        };
                        let meta_json = match item.blob_ref.as_deref() {
                            Some(s) => s.to_owned(),
                            None => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INTERNAL_ERROR,
                                    format!("image item {} missing blob_ref metadata", id),
                                )
                            }
                        };
                        let file_id = match parse_image_file_id(&meta_json) {
                            Ok(fid) => fid,
                            Err(e) => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INTERNAL_ERROR,
                                    format!("image item {id} blob_ref parse error: {e}"),
                                )
                            }
                        };
                        let chunks = match chunks_from_blob(&content) {
                            Ok(c) => c,
                            Err(e) => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INTERNAL_ERROR,
                                    format!("image item {id} chunks_from_blob failed: {e}"),
                                )
                            }
                        };
                        let local_key: [u8; 32] = **self.local_key;
                        let png_bytes = match decode_image(&chunks, &local_key, &file_id) {
                            Ok(b) => b,
                            Err(e) => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_AUTH_FAILED,
                                    format!("image item {id} decode failed: {e}"),
                                )
                            }
                        };
                        use base64::Engine as _;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
                        // The stored content_type is "image" (legacy) or a real
                        // MIME type. For the data URI we always emit "image/png"
                        // because decode_image always returns PNG bytes.
                        let data_uri = format!("data:image/png;base64,{b64}");
                        Response::ok(req.id, serde_json::json!({ "data_uri": data_uri }))
                    }
                    Ok(Ok(None)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_NOT_FOUND,
                        format!("item not found: {id}"),
                    ),
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "history_page" => {
                // Paginated history with content preview — used by UI (HistoryWindow)
                let raw_limit = req
                    .params
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(50) as usize;
                let limit = raw_limit.min(MAX_PAGE);
                let offset = req
                    .params
                    .get("offset")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                let db_arc = self.db.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    // Use pinned-first ordering: pinned items always appear at
                    // the top, then unpinned items ordered newest-first.
                    let items = get_page_pinned_first(&db, limit, offset)?;
                    let total = count_items(&db).unwrap_or(0);
                    // Build previews inside the blocking task while `db` is
                    // still held.  Text items: read from the FTS5 plaintext
                    // index (capped at MAX_PREVIEW_BYTES = 1 KiB).
                    // Image items: return a placeholder (full preview in v0.4).
                    // Sensitive items: never expose plaintext in list view.
                    //
                    // Fix SENSITIVE-SPAN #38: for non-sensitive text items,
                    // run the sensitive detector against the preview string and
                    // include `sensitive_spans: [[start,end],...]` (char offsets
                    // into the preview) so the UI can redact just the secret
                    // substrings rather than masking the whole row. Only exposed
                    // on text items where `is_sensitive == false` — items already
                    // flagged sensitive have their preview suppressed entirely.
                    let detector = SensitiveDetector::new();
                    let json_items: Vec<serde_json::Value> = items
                        .iter()
                        .map(|item| {
                            let preview = if item.is_sensitive {
                                format!("[sensitive — id:{}]", &item.id[..8])
                            } else if item.content_type == "text" {
                                fetch_text_preview(&db, &item.id)
                                    .unwrap_or(None)
                                    .unwrap_or_else(|| format!("[text — id:{}]", &item.id[..8]))
                            } else {
                                // image (and any future non-text type)
                                format!("[image — id:{}]", &item.id[..8])
                            };

                            // Compute sensitive_spans for non-sensitive text items.
                            // We run the detector against the *preview* (the same
                            // UTF-8 string the UI will display) so the char offsets
                            // are correct without the UI having to re-detect.
                            // For sensitive items the spans are empty — the whole
                            // preview is already replaced with a placeholder.
                            let sensitive_spans: Vec<serde_json::Value> = if !item.is_sensitive
                                && item.content_type == "text"
                            {
                                // `detector.detect` returns byte ranges over the
                                // NFKC-NORMALISED form of its input, NOT over the
                                // string we pass. Slicing the original `preview`
                                // with those byte offsets panicked whenever NFKC
                                // changed widths (ligatures, full-width forms),
                                // because the offsets could land mid-char or past
                                // the end. Run the detector against the SAME string
                                // we slice (the NFKC form), then map byte offsets
                                // to char offsets with `byte_to_char_offset`, which
                                // clamps to a valid char boundary and never panics.
                                let normalised =
                                    copypaste_core::sensitive::nfkc_normalize(&preview);
                                detector
                                    .detect(&normalised)
                                    .into_iter()
                                    .map(|m| {
                                        let start =
                                            byte_to_char_offset(&normalised, m.matched_range.start);
                                        let end =
                                            byte_to_char_offset(&normalised, m.matched_range.end);
                                        serde_json::json!([start, end])
                                    })
                                    .collect()
                            } else {
                                vec![]
                            };

                            serde_json::json!({
                                "id": item.id,
                                "content_type": item.content_type,
                                "is_sensitive": item.is_sensitive,
                                "wall_time": item.wall_time,
                                "lamport_ts": item.lamport_ts,
                                "preview": preview,
                                "pinned": item.pinned,
                                "sensitive_spans": sensitive_spans,
                            })
                        })
                        .collect();
                    Ok::<_, anyhow::Error>((json_items, total))
                })
                .await;
                match join {
                    Ok(Ok((json_items, total))) => Response::ok(
                        req.id,
                        serde_json::json!({"items": json_items, "total": total}),
                    ),
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "get_config" => {
                // Never ship account credentials over IPC. `get_config` feeds
                // the UI settings form and the CLI's read-merge-write in
                // `cloud setup`; neither needs the raw GoTrue password or email
                // back (the CLI re-supplies both on every `set_config`, the UI
                // does not surface them at all). `redact_config_secrets`
                // replaces them with boolean presence flags. The Supabase
                // anon/public key is, by design, a publishable key and is kept
                // so the UI can prefill the settings field.
                let cfg = read_config();
                match serde_json::to_value(&cfg) {
                    Ok(mut v) => {
                        redact_config_secrets(&mut v);
                        Response::ok(req.id, v)
                    }
                    Err(e) => Response::err(req.id, e.to_string()),
                }
            }
            "set_config" => {
                let cfg: AppConfig = match serde_json::from_value(req.params.clone()) {
                    Ok(c) => c,
                    Err(e) => return Response::err(req.id, format!("invalid config: {e}")),
                };
                match write_config(&cfg) {
                    Ok(()) => Response::ok(req.id, serde_json::json!({"saved": true})),
                    Err(e) => Response::err(req.id, e.to_string()),
                }
            }
            // Cloud auth — stubs until Supabase integration lands.
            // Route through `Response::not_implemented` so clients see a
            // machine-readable `error_code: "not_implemented"` instead of an
            // ambiguous `ok: true` carrying a "not yet implemented" note.
            "cloud_sign_in" => {
                tracing::info!("cloud_sign_in stub called");
                Response::not_implemented(req.id, "cloud-sync")
            }
            "cloud_sign_out" => {
                tracing::info!("cloud_sign_out stub called");
                Response::not_implemented(req.id, "cloud-sync")
            }

            // ── cloud-sync IPC methods ──────────────────────────────────────
            //
            // `set_sync_passphrase` and `get_sync_status` are the UI-facing
            // surface for the cross-device shared encryption key. Both are
            // compiled in only when the `cloud-sync` Cargo feature is active.
            #[cfg(feature = "cloud-sync")]
            "set_sync_passphrase" => {
                let passphrase = match req.params.get("passphrase").and_then(|v| v.as_str()) {
                    Some(p) if !p.is_empty() => p.to_owned(),
                    _ => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing or empty param: passphrase",
                        )
                    }
                };

                // Derive the sync key via Argon2id (this is intentionally slow —
                // one-time cost on passphrase entry, not per-item).
                let new_key = match derive_sync_key(&passphrase) {
                    Ok(k) => k,
                    Err(e) => {
                        tracing::warn!("set_sync_passphrase: key derivation failed: {e}");
                        return Response::err(req.id, format!("key derivation failed: {e}"));
                    }
                };

                // Persist the raw key bytes to the macOS Keychain under the
                // cloud-sync account so they survive a daemon restart.
                #[cfg(target_os = "macos")]
                if crate::keychain::keychain_bypassed() {
                    // Dev/test bypass: do not persist (would prompt / touch
                    // disk). The key stays active in-memory for this session.
                    tracing::debug!(
                        "set_sync_passphrase: COPYPASTE_EPHEMERAL_KEY set; skipping key persist"
                    );
                } else {
                    // Persist via the SAME backend the device key uses. On
                    // ad-hoc / unsigned installs that is the non-prompting
                    // 0600 file store — using the Keychain here would raise
                    // the login-password prompt that this change eliminates.
                    // See `keychain::signing` / `keychain::file_store`.
                    match crate::keychain::signing::choose_key_backend() {
                        crate::keychain::signing::KeyBackend::File => {
                            if let Err(e) = crate::keychain::file_store::store_cloud_sync_key(
                                new_key.as_bytes(),
                            ) {
                                tracing::warn!(
                                    "set_sync_passphrase: file-store persist failed ({e}); \
                                     key is active in-memory only until daemon restart"
                                );
                            }
                        }
                        crate::keychain::signing::KeyBackend::Keychain => {
                            use security_framework::passwords::set_generic_password;
                            if let Err(e) = set_generic_password(
                                crate::keychain::SERVICE,
                                crate::keychain::CLOUD_SYNC_ACCOUNT,
                                new_key.as_bytes(),
                            ) {
                                tracing::warn!(
                                    "set_sync_passphrase: keychain persist failed ({e}); \
                                     key is active in-memory only until daemon restart"
                                );
                            }
                        }
                    }
                }

                // Store in shared state so push/poll loops pick it up
                // immediately (they hold an Arc to the same Mutex).
                *self.sync_key.lock().await = Some(new_key);
                tracing::info!("set_sync_passphrase: sync key updated");
                Response::ok(req.id, serde_json::json!({"ok": true}))
            }

            #[cfg(feature = "cloud-sync")]
            "get_sync_status" => {
                let passphrase_set = self.sync_key.lock().await.is_some();
                let app_cfg = read_config();
                let supabase_configured = app_cfg.supabase_url.is_some()
                    && app_cfg.supabase_anon_key.is_some()
                    || std::env::var("SUPABASE_URL").is_ok();
                // BUG 2 fix: report the REAL GoTrue auth state published by the
                // cloud loops, not the old `signed_in = supabase_configured`
                // placeholder. The flag is set `true` once `start_cloud` resolves
                // a bearer and `false` on a bearer-resolution / 401-refresh
                // failure, so the UI no longer claims "signed in" after a
                // `CloudError::AuthFailed` aborted cloud sync.
                let signed_in = self
                    .cloud_signed_in
                    .load(std::sync::atomic::Ordering::Relaxed);
                let raw_ts = self.last_sync_ms.load(std::sync::atomic::Ordering::Relaxed);
                let last_sync_ms_val: Option<i64> = if raw_ts > 0 { Some(raw_ts) } else { None };
                // B. Expose the non-secret Supabase URL and email so the UI can
                // show/prefill them. We do NOT expose the anon key, password, or
                // passphrase. Priority: env vars override AppConfig (same as
                // CloudConfig::from_env).
                let supabase_url_val: Option<String> = std::env::var("SUPABASE_URL")
                    .ok()
                    .or_else(|| app_cfg.supabase_url.clone());
                // Email: env var first, else the persisted config (written by
                // `copypaste cloud setup`). We surface only the email — never the
                // password, anon key, or passphrase.
                let email_val: Option<String> = std::env::var("SUPABASE_EMAIL")
                    .ok()
                    .or_else(|| app_cfg.supabase_email.clone());
                Response::ok(
                    req.id,
                    serde_json::json!({
                        "passphrase_set": passphrase_set,
                        "supabase_configured": supabase_configured,
                        "signed_in": signed_in,
                        "last_sync_ms": last_sync_ms_val,
                        "supabase_url": supabase_url_val,
                        "email": email_val,
                    }),
                )
            }

            // `cloud_test_connection` validates the configured Supabase
            // credentials end-to-end so the UI/CLI can give a precise, actionable
            // diagnostic instead of leaving the user to guess why sync is silent.
            // It performs a single cheap `GET /rest/v1/clipboard_items?limit=0`
            // with the anon key (+ optional email/password bearer) and classifies
            // the outcome (URL reachable? key valid? table present? RLS ok?).
            #[cfg(feature = "cloud-sync")]
            "cloud_test_connection" => {
                let result = test_cloud_connection().await;
                Response::ok(req.id, result)
            }

            // When cloud-sync is not compiled in, return not_implemented so
            // the UI gets a machine-readable code rather than "method not found".
            #[cfg(not(feature = "cloud-sync"))]
            "set_sync_passphrase" | "get_sync_status" | "cloud_test_connection" => {
                Response::not_implemented(req.id, "cloud-sync")
            }
            "set_private_mode" => {
                let enabled = match req.params.get("enabled").and_then(|v| v.as_bool()) {
                    Some(b) => b,
                    None => return Response::err(req.id, "missing param: enabled (bool)"),
                };
                self.private_mode.store(enabled, Ordering::Relaxed);
                // Persist so the setting survives a daemon restart (restored by
                // `daemon::load_private_mode` at startup). Best-effort: the
                // in-memory atomic above is authoritative for this process.
                crate::daemon::persist_private_mode(enabled);
                tracing::info!("private mode set to {enabled}");
                Response::ok(req.id, serde_json::json!({"private_mode": enabled}))
            }
            "get_private_mode" => {
                let enabled = self.private_mode.load(Ordering::Relaxed);
                Response::ok(req.id, serde_json::json!({"private_mode": enabled}))
            }
            "status" => {
                let enabled = self.private_mode.load(Ordering::Relaxed);
                // In degraded startup the daemon is alive and the socket is
                // bound, but the backing DB is unavailable (e.g. the Keychain
                // SQLCipher key could not be read after a reinstall). Report
                // status="degraded" + a machine-readable reason + a flag so the
                // UI shows a recovery banner instead of treating the reachable
                // socket as "everything is fine". When healthy, `ready` is true
                // and `degraded_reason` is absent — unchanged shape for clients
                // that only read `status`/`private_mode`.
                match self.degraded_reason.as_deref() {
                    Some(reason) => Response::ok(
                        req.id,
                        serde_json::json!({
                            "status": "degraded",
                            "private_mode": enabled,
                            "ready": false,
                            "degraded": true,
                            "degraded_reason": reason,
                        }),
                    ),
                    None => Response::ok(
                        req.id,
                        serde_json::json!({
                            "status": "running",
                            "private_mode": enabled,
                            "ready": self.ready.load(Ordering::Relaxed),
                            "degraded": false,
                        }),
                    ),
                }
            }

            // ------------------------------------------------------------------
            // P2P IPC methods
            // ------------------------------------------------------------------
            "get_own_fingerprint" => {
                // CRITICAL-1 fix: advertise the live mTLS **certificate**
                // fingerprint — the value peers pin and the mTLS verifier
                // compares (`PeerTransport::fingerprint` / `fingerprint_of`) —
                // NOT the device-key fingerprint
                // (`keychain::own_fingerprint`, SHA-256 of the X25519 public
                // key). The latter is never compared by the mTLS allowlist, so
                // pinning it could never authenticate a channel.
                //
                // When P2P is disabled there is no running transport and thus
                // no cert to advertise; return a clear error rather than a
                // fingerprint that cannot authenticate anything.
                match self.cert_fingerprint.as_ref() {
                    Some(fingerprint) => {
                        Response::ok(req.id, serde_json::json!({ "fingerprint": fingerprint }))
                    }
                    None => Response::err(
                        req.id,
                        "P2P is disabled (set COPYPASTE_P2P=1): no mTLS certificate \
                         to advertise for pairing",
                    ),
                }
            }

            "list_peers" => match load_peers() {
                Ok(peers) => Response::ok(req.id, serde_json::json!({ "peers": peers })),
                Err(e) => Response::err(req.id, format!("failed to load peers: {e}")),
            },

            "pair_peer" => {
                let fingerprint = match req.params.get("fingerprint").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: fingerprint"),
                };
                let name = match req.params.get("name").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: name"),
                };

                if !is_valid_fingerprint(&fingerprint) {
                    return Response::err(
                        req.id,
                        format!("invalid fingerprint format: {fingerprint}"),
                    );
                }

                match load_peers() {
                    Ok(mut peers) => {
                        // Check for duplicates
                        let already_paired = peers.iter().any(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| f == fingerprint)
                                .unwrap_or(false)
                        });
                        if already_paired {
                            return Response::err(
                                req.id,
                                format!("peer already paired: {fingerprint}"),
                            );
                        }

                        let added_at = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        peers.push(serde_json::json!({
                            "name": name,
                            "fingerprint": fingerprint,
                            "added_at": added_at,
                        }));

                        match save_peers(&peers) {
                            Ok(_) => Response::ok(req.id, serde_json::json!({ "ok": true })),
                            Err(e) => Response::err(req.id, format!("failed to save peers: {e}")),
                        }
                    }
                    Err(e) => Response::err(req.id, format!("failed to load peers: {e}")),
                }
            }

            "unpair_peer" => {
                let fingerprint = match req.params.get("fingerprint").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: fingerprint"),
                };

                match load_peers() {
                    Ok(mut peers) => {
                        let before_len = peers.len();
                        peers.retain(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| f != fingerprint)
                                .unwrap_or(true)
                        });
                        let removed = peers.len() < before_len;

                        match save_peers(&peers) {
                            Ok(_) => Response::ok(
                                req.id,
                                serde_json::json!({ "ok": true, "removed": removed }),
                            ),
                            Err(e) => Response::err(req.id, format!("failed to save peers: {e}")),
                        }
                    }
                    Err(e) => Response::err(req.id, format!("failed to load peers: {e}")),
                }
            }

            // T4 (v0.3) — manual peer revocation. Atomic with respect to the
            // user: a single click both (a) removes the peer from the local
            // JSON peer store so future sync attempts won't re-discover the
            // device by name, and (b) writes a row to the SQLite
            // `revoked_devices` audit table. The v1.0 cryptographic
            // revocation protocol will later consume that table to broadcast
            // revocation markers. For v0.3 the audit row is the only durable
            // record — mTLS rejection on unknown fingerprint is what blocks
            // the revoked peer from continuing to sync.
            "revoke_peer" => {
                let fingerprint = match req.params.get("fingerprint").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: fingerprint",
                        )
                    }
                };
                if !is_valid_fingerprint(&fingerprint) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("invalid fingerprint format: {fingerprint}"),
                    );
                }

                // Capture the peer's display name *before* deleting so the
                // audit row preserves the human-readable label. Falls back
                // to an empty string if the peer wasn't in the store
                // (revoking an unknown fingerprint is allowed — useful when
                // the local peer list is out of sync with reality).
                let (removed, captured_name) = match load_peers() {
                    Ok(mut peers) => {
                        let before_len = peers.len();
                        let name = peers
                            .iter()
                            .find(|p| {
                                p.get("fingerprint")
                                    .and_then(|v| v.as_str())
                                    .map(|f| f == fingerprint)
                                    .unwrap_or(false)
                            })
                            .and_then(|p| p.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        peers.retain(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| f != fingerprint)
                                .unwrap_or(true)
                        });
                        if let Err(e) = save_peers(&peers) {
                            return Response::err(req.id, format!("failed to save peers: {e}"));
                        }
                        (peers.len() < before_len, name)
                    }
                    Err(e) => return Response::err(req.id, format!("failed to load peers: {e}")),
                };

                // Write the audit row. Done on the blocking thread pool
                // because rusqlite is sync; the mutex is held only for the
                // duration of the two short statements inside
                // `revoke_device`.
                let db_arc = self.db.clone();
                let fp_for_db = fingerprint.clone();
                let name_for_db = captured_name.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    revoke_device(db.conn(), &fp_for_db, &name_for_db)
                })
                .await;

                match join {
                    Ok(Ok(revoked_at)) => Response::ok(
                        req.id,
                        serde_json::json!({
                            "ok": true,
                            "removed": removed,
                            "revoked_at": revoked_at,
                            "fingerprint": fingerprint,
                        }),
                    ),
                    Ok(Err(e)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("failed to record revocation: {e}"),
                    ),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("revoke task join error: {e}"),
                    ),
                }
            }

            // T5.x — revoke ALL paired peers in one call (Settings →
            // "Reset pairings"). Clears the local JSON peer store and writes
            // a `revoked_devices` audit row for each peer, reusing the same
            // single-peer `revoke_device` primitive. An empty store is a
            // success returning `{revoked: 0}` rather than an error.
            "revoke_all_peers" => {
                // Snapshot the current peers (fingerprint + display name)
                // before clearing the store so we can write audit rows.
                let peers = match load_peers() {
                    Ok(p) => p,
                    Err(e) => return Response::err(req.id, format!("failed to load peers: {e}")),
                };
                let captured: Vec<(String, String)> = peers
                    .iter()
                    .filter_map(|p| {
                        let fp = p.get("fingerprint").and_then(|v| v.as_str())?.to_string();
                        let name = p
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        Some((fp, name))
                    })
                    .collect();

                // Write every audit row in a single transaction FIRST, and only
                // clear the JSON peer store once that transaction has durably
                // committed. The previous order (clear store → loop inserting
                // audit rows, swallowing per-row errors) could leave the store
                // empty with audit rows missing on a partial failure, with the
                // loss only logged. With this order a failure leaves *both*
                // stores untouched so the caller can safely retry.
                let db_arc = self.db.clone();
                let captured_for_db = captured.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    revoke_devices(db.conn(), &captured_for_db)
                })
                .await;

                let revoked_at = match join {
                    Ok(Ok(ts)) => ts,
                    Ok(Err(e)) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("failed to record revocations: {e}"),
                        )
                    }
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("revoke_all task join error: {e}"),
                        )
                    }
                };

                // Audit log committed — now clear the local peer store. If this
                // fails the audit rows are already durable (idempotent on a
                // retry via the UPSERT), so we surface the error rather than
                // silently leaving stale peers behind.
                if let Err(e) = save_peers(&[]) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("revocations recorded but failed to clear peers: {e}"),
                    );
                }

                Response::ok(
                    req.id,
                    serde_json::json!({
                        "ok": true,
                        "revoked": captured.len(),
                        "cleared": captured.len(),
                        "revoked_at": revoked_at,
                    }),
                )
            }

            // W2.4 — PAKE-based password pairing (initiator side).
            //
            // Two-step protocol over IPC:
            //   step="initiate": validates inputs, creates PakeInitiator,
            //     stores session in pake_sessions, returns {session_id, message1_b64}.
            //   step="finish": looks up PakeInitiator by session_id, completes
            //     handshake with server's message2, stores peer, returns
            //     {ok: true, message3_b64}.
            "pair_peer_with_password" => {
                use base64::Engine as _;
                let b64 = base64::engine::general_purpose::STANDARD;

                let peer_fingerprint =
                    match req.params.get("peer_fingerprint").and_then(|v| v.as_str()) {
                        Some(s) => s.to_string(),
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "missing peer_fingerprint",
                            )
                        }
                    };

                if !is_valid_fingerprint(&peer_fingerprint) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("invalid peer_fingerprint format: {peer_fingerprint}"),
                    );
                }

                let step = req
                    .params
                    .get("step")
                    .and_then(|v| v.as_str())
                    .unwrap_or("initiate")
                    .to_string();

                match step.as_str() {
                    "initiate" => {
                        let password = match req.params.get("password").and_then(|v| v.as_str()) {
                            Some(s) => s.to_string(),
                            None => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INVALID_ARGUMENT,
                                    "missing password",
                                )
                            }
                        };

                        if password.chars().count() < 6 {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "password must be at least 6 characters",
                            );
                        }

                        let (initiator, msg1_bytes) = match PakeInitiator::new(&password) {
                            Ok(pair) => pair,
                            Err(e) => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INTERNAL_ERROR,
                                    format!("PAKE init failed: {e}"),
                                )
                            }
                        };

                        let session_id = uuid::Uuid::new_v4().to_string();
                        let msg1_b64 = b64.encode(&msg1_bytes);

                        if let Err(msg) = self
                            .insert_pake_session(
                                session_id.clone(),
                                PakeSession::Initiator(Box::new(initiator)),
                            )
                            .await
                        {
                            return Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg);
                        }

                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "session_id": session_id,
                                "message1_b64": msg1_b64,
                            }),
                        )
                    }

                    "finish" => {
                        let session_id = match req.params.get("session_id").and_then(|v| v.as_str())
                        {
                            Some(s) => s.to_string(),
                            None => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INVALID_ARGUMENT,
                                    "missing session_id for step=finish",
                                )
                            }
                        };
                        let msg2_b64 = match req.params.get("message2_b64").and_then(|v| v.as_str())
                        {
                            Some(s) => s.to_string(),
                            None => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INVALID_ARGUMENT,
                                    "missing message2_b64 for step=finish",
                                )
                            }
                        };

                        let msg2_bytes = match b64.decode(&msg2_b64) {
                            Ok(b) => b,
                            Err(e) => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INVALID_ARGUMENT,
                                    format!("invalid base64 in message2_b64: {e}"),
                                )
                            }
                        };

                        // Extract and consume the initiator session.
                        let initiator = {
                            let mut sessions = self.pake_sessions.lock().await;
                            match sessions.remove(&session_id) {
                                Some(StampedPakeSession {
                                    session: PakeSession::Initiator(i),
                                    ..
                                }) => *i,
                                Some(other) => {
                                    // Wrong session type — put it back and error.
                                    let key = session_id.clone();
                                    sessions.insert(key, other);
                                    return Response::err_with_code(
                                        req.id,
                                        ERR_CODE_INVALID_ARGUMENT,
                                        "session_id refers to a responder session, not initiator",
                                    );
                                }
                                None => {
                                    return Response::err_with_code(
                                        req.id,
                                        ERR_CODE_INVALID_ARGUMENT,
                                        format!("unknown session_id: {session_id}"),
                                    )
                                }
                            }
                        };

                        // TODO(S3): the PAKE `SessionKey` is derived here and
                        // immediately dropped. It SHOULD be mixed with the
                        // RFC 5705 TLS channel binder (see
                        // `copypaste_p2p::transport::tls_channel_binder_*` and
                        // `SessionKey::bind_to_tls_channel`) and verified against
                        // the peer to defeat a relay/MitM that terminates the
                        // PAKE on one socket and the mTLS on another. Wiring it
                        // is a deliberate design decision left to the human
                        // owner; until then pairing authenticity rests on the
                        // mTLS cert-fingerprint pinning alone.
                        let (_session_key, msg3_bytes) = match initiator.finish(&msg2_bytes) {
                            Ok(pair) => pair,
                            Err(e) => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_AUTH_FAILED,
                                    format!("PAKE finish failed: {e}"),
                                )
                            }
                        };

                        let msg3_b64 = b64.encode(&msg3_bytes);

                        // Store the paired peer on the initiator side (no PasswordFile).
                        let added_at = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        match load_peers() {
                            Ok(mut peers) => {
                                // Only add if not already present.
                                let already = peers.iter().any(|p| {
                                    p.get("fingerprint")
                                        .and_then(|v| v.as_str())
                                        .map(|f| f == peer_fingerprint)
                                        .unwrap_or(false)
                                });
                                if !already {
                                    peers.push(serde_json::json!({
                                        "fingerprint": peer_fingerprint,
                                        "added_at": added_at,
                                    }));
                                    if let Err(e) = save_peers(&peers) {
                                        return Response::err(
                                            req.id,
                                            format!("failed to save peers: {e}"),
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                return Response::err(req.id, format!("failed to load peers: {e}"))
                            }
                        }

                        // Feed the newly-paired peer into the live allowlist so
                        // the mTLS accept loop honours it without a restart.
                        self.register_live_peer(&peer_fingerprint);

                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "ok": true,
                                "message3_b64": msg3_b64,
                            }),
                        )
                    }

                    other => Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("unknown step '{other}'; expected 'initiate' or 'finish'"),
                    ),
                }
            }

            // W2.4 — PAKE responder: receives message1 from initiator,
            // runs PakeResponder::respond, stores session, returns message2.
            // Params: {message1_b64, peer_fingerprint, password}
            // Response: {session_id, message2_b64}
            "pair_accept_password" => {
                use base64::Engine as _;
                let b64 = base64::engine::general_purpose::STANDARD;

                let message1_b64 = match req.params.get("message1_b64").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing message1_b64",
                        )
                    }
                };
                let peer_fingerprint =
                    match req.params.get("peer_fingerprint").and_then(|v| v.as_str()) {
                        Some(s) => s.to_string(),
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "missing peer_fingerprint",
                            )
                        }
                    };
                let password = match req.params.get("password").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing password",
                        )
                    }
                };

                if !is_valid_fingerprint(&peer_fingerprint) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("invalid peer_fingerprint format: {peer_fingerprint}"),
                    );
                }

                // fix/p2p-c-review #5: enforce the same 6-char minimum the
                // initiator does. Without this the responder would happily
                // register a PasswordFile for a 1-char password if the peer
                // (or a malicious initiator) skipped the initiator-side check.
                if password.chars().count() < 6 {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "password must be at least 6 characters",
                    );
                }

                let msg1_bytes = match b64.decode(&message1_b64) {
                    Ok(b) => b,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            format!("invalid base64 in message1_b64: {e}"),
                        )
                    }
                };

                // Register the password so we have a PasswordFile for respond.
                let password_file = match copypaste_p2p::pake::PasswordFile::register(&password) {
                    Ok(pf) => pf,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("PasswordFile::register failed: {e}"),
                        )
                    }
                };

                let (responder, msg2_bytes) =
                    match PakeResponder::respond(&password_file, &msg1_bytes) {
                        Ok(pair) => pair,
                        Err(e) => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_AUTH_FAILED,
                                format!("PAKE respond failed: {e}"),
                            )
                        }
                    };

                let session_id = uuid::Uuid::new_v4().to_string();
                let msg2_b64 = b64.encode(&msg2_bytes);

                if let Err(msg) = self
                    .insert_pake_session(
                        session_id.clone(),
                        PakeSession::Responder {
                            responder: Box::new(responder),
                            password_file,
                            peer_fingerprint,
                        },
                    )
                    .await
                {
                    return Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg);
                }

                Response::ok(
                    req.id,
                    serde_json::json!({
                        "session_id": session_id,
                        "message2_b64": msg2_b64,
                    }),
                )
            }

            // W2.4 — PAKE responder finish: receives message3 from initiator,
            // completes handshake, persists peer + PasswordFile.
            // Params: {session_id, message3_b64, peer_fingerprint}
            // Response: {ok: true}
            "pair_accept_finish" => {
                use base64::Engine as _;
                let b64 = base64::engine::general_purpose::STANDARD;

                let session_id = match req.params.get("session_id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing session_id",
                        )
                    }
                };
                let msg3_b64 = match req.params.get("message3_b64").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing message3_b64",
                        )
                    }
                };

                let msg3_bytes = match b64.decode(&msg3_b64) {
                    Ok(b) => b,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            format!("invalid base64 in message3_b64: {e}"),
                        )
                    }
                };

                // Extract and consume the responder session.
                let (responder, password_file, peer_fingerprint) = {
                    let mut sessions = self.pake_sessions.lock().await;
                    match sessions.remove(&session_id) {
                        Some(StampedPakeSession {
                            session:
                                PakeSession::Responder {
                                    responder,
                                    password_file,
                                    peer_fingerprint,
                                },
                            ..
                        }) => (*responder, password_file, peer_fingerprint),
                        Some(other) => {
                            let key = session_id.clone();
                            sessions.insert(key, other);
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "session_id refers to an initiator session, not responder",
                            );
                        }
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                format!("unknown session_id: {session_id}"),
                            )
                        }
                    }
                };

                // Finalize the handshake (validates the initiator's authenticator).
                //
                // TODO(S3): `responder.finish` returns the shared `SessionKey`,
                // which we discard here. It SHOULD be mixed with the RFC 5705
                // TLS channel binder (`tls_channel_binder_server` +
                // `SessionKey::bind_to_tls_channel`) and confirmed with the peer
                // so a relay/MitM cannot bridge a PAKE on one connection to an
                // mTLS session on another. Deferred — design decision left to
                // the human owner; pairing currently relies on mTLS
                // cert-fingerprint pinning for channel authenticity.
                if let Err(e) = responder.finish(&msg3_bytes) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_AUTH_FAILED,
                        format!("PAKE accept_finish failed: {e}"),
                    );
                }

                // Persist the peer with the PasswordFile blob on the responder side.
                let password_file_b64 = b64.encode(&password_file.serialized);
                let added_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                match load_peers() {
                    Ok(mut peers) => {
                        let already = peers.iter().any(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| f == peer_fingerprint)
                                .unwrap_or(false)
                        });
                        if !already {
                            peers.push(serde_json::json!({
                                "fingerprint": peer_fingerprint,
                                "password_file_b64": password_file_b64,
                                "added_at": added_at,
                            }));
                        } else {
                            // Update existing peer with the new PasswordFile.
                            for p in peers.iter_mut() {
                                if p.get("fingerprint")
                                    .and_then(|v| v.as_str())
                                    .map(|f| f == peer_fingerprint)
                                    .unwrap_or(false)
                                {
                                    p["password_file_b64"] =
                                        serde_json::Value::String(password_file_b64.clone());
                                    break;
                                }
                            }
                        }
                        if let Err(e) = save_peers(&peers) {
                            return Response::err(req.id, format!("failed to save peers: {e}"));
                        }
                    }
                    Err(e) => return Response::err(req.id, format!("failed to load peers: {e}")),
                }

                // Feed the newly-paired peer into the live allowlist so the
                // mTLS accept loop honours it without a restart.
                self.register_live_peer(&peer_fingerprint);

                Response::ok(req.id, serde_json::json!({ "ok": true }))
            }

            // ----------------------------------------------------------------
            // QR pairing — displaying side. Generate a fresh pairing token,
            // store it for the matching `pair_accept_qr` step, and return a
            // single-line QR payload (the `copypaste-core::PairingPayload`
            // wire form) the *other* device scans. The token is the PAKE
            // password; the scanner derives it from the QR and drives the
            // existing `pair_peer_with_password` initiator flow. No new crypto:
            // QR is purely a transport for the token + this device's
            // fingerprint. See `copypaste_core::crypto::pairing_qr`.
            //
            // Request params: {} (device identity is taken from daemon state).
            // Response data: { "qr": "CPPAIR1...", "expires_in_secs": <u64> }
            // ----------------------------------------------------------------
            "pair_generate_qr" => {
                // CRITICAL-1 fix: the QR must carry the live mTLS **certificate**
                // fingerprint (the value the scanner pins and the mTLS verifier
                // compares — `PeerTransport::fingerprint` / `fingerprint_of`),
                // NOT the device-key fingerprint (`keychain::own_fingerprint`).
                // The QR payload already documents this field as the cert
                // fingerprint (see `copypaste_core::crypto::pairing_qr`), so the
                // payload format/version is unchanged — only the value sourced
                // here was wrong, making cert-pinning unable to ever match.
                //
                // No cert exists when P2P is disabled; refuse rather than
                // advertise a fingerprint that cannot authenticate the channel.
                let fingerprint = match self.cert_fingerprint.as_ref() {
                    Some(fp) => fp.clone(),
                    None => {
                        return Response::err(
                            req.id,
                            "P2P is disabled (set COPYPASTE_P2P=1): cannot generate a \
                             pairing QR without an mTLS certificate to advertise",
                        )
                    }
                };

                // Device name mirrors the P2P subsystem's source (HOSTNAME /
                // COMPUTERNAME, falling back to "CopyPaste") so the scanning
                // device shows a consistent label.
                let device_name = std::env::var("HOSTNAME")
                    .or_else(|_| std::env::var("COMPUTERNAME"))
                    .unwrap_or_else(|_| "CopyPaste".to_string());

                // device_id is best-effort: the canonical fingerprint doubles
                // as a stable identifier when no UUID is threaded here. The QR
                // `device_id` field is informational on the scanning side; peer
                // pinning uses the fingerprint.
                let device_id = fingerprint.clone();

                // Generate the single-use pairing token up front so the same
                // value feeds (a) the QR the scanner reads, (b) the legacy IPC
                // PAKE path's stored token, and (c) the bootstrap responder's
                // PAKE password — all derived from one token.
                let token = copypaste_core::PairingToken::generate();
                let password = token.to_pake_password();

                // P2P Phase 1: spawn an ephemeral, *unauthenticated* bootstrap
                // TLS listener and advertise its `host:port` in the QR's
                // `addr_hint`. The initiator dials it and the responder side of
                // the PAKE handshake runs over that TLS stream (PAKE provides
                // the mutual auth from the shared QR secret; the channel is
                // unpinned because neither side knows the other's cert yet).
                //
                // When P2P is disabled / the cert is absent we leave `addr_hint`
                // empty and fall back to the legacy IPC-relayed PAKE path.
                let addr_hint = if let Some(cert) = self.p2p_cert.clone() {
                    let (cert_der, key_der) = (cert.0.clone(), cert.1.clone());
                    match copypaste_p2p::bootstrap::BootstrapResponder::bind(cert_der, key_der)
                        .await
                    {
                        Ok(responder) => match responder.local_addr() {
                            Ok(local) => {
                                // The listener binds 0.0.0.0, so it's reachable on
                                // every interface — but the QR must carry one
                                // concrete host. A loopback hint (127.0.0.1) is
                                // unreachable from another device/emulator, so we
                                // advertise a real LAN-routable host via the shared
                                // `advertise_sync_addr` policy (same selection the
                                // in-band sync-listener address uses), falling back
                                // to 127.0.0.1 only when no LAN interface exists so
                                // same-host (and loopback-test) pairing still works.
                                let hint =
                                    copypaste_p2p::interfaces::advertise_sync_addr(local.port())
                                        .to_string();
                                self.spawn_bootstrap_responder(responder, password.clone());
                                hint
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "bootstrap listener local_addr failed ({e}); \
                                     falling back to mDNS-only addr_hint"
                                );
                                String::new()
                            }
                        },
                        Err(e) => {
                            tracing::warn!(
                                "bootstrap listener bind failed ({e}); \
                                 falling back to mDNS-only addr_hint"
                            );
                            String::new()
                        }
                    }
                } else {
                    String::new()
                };

                // Build the payload directly from the pre-generated token so the
                // QR, the stored token, and the bootstrap password all agree.
                let payload = copypaste_core::PairingPayload {
                    fingerprint,
                    token,
                    device_id,
                    device_name,
                    addr_hint,
                };

                let qr = payload.encode();

                // Store the token (replacing any prior active QR) so the legacy
                // IPC `pair_accept_qr` path can re-derive the same PAKE password.
                {
                    let mut slot = self.pending_qr_token.lock().await;
                    *slot = Some((payload.token, std::time::Instant::now()));
                }

                Response::ok(
                    req.id,
                    serde_json::json!({
                        "qr": qr,
                        "expires_in_secs": PAKE_SESSION_TTL.as_secs(),
                    }),
                )
            }

            // ----------------------------------------------------------------
            // QR pairing — displaying side, accept step. The scanning device
            // (initiator) has derived the PAKE password from the QR token and
            // sent `message1`. We look up the stored token, re-derive the same
            // password, register a PasswordFile and respond exactly as
            // `pair_accept_password` does — but without the user typing the
            // password (it came from the QR we generated). The follow-up
            // `pair_accept_finish` step is unchanged.
            //
            // Request params: { "message1_b64", "peer_fingerprint" }
            // Response data:  { "session_id", "message2_b64" }
            // ----------------------------------------------------------------
            "pair_accept_qr" => {
                use base64::Engine as _;
                let b64 = base64::engine::general_purpose::STANDARD;

                // ── P2P Phase 1: network bootstrap path ─────────────────────
                // When the caller supplies the scanned `qr` string (rather than
                // a relayed `message1_b64`), this daemon is the *initiator*: it
                // decodes the QR, dials the responder's `addr_hint` over the
                // unauthenticated bootstrap TLS channel, and runs the full PAKE
                // initiator handshake over the network. PAKE provides mutual auth
                // from the shared QR secret; the channel is unpinned. On success
                // the responder's cert fingerprint (learned over the channel) is
                // registered in the live mTLS allowlist.
                if let Some(qr) = req.params.get("qr").and_then(|v| v.as_str()) {
                    let qr = qr.to_string();
                    return self.pair_accept_qr_network(req.id.clone(), &qr).await;
                }

                let message1_b64 = match req.params.get("message1_b64").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing message1_b64",
                        )
                    }
                };
                let peer_fingerprint =
                    match req.params.get("peer_fingerprint").and_then(|v| v.as_str()) {
                        Some(s) => s.to_string(),
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "missing peer_fingerprint",
                            )
                        }
                    };

                if !is_valid_fingerprint(&peer_fingerprint) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("invalid peer_fingerprint format: {peer_fingerprint}"),
                    );
                }

                // Retrieve the active QR token, enforcing the TTL. Take it out
                // so a stale/expired token cannot linger.
                let password = {
                    let mut slot = self.pending_qr_token.lock().await;
                    match slot.take() {
                        Some((token, issued)) if issued.elapsed() < PAKE_SESSION_TTL => {
                            token.to_pake_password()
                        }
                        Some(_) => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "QR pairing token expired; regenerate the code",
                            )
                        }
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "no active QR pairing token; generate a code first",
                            )
                        }
                    }
                };

                let msg1_bytes = match b64.decode(&message1_b64) {
                    Ok(b) => b,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            format!("invalid base64 in message1_b64: {e}"),
                        )
                    }
                };

                let password_file = match copypaste_p2p::pake::PasswordFile::register(&password) {
                    Ok(pf) => pf,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("PasswordFile::register failed: {e}"),
                        )
                    }
                };

                let (responder, msg2_bytes) =
                    match PakeResponder::respond(&password_file, &msg1_bytes) {
                        Ok(pair) => pair,
                        Err(e) => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_AUTH_FAILED,
                                format!("PAKE respond failed: {e}"),
                            )
                        }
                    };

                let session_id = uuid::Uuid::new_v4().to_string();
                let msg2_b64 = b64.encode(&msg2_bytes);

                if let Err(msg) = self
                    .insert_pake_session(
                        session_id.clone(),
                        PakeSession::Responder {
                            responder: Box::new(responder),
                            password_file,
                            peer_fingerprint,
                        },
                    )
                    .await
                {
                    return Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg);
                }

                Response::ok(
                    req.id,
                    serde_json::json!({
                        "session_id": session_id,
                        "message2_b64": msg2_b64,
                    }),
                )
            }

            // ----------------------------------------------------------------
            // `import` — bulk-insert items previously exported by another
            // CopyPaste instance. The CLI sends a list of `ImportItem`
            // records; each is hashed (SHA-256 of the decoded bytes) and
            // deduplicated against rows inserted in the last 5 minutes.
            //
            // Request params:
            //   {
            //     "items": [
            //       { "content_type": "text",
            //         "content_bytes_b64": "...",
            //         "created_at_ms": 1234567890,
            //         "metadata": null | { ... } }
            //     ]
            //   }
            //
            // Response data:
            //   { "inserted": <u32>, "skipped": <u32> }
            //
            // Errors:
            //   * `invalid_argument` — missing `items`, missing required field,
            //     or `content_bytes_b64` failed to decode.
            //   * `internal_error` — SQLite failure or task panic.
            // ----------------------------------------------------------------
            "import" => {
                use base64::Engine as _;
                use sha2::{Digest, Sha256};

                // 1. Parse params.items into Vec<ImportItem>.
                let items_value = match req.params.get("items") {
                    Some(v) => v,
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: items",
                        );
                    }
                };
                let raw_items: &[serde_json::Value] = match items_value.as_array() {
                    Some(a) => a.as_slice(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "param 'items' must be an array",
                        );
                    }
                };

                // 2. Validate + decode each item up-front so a malformed entry
                //    aborts the whole import with a clear error (rather than
                //    silently skipping or partially inserting).
                let b64 = base64::engine::general_purpose::STANDARD;
                #[derive(Clone)]
                struct DecodedImport {
                    content_type: String,
                    bytes: Vec<u8>,
                    created_at_ms: i64,
                    #[allow(dead_code)]
                    metadata: Option<serde_json::Value>,
                }
                let mut decoded: Vec<DecodedImport> = Vec::with_capacity(raw_items.len());
                for (idx, raw) in raw_items.iter().enumerate() {
                    let content_type = match raw.get("content_type").and_then(|v| v.as_str()) {
                        Some(s) => s.to_string(),
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                format!("item[{idx}]: missing 'content_type'"),
                            );
                        }
                    };
                    let b64_str = match raw.get("content_bytes_b64").and_then(|v| v.as_str()) {
                        Some(s) => s,
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                format!("item[{idx}]: missing 'content_bytes_b64'"),
                            );
                        }
                    };
                    let bytes = match b64.decode(b64_str) {
                        Ok(b) => b,
                        Err(e) => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                format!("item[{idx}]: invalid base64 in 'content_bytes_b64': {e}"),
                            );
                        }
                    };
                    // Audit MED #4: enforce per-item ceiling BEFORE storage so
                    // a hostile/corrupt export cannot exhaust daemon memory or
                    // SQLite blob limits. Reject the whole import on first
                    // oversized item — matches the "malformed entry aborts
                    // the batch" contract documented above.
                    if bytes.len() > MAX_IMPORT_ITEM_BYTES {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            format!(
                                "item[{idx}]: decoded payload {} bytes exceeds max {} bytes",
                                bytes.len(),
                                MAX_IMPORT_ITEM_BYTES
                            ),
                        );
                    }
                    let created_at_ms = match raw.get("created_at_ms").and_then(|v| v.as_i64()) {
                        Some(n) => n,
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                format!("item[{idx}]: missing or non-integer 'created_at_ms'"),
                            );
                        }
                    };
                    let metadata = raw.get("metadata").cloned();
                    decoded.push(DecodedImport {
                        content_type,
                        bytes,
                        created_at_ms,
                        metadata,
                    });
                }

                // 3. Persist on the blocking pool — SQLite is sync.
                //    For each item: hash; if a row with the same hash exists
                //    within the dedupe window, skip; otherwise insert.
                let db_arc = self.db.clone();
                // Move a copy of the device's v1 storage key into the blocking
                // task so imported content can be ENCRYPTED with the same
                // (key, AAD, key_version) the normal ingest path uses — see
                // the per-item block below.
                let local_key_v1: [u8; 32] = **self.local_key;
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    // v0.3 post-T2: dedup is now enforced atomically by the
                    // v5 UNIQUE indexes (content_hash + minute_bucket) inside
                    // insert_item_with_fts. The previous explicit
                    // `find_recent_by_hash` precheck created a TOCTOU window
                    // — two concurrent imports of the same payload could both
                    // pass the precheck and then race on insert. The new
                    // path returns the existing row's id on a unique-violation,
                    // which we treat as a dedup skip.
                    let mut inserted: u32 = 0;
                    let mut skipped: u32 = 0;
                    // P2P Phase 3: collect successfully-inserted rows so the
                    // handler can broadcast them to the sync orchestrator (which
                    // re-keys + pushes them to paired peers).
                    let mut inserted_clips: Vec<copypaste_core::ClipboardItem> = Vec::new();
                    // Derive the v2 storage key once: imported content is
                    // encrypted exactly as `daemon::encrypt_text_for_storage`
                    // does (v2 key + v4 AAD, stamped key_version = 2), so the
                    // read path (`decrypt_item_by_version`, dispatched by the
                    // `copy`/`paste` IPC verb) can decrypt it.
                    let v2_key = derive_v2(&local_key_v1);
                    for item in decoded {
                        let mut hasher = Sha256::new();
                        hasher.update(&item.bytes);
                        let hash_hex = hex::encode(hasher.finalize());

                        // Audit fix (import round-trip): previously imported
                        // bytes were stored VERBATIM with an EMPTY nonce while
                        // `ClipboardItem::new_text` stamped key_version = 2.
                        // The read path then tried to XChaCha20-Poly1305-decrypt
                        // them under the v2 key and failed with AuthFailed, so
                        // imported items could never be retrieved.
                        //
                        // Now we ENCRYPT the content the same way fresh ingest
                        // does: build the AAD from the row's own item_id with
                        // the v4 schema + key_version 2, encrypt with the v2
                        // key, and store the real (nonce, ciphertext). The row
                        // stays at key_version = 2 (set by new_text) so the
                        // read path selects the matching key/AAD.
                        //
                        // lamport_ts = 0 is a deliberate "imported, unknown
                        // origin" sentinel; sync will reassign on first push.
                        let item_id = uuid::Uuid::new_v4().to_string();
                        let aad = copypaste_core::build_item_aad_v2(
                            &item_id,
                            copypaste_core::AAD_SCHEMA_VERSION_V4,
                            copypaste_core::ITEM_KEY_VERSION_CURRENT as u32,
                        );
                        let (nonce, ciphertext) =
                            match copypaste_core::encrypt_item_with_aad(&item.bytes, &v2_key, &aad)
                            {
                                Ok(v) => v,
                                Err(e) => {
                                    return Err::<
                                        (u32, u32, Vec<copypaste_core::ClipboardItem>),
                                        anyhow::Error,
                                    >(anyhow::anyhow!(
                                        "encrypt imported item failed: {e}"
                                    ));
                                }
                            };
                        let mut clip =
                            copypaste_core::ClipboardItem::new_text(ciphertext, nonce.to_vec(), 0);
                        clip.item_id = item_id;
                        clip.content_type = item.content_type;
                        clip.wall_time = item.created_at_ms;
                        clip.content_hash = Some(hash_hex);

                        // FTS indexing: pass "" to skip the FTS write. The
                        // searchable plaintext is no longer available as a
                        // stored column (content is now ciphertext), matching
                        // the image path semantics — search over imported
                        // items is out of scope for this fix.
                        let requested_id = clip.id.clone();
                        match copypaste_core::insert_item_with_fts(&db, &clip, "") {
                            Ok(stored_id) if stored_id == requested_id => {
                                inserted += 1;
                                inserted_clips.push(clip);
                            }
                            Ok(_) => {
                                // Returned id differs => dedup hit (existing
                                // row with same content_hash/item_id).
                                skipped += 1;
                            }
                            Err(e) => {
                                return Err::<
                                    (u32, u32, Vec<copypaste_core::ClipboardItem>),
                                    anyhow::Error,
                                >(e.into());
                            }
                        }
                    }
                    Ok::<(u32, u32, Vec<copypaste_core::ClipboardItem>), anyhow::Error>((
                        inserted,
                        skipped,
                        inserted_clips,
                    ))
                })
                .await;

                match join {
                    Ok(Ok((inserted, skipped, inserted_clips))) => {
                        // P2P Phase 3: notify the sync orchestrator of each newly
                        // imported row so it is re-keyed and pushed to paired
                        // peers (a closed/absent channel is a no-op — no peers).
                        if let Some(ref tx) = self.new_item_tx {
                            for clip in inserted_clips {
                                let _ = tx.send(clip);
                            }
                        }
                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "inserted": inserted,
                                "skipped": skipped,
                            }),
                        )
                    }
                    Ok(Err(e)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("import failed: {e}"),
                    ),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }

            other => Response::err(req.id, format!("unknown method: {other}")),
        }
    }

    /// Write a clipboard item's *decrypted* content back to NSPasteboard
    /// (macOS) or no-op on other platforms.
    ///
    /// Audit CRIT #1 fix: the daemon stores every clipboard item encrypted
    /// (XChaCha20-Poly1305 for text, chunked AEAD for images) — the legacy
    /// implementation wrote `item.content` raw, so users saw ciphertext on
    /// paste. This now:
    ///
    /// 1. Decrypts text via [`decrypt_item_with_aad`] with the per-item nonce,
    ///    rebuilding the AAD from the row's `item_id` so a tampered or
    ///    misbound ciphertext surfaces as `AuthFailed` instead of garbage.
    /// 2. Reassembles + decrypts image chunks via [`chunks_from_blob`] +
    ///    [`decode_image`], using the `file_id` parsed out of `blob_ref`.
    /// 3. Maps the daemon's internal `content_type` to a real macOS UTI
    ///    (`"image"` is **not** a valid UTI — audit HIGH #2). Text uses
    ///    `NSPasteboardTypeString`; image always writes `public.png` since
    ///    `encode_image` re-encodes raw clipboard bytes to PNG before
    ///    chunking. Anything already shaped like a UTI (`public.*`,
    ///    `com.*`, `org.*`) is passed through unchanged.
    fn write_to_pasteboard(
        &self,
        item: &copypaste_core::ClipboardItem,
    ) -> Result<(), PasteboardError> {
        #[cfg(target_os = "macos")]
        {
            // Drain the autorelease pool around the entire Cocoa body. Without
            // this, every paste-back (NSString::from_str, NSData::with_bytes for
            // multi-MB images, clearContents/setData_forType, and the
            // changeCount read in `record_self_write`) leaks autoreleased Cocoa
            // objects on this tokio worker thread — the same leak class fixed in
            // `clipboard.rs::poll`.
            objc2::rc::autoreleasepool(|_pool| {
                let content = match &item.content {
                    Some(bytes) => bytes.as_slice(),
                    None => return Err(PasteboardError::other("item has no content")),
                };

                // DUP-ON-COPY helper: reads and stores the post-write changeCount so
                // the monitor's next tick can identify and suppress this self-write.
                // Defined as a closure to avoid repeating the unsafe block.
                let record_self_write = |self_write_cc: &Arc<std::sync::atomic::AtomicI64>| {
                    use objc2_app_kit::NSPasteboard;
                    let new_count =
                        unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                    self_write_cc.store(new_count, std::sync::atomic::Ordering::Release);
                    tracing::debug!(
                        change_count = new_count,
                        "clipboard: recorded self-write changeCount to suppress re-capture"
                    );
                };

                use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
                use objc2_foundation::{NSData, NSString};

                if item.content_type == "text" {
                    // ----- text: decrypt per-item ciphertext, then write -----
                    let nonce_vec = item
                        .content_nonce
                        .as_ref()
                        .ok_or_else(|| PasteboardError::other("text item missing content_nonce"))?;
                    let nonce: &[u8; 24] = nonce_vec.as_slice().try_into().map_err(|_| {
                        PasteboardError::other(format!(
                            "text item content_nonce wrong length: expected 24, got {}",
                            nonce_vec.len()
                        ))
                    })?;

                    // Dispatch decrypt on the row's key_version so ciphertexts
                    // produced under different HKDF key families are always
                    // decrypted with the matching key and AAD format:
                    //
                    //   key_version = 1 → v1 key (local_enc_key / HKDF-SHA-256),
                    //                     AAD = build_item_aad(item_id, 3)
                    //   key_version = 2 → v2 key (derive_v2 / HKDF-SHA-512),
                    //                     AAD = build_item_aad_v2(item_id, 4, 2)
                    //   other           → UnknownKeyVersion → auth_failed error
                    //
                    // Previously this always used the v1 AAD regardless of
                    // key_version, so any item written with key_version = 2 (the
                    // current default since ITEM_KEY_VERSION_CURRENT = 2) would
                    // fail with "authentication tag mismatch" on paste-back.
                    //
                    // Note: IpcServer only holds one key (local_key = v1 key from
                    // Keychain). key_version = 2 items are derived from the same
                    // seed via derive_v2; we derive it inline here so the server
                    // struct does not need a second Arc field.
                    let v1_key: [u8; 32] = **self.local_key;
                    let v2_key = derive_v2(&v1_key);
                    let plaintext_bytes = decrypt_item_by_version(
                        item.key_version,
                        &v1_key,
                        &v2_key,
                        &item.item_id,
                        nonce,
                        content,
                    )
                    .map_err(|e| match e {
                        EncryptError::AuthFailed | EncryptError::AadMismatch => {
                            PasteboardError::decrypt(
                                "Decryption failed: authentication tag mismatch".to_string(),
                            )
                        }
                        EncryptError::UnknownKeyVersion(_) => PasteboardError::decrypt(
                            "Item encrypted with a previous key — cannot be recovered. \
                             Clear history to start fresh."
                                .to_string(),
                        ),
                        other => PasteboardError::decrypt(other.to_string()),
                    })?;
                    let text = std::str::from_utf8(&plaintext_bytes).map_err(|e| {
                        PasteboardError::decrypt(format!("decrypted content is not UTF-8: {e}"))
                    })?;
                    unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let ns_str = NSString::from_str(text);
                        let ok = pb.setString_forType(&ns_str, NSPasteboardTypeString);
                        if !ok {
                            return Err(PasteboardError::other(
                                "NSPasteboard setString:forType: returned false",
                            ));
                        }
                    }
                    record_self_write(&self.self_write_change_count);
                    Ok(())
                } else if item.content_type == "image" {
                    // ----- image: reassemble chunks → decrypt → write as PNG -----
                    // `file_id` is embedded in the JSON metadata stored in
                    // `blob_ref` (see ClipboardItem::new_image in
                    // storage/items.rs).
                    let meta_json = item.blob_ref.as_deref().ok_or_else(|| {
                        PasteboardError::other("image item missing blob_ref metadata")
                    })?;
                    let file_id = parse_image_file_id(meta_json).map_err(PasteboardError::other)?;

                    let chunks = chunks_from_blob(content).map_err(|e| {
                        PasteboardError::other(format!("image chunks_from_blob failed: {e}"))
                    })?;
                    let png_bytes =
                        decode_image(&chunks, &self.local_key, &file_id).map_err(|e| {
                            PasteboardError::decrypt(format!("image decode failed: {e}"))
                        })?;

                    unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let type_str = NSString::from_str("public.png");
                        let data = NSData::with_bytes(&png_bytes);
                        let ok = pb.setData_forType(Some(&data), &type_str);
                        if !ok {
                            return Err(PasteboardError::other(
                                "NSPasteboard setData:forType: returned false for public.png",
                            ));
                        }
                    }
                    record_self_write(&self.self_write_change_count);
                    Ok(())
                } else {
                    // Unknown content_type — keep a best-effort raw-bytes write,
                    // but map to a real UTI when possible. We do NOT attempt
                    // decryption here because we don't know the shape of the
                    // ciphertext (no nonce / no chunk metadata). Used only by
                    // future content_types added without updating this handler.
                    let uti = map_content_type_to_uti(&item.content_type);
                    unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let type_str = NSString::from_str(&uti);
                        let data = NSData::with_bytes(content);
                        let ok = pb.setData_forType(Some(&data), &type_str);
                        if !ok {
                            return Err(PasteboardError::other(format!(
                                "NSPasteboard setData:forType: returned false for type '{uti}'"
                            )));
                        }
                    }
                    record_self_write(&self.self_write_change_count);
                    Ok(())
                }
            })
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = item;
            // No clipboard support on non-macOS platforms in this crate
            Ok(())
        }
    }
}

/// Probe whether a Unix-domain socket at `socket_path` has a *live* listener.
///
/// A stale socket file (left behind by a daemon that crashed or was killed
/// without a clean shutdown) still exists on disk but no process is accepting
/// connections on it: `connect()` then fails with `ECONNREFUSED`. A socket
/// owned by a running daemon accepts the connection. We connect and
/// immediately drop the stream — this is a zero-byte probe the daemon's accept
/// loop tolerates (it spawns a handler that reads EOF and exits).
///
/// Returns `false` when the path does not exist, is not a socket, or the
/// connect is refused (stale). Returns `true` only when a live listener
/// actually accepts the connection.
fn is_socket_live(socket_path: &std::path::Path) -> bool {
    if !socket_path.exists() {
        return false;
    }
    std::os::unix::net::UnixStream::connect(socket_path).is_ok()
}

/// Bind a [`UnixListener`] at `socket_path`, self-healing a stale socket file.
///
/// macOS / Linux refuse to `bind()` over an existing socket path
/// (`EADDRINUSE`), so a socket file left behind by a previous daemon would
/// otherwise permanently block startup — the exact "process alive but IPC
/// socket not reachable" symptom seen after a v0.3.4 → v0.4.0 upgrade where an
/// old daemon died without cleaning up.
///
/// Policy:
///   * No file present  → bind directly.
///   * File present, NO live listener → stale; remove it and bind.
///   * File present, live listener answers → another healthy daemon already
///     owns the socket. Do NOT steal it (that would orphan the running
///     daemon); return an error so the caller logs and exits cleanly.
fn bind_with_stale_cleanup(socket_path: &std::path::Path) -> anyhow::Result<UnixListener> {
    if socket_path.exists() {
        if is_socket_live(socket_path) {
            anyhow::bail!(
                "another daemon is already listening on {} — refusing to steal the socket",
                socket_path.display()
            );
        }
        tracing::warn!(
            "removing stale IPC socket at {} (no live listener answered)",
            socket_path.display()
        );
        // Best-effort: if removal races with another process recreating it,
        // the subsequent bind error is the authoritative signal.
        let _ = std::fs::remove_file(socket_path);
    }
    let listener = UnixListener::bind(socket_path)?;
    Ok(listener)
}

/// Internal error type for the paste-back path so the dispatcher can
/// distinguish authentication / decryption failures (which deserve a
/// dedicated error code so a tampered row is surfaced to the caller) from
/// generic write failures.
#[derive(Debug)]
#[allow(dead_code)]
enum PasteboardError {
    DecryptFailed(String),
    Other(String),
}

impl PasteboardError {
    fn decrypt(msg: impl Into<String>) -> Self {
        Self::DecryptFailed(msg.into())
    }
    fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

/// Parse the `file_id` field out of the JSON metadata embedded in an
/// image item's `blob_ref`. The metadata shape is produced by
/// `daemon::handle_image` (`{"width":...,"file_id":[u8; 16]}` — Rust
/// `{:?}` debug formatting of the byte array).
///
/// Lives here as `pub(crate)` (not behind `#[cfg(macos)]`) so the daemon's
/// image round-trip tests can drive the exact same read-path parser on any
/// host. Only the macOS `write_to_pasteboard` path calls it at runtime, hence
/// the dead-code allowance on non-macOS builds.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn parse_image_file_id(meta_json: &str) -> Result<[u8; 16], String> {
    let value: serde_json::Value =
        serde_json::from_str(meta_json).map_err(|e| format!("image meta_json parse error: {e}"))?;
    let arr = value
        .get("file_id")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "image meta_json missing 'file_id' array".to_string())?;
    if arr.len() != 16 {
        return Err(format!(
            "image meta_json 'file_id' has wrong length: expected 16, got {}",
            arr.len()
        ));
    }
    let mut out = [0u8; 16];
    for (i, v) in arr.iter().enumerate() {
        out[i] = v
            .as_u64()
            .and_then(|n| u8::try_from(n).ok())
            .ok_or_else(|| format!("image meta_json 'file_id[{i}]' not a u8"))?;
    }
    Ok(out)
}

/// Map the daemon's internal `content_type` string to a macOS UTI suitable
/// for `setData:forType:`. Audit HIGH #2: bare `"image"` is not a UTI and
/// macOS refuses to set the pasteboard data for it.
///
/// Heuristic: anything already shaped like a UTI (`public.*`, `com.*`,
/// `org.*`) is passed through; bare `"image"` defaults to `public.png`;
/// `"text"` to `public.utf8-plain-text`; everything else gets
/// `public.data` so the write doesn't silently no-op.
#[cfg(target_os = "macos")]
fn map_content_type_to_uti(content_type: &str) -> String {
    if content_type.starts_with("public.")
        || content_type.starts_with("com.")
        || content_type.starts_with("org.")
    {
        return content_type.to_string();
    }
    match content_type {
        "image" => "public.png".to_string(),
        "text" => "public.utf8-plain-text".to_string(),
        _ => "public.data".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Cloud connection diagnostics
// ---------------------------------------------------------------------------

/// Probe the configured Supabase project and return a structured diagnostic.
///
/// This is what backs the `cloud_test_connection` IPC method (and `copypaste
/// cloud test`). It performs at most one authenticated round-trip:
/// `GET /rest/v1/clipboard_items?limit=0` with the anon key in `apikey` and an
/// `Authorization: Bearer` header (email/password token when configured, anon
/// key otherwise). The HTTP outcome is mapped to an actionable message so the
/// user learns *which* step is wrong (credentials missing, URL unreachable,
/// key invalid, table not provisioned, RLS misconfigured) rather than seeing
/// silent no-op sync.
///
/// The returned JSON shape is stable (consumed by the CLI/UI):
/// ```json
/// { "ok": bool, "configured": bool, "stage": "<step>", "message": "<human>" }
/// ```
/// `ok` is the single source of truth ("is cloud sync ready?"); `stage` and
/// `message` are for display. No secrets are ever included in the output.
#[cfg(feature = "cloud-sync")]
async fn test_cloud_connection() -> serde_json::Value {
    use crate::cloud::CloudConfig;

    // Resolve credentials the same way the daemon's cloud orchestrator does
    // (env vars first, then the persisted AppConfig the UI writes).
    let cfg = match CloudConfig::from_env() {
        Some(c) => c,
        None => {
            return serde_json::json!({
                "ok": false,
                "configured": false,
                "stage": "config",
                "message": "Supabase is not configured. Set the project URL and anon key \
                            (Settings → Sync, or `copypaste cloud setup`).",
            });
        }
    };

    // Mirror the daemon's HTTPS-only gate so the diagnostic matches what
    // start_cloud would actually accept.
    if !cfg
        .supabase_url
        .to_ascii_lowercase()
        .starts_with("https://")
    {
        return serde_json::json!({
            "ok": false,
            "configured": true,
            "stage": "url",
            "message": format!(
                "Supabase URL must use https:// (got {}). Cloud sync refuses plain http.",
                cfg.supabase_url
            ),
        });
    }

    // Bearer: prefer an email/password GoTrue token (authenticated scope, the
    // scope RLS expects), falling back to the anon key. Credentials come from
    // `CloudConfig` (env vars first, then the persisted `0600` config written by
    // `copypaste cloud setup`) — the same resolution the orchestrator uses. We
    // do NOT fail the whole probe if sign-in fails — we report it as the failing
    // stage so the user can fix credentials specifically.
    let (bearer, signed_in) = match (cfg.email.as_deref(), cfg.password.as_deref()) {
        (Some(email), Some(password)) if !email.is_empty() && !password.is_empty() => {
            let auth = copypaste_supabase::auth::AuthClient::new(&cfg.supabase_url, &cfg.anon_key);
            match auth.sign_in(email, password).await {
                Ok(session) => (session.access_token, true),
                Err(e) => {
                    return serde_json::json!({
                        "ok": false,
                        "configured": true,
                        "stage": "auth",
                        "message": format!(
                            "Sign-in failed for {email}: {e}. Re-check the email/password \
                             (run `copypaste cloud setup` again, or set SUPABASE_EMAIL / \
                             SUPABASE_PASSWORD), and that the user is confirmed."
                        ),
                    });
                }
            }
        }
        _ => (cfg.anon_key.clone(), false),
    };

    // One cheap REST round-trip. `limit=0` returns an empty array on success
    // without transferring any rows, so it is safe even on a large table.
    let url = format!("{}/rest/v1/clipboard_items?limit=0", cfg.supabase_url);
    let client = reqwest::Client::new();
    let resp = match client
        .get(&url)
        .header("apikey", &cfg.anon_key)
        .header("Authorization", format!("Bearer {bearer}"))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return serde_json::json!({
                "ok": false,
                "configured": true,
                "stage": "network",
                "message": format!(
                    "Could not reach {}: {e}. Check the URL and your network/proxy.",
                    cfg.supabase_url
                ),
            });
        }
    };

    let status = resp.status();
    let code = status.as_u16();
    if status.is_success() {
        let scope = if signed_in {
            "signed in (authenticated scope)"
        } else {
            "anon key (sign in for full scope)"
        };
        return serde_json::json!({
            "ok": true,
            "configured": true,
            "stage": "done",
            "message": format!("Connected to Supabase — table reachable, {scope}."),
        });
    }

    // Classify the common failure HTTP codes into actionable guidance.
    let body = resp.text().await.unwrap_or_default();
    let (stage, message) = match code {
        // 401 has two distinct root causes. When we already hold an
        // authenticated bearer (`signed_in`), the anon key itself must be
        // wrong/expired. When the probe used only the anon key (no sign-in),
        // the project's `authenticated`-only RLS rejects the request and the
        // fix is to supply email/password, not to re-copy the anon key.
        401 if signed_in => (
            "auth",
            "401 Unauthorized — the anon key is wrong or expired. Re-copy it from \
             Supabase → Project Settings → API."
                .to_string(),
        ),
        401 => (
            "auth",
            "401 Unauthorized — the request used the anon key with no signed-in \
             session, and the table's RLS grants only the `authenticated` role. \
             Provide email/password (run `copypaste cloud setup` and supply them, \
             or set SUPABASE_EMAIL / SUPABASE_PASSWORD) so the daemon authenticates."
                .to_string(),
        ),
        404 => (
            "schema",
            "404 Not Found — the clipboard_items table is missing. Run the \
             provisioning SQL: `copypaste cloud setup-sql` then paste it into the \
             Supabase SQL Editor."
                .to_string(),
        ),
        // PostgREST returns 400/406 with a 'relation does not exist' hint when
        // the table is absent under some configs; surface the body for clarity.
        400 | 406 => (
            "schema",
            format!(
                "{code} from PostgREST — the table may be missing or misconfigured. \
                 Run `copypaste cloud setup-sql`. Server said: {}",
                body.trim()
            ),
        ),
        403 => (
            "rls",
            "403 Forbidden — row-level security rejected the request. Re-run the RLS \
             part of `copypaste cloud setup-sql`."
                .to_string(),
        ),
        _ => (
            "http",
            format!("Unexpected HTTP {code} from Supabase: {}", body.trim()),
        ),
    };
    serde_json::json!({
        "ok": false,
        "configured": true,
        "stage": stage,
        "message": message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::Database;
    use tempfile::tempdir;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    /// `get_config` must never ship the GoTrue password or email over IPC.
    /// `redact_config_secrets` strips both and replaces them with `*_set`
    /// presence flags, while leaving the publishable anon key intact.
    #[test]
    fn redact_config_secrets_strips_password_and_email() {
        let mut v = serde_json::json!({
            "p2p_enabled": true,
            "supabase_url": "https://x.supabase.co",
            "supabase_anon_key": "eyJpublishable",
            "supabase_email": "user@example.com",
            "supabase_password": "hunter2",
        });
        redact_config_secrets(&mut v);
        let obj = v.as_object().unwrap();
        // Secrets are gone from the wire.
        assert!(!obj.contains_key("supabase_password"));
        assert!(!obj.contains_key("supabase_email"));
        // Presence flags reflect that both were set.
        assert_eq!(obj["supabase_password_set"], serde_json::json!(true));
        assert_eq!(obj["supabase_email_set"], serde_json::json!(true));
        // Non-secret fields (incl. the publishable anon key) are untouched.
        assert_eq!(
            obj["supabase_anon_key"],
            serde_json::json!("eyJpublishable")
        );
        assert_eq!(
            obj["supabase_url"],
            serde_json::json!("https://x.supabase.co")
        );
        assert_eq!(obj["p2p_enabled"], serde_json::json!(true));
    }

    /// When the credentials are absent (null), the presence flags must be
    /// `false` and no secret key should appear on the wire.
    #[test]
    fn redact_config_secrets_reports_unset_when_null() {
        let mut v = serde_json::json!({
            "supabase_email": serde_json::Value::Null,
            "supabase_password": serde_json::Value::Null,
        });
        redact_config_secrets(&mut v);
        let obj = v.as_object().unwrap();
        assert_eq!(obj["supabase_password_set"], serde_json::json!(false));
        assert_eq!(obj["supabase_email_set"], serde_json::json!(false));
        assert!(!obj.contains_key("supabase_password"));
        assert!(!obj.contains_key("supabase_email"));
    }

    /// RAII guard that snapshots one or more env vars, sets them for the test,
    /// and restores the previous values (or unsets them) on drop — even on
    /// panic.  Holds `crate::TEST_ENV_LOCK` (the *process-wide* env lock shared
    /// with every other daemon test module) for its whole lifetime so env state
    /// cannot race tests in `paths`, `keychain`, or any other module that also
    /// mutates `HOME`/`XDG_CONFIG_HOME`.
    struct EnvGuard {
        saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        /// Point every given env var at `value`. Used to redirect the config
        /// dir to a temp path across platforms: `dirs::config_dir()` honours
        /// `XDG_CONFIG_HOME` on Linux/BSD and `$HOME` (→ Library/Application
        /// Support) on macOS, so callers set both.
        fn set_all(keys: &[&'static str], value: &std::path::Path) -> Self {
            let lock = crate::TEST_ENV_LOCK
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let mut saved = Vec::with_capacity(keys.len());
            for &key in keys {
                saved.push((key, std::env::var_os(key)));
                // SAFETY: serialised via `crate::TEST_ENV_LOCK`; no other
                // thread reads or writes these vars concurrently for the
                // guard's lifetime.
                unsafe { std::env::set_var(key, value) };
            }
            Self { saved, _lock: lock }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: still holding `crate::TEST_ENV_LOCK` (`_lock`), so the
            // restore is serialised against every other env-mutating test.
            unsafe {
                for (key, original) in self.saved.drain(..) {
                    match original {
                        Some(v) => std::env::set_var(key, v),
                        None => std::env::remove_var(key),
                    }
                }
            }
        }
    }

    async fn start_test_server(socket_path: &std::path::Path) -> Arc<AtomicBool> {
        start_test_server_with_mode(socket_path, false).await
    }

    async fn start_test_server_with_mode(
        socket_path: &std::path::Path,
        initial_private_mode: bool,
    ) -> Arc<AtomicBool> {
        let (private_mode, _db) =
            start_test_server_returning_db(socket_path, initial_private_mode).await;
        private_mode
    }

    /// Like `start_test_server_with_mode` but also hands back the shared
    /// `Database` handle so a test can seed rows / inspect audit tables.
    async fn start_test_server_returning_db(
        socket_path: &std::path::Path,
        initial_private_mode: bool,
    ) -> (Arc<AtomicBool>, Arc<Mutex<Database>>) {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let private_mode = Arc::new(AtomicBool::new(initial_private_mode));
        // Dummy keys: in-process tests do not hit paste-back or fingerprint
        // surfaces — they only validate dispatch / state-machine behaviour.
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let device_pub = Arc::new([0u8; 32]);
        // Give the test server a realistic mTLS cert fingerprint (colon-hex of a
        // 32-byte SHA-256) so the pairing handlers (`pair_generate_qr`,
        // `get_own_fingerprint`) behave as they do with P2P enabled. Generating a
        // real cert keeps this honest: the advertised value is exactly what the
        // transport would pin.
        let cert = copypaste_p2p::cert::SelfSignedCert::generate("test-device").unwrap();
        let server = IpcServer::new(db.clone(), private_mode.clone(), local_key, device_pub)
            .with_cert_fingerprint(display_fingerprint(&cert.fingerprint()));
        let path = socket_path.to_path_buf();
        tokio::spawn(async move {
            if let Err(e) = server.serve(&path, CancellationToken::new()).await {
                tracing::error!("ipc: server on {:?} exited with error: {e}", &path);
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        (private_mode, db)
    }

    // -----------------------------------------------------------------------
    // Stale-socket self-heal (fix/daemon-ipc-selfheal)
    // -----------------------------------------------------------------------

    /// A path that does not exist is never "live".
    #[test]
    fn is_socket_live_false_for_missing_path() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("missing.sock");
        assert!(!is_socket_live(&sock));
    }

    /// A regular file sitting at the socket path is not a live listener —
    /// `connect()` on a non-socket fails, so we treat it as not-live (and the
    /// bind helper will clean it up).
    #[test]
    fn is_socket_live_false_for_stale_regular_file() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("stale.sock");
        std::fs::write(&sock, b"not a socket").unwrap();
        assert!(!is_socket_live(&sock));
    }

    /// A leftover socket *file* with no process accepting on it is stale:
    /// `bind_with_stale_cleanup` must remove it and successfully rebind,
    /// rather than failing with `EADDRINUSE`. This is the core self-heal for
    /// the "process alive but socket not reachable" upgrade bug.
    ///
    /// Uses `std::os::unix::net::UnixListener` to seed the stale socket so the
    /// "previous daemon" half does not depend on a Tokio reactor; the helper
    /// under test (`bind_with_stale_cleanup`) binds a `tokio` listener, hence
    /// `#[tokio::test]`.
    #[tokio::test]
    async fn bind_with_stale_cleanup_removes_dead_socket_and_rebinds() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("daemon.sock");

        // Create a real socket then drop its listener so the path is left
        // behind with no live acceptor — exactly what a crashed daemon leaves.
        {
            let dead = std::os::unix::net::UnixListener::bind(&sock).expect("seed bind");
            drop(dead);
        }
        assert!(sock.exists(), "socket file must remain after listener drop");
        assert!(
            !is_socket_live(&sock),
            "dropped listener must not be detected as live"
        );

        // The helper must clean up and bind successfully.
        let listener =
            bind_with_stale_cleanup(&sock).expect("must self-heal a stale socket and rebind");
        assert!(is_socket_live(&sock), "rebound socket must accept connects");
        drop(listener);
    }

    /// When a *live* daemon already owns the socket, the helper must refuse to
    /// steal it (returning an error) so the running daemon is not orphaned.
    #[tokio::test]
    async fn bind_with_stale_cleanup_refuses_to_steal_live_socket() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("daemon.sock");

        // Hold a live listener (std, no reactor needed) for the whole test.
        let _live = std::os::unix::net::UnixListener::bind(&sock).expect("seed live bind");
        assert!(is_socket_live(&sock), "seeded listener must be live");

        let err =
            bind_with_stale_cleanup(&sock).expect_err("must refuse to bind over a live socket");
        let msg = err.to_string();
        assert!(
            msg.contains("already listening"),
            "expected a 'already listening' refusal, got: {msg}"
        );
    }

    #[tokio::test]
    async fn status_returns_running() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"status\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["status"], "running");
    }

    #[tokio::test]
    async fn list_empty_db_returns_zero() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test2.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"2\",\"method\":\"list\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["total"], 0);
    }

    #[tokio::test]
    async fn unknown_method_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test3.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"3\",\"method\":\"bogus\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"].as_str().unwrap().contains("unknown method"));
    }

    /// ADR-007 — a request carrying a `protocol_version` outside the
    /// supported window must be rejected with a stable error code BEFORE
    /// the dispatcher tries to interpret the method.
    #[tokio::test]
    async fn unsupported_protocol_version_rejected_with_error_code() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test-proto-ver.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        // Use a method that would normally succeed (`status`) to prove the
        // version gate fires first.
        let unsupported = CURRENT_PROTOCOL_VERSION + 99;
        let payload = format!(
            "{{\"id\":\"pv1\",\"method\":\"status\",\"protocol_version\":{}}}\n",
            unsupported
        );
        stream.write_all(payload.as_bytes()).await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false, "version gate must reject: {line}");
        assert_eq!(resp["error_code"], "invalid_argument");
        assert_eq!(resp["protocol_version"], CURRENT_PROTOCOL_VERSION);
        assert!(
            resp["error"]
                .as_str()
                .unwrap()
                .contains("unsupported protocol version"),
            "expected version-mismatch message, got: {}",
            resp["error"]
        );
    }

    /// W3.6 — stubbed methods (`cloud_sign_in`, `cloud_sign_out`) must carry
    /// a stable machine-readable `error_code: "not_implemented"` so clients
    /// can branch deterministically without parsing the English `error` text.
    #[tokio::test]
    async fn ipc_responses_carry_machine_readable_error_code() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test_err_code.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"42\",\"method\":\"cloud_sign_in\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

        assert_eq!(resp["ok"], false, "stub should report failure, not fake ok");
        assert_eq!(
            resp["error_code"], "not_implemented",
            "cloud stub must tag response with machine-readable not_implemented code"
        );
        assert!(
            resp["error"].as_str().unwrap().contains("cloud-sync"),
            "human-readable error should name the unimplemented feature"
        );
    }

    #[tokio::test]
    async fn search_with_no_fts_data_returns_empty() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test_search.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"s1\",\"method\":\"search\",\"params\":{\"query\":\"hello\",\"limit\":10}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["items"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn search_missing_query_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test_search_err.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"s2\",\"method\":\"search\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"]
            .as_str()
            .unwrap()
            .contains("missing param: query"));
    }

    #[tokio::test]
    async fn copy_unknown_id_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_test.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"copy\",\"params\":{\"id\":\"nonexistent\"}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
    }

    #[tokio::test]
    async fn copy_missing_id_param_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_missing_param.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"2\",\"method\":\"copy\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"]
            .as_str()
            .unwrap()
            .contains("missing param: id"));
    }

    #[tokio::test]
    async fn stats_returns_zero_for_empty_db() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("stats.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"stats\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["total_items"], 0);
    }

    #[tokio::test]
    async fn delete_all_returns_count() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("del_all.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"delete_all\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert!(resp["data"]["deleted"].as_i64().is_some());
    }

    // --- private mode IPC tests ---

    #[tokio::test]
    async fn get_private_mode_returns_false_by_default() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pm_get_default.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"get_private_mode\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["private_mode"], false);
    }

    #[tokio::test]
    async fn set_private_mode_enable_then_get() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pm_set_enable.sock");
        start_test_server(&sock).await;

        // Enable private mode — first connection
        {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            stream
                .write_all(b"{\"id\":\"1\",\"method\":\"set_private_mode\",\"params\":{\"enabled\":true}}\n")
                .await
                .unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(resp["ok"], true);
            assert_eq!(resp["data"]["private_mode"], true);
        }

        // Verify get_private_mode reflects the change — second connection
        {
            let mut stream2 = UnixStream::connect(&sock).await.unwrap();
            stream2
                .write_all(b"{\"id\":\"2\",\"method\":\"get_private_mode\"}\n")
                .await
                .unwrap();
            let mut lines2 = BufReader::new(&mut stream2).lines();
            let line2 = lines2.next_line().await.unwrap().unwrap();
            let resp2: serde_json::Value = serde_json::from_str(&line2).unwrap();
            assert_eq!(resp2["ok"], true);
            assert_eq!(resp2["data"]["private_mode"], true);
        }
    }

    #[tokio::test]
    async fn set_private_mode_then_disable() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pm_disable.sock");
        start_test_server_with_mode(&sock, true).await;

        // Confirm it starts enabled — first connection
        {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            stream
                .write_all(b"{\"id\":\"1\",\"method\":\"get_private_mode\"}\n")
                .await
                .unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(resp["data"]["private_mode"], true);
        }

        // Disable — second connection
        {
            let mut stream2 = UnixStream::connect(&sock).await.unwrap();
            stream2
                .write_all(b"{\"id\":\"2\",\"method\":\"set_private_mode\",\"params\":{\"enabled\":false}}\n")
                .await
                .unwrap();
            let mut lines2 = BufReader::new(&mut stream2).lines();
            let line2 = lines2.next_line().await.unwrap().unwrap();
            let resp2: serde_json::Value = serde_json::from_str(&line2).unwrap();
            assert_eq!(resp2["ok"], true);
            assert_eq!(resp2["data"]["private_mode"], false);
        }
    }

    #[tokio::test]
    async fn set_private_mode_missing_param_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pm_missing.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"set_private_mode\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"].as_str().unwrap().contains("enabled"));
    }

    #[tokio::test]
    async fn status_includes_private_mode_field() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("status_pm.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"status\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["status"], "running");
        assert!(resp["data"]["private_mode"].is_boolean());
    }

    #[tokio::test]
    async fn set_private_mode_updates_shared_atomic() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pm_atomic.sock");
        let flag = start_test_server(&sock).await;

        // Initially false
        assert!(!flag.load(Ordering::Relaxed));

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(
                b"{\"id\":\"1\",\"method\":\"set_private_mode\",\"params\":{\"enabled\":true}}\n",
            )
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let _line = lines.next_line().await.unwrap().unwrap();

        // The shared atomic should now be true
        assert!(flag.load(Ordering::Relaxed));
    }

    // --- history_page ---

    #[tokio::test]
    async fn history_page_empty_db_returns_zero() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hp_empty.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"hp1\",\"method\":\"history_page\",\"params\":{\"limit\":50,\"offset\":0}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["total"], 0);
        assert_eq!(resp["data"]["items"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn history_page_default_params_succeed() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hp_default.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        // No params — should default to limit=50, offset=0
        stream
            .write_all(b"{\"id\":\"hp2\",\"method\":\"history_page\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert!(resp["data"]["items"].is_array());
    }

    // --- paste ---

    #[tokio::test]
    async fn paste_missing_id_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("paste_missing.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"p1\",\"method\":\"paste\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"]
            .as_str()
            .unwrap()
            .contains("missing param: id"));
    }

    #[tokio::test]
    async fn paste_unknown_id_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("paste_unknown.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(
                b"{\"id\":\"p2\",\"method\":\"paste\",\"params\":{\"id\":\"00000000-0000-0000-0000-000000000000\"}}\n",
            )
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"].as_str().unwrap().contains("not found"));
    }

    // ------------------------------------------------------------------
    // Wave 1.1 IPC hardening tests
    //
    // These verify the security guarantees added in
    // `fix(daemon-ipc): wave1.1 — socket chmod 0o600 + request size cap +
    //  handle disconnect`:
    //   * the Unix listener socket is created with mode 0600 (user-only),
    //   * a request line exceeding MAX_REQUEST_BYTES (16 MiB) is rejected
    //     with an error response without crashing the server,
    //   * a client that connects and disconnects abruptly (no newline,
    //     partial write, or zero bytes) does not panic the spawned task.
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn ipc_socket_chmod_is_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hardening_chmod.sock");
        start_test_server(&sock).await;

        let meta = std::fs::metadata(&sock).expect("socket file should exist");
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(
            mode,
            0o600,
            "socket {} has mode {:o}, expected 0600",
            sock.display(),
            mode
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ipc_oversized_request_rejected_not_crashed() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hardening_oversize.sock");
        start_test_server(&sock).await;

        // Client A: send 17 MiB without a newline. The server reads up to
        // MAX_REQUEST_BYTES + 1 (16 MiB + 1) and trips the oversize branch,
        // returns an error response, and closes the connection.
        {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            let payload = vec![b'A'; 17 * 1024 * 1024];
            // The server may close before we finish writing — that's fine.
            let _ = stream.write_all(&payload).await;
            // Half-close write so the server's read_until unblocks.
            let _ = stream.shutdown().await;

            // Try to read the error response, bounded by a timeout so a
            // misbehaving server can't hang the test.
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();
            if let Ok(Ok(_n)) = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                reader.read_line(&mut line),
            )
            .await
            {
                if !line.trim().is_empty() {
                    let resp: serde_json::Value = serde_json::from_str(line.trim())
                        .expect("oversize response should be valid JSON");
                    assert_eq!(resp["ok"], false, "expected error response, got: {resp}");
                    let err = resp["error"].as_str().unwrap_or_default();
                    assert!(
                        err.contains("too large"),
                        "expected 'too large' in error, got: {err}"
                    );
                }
                // If we got no bytes back (race with server close), the
                // next client below proves the server didn't crash.
            }
        }

        // Client B: a normal request must still succeed — proves the server
        // survived the oversize client.
        {
            let mut stream = UnixStream::connect(&sock)
                .await
                .expect("server must still accept new connections after oversize client");
            stream
                .write_all(b"{\"id\":\"after-oversize\",\"method\":\"status\"}\n")
                .await
                .unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();
            let n = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                reader.read_line(&mut line),
            )
            .await
            .expect("status read timed out — server may have crashed")
            .expect("status read failed");
            assert!(n > 0, "expected a status response line");
            let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
            assert_eq!(
                resp["ok"], true,
                "status should be ok after oversize, got: {resp}"
            );
            assert_eq!(resp["data"]["status"], "running");
        }
    }

    // ------------------------------------------------------------------
    // Wave 2.3 IPC hardening tests
    //
    // Cover edge cases that the binary-driven integration suite cannot
    // reach in-process:
    //   * IPC_NOT_READY when a DB-touching method fires before the
    //     readiness flag flips,
    //   * MAX_PAGE clamping on `list` and `history_page` enforced by the
    //     dispatcher itself (independent of DB row count).
    // ------------------------------------------------------------------

    /// Spawn an IpcServer whose readiness flag starts `false`, returning
    /// the socket path and the flag handle so the test can flip it.
    async fn start_not_ready_server(socket_path: &std::path::Path) -> Arc<AtomicBool> {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let private_mode = Arc::new(AtomicBool::new(false));
        let ready = Arc::new(AtomicBool::new(false));
        let ready_clone = ready.clone();
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let device_pub = Arc::new([0u8; 32]);
        let server =
            IpcServer::new_with_ready(db, private_mode, local_key, device_pub, ready_clone);
        let path = socket_path.to_path_buf();
        tokio::spawn(async move {
            if let Err(e) = server.serve(&path, CancellationToken::new()).await {
                tracing::error!("ipc: server on {:?} exited with error: {e}", &path);
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        ready
    }

    #[tokio::test]
    async fn dispatch_returns_ipc_not_ready_when_not_ready() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("not_ready.sock");
        let ready = start_not_ready_server(&sock).await;

        // DB-touching methods must be rejected with IPC_NOT_READY.
        for (method, params) in [
            ("list", "{}"),
            ("count", "{}"),
            ("stats", "{}"),
            ("history_page", "{}"),
            ("delete_all", "{}"),
        ] {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            let req =
                format!("{{\"id\":\"nr-{method}\",\"method\":\"{method}\",\"params\":{params}}}\n");
            stream.write_all(req.as_bytes()).await.unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(resp["ok"], false, "{method} should be rejected: {resp}");
            assert_eq!(
                resp["error"].as_str().unwrap_or_default(),
                "IPC_NOT_READY",
                "{method} should return IPC_NOT_READY, got: {resp}"
            );
        }

        // Non-DB methods (status, get_private_mode) must still work, so the
        // client can introspect the daemon and decide whether to retry.
        {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            stream
                .write_all(b"{\"id\":\"nr-status\",\"method\":\"status\"}\n")
                .await
                .unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(resp["ok"], true, "status should pass: {resp}");
        }

        // After the readiness flag flips, previously-rejected methods succeed.
        ready.store(true, Ordering::Relaxed);
        {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            stream
                .write_all(b"{\"id\":\"nr-stats-after\",\"method\":\"stats\"}\n")
                .await
                .unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(resp["ok"], true, "stats should pass after ready: {resp}");
            assert!(resp["data"]["total_items"].is_number());
        }
    }

    #[tokio::test]
    async fn list_clamps_oversize_limit_to_max_page() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("cap_list.sock");
        start_test_server(&sock).await;

        // Empty DB — we cannot directly observe the clamp on item count,
        // but we *can* verify the dispatcher accepts the request and
        // returns at most MAX_PAGE items. The count_items helper is the
        // path that would blow up if the unclamped limit reached the DB.
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"cap-list\",\"method\":\"list\",\"params\":{\"limit\":5000,\"offset\":0}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            resp["ok"], true,
            "list with limit=5000 should be ok: {resp}"
        );
        let items = resp["data"]["items"].as_array().unwrap();
        assert!(
            items.len() <= 1000,
            "list returned {} items, exceeds MAX_PAGE=1000",
            items.len()
        );
    }

    #[tokio::test]
    async fn history_page_clamps_oversize_limit_to_max_page() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("cap_hp.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"cap-hp\",\"method\":\"history_page\",\"params\":{\"limit\":9999,\"offset\":0}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        let items = resp["data"]["items"].as_array().unwrap();
        assert!(
            items.len() <= 1000,
            "history_page returned {} items, exceeds MAX_PAGE=1000",
            items.len()
        );
    }

    /// daemon-core backlog #2: the `search` handler must clamp an oversized
    /// `limit` to MAX_PAGE just like `list` / `history_page`. We seed more than
    /// MAX_PAGE rows all matching one FTS term, then request `limit=5000`. The
    /// SQL applies `LIMIT ?`, so without the `.min(MAX_PAGE)` clamp the response
    /// would carry > MAX_PAGE items; with it, exactly MAX_PAGE.
    #[tokio::test]
    async fn search_clamps_oversize_limit_to_max_page() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("cap_search.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        // Seed MAX_PAGE + 5 text rows whose FTS plaintext all contains "needle".
        {
            let guard = db.lock().await;
            for i in 0..(MAX_PAGE + 5) {
                let item = copypaste_core::ClipboardItem::new_text(
                    vec![0xAB],
                    vec![0u8; 24],
                    i as i64 + 1,
                );
                copypaste_core::insert_item_with_fts(&guard, &item, &format!("needle row {i}"))
                    .unwrap();
            }
        }

        let resp = call_one(
            &sock,
            r#"{"id":"cap-search","method":"search","params":{"query":"needle","limit":5000}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true, "search should be ok: {resp}");
        let items = resp["data"]["items"].as_array().unwrap();
        assert_eq!(
            items.len(),
            MAX_PAGE,
            "search must clamp to MAX_PAGE={MAX_PAGE}, got {} items",
            items.len()
        );
    }

    /// daemon-core backlog #3: list_view (`history_page`) preview offsets must
    /// not panic on width-changing Unicode normalisation. The sensitive detector
    /// reports byte ranges over the NFKC-normalised string; slicing the original
    /// preview with those offsets used to panic on a non-char-boundary. With a
    /// secret embedded after a ligature/full-width run, the handler must return
    /// without panicking and produce in-range, ordered char offsets.
    #[tokio::test]
    async fn history_page_adversarial_unicode_preview_no_panic() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("adv_unicode.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        // Full-width "AKIA" (U+FF21..) + 16 ASCII chars normalises (NFKC) to a
        // valid AWS access-key id, which the detector flags. The full-width
        // prefix is 3 bytes/char in the original but 1 byte/char after NFKC, so
        // the detector's byte offsets do not line up with the original string —
        // exactly the mismatch that triggered the slice panic.
        let plaintext = "ＡＫＩＡ0123456789ABCDEF and some trailing prose";
        {
            let guard = db.lock().await;
            let item = copypaste_core::ClipboardItem::new_text(vec![0xCD], vec![0u8; 24], 1);
            copypaste_core::insert_item_with_fts(&guard, &item, plaintext).unwrap();
        }

        // Must not panic — a panic in the blocking task would surface as an
        // internal error / dropped connection rather than an `ok` response.
        let resp = call_one(
            &sock,
            r#"{"id":"adv","method":"history_page","params":{"limit":10,"offset":0}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true, "history_page must not panic: {resp}");
        let items = resp["data"]["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        let preview = items[0]["preview"].as_str().unwrap();
        let preview_char_len = preview.chars().count();
        let spans = items[0]["sensitive_spans"].as_array().unwrap();
        for span in spans {
            let pair = span.as_array().unwrap();
            let start = pair[0].as_u64().unwrap() as usize;
            let end = pair[1].as_u64().unwrap() as usize;
            assert!(start <= end, "span start {start} must be <= end {end}");
            assert!(
                end <= preview_char_len,
                "span end {end} must be within preview char-len {preview_char_len}"
            );
        }
    }

    /// `byte_to_char_offset` clamps out-of-range and mid-codepoint byte indices
    /// to a valid char boundary and never panics.
    #[test]
    fn byte_to_char_offset_clamps_and_never_panics() {
        let s = "café"; // 'é' is 2 bytes (0xC3 0xA9): bytes 0..5, chars 0..4
        assert_eq!(byte_to_char_offset(s, 0), 0);
        assert_eq!(byte_to_char_offset(s, 3), 3); // boundary before 'é'
        assert_eq!(byte_to_char_offset(s, 4), 3); // mid-'é' → walk back → 3 chars
        assert_eq!(byte_to_char_offset(s, 5), 4); // end
        assert_eq!(byte_to_char_offset(s, 9999), 4); // past end → clamp to char-len
    }

    // --- FIX 1: history_page returns pinned field and pinned-first order ---

    /// Each item in `history_page` must carry a boolean `pinned` field.
    #[tokio::test]
    async fn history_page_items_include_pinned_field() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hp_pinned_field.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        // Seed one item.
        {
            let guard = db.lock().await;
            let item = copypaste_core::ClipboardItem::new_text(vec![0xAA], vec![0u8; 24], 1);
            copypaste_core::insert_item(&guard, &item).unwrap();
        }

        let resp = call_one(
            &sock,
            r#"{"id":"hpf1","method":"history_page","params":{"limit":10,"offset":0}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true);
        let items = resp["data"]["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        // The `pinned` field must be present and be a boolean.
        assert!(
            items[0]["pinned"].is_boolean(),
            "each item must have a boolean 'pinned' field, got: {}",
            items[0]
        );
        assert_eq!(
            items[0]["pinned"], false,
            "freshly inserted item must have pinned=false"
        );
    }

    /// `history_page` must return pinned items before unpinned items,
    /// regardless of wall_time ordering.
    #[tokio::test]
    async fn history_page_pinned_items_sort_first() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hp_pinned_sort.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        // Insert two items: item_old (lower wall_time) and item_new (higher).
        // Then pin item_old — it must appear first in history_page even though
        // it is older.
        let (id_old, _id_new) = {
            let guard = db.lock().await;
            let mut item_old =
                copypaste_core::ClipboardItem::new_text(vec![0x01], vec![0u8; 24], 1);
            item_old.wall_time = 1_000;
            let id_old = item_old.id.clone();
            copypaste_core::insert_item(&guard, &item_old).unwrap();

            let mut item_new =
                copypaste_core::ClipboardItem::new_text(vec![0x02], vec![0u8; 24], 2);
            item_new.wall_time = 2_000;
            let id_new = item_new.id.clone();
            copypaste_core::insert_item(&guard, &item_new).unwrap();

            (id_old, id_new)
        };

        // Pin the older item via the IPC verb.
        let pin_body = format!(
            r#"{{"id":"hps-pin","method":"pin_item","params":{{"id":"{id_old}","pinned":true}}}}"#
        );
        let pin_resp = call_one(&sock, &pin_body).await;
        assert_eq!(pin_resp["ok"], true, "pin must succeed: {pin_resp}");

        // Now history_page must return item_old first.
        let hp_resp = call_one(
            &sock,
            r#"{"id":"hps-hp","method":"history_page","params":{"limit":10,"offset":0}}"#,
        )
        .await;
        assert_eq!(hp_resp["ok"], true);
        let items = hp_resp["data"]["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0]["id"].as_str().unwrap(),
            id_old,
            "pinned (older) item must be first"
        );
        assert_eq!(items[0]["pinned"], true, "first item must have pinned=true");
        assert_eq!(
            items[1]["pinned"], false,
            "second item must have pinned=false"
        );
    }

    /// After unpinning, the item reverts to recency order in history_page.
    #[tokio::test]
    async fn history_page_unpinned_item_reverts_to_recency_order() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hp_unpin.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        let (id_old, _id_new) = {
            let guard = db.lock().await;
            let mut item_old =
                copypaste_core::ClipboardItem::new_text(vec![0x01], vec![0u8; 24], 1);
            item_old.wall_time = 1_000;
            let id_old = item_old.id.clone();
            copypaste_core::insert_item(&guard, &item_old).unwrap();

            let mut item_new =
                copypaste_core::ClipboardItem::new_text(vec![0x02], vec![0u8; 24], 2);
            item_new.wall_time = 2_000;
            let id_new = item_new.id.clone();
            copypaste_core::insert_item(&guard, &item_new).unwrap();

            (id_old, id_new)
        };

        // Pin then unpin item_old.
        let pin_body = format!(
            r#"{{"id":"hpu-pin","method":"pin_item","params":{{"id":"{id_old}","pinned":true}}}}"#
        );
        call_one(&sock, &pin_body).await;
        let unpin_body = format!(
            r#"{{"id":"hpu-unpin","method":"pin_item","params":{{"id":"{id_old}","pinned":false}}}}"#
        );
        call_one(&sock, &unpin_body).await;

        // After unpin, history_page must return newest-first (item_new first).
        let hp_resp = call_one(
            &sock,
            r#"{"id":"hpu-hp","method":"history_page","params":{"limit":10,"offset":0}}"#,
        )
        .await;
        assert_eq!(hp_resp["ok"], true);
        let items = hp_resp["data"]["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0]["pinned"], false,
            "first item must be unpinned after unpin"
        );
        assert!(
            items[0]["wall_time"].as_i64().unwrap() >= items[1]["wall_time"].as_i64().unwrap(),
            "items must be newest-first after unpin"
        );
    }

    /// In-process burst that exercises the same accept-spawn path used by
    /// the binary subprocess test, but without requiring a built binary.
    /// 10 tokio tasks each issue a status+stats roundtrip on its own
    /// connection; all must succeed.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_clients_in_process_consistent_state() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("concurrent.sock");
        start_test_server(&sock).await;

        const N: usize = 10;
        let mut handles = Vec::with_capacity(N);
        for i in 0..N {
            let sock = sock.clone();
            handles.push(tokio::spawn(async move {
                // status
                let mut s = UnixStream::connect(&sock).await.unwrap();
                let req = format!("{{\"id\":\"c{i}-status\",\"method\":\"status\"}}\n");
                s.write_all(req.as_bytes()).await.unwrap();
                let mut lines = BufReader::new(&mut s).lines();
                let line = lines.next_line().await.unwrap().unwrap();
                let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
                assert_eq!(resp["ok"], true, "client {i} status: {resp}");

                // stats — fresh connection
                let mut s2 = UnixStream::connect(&sock).await.unwrap();
                let req2 = format!("{{\"id\":\"c{i}-stats\",\"method\":\"stats\"}}\n");
                s2.write_all(req2.as_bytes()).await.unwrap();
                let mut lines2 = BufReader::new(&mut s2).lines();
                let line2 = lines2.next_line().await.unwrap().unwrap();
                let resp2: serde_json::Value = serde_json::from_str(&line2).unwrap();
                assert_eq!(resp2["ok"], true, "client {i} stats: {resp2}");
                assert!(resp2["data"]["total_items"].is_number());
            }));
        }
        for h in handles {
            h.await.expect("client task panicked");
        }

        // Survivor request after the burst.
        let mut s = UnixStream::connect(&sock).await.unwrap();
        s.write_all(b"{\"id\":\"survivor\",\"method\":\"status\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut s).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ipc_client_mid_request_disconnect_does_not_panic() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hardening_disconnect.sock");
        start_test_server(&sock).await;

        // Open + close 10 times without writing anything (clean EOF on
        // first read — must be handled, not panic).
        for _ in 0..10 {
            let stream = UnixStream::connect(&sock).await.unwrap();
            drop(stream);
        }

        // Partial write disconnect: write bytes but no newline, then drop.
        // Server's read_until returns >0 bytes then EOF on next iteration.
        {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            stream
                .write_all(b"{\"id\":\"partial\",\"meth")
                .await
                .unwrap();
            drop(stream);
        }

        // Give server tasks a moment to settle.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Fresh client must still get an answer — proves no listener crash.
        let mut stream = UnixStream::connect(&sock)
            .await
            .expect("server must still accept new connections after abrupt disconnects");
        stream
            .write_all(b"{\"id\":\"survivor\",\"method\":\"status\"}\n")
            .await
            .unwrap();
        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        let n = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            reader.read_line(&mut line),
        )
        .await
        .expect("survivor read timed out — server may have crashed")
        .expect("survivor read failed");
        assert!(n > 0, "expected a status response line");
        let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(
            resp["ok"], true,
            "status should be ok after disconnects, got: {resp}"
        );
    }

    /// beta-W3.1 — DB-touching IPC handlers must run on spawn_blocking so a
    /// slow rusqlite read does not block tokio worker threads. We exercise
    /// this by issuing N concurrent `list` requests on a single-threaded
    /// runtime (`#[tokio::test]` default). If any handler held a tokio worker
    /// across the SQLite call, the requests would serialize and the wall
    /// clock would exceed N × per-request latency. With spawn_blocking they
    /// fan out across the blocking pool and complete near-concurrently.
    ///
    /// We assert a *generous* upper bound (well below strict serialization)
    /// rather than a tight one so the test stays robust on slow CI.
    #[tokio::test]
    async fn spawn_blocking_does_not_block_tokio_worker() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test-spawn-blocking.sock");
        start_test_server(&sock).await;

        // Fire 4 concurrent `list` requests, each on its own connection.
        const N: usize = 4;
        let started = std::time::Instant::now();
        let mut handles = Vec::with_capacity(N);
        for i in 0..N {
            let sock_path = sock.clone();
            handles.push(tokio::spawn(async move {
                let mut stream = UnixStream::connect(&sock_path).await.unwrap();
                let payload = format!("{{\"id\":\"sb{i}\",\"method\":\"list\"}}\n");
                stream.write_all(payload.as_bytes()).await.unwrap();
                let mut lines = BufReader::new(&mut stream).lines();
                let line = lines.next_line().await.unwrap().unwrap();
                let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
                assert_eq!(resp["ok"], true, "list must succeed: {line}");
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        let elapsed = started.elapsed();

        // Sanity bound: 4 in-memory `list` calls on an empty DB should finish
        // in well under a second even with sequential serialization, so 5s
        // catches catastrophic regressions (e.g., a single-thread deadlock)
        // without flaking on slow CI runners.
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "4 concurrent list requests took {elapsed:?} — tokio worker likely blocked"
        );
    }

    /// beta-W3.2 — `pair_peer_with_password` validates required params and
    /// returns `not_implemented` once inputs check out, so the UI can rely
    /// on a stable error_code for the not-yet-wired Transport path.
    #[tokio::test]
    async fn pair_peer_with_password_validates_inputs() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test-pair-pw.sock");
        start_test_server(&sock).await;

        async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
            let mut stream = UnixStream::connect(sock).await.unwrap();
            stream.write_all(body.as_bytes()).await.unwrap();
            stream.write_all(b"\n").await.unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            serde_json::from_str(&line).unwrap()
        }

        // Missing peer_fingerprint → invalid_argument
        let resp = call(
            &sock,
            r#"{"id":"p1","method":"pair_peer_with_password","params":{"password":"hunter22"}}"#,
        )
        .await;
        assert_eq!(resp["ok"], false, "missing peer_fingerprint must fail");
        assert_eq!(resp["error_code"], "invalid_argument");

        // Missing password → invalid_argument
        let valid_fp = std::iter::repeat_n("ab", 32).collect::<Vec<_>>().join(":");
        let body = format!(
            r#"{{"id":"p2","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{valid_fp}"}}}}"#
        );
        let resp = call(&sock, &body).await;
        assert_eq!(resp["ok"], false, "missing password must fail");
        assert_eq!(resp["error_code"], "invalid_argument");

        // Short password → invalid_argument (UI enforces but daemon double-checks)
        let body = format!(
            r#"{{"id":"p3","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{valid_fp}","password":"ab"}}}}"#
        );
        let resp = call(&sock, &body).await;
        assert_eq!(resp["ok"], false, "short password must fail");
        assert_eq!(resp["error_code"], "invalid_argument");

        // Bad fingerprint hex → invalid_argument
        let resp = call(
            &sock,
            r#"{"id":"p4","method":"pair_peer_with_password","params":{"peer_fingerprint":"not-hex","password":"hunter22"}}"#,
        )
        .await;
        assert_eq!(resp["ok"], false, "bad fingerprint must fail");
        assert_eq!(resp["error_code"], "invalid_argument");

        // Missing step → defaults to "initiate"; valid request returns session_id + message1_b64
        let body = format!(
            r#"{{"id":"p5","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{valid_fp}","password":"hunter22","step":"initiate"}}}}"#
        );
        let resp = call(&sock, &body).await;
        assert_eq!(resp["ok"], true, "initiate step must succeed: {resp}");
        assert!(
            resp["data"]["session_id"].is_string(),
            "response must contain session_id"
        );
        assert!(
            resp["data"]["message1_b64"].is_string(),
            "response must contain message1_b64"
        );
    }

    /// W2.4 — `pair_peer_with_password` with step="initiate" returns a
    /// session_id and base64-encoded message1 to send to the responder.
    #[tokio::test]
    async fn pair_peer_with_password_initiate_returns_session_and_message1() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test-pake-init.sock");
        start_test_server(&sock).await;

        let valid_fp = std::iter::repeat_n("ab", 32).collect::<Vec<_>>().join(":");
        let body = format!(
            r#"{{"id":"pi1","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{valid_fp}","password":"correct-horse","step":"initiate"}}}}"#
        );
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

        assert_eq!(resp["ok"], true, "initiate must succeed: {resp}");
        let session_id = resp["data"]["session_id"].as_str().unwrap();
        assert!(!session_id.is_empty(), "session_id must not be empty");
        let msg1_b64 = resp["data"]["message1_b64"].as_str().unwrap();
        // Verify it decodes as valid base64 bytes
        use base64::Engine as _;
        let msg1_bytes = base64::engine::general_purpose::STANDARD
            .decode(msg1_b64)
            .expect("message1_b64 must be valid base64");
        assert!(!msg1_bytes.is_empty(), "message1 must not be empty");
    }

    /// W2.4 — `pair_accept_password` returns a session_id and message2 in
    /// response to a valid message1.
    #[tokio::test]
    async fn pair_accept_password_returns_session_and_message2() {
        use base64::Engine as _;
        use copypaste_p2p::pake::PakeInitiator;

        let dir = tempdir().unwrap();
        let sock = dir.path().join("test-pake-accept.sock");
        start_test_server(&sock).await;

        // Simulate the initiator side locally.
        let password = "correct-horse";
        let (_initiator, msg1_bytes) = PakeInitiator::new(password).expect("PakeInitiator::new");
        let msg1_b64 = base64::engine::general_purpose::STANDARD.encode(&msg1_bytes);

        let valid_fp = std::iter::repeat_n("cd", 32).collect::<Vec<_>>().join(":");
        let body = format!(
            r#"{{"id":"pa1","method":"pair_accept_password","params":{{"message1_b64":"{msg1_b64}","peer_fingerprint":"{valid_fp}","password":"{password}"}}}}"#
        );
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

        assert_eq!(
            resp["ok"], true,
            "pair_accept_password must succeed: {resp}"
        );
        assert!(
            resp["data"]["session_id"].is_string(),
            "must return session_id"
        );
        let msg2_b64 = resp["data"]["message2_b64"].as_str().unwrap();
        let msg2_bytes = base64::engine::general_purpose::STANDARD
            .decode(msg2_b64)
            .expect("message2_b64 must be valid base64");
        assert!(!msg2_bytes.is_empty(), "message2 must not be empty");
    }

    /// W2.4 — full PAKE round-trip through IPC: initiator initiate →
    /// responder accept → initiator finish → responder finish → both sides
    /// complete and peer is stored.
    #[tokio::test]
    async fn pair_peer_with_password_full_round_trip() {
        use base64::Engine as _;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let dir = tempdir().unwrap();
        // Redirect config dir so the pairing handlers never write to the
        // developer's real peers.json — and so concurrent tests that also
        // redirect HOME (e.g. revoke_all_peers_revokes_every_peer) don't pick
        // up peers.json entries written by this test's servers. `EnvGuard`
        // holds ENV_LOCK for the duration, serialising env mutations.
        //
        // `COPYPASTE_CONFIG_DIR` is set first because `peers_file_path` checks
        // it ahead of `dirs::config_dir()`; pinning it to this tempdir keeps the
        // test hermetic even when the host/CI environment already exports a
        // `COPYPASTE_CONFIG_DIR` that points at a dir which may not exist.
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(
            &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
            &cfg_home,
        );

        // Use two server instances to simulate two separate daemons.
        let sock_a = dir.path().join("test-pake-rt-a.sock");
        let sock_b = dir.path().join("test-pake-rt-b.sock");
        start_test_server(&sock_a).await;
        start_test_server(&sock_b).await;

        // Helper closure for a single IPC call.
        async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
            let mut stream = UnixStream::connect(sock).await.unwrap();
            stream.write_all(body.as_bytes()).await.unwrap();
            stream.write_all(b"\n").await.unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            serde_json::from_str(&line).unwrap()
        }

        let b64 = base64::engine::general_purpose::STANDARD;
        let password = "correct-horse-battery";
        // Use realistic (non-placeholder) fingerprints — the daemon filters out
        // all-same-byte fingerprints (e.g. aa:aa:...) to drop stale test data
        // from peers.json.
        let fp_a = "a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90:a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90";
        let fp_b = "f0:e1:d2:c3:b4:a5:96:87:78:69:5a:4b:3c:2d:1e:0f:f0:e1:d2:c3:b4:a5:96:87:78:69:5a:4b:3c:2d:1e:0f";

        // Step 1: Device A initiates.
        let body = format!(
            r#"{{"id":"rt1","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{fp_b}","password":"{password}","step":"initiate"}}}}"#
        );
        let resp = call(&sock_a, &body).await;
        assert_eq!(resp["ok"], true, "initiate step failed: {resp}");
        let session_id_a = resp["data"]["session_id"].as_str().unwrap().to_string();
        let msg1_b64 = resp["data"]["message1_b64"].as_str().unwrap().to_string();

        // Step 2: Device B accepts (responder side).
        let body = format!(
            r#"{{"id":"rt2","method":"pair_accept_password","params":{{"message1_b64":"{msg1_b64}","peer_fingerprint":"{fp_a}","password":"{password}"}}}}"#
        );
        let resp = call(&sock_b, &body).await;
        assert_eq!(resp["ok"], true, "pair_accept_password failed: {resp}");
        let session_id_b = resp["data"]["session_id"].as_str().unwrap().to_string();
        let msg2_b64 = resp["data"]["message2_b64"].as_str().unwrap().to_string();

        // Step 3: Device A finishes.
        let body = format!(
            r#"{{"id":"rt3","method":"pair_peer_with_password","params":{{"step":"finish","session_id":"{session_id_a}","message2_b64":"{msg2_b64}","peer_fingerprint":"{fp_b}","password":"{password}"}}}}"#
        );
        let resp = call(&sock_a, &body).await;
        assert_eq!(resp["ok"], true, "initiator finish failed: {resp}");
        let msg3_b64 = resp["data"]["message3_b64"].as_str().unwrap().to_string();

        // Step 4: Device B finishes.
        let body = format!(
            r#"{{"id":"rt4","method":"pair_accept_finish","params":{{"session_id":"{session_id_b}","message3_b64":"{msg3_b64}","peer_fingerprint":"{fp_a}"}}}}"#
        );
        let resp = call(&sock_b, &body).await;
        assert_eq!(resp["ok"], true, "responder finish failed: {resp}");
        assert_eq!(
            resp["data"]["ok"], true,
            "pair_accept_finish data.ok must be true"
        );

        // Verify Device B stored the peer (with password_file_b64) in peers.json.
        // We check via the list_peers IPC method.
        let list_resp = call(&sock_b, r#"{"id":"rt5","method":"list_peers","params":{}}"#).await;
        assert_eq!(list_resp["ok"], true, "list_peers failed: {list_resp}");
        let peers = list_resp["data"]["peers"].as_array().unwrap();
        let stored = peers.iter().find(|p| {
            p.get("fingerprint")
                .and_then(|v| v.as_str())
                .map(|f| f == fp_a)
                .unwrap_or(false)
        });
        assert!(
            stored.is_some(),
            "peer {fp_a} must be stored on device B after finish"
        );

        // Verify the stored peer has the password_file_b64 field (PasswordFile blob).
        let pf_b64 = stored
            .unwrap()
            .get("password_file_b64")
            .and_then(|v| v.as_str());
        assert!(pf_b64.is_some(), "peer must have password_file_b64 stored");
        let pf_bytes = b64
            .decode(pf_b64.unwrap())
            .expect("password_file_b64 is valid base64");
        assert!(!pf_bytes.is_empty(), "PasswordFile blob must not be empty");

        // Verify Device A also stored the peer (without PasswordFile — initiator side).
        let list_resp = call(&sock_a, r#"{"id":"rt6","method":"list_peers","params":{}}"#).await;
        assert_eq!(list_resp["ok"], true, "list_peers on A failed: {list_resp}");
        let peers = list_resp["data"]["peers"].as_array().unwrap();
        let stored_a = peers.iter().find(|p| {
            p.get("fingerprint")
                .and_then(|v| v.as_str())
                .map(|f| f == fp_b)
                .unwrap_or(false)
        });
        assert!(
            stored_a.is_some(),
            "peer {fp_b} must be stored on device A after finish"
        );
    }

    /// QR pairing end-to-end: device B (displaying) generates a QR, device A
    /// (scanning) decodes it via `copypaste_core::PairingPayload`, derives the
    /// PAKE password from the embedded token, and completes the 4-message
    /// handshake using `pair_accept_qr` on B in place of `pair_accept_password`.
    /// No password is ever typed — it travels in the QR token.
    #[tokio::test]
    async fn pair_qr_full_round_trip() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let dir = tempdir().unwrap();
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(&["HOME", "XDG_CONFIG_HOME"], &cfg_home);

        let sock_a = dir.path().join("test-qr-a.sock");
        let sock_b = dir.path().join("test-qr-b.sock");
        start_test_server(&sock_a).await;
        start_test_server(&sock_b).await;

        async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
            let mut stream = UnixStream::connect(sock).await.unwrap();
            stream.write_all(body.as_bytes()).await.unwrap();
            stream.write_all(b"\n").await.unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            serde_json::from_str(&line).unwrap()
        }

        // Realistic non-placeholder fingerprints (all-same-byte ones are
        // filtered as stale test data by the peer store).
        let fp_a = "a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90:a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90";

        // Step 0: Device B generates a QR pairing code.
        let resp = call(
            &sock_b,
            r#"{"id":"qr0","method":"pair_generate_qr","params":{}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true, "pair_generate_qr failed: {resp}");
        let qr = resp["data"]["qr"].as_str().unwrap().to_string();
        assert!(qr.starts_with("CPPAIR1."), "QR must use the v1 magic: {qr}");

        // Step 0b: Device A scans + decodes the QR and derives the PAKE password.
        let payload = copypaste_core::PairingPayload::decode(&qr)
            .expect("scanning device must decode the QR");
        let password = payload.token.to_pake_password();
        // The fingerprint A pins is the one carried in the QR (B's fingerprint).
        let fp_b = payload.fingerprint.clone();
        assert!(!fp_b.is_empty(), "QR must carry B's fingerprint");

        // Step 1: Device A initiates using the QR-derived password.
        let body = format!(
            r#"{{"id":"qr1","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{fp_b}","password":"{password}","step":"initiate"}}}}"#
        );
        let resp = call(&sock_a, &body).await;
        assert_eq!(resp["ok"], true, "initiate failed: {resp}");
        let session_id_a = resp["data"]["session_id"].as_str().unwrap().to_string();
        let msg1_b64 = resp["data"]["message1_b64"].as_str().unwrap().to_string();

        // Step 2: Device B accepts via pair_accept_qr (looks up its stored token).
        let body = format!(
            r#"{{"id":"qr2","method":"pair_accept_qr","params":{{"message1_b64":"{msg1_b64}","peer_fingerprint":"{fp_a}"}}}}"#
        );
        let resp = call(&sock_b, &body).await;
        assert_eq!(resp["ok"], true, "pair_accept_qr failed: {resp}");
        let session_id_b = resp["data"]["session_id"].as_str().unwrap().to_string();
        let msg2_b64 = resp["data"]["message2_b64"].as_str().unwrap().to_string();

        // Step 3: Device A finishes.
        let body = format!(
            r#"{{"id":"qr3","method":"pair_peer_with_password","params":{{"step":"finish","session_id":"{session_id_a}","message2_b64":"{msg2_b64}","peer_fingerprint":"{fp_b}","password":"{password}"}}}}"#
        );
        let resp = call(&sock_a, &body).await;
        assert_eq!(resp["ok"], true, "initiator finish failed: {resp}");
        let msg3_b64 = resp["data"]["message3_b64"].as_str().unwrap().to_string();

        // Step 4: Device B finishes — the OPAQUE authenticator must validate,
        // proving both sides agreed on the QR token as the shared secret.
        let body = format!(
            r#"{{"id":"qr4","method":"pair_accept_finish","params":{{"session_id":"{session_id_b}","message3_b64":"{msg3_b64}","peer_fingerprint":"{fp_a}"}}}}"#
        );
        let resp = call(&sock_b, &body).await;
        assert_eq!(resp["ok"], true, "responder finish failed: {resp}");
        assert_eq!(resp["data"]["ok"], true, "pair_accept_finish must succeed");
    }

    /// `pair_accept_qr` with no prior `pair_generate_qr` must be rejected
    /// rather than registering an empty/garbage PasswordFile.
    #[tokio::test]
    async fn pair_accept_qr_without_token_is_rejected() {
        use base64::Engine as _;
        let dir = tempdir().unwrap();
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(&["HOME", "XDG_CONFIG_HOME"], &cfg_home);
        let sock = dir.path().join("test-qr-notoken.sock");
        start_test_server(&sock).await;

        let fp = "a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90:a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90";
        let msg1 = base64::engine::general_purpose::STANDARD.encode([0u8; 32]);
        let body = format!(
            r#"{{"id":"nt1","method":"pair_accept_qr","params":{{"message1_b64":"{msg1}","peer_fingerprint":"{fp}"}}}}"#
        );
        let resp = call_one(&sock, &body).await;
        assert_eq!(
            resp["ok"], false,
            "pair_accept_qr without a generated token must fail: {resp}"
        );
    }

    /// T4 (v0.3) — `revoke_peer` validates its fingerprint argument and, for
    /// a well-formed request, writes a row to the `revoked_devices` audit
    /// table even when the peer was never in the local JSON peer store
    /// (revoking an unknown fingerprint is intentionally allowed so the UI
    /// can recover from a corrupted peers.json).
    #[tokio::test]
    async fn revoke_peer_validates_and_records_audit_row() {
        use copypaste_core::list_revoked_devices;

        let dir = tempdir().unwrap();
        let sock = dir.path().join("test-revoke.sock");

        // Redirect the config dir to this test's own tempdir so the
        // `revoke_peer` handler's `save_peers` never writes to (and never
        // depends on the existence of) the machine's real config dir. Under
        // parallel CI execution the platform `dirs::config_dir()` may not
        // exist, which previously made `save_peers` fail with ENOENT. Setting
        // `COPYPASTE_CONFIG_DIR` (checked first by `peers_file_path`) plus the
        // HOME/XDG fallbacks makes the test fully hermetic. `EnvGuard` holds
        // the process-wide `TEST_ENV_LOCK` for its lifetime, so this does not
        // race the other env-mutating tests in the workspace.
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(
            &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
            &cfg_home,
        );

        // Build the server manually so we can reach the shared Database
        // handle for assertions after the call.
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let private_mode = Arc::new(AtomicBool::new(false));
        let server = IpcServer::new(
            db.clone(),
            private_mode,
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        );
        let sock_path = sock.clone();
        tokio::spawn(async move {
            if let Err(e) = server.serve(&sock_path, CancellationToken::new()).await {
                tracing::error!("ipc: server on {:?} exited with error: {e}", &sock_path);
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
            let mut stream = UnixStream::connect(sock).await.unwrap();
            stream.write_all(body.as_bytes()).await.unwrap();
            stream.write_all(b"\n").await.unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            serde_json::from_str(&line).unwrap()
        }

        // Missing fingerprint → invalid_argument
        let resp = call(&sock, r#"{"id":"r1","method":"revoke_peer","params":{}}"#).await;
        assert_eq!(resp["ok"], false, "missing fingerprint must fail");
        assert_eq!(resp["error_code"], "invalid_argument");

        // Bad fingerprint hex → invalid_argument
        let resp = call(
            &sock,
            r#"{"id":"r2","method":"revoke_peer","params":{"fingerprint":"not-hex"}}"#,
        )
        .await;
        assert_eq!(resp["ok"], false, "bad fingerprint must fail");
        assert_eq!(resp["error_code"], "invalid_argument");

        // Valid request — unknown peer, but revoke still succeeds and writes
        // the audit row.
        let fp = std::iter::repeat_n("ab", 32).collect::<Vec<_>>().join(":");
        let body =
            format!(r#"{{"id":"r3","method":"revoke_peer","params":{{"fingerprint":"{fp}"}}}}"#);
        let resp = call(&sock, &body).await;
        assert_eq!(resp["ok"], true, "valid revoke must succeed: {resp}");
        assert_eq!(resp["data"]["fingerprint"], fp);
        assert!(
            resp["data"]["revoked_at"].as_u64().unwrap_or(0) > 0,
            "revoked_at must be populated"
        );

        // Audit row must be persisted in the shared SQLite DB.
        let db_guard = db.lock().await;
        let rows = list_revoked_devices(db_guard.conn()).unwrap();
        assert_eq!(rows.len(), 1, "exactly one audit row expected");
        assert_eq!(rows[0].fingerprint, fp);
    }

    // ------------------------------------------------------------------
    // T5.x — clipboard-history UI action wiring
    //
    // New verbs added so the UI can drive history actions end-to-end over
    // the Unix socket: `pin_item`, `delete_item`, `copy_item`, and
    // `revoke_all_peers`. Each validates its arguments and returns the
    // documented error code on missing/bad params, mirroring the
    // beta-W3.2 (`pair_peer_with_password`) and T4 (`revoke_peer`) tests.
    // ------------------------------------------------------------------

    async fn call_one(sock: &std::path::Path, body: &str) -> serde_json::Value {
        let mut stream = UnixStream::connect(sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        serde_json::from_str(&line).unwrap()
    }

    /// Build a bare in-process `IpcServer` (no socket) for exercising private
    /// helpers like `insert_pake_session` directly.
    fn bare_server() -> IpcServer {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        IpcServer::new(
            db,
            Arc::new(AtomicBool::new(false)),
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        )
    }

    /// CRITICAL-1: `display_fingerprint` renders the mTLS canonical fingerprint
    /// (colon-free 64-hex from `fingerprint_of`) into the user-facing colon-hex
    /// form, and `canonical_fingerprint` round-trips it back to the exact value
    /// the mTLS verifier compares — so a pinned QR fingerprint authenticates.
    #[test]
    fn display_fingerprint_round_trips_cert_fingerprint() {
        let cert = copypaste_p2p::cert::SelfSignedCert::generate("rt-device").unwrap();
        let canonical = cert.fingerprint(); // hex(SHA-256(cert_der)), 64 hex chars, no colons
        assert_eq!(canonical.len(), 64, "cert fingerprint must be 64 hex chars");

        let display = display_fingerprint(&canonical);
        // 32 colon-separated 2-hex groups.
        assert_eq!(
            display.split(':').count(),
            32,
            "must be 32 colon groups: {display}"
        );
        assert!(
            is_valid_fingerprint(&display),
            "display form must validate: {display}"
        );

        // The mTLS boundary strips colons; it MUST equal the original canonical
        // value the verifier (`fingerprint_of`) produces.
        assert_eq!(
            canonical_fingerprint(&display),
            canonical,
            "round-trip must recover the exact canonical fingerprint the verifier pins"
        );
    }

    /// CRITICAL-1: with no cert fingerprint set (P2P disabled), the pairing
    /// handlers must refuse rather than advertise the device-key fingerprint the
    /// mTLS layer never pins.
    #[tokio::test]
    async fn pairing_handlers_error_when_p2p_disabled() {
        let server = bare_server(); // no .with_cert_fingerprint → cert_fingerprint == None

        let resp = server
            .dispatch(r#"{"id":"f1","method":"get_own_fingerprint","params":{}}"#)
            .await;
        assert!(!resp.ok, "get_own_fingerprint must error without a cert");
        assert!(
            resp.error
                .as_deref()
                .unwrap_or_default()
                .contains("P2P is disabled"),
            "must be the disabled-P2P error, not a parse error: {resp:?}"
        );

        let resp = server
            .dispatch(r#"{"id":"q1","method":"pair_generate_qr","params":{}}"#)
            .await;
        assert!(!resp.ok, "pair_generate_qr must error without a cert");
        assert!(
            resp.error
                .as_deref()
                .unwrap_or_default()
                .contains("P2P is disabled"),
            "must be the disabled-P2P error, not a parse error: {resp:?}"
        );
    }

    /// CRITICAL-1: when a cert fingerprint IS configured, `get_own_fingerprint`
    /// returns exactly that colon-hex cert fingerprint (not the device key).
    #[tokio::test]
    async fn get_own_fingerprint_returns_cert_fingerprint() {
        let cert = copypaste_p2p::cert::SelfSignedCert::generate("own-fp-device").unwrap();
        let expected = display_fingerprint(&cert.fingerprint());
        let server = bare_server().with_cert_fingerprint(expected.clone());

        let resp = server
            .dispatch(r#"{"id":"f2","method":"get_own_fingerprint","params":{}}"#)
            .await;
        assert!(resp.ok, "must succeed with a cert: {resp:?}");
        let data = resp.data.expect("data present");
        assert_eq!(data["fingerprint"], serde_json::Value::String(expected));
    }

    /// fix/p2p-c-review #1 — a session older than `PAKE_SESSION_TTL` is evicted
    /// on the next `insert_pake_session`, so the map cannot grow with abandoned
    /// (crashed-client) sessions.
    #[tokio::test]
    async fn stale_pake_sessions_are_evicted_on_insert() {
        let server = bare_server();

        // Insert a first session, then back-date it past the TTL so it is
        // considered stale. (`Instant` can't be constructed directly; we patch
        // the stored `created_at` in place — this module has field access.)
        let (init1, _msg1) = PakeInitiator::new("hunter2-pw").unwrap();
        server
            .insert_pake_session("stale".into(), PakeSession::Initiator(Box::new(init1)))
            .await
            .unwrap();
        {
            let mut sessions = server.pake_sessions.lock().await;
            let stamped = sessions.get_mut("stale").expect("stale session present");
            stamped.created_at =
                std::time::Instant::now() - (PAKE_SESSION_TTL + std::time::Duration::from_secs(1));
        }

        // Inserting a fresh session triggers TTL eviction of the stale one.
        let (init2, _msg2) = PakeInitiator::new("hunter2-pw").unwrap();
        server
            .insert_pake_session("fresh".into(), PakeSession::Initiator(Box::new(init2)))
            .await
            .unwrap();

        let sessions = server.pake_sessions.lock().await;
        assert!(
            !sessions.contains_key("stale"),
            "stale session must be evicted on insert"
        );
        assert!(
            sessions.contains_key("fresh"),
            "fresh session must remain after eviction pass"
        );
        assert_eq!(sessions.len(), 1, "exactly one live session expected");
    }

    /// fix/p2p-c-review #1 — once `MAX_PAKE_SESSIONS` non-stale sessions are
    /// live, a further insert is rejected (rather than growing without bound).
    #[tokio::test]
    async fn pake_session_cap_rejects_excess() {
        let server = bare_server();

        for i in 0..MAX_PAKE_SESSIONS {
            let (init, _m) = PakeInitiator::new("hunter2-pw").unwrap();
            server
                .insert_pake_session(format!("s{i}"), PakeSession::Initiator(Box::new(init)))
                .await
                .expect("inserts up to the cap must succeed");
        }

        let (init, _m) = PakeInitiator::new("hunter2-pw").unwrap();
        let over_cap = server
            .insert_pake_session("over".into(), PakeSession::Initiator(Box::new(init)))
            .await;
        assert!(over_cap.is_err(), "insert past the cap must be rejected");
        assert_eq!(
            server.pake_sessions.lock().await.len(),
            MAX_PAKE_SESSIONS,
            "map must not exceed the cap"
        );
    }

    /// fix/p2p-c-review #5 — the responder (`pair_accept_password`) enforces the
    /// 6-char minimum password, matching the initiator side.
    #[tokio::test]
    async fn pair_accept_password_rejects_short_password() {
        use base64::Engine as _;

        let dir = tempdir().unwrap();
        let sock = dir.path().join("test-short-pw.sock");
        start_test_server(&sock).await;

        let (_init, msg1) = PakeInitiator::new("short").unwrap();
        let msg1_b64 = base64::engine::general_purpose::STANDARD.encode(&msg1);
        let fp = std::iter::repeat_n("ab", 32).collect::<Vec<_>>().join(":");
        let body = format!(
            r#"{{"id":"sp1","method":"pair_accept_password","params":{{"message1_b64":"{msg1_b64}","peer_fingerprint":"{fp}","password":"short"}}}}"#
        );
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

        assert_eq!(
            resp["ok"], false,
            "5-char password must be rejected: {resp}"
        );
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    /// fix/p2p-c-review #2 — when a live P2P allowlist is attached, finishing a
    /// PAKE pairing registers the peer in it (normalised to canonical hex) so
    /// the mTLS accept loop honours the peer without a restart.
    #[tokio::test]
    async fn register_live_peer_feeds_shared_allowlist() {
        let peers = copypaste_p2p::transport::PairedPeers::new();
        let server = bare_server().with_p2p_peers(peers.clone());

        let colon_fp = std::iter::repeat_n("aa", 32).collect::<Vec<_>>().join(":");
        let canonical = canonical_fingerprint(&colon_fp);
        assert!(!peers.is_known(&canonical), "precondition: not yet known");

        server.register_live_peer(&colon_fp);

        assert!(
            peers.is_known(&canonical),
            "paired peer must be accepted by the live allowlist after finish"
        );
    }

    #[tokio::test]
    async fn pin_item_missing_id_returns_invalid_argument() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pin_item_missing.sock");
        start_test_server(&sock).await;
        let resp = call_one(
            &sock,
            r#"{"id":"pi1","method":"pin_item","params":{"pinned":true}}"#,
        )
        .await;
        assert_eq!(resp["ok"], false, "missing id must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn pin_item_missing_pinned_returns_invalid_argument() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pin_item_no_flag.sock");
        start_test_server(&sock).await;
        let fp_id = "00000000-0000-0000-0000-000000000000";
        let body = format!(r#"{{"id":"pi2","method":"pin_item","params":{{"id":"{fp_id}"}}}}"#);
        let resp = call_one(&sock, &body).await;
        assert_eq!(resp["ok"], false, "missing pinned bool must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn pin_item_bad_uuid_returns_invalid_argument() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pin_item_bad_uuid.sock");
        start_test_server(&sock).await;
        let resp = call_one(
            &sock,
            r#"{"id":"pi3","method":"pin_item","params":{"id":"not-a-uuid","pinned":true}}"#,
        )
        .await;
        assert_eq!(resp["ok"], false, "bad uuid must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn pin_item_valid_uuid_pins_and_unpins() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pin_item_ok.sock");
        start_test_server(&sock).await;
        let id = "00000000-0000-0000-0000-000000000000";
        // Pin: even when the row does not exist, the UPDATE affects 0 rows
        // and succeeds (the UI optimistically pins; a stale id is harmless).
        let body =
            format!(r#"{{"id":"pi4","method":"pin_item","params":{{"id":"{id}","pinned":true}}}}"#);
        let resp = call_one(&sock, &body).await;
        assert_eq!(resp["ok"], true, "valid pin must succeed: {resp}");
        assert_eq!(resp["data"]["pinned"], true);
        assert_eq!(resp["data"]["id"], id);
        // Unpin path.
        let body = format!(
            r#"{{"id":"pi5","method":"pin_item","params":{{"id":"{id}","pinned":false}}}}"#
        );
        let resp = call_one(&sock, &body).await;
        assert_eq!(resp["ok"], true, "valid unpin must succeed: {resp}");
        assert_eq!(resp["data"]["pinned"], false);
    }

    #[tokio::test]
    async fn delete_item_missing_id_returns_invalid_argument() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("del_item_missing.sock");
        start_test_server(&sock).await;
        let resp = call_one(&sock, r#"{"id":"di1","method":"delete_item","params":{}}"#).await;
        assert_eq!(resp["ok"], false, "missing id must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn delete_item_bad_uuid_returns_invalid_argument() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("del_item_bad_uuid.sock");
        start_test_server(&sock).await;
        let resp = call_one(
            &sock,
            r#"{"id":"di2","method":"delete_item","params":{"id":"not-a-uuid"}}"#,
        )
        .await;
        assert_eq!(resp["ok"], false, "bad uuid must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn delete_item_valid_uuid_succeeds() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("del_item_ok.sock");
        start_test_server(&sock).await;
        let id = "00000000-0000-0000-0000-000000000000";
        let body = format!(r#"{{"id":"di3","method":"delete_item","params":{{"id":"{id}"}}}}"#);
        let resp = call_one(&sock, &body).await;
        // Deleting a non-existent row is a no-op DELETE → request still ok,
        // but `deleted` reflects rows-affected (0 → false) so the response
        // matches reality rather than always claiming a deletion happened.
        assert_eq!(resp["ok"], true, "valid delete must succeed: {resp}");
        assert_eq!(resp["data"]["deleted"], false, "no row existed: {resp}");
        assert_eq!(resp["data"]["id"], id);
    }

    #[tokio::test]
    async fn copy_item_missing_id_returns_invalid_argument() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_item_missing.sock");
        start_test_server(&sock).await;
        let resp = call_one(&sock, r#"{"id":"ci1","method":"copy_item","params":{}}"#).await;
        assert_eq!(resp["ok"], false, "missing id must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn copy_item_bad_uuid_returns_invalid_argument() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_item_bad_uuid.sock");
        start_test_server(&sock).await;
        let resp = call_one(
            &sock,
            r#"{"id":"ci2","method":"copy_item","params":{"id":"not-a-uuid"}}"#,
        )
        .await;
        assert_eq!(resp["ok"], false, "bad uuid must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn copy_item_unknown_id_returns_not_found() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_item_unknown.sock");
        start_test_server(&sock).await;
        let id = "00000000-0000-0000-0000-000000000000";
        let body = format!(r#"{{"id":"ci3","method":"copy_item","params":{{"id":"{id}"}}}}"#);
        let resp = call_one(&sock, &body).await;
        assert_eq!(resp["ok"], false, "unknown id must fail");
        assert_eq!(resp["error_code"], "not_found");
    }

    #[tokio::test]
    async fn copy_item_seeded_id_is_resolved() {
        // Regression for the data-loss fix: copy_item must resolve a row by its
        // primary key (`get_item_by_id`) rather than paging + scanning. We seed
        // a text item with a deliberately wrong-length nonce so the paste-back
        // path returns a deterministic error *without* touching the real
        // NSPasteboard — the key assertion is that the lookup found the row, so
        // the response is anything except `not_found`.
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_item_seeded.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        let id = {
            let guard = db.lock().await;
            // 0xAA/0xBB content with a 1-byte nonce (invalid: must be 24) so
            // write_to_pasteboard short-circuits before any NSPasteboard call.
            let item = copypaste_core::ClipboardItem::new_text(vec![0xAA, 0xBB], vec![0u8; 1], 1);
            let id = item.id.clone();
            copypaste_core::insert_item(&guard, &item).unwrap();
            id
        };

        let body = format!(r#"{{"id":"ci4","method":"copy_item","params":{{"id":"{id}"}}}}"#);
        let resp = call_one(&sock, &body).await;
        assert_ne!(
            resp["error_code"], "not_found",
            "seeded item must be resolved by id, not reported missing: {resp}"
        );
    }

    #[tokio::test]
    async fn revoke_all_peers_empty_store_succeeds() {
        // With no peers.json present, revoke_all_peers must succeed and
        // report zero revoked rather than erroring.
        let dir = tempdir().unwrap();
        let sock = dir.path().join("revoke_all_empty.sock");
        // Isolate the config dir so this test never touches the developer's
        // real peers.json. `dirs::config_dir()` reads XDG_CONFIG_HOME on
        // Linux/BSD and $HOME (→ Library/Application Support) on macOS, so set
        // both. Held until end of test (RAII restore).
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(&["HOME", "XDG_CONFIG_HOME"], &cfg_home);
        start_test_server(&sock).await;
        let resp = call_one(
            &sock,
            r#"{"id":"ra1","method":"revoke_all_peers","params":{}}"#,
        )
        .await;
        assert_eq!(
            resp["ok"], true,
            "revoke_all on empty store must succeed: {resp}"
        );
        assert_eq!(
            resp["data"]["revoked"].as_u64(),
            Some(0),
            "empty store revokes zero peers: {resp}"
        );
    }

    #[tokio::test]
    async fn revoke_all_peers_revokes_every_peer() {
        // Happy path: seed N peers in peers.json, call revoke_all_peers, and
        // assert all N are revoked, the store is cleared, and an audit row was
        // written for each (atomic batch via revoke_devices).
        let dir = tempdir().unwrap();
        let sock = dir.path().join("revoke_all_n.sock");
        // Redirect the config dir (both Linux XDG and macOS HOME) to a temp
        // path so we read/write an isolated peers.json, never the real one.
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(&["HOME", "XDG_CONFIG_HOME"], &cfg_home);

        // Resolve the actual peers.json location the same way the daemon does
        // (`dirs::config_dir()/copypaste/peers.json`) so the seed lands exactly
        // where the handler will read it, on whatever platform we run.
        let peers_dir = dirs::config_dir()
            .expect("config_dir resolvable under redirected HOME/XDG_CONFIG_HOME")
            .join("copypaste");
        std::fs::create_dir_all(&peers_dir).unwrap();
        let peers_json = peers_dir.join("peers.json");
        // Use realistic (non-placeholder) fingerprints — the daemon filters out
        // all-same-byte fingerprints (e.g. aa:aa:aa:aa:aa:aa:aa:aa) to drop
        // stale test data from peers.json.
        let peers = serde_json::json!([
            {"name": "Laptop", "fingerprint": "a1:b2:c3:d4:e5:f6:07:18", "added_at": 1},
            {"name": "Phone",  "fingerprint": "f0:e1:d2:c3:b4:a5:96:87", "added_at": 2},
            {"name": "Tablet", "fingerprint": "12:34:56:78:9a:bc:de:f0", "added_at": 3},
        ]);
        std::fs::write(&peers_json, serde_json::to_string(&peers).unwrap()).unwrap();

        let (_pm, db) = start_test_server_returning_db(&sock, false).await;
        let resp = call_one(
            &sock,
            r#"{"id":"ra2","method":"revoke_all_peers","params":{}}"#,
        )
        .await;

        assert_eq!(resp["ok"], true, "revoke_all must succeed: {resp}");
        assert_eq!(
            resp["data"]["revoked"].as_u64(),
            Some(3),
            "all three peers must be revoked: {resp}"
        );
        assert_eq!(resp["data"]["cleared"].as_u64(), Some(3));

        // Store must now be empty.
        let remaining = std::fs::read_to_string(&peers_json).unwrap_or_else(|_| "[]".into());
        let remaining: Vec<serde_json::Value> = serde_json::from_str(&remaining).unwrap();
        assert!(remaining.is_empty(), "peer store must be cleared");

        // An audit row must exist for every revoked fingerprint.
        let audit = {
            let guard = db.lock().await;
            copypaste_core::list_revoked_devices(guard.conn()).unwrap()
        };
        assert_eq!(audit.len(), 3, "one audit row per revoked peer");
        for fp in [
            "a1:b2:c3:d4:e5:f6:07:18",
            "f0:e1:d2:c3:b4:a5:96:87",
            "12:34:56:78:9a:bc:de:f0",
        ] {
            assert!(
                audit.iter().any(|r| r.fingerprint == fp),
                "missing audit row for {fp}"
            );
        }
    }

    /// BUG 2 — `get_sync_status` must report the REAL `signed_in` auth state
    /// published by the cloud loops via the shared `cloud_signed_in` flag, not
    /// the old hardcoded `signed_in = supabase_configured`. We build a server,
    /// wire a shared flag, and assert the IPC response tracks the flag both ways.
    #[cfg(feature = "cloud-sync")]
    #[tokio::test]
    async fn get_sync_status_reports_real_signed_in_flag() {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let private_mode = Arc::new(AtomicBool::new(false));
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let device_pub = Arc::new([0u8; 32]);

        let sync_key = Arc::new(Mutex::new(None));
        let last_sync_ms = Arc::new(std::sync::atomic::AtomicI64::new(0));
        let signed_in = Arc::new(AtomicBool::new(false));

        let server = IpcServer::new(db, private_mode, local_key, device_pub).with_cloud_sync_state(
            sync_key,
            last_sync_ms,
            signed_in.clone(),
        );

        let line = r#"{"id":"1","method":"get_sync_status","params":{}}"#;

        // Flag false (e.g. after CloudError::AuthFailed) → signed_in == false,
        // even though supabase may be "configured".
        let resp = server.dispatch(line).await;
        let data = resp.data.expect("get_sync_status must return data");
        assert_eq!(
            data["signed_in"], false,
            "signed_in must reflect the false auth flag, not supabase_configured: {data}"
        );

        // Flip the shared flag true (successful bearer resolution) → reflected.
        signed_in.store(true, Ordering::Relaxed);
        let resp2 = server.dispatch(line).await;
        let data2 = resp2.data.expect("get_sync_status must return data");
        assert_eq!(
            data2["signed_in"], true,
            "signed_in must track the real auth flag once set true: {data2}"
        );
    }
}
