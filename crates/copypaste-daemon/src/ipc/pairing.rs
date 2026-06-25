//! P2P pairing helpers: fingerprint normalisation, PAKE session types,
//! peer list management, and related utilities.
//!
//! Extracted from `ipc.rs` for organisation — behaviour unchanged.
//! All public items are re-exported from `ipc/mod.rs`.

use super::config::config_base_dir;
use copypaste_core::{decrypt_item_with_aad, encrypt_item_with_aad, NONCE_SIZE};
use copypaste_p2p::pake::{PakeInitiator, PakeResponder, PasswordFile};
use std::path::PathBuf;
use std::sync::Arc;

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
pub(crate) fn byte_to_char_offset(s: &str, byte: usize) -> usize {
    let mut idx = byte.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    s[..idx].chars().count()
}

/// Whether a stored item would be dropped by the local sync pipeline for being
/// too large, so the UIs can badge it. This is the single source of truth —
/// the desktop/Android UIs just read the `too_large_to_sync` boolean.
///
/// Threshold: [`crate::sync_orch::SYNC_MAX_BLOB_BYTES`] (8 MiB). This is the
/// ceiling the *local* sync pipeline actually enforces on the wrapped plaintext
/// for ALL content types — text via
/// `wrap_and_check_cloud_upload_plaintext` and image/file
/// via `crate::sync_orch::rekey_blob_outbound`. An item above this size is kept
/// locally but never forwarded, regardless of the relay's nominal 10 MiB
/// image/file tier (a higher transport cap, not what drops the item). Using the
/// same constant keeps the badge faithful to what really won't sync.
///
/// Size source: the stored `content` blob length. `content` is the at-rest
/// CIPHERTEXT (text: XChaCha20-Poly1305 ct = plaintext + 16-byte tag, nonce
/// stored separately; image/file: chunked self-framed blob), whereas the sync
/// path measures the recovered PLAINTEXT. Ciphertext is always >= plaintext, so
/// comparing the stored blob length against the ceiling is a safe, conservative
/// proxy: it never under-reports an oversized item, and the only inaccuracy is
/// a thin band just under 8 MiB where AEAD/chunk overhead tips the ciphertext
/// over. Decrypting every row purely to measure exact plaintext is not worth
/// the cost for a list-view badge, so we use the cheaply-available blob length.
pub(crate) fn too_large_to_sync(item: &copypaste_core::ClipboardItem) -> bool {
    item.content
        .as_ref()
        .is_some_and(|c| c.len() > crate::sync_orch::SYNC_MAX_BLOB_BYTES)
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
    // Share the resolver with `config_path` so config.json and peers.json
    // always co-locate under the same directory. `config_base_dir` now
    // delegates to `paths::config_dir()` which is infallible, so the
    // None case (fallback to `./copypaste`) is only reached if somehow
    // config_base_dir returns None (currently unreachable).
    config_base_dir()
        .unwrap_or_else(|| {
            FALLBACK_WARNED.get_or_init(|| {
                tracing::warn!(
                    "neither COPYPASTE_CONFIG_DIR nor dirs::config_dir() available — \
                     falling back to CWD for peers.json. Set $XDG_CONFIG_HOME or $HOME \
                     to silence this warning."
                );
            });
            PathBuf::from(".").join("copypaste")
        })
        .join("peers.json")
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

/// AAD prefix for PAKE `PasswordFile` at-rest encryption (CopyPaste-5lm).
///
/// The full AAD is `b"pake_password_file|{canonical_fingerprint}"`, binding
/// the ciphertext to both its purpose and the specific peer it belongs to.
/// This prevents a ciphertext from one peer record from being transplanted
/// into another peer record (AEAD auth tag would reject the mismatched AAD).
const PAKE_PASSWORD_FILE_AAD_PREFIX: &[u8] = b"pake_password_file|";

/// Encrypt the raw `PasswordFile` blob for at-rest storage in `peers.json`.
///
/// Returns base64-standard of `nonce[24] || ciphertext` (suitable for
/// storing in the `password_file_enc` field of `PairedDevice`).
///
/// AAD = `"pake_password_file|{canonical_fingerprint}"` — binds the
/// ciphertext to the peer it belongs to.
pub(crate) fn encrypt_pake_password_file(
    plaintext: &[u8],
    canonical_fingerprint: &str,
    local_key: &[u8; 32],
) -> Result<String, String> {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;

    let aad = [
        PAKE_PASSWORD_FILE_AAD_PREFIX,
        canonical_fingerprint.as_bytes(),
    ]
    .concat();
    let (nonce, ciphertext) =
        encrypt_item_with_aad(plaintext, local_key, &aad).map_err(|e| e.to_string())?;

    // Encode as nonce[24] || ciphertext so decrypt can split on the fixed nonce size.
    let mut blob = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&ciphertext);
    Ok(b64.encode(&blob))
}

