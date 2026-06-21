//! P2P clipboard sync session FFI exports.
//!
//! Covers: `LocalItem`, `SyncedItem`, `P2pSyncResult`, `sync_with_peer`,
//! and private helpers `shared_sync_key_from_session`, `canonicalize_fingerprint`,
//! `is_fingerprint_revoked`, and the `P2P_SYNC_KEY_SALT` / `P2P_WIRE_KEY_VERSION`
//! constants.

use copypaste_core::{decrypt_from_cloud, encrypt_for_cloud, ITEM_KEY_VERSION_CURRENT};

use crate::{ffi_pairing::runtime, panic_boundary, CopypasteError};

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
pub const P2P_SYNC_KEY_SALT: &[u8] = b"copypaste/p2p/content-sync-key/v1";

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
pub const P2P_WIRE_KEY_VERSION: u8 = ITEM_KEY_VERSION_CURRENT as u8;

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
pub fn shared_sync_key_from_session(
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
pub fn canonicalize_fingerprint(fp: &str) -> String {
    fp.chars()
        .filter(|c| *c != ':')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Returns `true` if `fingerprint` is present in `revoked` after canonicalizing
/// both sides. This is the security predicate enforced at the top of
/// [`sync_with_peer`]; it is unit-tested directly so the refusal can be
/// verified without a live socket.
pub fn is_fingerprint_revoked(fingerprint: &str, revoked: &[String]) -> bool {
    let target = canonicalize_fingerprint(fingerprint);
    revoked
        .iter()
        .any(|r| canonicalize_fingerprint(r) == target)
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
    session_keys: Vec<crate::p2p_listener::PeerSessionKey>,
    local_items: Vec<LocalItem>,
    device_id: String,
) -> Result<crate::p2p_listener::P2pListenerHandle, CopypasteError> {
    panic_boundary::catch_result(|| {
        crate::p2p_listener::start(
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
    panic_boundary::catch_result(|| crate::p2p_listener::poll(listener_id))
}

/// Live roster/denylist/session-key refresh without restarting the listener.
/// Removes any no-longer-allowed or revoked fingerprint from the pinned
/// allowlist immediately (rejected at the next TLS handshake) and replaces the
/// denylist + per-peer session keys. No-op for an unknown `listener_id`.
pub fn update_p2p_listener_peers(
    listener_id: u64,
    allowed: Vec<String>,
    revoked: Vec<String>,
    session_keys: Vec<crate::p2p_listener::PeerSessionKey>,
) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        crate::p2p_listener::update_peers(listener_id, allowed, revoked, session_keys)
    })
}

/// Cancel and deregister the listener. Idempotent: a second call (or an unknown
/// id) is a no-op. Fires the cancel token so the accept loop and its
/// per-connection tasks exit and the listener socket is dropped.
pub fn stop_p2p_listener(listener_id: u64) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| crate::p2p_listener::stop(listener_id))
}