/// Decrypt a `password_file_enc` value from `peers.json` back to the raw
/// `PasswordFile` blob bytes.
///
/// Returns `Err` if the base64 is malformed, the blob is too short (< 24
/// bytes for the nonce), or AEAD authentication fails (wrong key / tampered
/// data). Callers should log and treat the entry as unusable.
pub(crate) fn decrypt_pake_password_file(
    enc_b64: &str,
    canonical_fingerprint: &str,
    local_key: &[u8; 32],
) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;

    let blob = b64
        .decode(enc_b64)
        .map_err(|e| format!("base64 decode: {e}"))?;
    if blob.len() < NONCE_SIZE {
        return Err(format!(
            "password_file_enc too short: {} bytes (expected ≥ {NONCE_SIZE})",
            blob.len()
        ));
    }
    let nonce: [u8; NONCE_SIZE] = blob[..NONCE_SIZE].try_into().expect("slice length checked");
    let ciphertext = &blob[NONCE_SIZE..];

    let aad = [
        PAKE_PASSWORD_FILE_AAD_PREFIX,
        canonical_fingerprint.as_bytes(),
    ]
    .concat();
    decrypt_item_with_aad(ciphertext, &nonce, local_key, &aad).map_err(|e| e.to_string())
}

/// Load peers list from peers.json via the canonical typed `crate::peers`
/// helper.  Returns `serde_json::Value` objects so that all existing call
/// sites (which rely on dynamic field access) continue to work without
/// change.  This wrapper is the SOLE reader used by the IPC handlers; the
/// typed `crate::peers::load_peers` is the underlying implementation, so
/// there is now exactly one deserialization path.
///
/// Filters out any peer whose fingerprint is an all-same-repeated-byte
/// placeholder (fix FAKE-PEERS #31 — test fixtures must not leak into runtime).
pub(crate) fn load_peers() -> anyhow::Result<Vec<serde_json::Value>> {
    let path = peers_file_path();
    let typed = crate::peers::load_peers(&path);
    // Strip placeholder fingerprints.  Log once so the admin knows the file
    // had stale test data; do NOT auto-delete peers.json (non-destructive).
    let filtered: Vec<serde_json::Value> = typed
        .into_iter()
        .filter_map(|p| {
            if is_placeholder_fingerprint(&p.fingerprint) {
                tracing::warn!(
                    fingerprint = %p.fingerprint,
                    "list_peers: skipping placeholder/test fingerprint in peers.json (all-same-byte)"
                );
                return None;
            }
            // Serialize the typed record back to a JSON Value so all
            // existing call-sites that do dynamic field access continue to
            // work.  The round-trip is lossless: every field on `PairedDevice`
            // (including `password_file_b64`) is preserved by serde.
            match serde_json::to_value(p) {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("load_peers: failed to serialize PairedDevice: {e}");
                    None
                }
            }
        })
        .collect();
    Ok(filtered)
}

/// HB-4: build the set of IP HOSTS we have already paired with, for correlating
/// mDNS-discovered peers against `peers.json`.
///
/// The mDNS `device_id` advertised by a peer is a random UUID, NOT its cert
/// fingerprint, so a fingerprint-compare never matched a discovered peer to a
/// paired record — already-paired devices kept showing "Pair". Instead we match
/// on the network identity: a peer's `local_ip` and the HOST part of its
/// `address` (`host:port`). A discovered peer is "paired" when any of its
/// resolved `ip_addrs` is in this set.
pub(crate) fn paired_ip_hosts(peers: &[serde_json::Value]) -> std::collections::HashSet<String> {
    let mut hosts = std::collections::HashSet::new();
    for p in peers {
        if let Some(ip) = p.get("local_ip").and_then(|v| v.as_str()) {
            if !ip.is_empty() {
                hosts.insert(ip.to_string());
            }
        }
        if let Some(addr) = p.get("address").and_then(|v| v.as_str()) {
            // `address` is `host:port`; keep only the host. `rsplit_once(':')`
            // tolerates bracketed IPv6 (`[::1]:9123`) by stripping the trailing
            // `:port` and leaving the bracketed host, which still matches the
            // bracket-free `ip_addrs` form below only for IPv4 — IPv6 hosts are
            // matched via `local_ip` instead.
            let host = match addr.rsplit_once(':') {
                Some((h, _port)) => h,
                None => addr,
            };
            let host = host.trim_start_matches('[').trim_end_matches(']');
            if !host.is_empty() {
                hosts.insert(host.to_string());
            }
        }
    }
    hosts
}

/// Persist peers list to peers.json atomically with mode 0600, via the
/// canonical typed `crate::peers::save_peers` helper.
///
/// This is the SOLE writer used by the IPC handlers.  The input
/// `serde_json::Value` slice is deserialized into the typed `PairedDevice`
/// form first, then handed to `crate::peers::save_peers` which performs the
/// atomic 0600 rename.  Unrecognised fields (e.g. from an older file format)
/// are silently dropped; all current fields — including `password_file_enc`
/// (encrypted PasswordFile) and the legacy `password_file_b64` — are
/// preserved by `PairedDevice`.
///
/// Unified from two former writers (`serde_json::Value` variant here and
/// `crate::peers::save_peers` via `persist_paired_peer`) to eliminate the
/// concurrent-writer race (CopyPaste-qvn).
pub(crate) fn save_peers(peers: &[serde_json::Value]) -> anyhow::Result<()> {
    let path = peers_file_path();
    let typed: Vec<crate::peers::PairedDevice> = peers
        .iter()
        .filter_map(
            |v| match serde_json::from_value::<crate::peers::PairedDevice>(v.clone()) {
                Ok(p) => Some(p),
                Err(e) => {
                    tracing::warn!("save_peers: skipping malformed record: {e}");
                    None
                }
            },
        )
        .collect();
    crate::peers::save_peers(&path, &typed)
}

/// Validate that a fingerprint string matches the XX:XX:... hex pattern.
pub(crate) fn is_valid_fingerprint(fp: &str) -> bool {
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
/// form; [`copypaste_p2p::transport::PairedPeers::is_known`] compares against `fingerprint_of` output.
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

/// Fire-and-forget: send a `ControlMsg::Unpair` signal to the peer identified
/// by `canonical_fp` if it currently has a live sink in `live_sinks`.
///
/// This is the **send side** of mutual unpair.  Called by `unpair_peer`,
/// `revoke_peer`, and `revoke_all_peers` after the local eviction has already
/// committed, so the peer learns it has been removed while the connection is
/// still open rather than waiting for the next mTLS handshake rejection.
///
/// Design properties:
/// - **Non-blocking**: uses `try_send`; a full or closed sink is silently
///   ignored.  The unpair has already taken effect locally; the signal is
///   best-effort delivery only.
/// - **No panic**: all `Mutex::lock` failures (poisoned lock) are silently
///   swallowed so a prior panic cannot prevent the caller from returning a
///   success response.
/// - **Minimal blast radius**: only the specific peer's sink is touched; other
///   connections are unaffected.
pub(crate) fn send_unpair_signal_if_connected(
    live_sinks: &Arc<std::sync::Mutex<Option<crate::p2p::LivePeerSinks>>>,
    canonical_fp: &str,
) {
    use copypaste_sync::protocol::{ControlMsg, PeerFrame};

    // Acquire the outer Mutex<Option<LivePeerSinks>> — this holds the Arc to the
    // inner async Mutex<HashMap> only for the brief clone, never across send.
    let sinks_arc_opt = match live_sinks.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return, // poisoned — skip silently
    };
    let sinks_arc = match sinks_arc_opt {
        Some(a) => a,
        None => return, // P2P not started
    };

    // `try_lock` on the async Mutex: if the map is momentarily locked by an
    // accept/fanout task we skip — it is only needed to clone the sender.
    let sender_opt = match sinks_arc.try_lock() {
        Ok(map) => map.get(canonical_fp).cloned(),
        Err(_) => return,
    };

    if let Some(tx) = sender_opt {
        // `try_send` never blocks; Closed/Full both mean "skip silently".
        let _ = tx.try_send(PeerFrame::Control(ControlMsg::Unpair));
        tracing::debug!(peer = %canonical_fp, "mutual unpair: sent Unpair signal to connected peer");
    }
}

/// Gap A (durable unpair): record a pending `ControlMsg::Unpair` delivery in
/// `pending_unpair.json` so the P2P connector loop can dial the (possibly
/// offline) peer on its next reconnect and deliver the signal there.
///
/// The live `send_unpair_signal_if_connected` above is fire-and-forget: if the
/// peer is not currently connected the signal is silently dropped and the peer
/// keeps treating us as paired. This durable queue closes that gap. Best-effort:
/// a write failure is logged, never surfaced — the local unpair already
/// committed. `address` is the peer's last-known `host:port`; a `None` address
/// is still queued (the connector skips it until an address is learned) so the
/// intent is not lost.
pub(crate) fn queue_unpair_for_offline_delivery(
    fingerprint: &str,
    address: Option<&str>,
    name: &str,
) {
    let pending_path = crate::peers::pending_unpair_path_for(&peers_file_path());
    if let Err(e) = crate::peers::queue_pending_unpair(&pending_path, fingerprint, address, name) {
        tracing::warn!(
            peer = %fingerprint,
            error = %e,
            "mutual unpair: failed to queue durable pending-unpair record"
        );
    } else {
        tracing::debug!(
            peer = %fingerprint,
            has_addr = address.is_some(),
            "mutual unpair: queued durable pending-unpair for offline delivery"
        );
    }
}

/// Extract and validate a UUID `"id"` param from an IPC request, returning a
/// typed `ERR_CODE_INVALID_ARGUMENT` error response on failure.
///
/// Used by the typed-error IPC arms (`delete_item`, `copy_item`, `pin_item`,
/// `reorder_pinned`, `get_item_image`, `get_item_thumbnail`, `get_item_file`)
/// to eliminate repeated boilerplate. Arms that use the legacy untyped
/// `Response::err` style (`delete`, `copy`/`paste`, `pin`) are left unchanged.
pub(crate) fn extract_uuid_param(
    params: &serde_json::Value,
    req_id: String,
) -> Result<String, crate::protocol::Response> {
    let id = match params.get("id").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => {
            return Err(crate::protocol::Response::err_with_code(
                req_id,
                crate::protocol::ERR_CODE_INVALID_ARGUMENT,
                "missing param: id",
            ))
        }
    };
    if uuid::Uuid::parse_str(&id).is_err() {
        return Err(crate::protocol::Response::err_with_code(
            req_id,
            crate::protocol::ERR_CODE_INVALID_ARGUMENT,
            "invalid param: id must be a valid UUID",
        ));
    }
    Ok(id)
}

/// Maximum lifetime of an in-progress PAKE session before it is evicted as
/// stale (fix/p2p-c-review #1 — DoS). The full 3-message handshake is two
/// user-driven IPC round-trips; 120 s is generous for a human typing a
/// pairing password on the second device while bounding how long a leaked /
/// abandoned session (crashed client) pins a `PakeInitiator`/`PakeResponder`
/// in memory.
pub(crate) const PAKE_SESSION_TTL: std::time::Duration = std::time::Duration::from_secs(120);

/// Hard cap on the number of simultaneously-live PAKE sessions (fix/p2p-c-review
/// #1 — DoS). Pairing is an interactive, one-at-a-time-per-user operation; a
/// healthy host never approaches this. The cap converts an unbounded-growth
/// memory-exhaustion vector into a bounded one: past the cap, new `initiate` /
/// `pair_accept_password` calls are rejected with a clear error rather than
/// allocating without limit.
pub(crate) const MAX_PAKE_SESSIONS: usize = 64;

/// A peer whose `last_sync_at` is within this many seconds of the current
/// clock is considered **online** in the `list_peers` response when no live
/// mTLS/mDNS signal is available (the `live_sink` path is authoritative and
/// unaffected by this threshold).
///
/// CopyPaste-1jms.25: this is derived from the SAME recency window the sync
/// badge chip uses (`copypaste_ipc::SYNC_BADGE_RECENT_MS`, 5 min) so the two
/// user-facing "recently heard from?" signals agree. Previously this was a
/// standalone `60` while the chip used 300 s, so a peer 75 s stale showed an
/// **offline** peer-card dot but a non-error chip — a contradictory state. The
/// `live_sink` path still flips a disconnected peer to offline immediately, so
/// widening this fallback only affects the P2P-disabled / pre-connect case.
pub(crate) const ONLINE_THRESHOLD_SECS: i64 = (copypaste_ipc::SYNC_BADGE_RECENT_MS / 1_000) as i64;

/// c4q2.21: Pure function for computing peer online status.
///
/// Priority:
/// 1. `live_sink` is `Some(true)` / `Some(false)` — P2P connection table is
///    authoritative; use it unconditionally.
/// 2. `live_sink` is `None` — P2P is disabled or not yet running; fall back to
///    `last_sync_at`: online iff within [`ONLINE_THRESHOLD_SECS`] of `now_secs`.
///
/// Extracted for unit-testability (c4q2.21).
pub(crate) fn compute_peer_online(
    live_sink: Option<bool>,
    last_sync_at: Option<i64>,
    now_secs: i64,
) -> bool {
    match live_sink {
        Some(is_live) => is_live,
        None => matches!(last_sync_at,
            Some(t) if now_secs.saturating_sub(t) <= ONLINE_THRESHOLD_SECS
        ),
    }
}

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
pub(crate) enum PakeSession {
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
pub(crate) struct StampedPakeSession {
    pub(crate) session: PakeSession,
    pub(crate) created_at: std::time::Instant,
}
