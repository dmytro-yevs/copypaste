use std::path::PathBuf;
// l07l: AtomicI64/Ordering are only exercised by the macOS pasteboard
// change-count path; allow them unused on non-macOS so -D warnings stays green.
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
use std::sync::atomic::AtomicI64;
use std::sync::Arc;

use copypaste_core::{
    build_item_aad_v2, decrypt_from_cloud, decrypt_item_by_version, derive_v2,
    encode_image_with_limit, encrypt_for_cloud, encrypt_item_with_aad, ClipboardItem, SyncKey,
    AAD_SCHEMA_VERSION_V4, NONCE_SIZE,
};
// c7fp: encrypt_chunks / IMAGE_CHUNK_SIZE / ImageMeta are only used in
// `rewrap_inbound_blob` and `read_png_dimensions` which are macOS-only
// (`#[cfg_attr(not(target_os = "macos"), allow(dead_code))]`).  Allow the
// import to be unused on non-macOS so -D warnings stays green.
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
use copypaste_core::{encrypt_chunks, ImageMeta, IMAGE_CHUNK_SIZE};
use copypaste_sync::{merge::wire_to_local, protocol::WireItem};
use tracing::{debug, warn};

/// Context passed to [`merge_incoming_with_crypto`] to enable the
/// Universal Clipboard auto-apply feature: when a genuinely fresh remote
/// item wins the LWW merge, write its decrypted plaintext directly to
/// NSPasteboard so it is ready to paste immediately.
///
/// The `self_write_change_count` is the **same** `Arc<AtomicI64>` the
/// [`ClipboardMonitor`](crate::clipboard::ClipboardMonitor) checks on every
/// poll tick.  Writing to NSPasteboard increments the system changeCount;
/// we stamp the new changeCount into this atomic before the monitor's next
/// tick so the poller recognises the write as ours and skips re-capturing it
/// (loop prevention — identical to the mechanism used by the `copy_item` IPC
/// handler).
pub struct AutoApplyCtx {
    /// Shared self-write sentinel for the pasteboard poller.
    pub self_write_change_count: Arc<AtomicI64>,
    /// This device's local encryption key (v1 seed).  Needed to decrypt image
    /// chunks for NSPasteboard writes.
    pub local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    /// Live daemon config.  The `auto_apply_synced_clip` flag is read here on
    /// every merge so toggling it via `set_config` takes effect immediately.
    pub core_config: Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
}

/// Cross-device content-key context for the sync orchestrator (P2P Phase 3).
///
/// Items are stored at rest encrypted under this device's *per-device*
/// local-storage key, so the on-wire ciphertext is undecryptable by any other
/// device. To make a synced item readable on a paired peer we re-key it through
/// a **shared content sync key** established at pairing (derived deterministically
/// from the PAKE session key — both peers hold the identical key):
///
/// * **outbound** — decrypt the row's ciphertext with the local key, then
///   re-encrypt the plaintext under the **per-peer** sync key (K_AB for peer B,
///   K_AC for peer C, etc. — [`encrypt_for_cloud`], XChaCha20-Poly1305 + per-item-id
///   AAD). The wire item carries that blob with `content_nonce = None` (the cloud
///   blob is self-framed: it prefixes its own 24-byte nonce).
/// * **inbound** — decrypt the wire blob with the shared sync key, then
///   re-encrypt the plaintext under THIS device's local v2 key before storing,
///   and index the plaintext into FTS so search + previews work for synced rows.
///
/// When no shared key is available (P2P disabled, or a legacy peer record with
/// no `sync_key_b64`) the orchestrator falls back to the legacy behaviour:
/// outgoing items ship their raw at-rest ciphertext (undecryptable on the peer,
/// exactly as before Phase 3) and incoming items are stored verbatim.
///
/// ## Key model (CopyPaste-716 fix)
///
/// Keys are **per-peer pairwise**: K_AB (shared between A and B) differs from
/// K_AC (shared between A and C). The previous implementation cached only the
/// FIRST peer's key and used it for all fanout targets, so peer C received a
/// blob encrypted under K_AB — which it could not decrypt (silent sync failure).
///
/// The fix: `cached_peer_keys` is a `HashMap<fingerprint, [u8; 32]>` populated
/// from **all** paired peers in `peers.json`. `sync_key_for_peer(fp)` does a
/// O(1) map lookup; the outbound fanout path calls it once per peer and
/// re-encrypts independently.
///
/// ## Caching (H8 perf fix, preserved)
///
/// The key map is wrapped in `Arc<Mutex<…>>` so all `SyncCrypto` clones
/// (including the temporary copy inside `merge_incoming_with_crypto::spawn_blocking`)
/// share the same backing store. `reload_sync_key` refreshes the entire map
/// atomically — visible to every live clone immediately.
#[derive(Clone)]
pub struct SyncCrypto {
    /// This device's v1 local-storage key (the raw seed from `load_local_key`).
    pub(super) v1_key: [u8; 32],
    /// This device's v2 local-storage key (`derive_v2(seed)`).
    /// Item 5: wrapped in `Zeroizing` so the key bytes are scrubbed on drop.
    pub(super) v2_key: zeroize::Zeroizing<[u8; 32]>,
    /// Path to `peers.json`. Only read during construction and `reload_sync_key`
    /// — NOT on every crypto operation (H8 fix).
    peers_path: PathBuf,
    /// Per-peer sync key cache (CopyPaste-716 fix).
    ///
    /// Maps canonical peer fingerprint → 32-byte pairwise sync key bytes.
    /// Populated from ALL paired peers in `peers.json` (not just the first).
    /// Shared via `Arc` so every `SyncCrypto` clone observes the same map.
    /// Updated atomically by `reload_sync_key` after any pairing write.
    cached_peer_keys: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, [u8; 32]>>>,
}

impl SyncCrypto {
    /// Build a crypto context from the device's local-storage seed and the
    /// `peers.json` path. Eagerly loads the shared sync key from `peers.json`
    /// so the hot-path `sync_key_for_peer()` never touches the filesystem.
    pub fn new(local_seed: [u8; 32], peers_path: PathBuf) -> Self {
        let cached = Self::load_keys_from_peers(&peers_path);
        Self {
            v1_key: local_seed,
            v2_key: derive_v2(&local_seed),
            cached_peer_keys: std::sync::Arc::new(std::sync::Mutex::new(cached)),
            peers_path,
        }
    }

    /// Read `peers.json` once and return a map of canonical fingerprint →
    /// 32-byte sync key for every paired peer that has a valid `sync_key_b64`.
    ///
    /// CopyPaste-716: previously this returned only the FIRST peer's key via
    /// `find_map`, causing all fanout targets beyond the first peer to receive
    /// a blob encrypted under the wrong key. Now returns ALL peers' keys.
    ///
    /// The map key is the **canonical** (colon-free lowercase hex) fingerprint —
    /// the same form used by the mTLS transport as `DeviceFingerprint` in
    /// `peer_sinks`. `peers.json` stores colon-hex (e.g. `"aa:bb:cc"`); we
    /// normalise via `canonical_fingerprint` so lookups by `DeviceFingerprint`
    /// always hit (CopyPaste-716 secondary fix).
    fn load_keys_from_peers(
        peers_path: &std::path::Path,
    ) -> std::collections::HashMap<String, [u8; 32]> {
        use base64::Engine as _;
        crate::peers::load_peers(peers_path)
            .into_iter()
            .filter_map(|dev| {
                let b64 = dev.sync_key_b64.as_deref()?;
                let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
                let key = <[u8; 32]>::try_from(bytes.as_slice()).ok()?;
                // Normalise to canonical (colon-free lowercase) so lookups by
                // DeviceFingerprint (the mTLS transport's canonical form) hit.
                let canonical = crate::ipc::canonical_fingerprint(&dev.fingerprint);
                Some((canonical, key))
            })
            .collect()
    }

    /// Return the sync key for a specific peer fingerprint.
    ///
    /// This is an O(1) map read (no file I/O — H8 preserved). Call
    /// `reload_sync_key` after any pairing write to refresh all peers' keys.
    ///
    /// The `fingerprint` parameter may be in either colon-hex (`aa:bb:cc`) or
    /// canonical colon-free lowercase form — this function normalises before
    /// lookup so both call sites (tests with colon-hex, production fanout with
    /// canonical DeviceFingerprint) work correctly.
    ///
    /// Returns `None` when the peer has no sync key (legacy peer record or
    /// no pairing yet).
    pub fn sync_key_for_peer(&self, fingerprint: &str) -> Option<SyncKey> {
        let canonical = crate::ipc::canonical_fingerprint(fingerprint);
        let guard = self
            .cached_peer_keys
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        guard
            .get(canonical.as_str())
            .copied()
            .map(SyncKey::from_bytes)
    }

    /// Return ANY available shared content sync key (if any peer has one).
    ///
    /// **Outbound use only** — used by `rekey_outbound` / `rekey_blob_outbound`
    /// for the legacy single-peer fallback path.  For the inbound path use
    /// [`Self::all_sync_keys`] (CopyPaste-kw2) to avoid the arbitrary-first-entry
    /// bias that breaks 3+ device topologies.
    ///
    /// This is an O(1) memory read — no file I/O (H8 fix).
    pub(super) fn shared_sync_key(&self) -> Option<SyncKey> {
        let guard = self
            .cached_peer_keys
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        guard.values().next().copied().map(SyncKey::from_bytes)
    }

    /// Return ALL cached pairwise sync keys as a `Vec<SyncKey>`.
    ///
    /// Used on the **inbound** path (CopyPaste-kw2 fix): because the mTLS
    /// authenticated sender fingerprint is dropped before items reach the
    /// merge path, we cannot look up the exact pairwise key by fingerprint.
    /// Instead we try every registered peer key until AEAD decryption
    /// succeeds — the authentication tag guarantees only the correct key
    /// accepts the ciphertext, so this is both correct and safe.
    ///
    /// In the common 2-device case there is exactly one key and the cost is
    /// identical to the previous `values().next()` path. In a 3+-device
    /// topology each sender encrypts under the pairwise key shared with
    /// THIS device, so at most one entry in the vec will ever succeed.
    pub(super) fn all_sync_keys(&self) -> Vec<SyncKey> {
        let guard = self
            .cached_peer_keys
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        guard.values().copied().map(SyncKey::from_bytes).collect()
    }

    /// Re-read `peers.json` and update the in-memory per-peer key map. Call
    /// this once after any write to `peers.json` (pairing completion, revoke)
    /// so the orchestrator picks up new/changed keys without a daemon restart.
    ///
    /// Because `cached_peer_keys` is an `Arc`, this update is visible to
    /// every `SyncCrypto` clone (including ones moved into `spawn_blocking`
    /// closures) immediately.
    pub fn reload_sync_key(&self) {
        let new_keys = Self::load_keys_from_peers(&self.peers_path);
        let mut guard = self
            .cached_peer_keys
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        *guard = new_keys;
    }

    /// Returns `true` if the in-memory per-peer key map contains at least one
    /// entry.
    ///
    /// Only available in test builds so production code cannot accidentally
    /// depend on the cache state as a signal (reload_sync_key is the contract).
    #[cfg(test)]
    pub fn has_cached_sync_key(&self) -> bool {
        !self
            .cached_peer_keys
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_empty()
    }
}

/// Outcome of an attempt to re-key an outgoing item under the shared sync key.
#[derive(Debug, PartialEq, Eq)]
pub enum RekeyOutcome {
    /// The payload was successfully re-wrapped under the shared sync key — the
    /// wire item is decryptable by the paired peer and safe to forward.
    Rewrapped,
    /// The item is not a re-key candidate (non-text, no content/nonce, or no
    /// shared key is available). The wire item is left unchanged and follows
    /// the legacy path — it may carry raw at-rest ciphertext (a no-crypto /
    /// legacy peer expects that) or be an image chunk handled elsewhere.
    NotApplicable,
    /// A shared key WAS available and the item WAS a candidate, but re-keying
    /// failed (wrong nonce length, local-decrypt, or shared-encrypt error). The
    /// wire item still carries raw at-rest ciphertext that the peer can never
    /// decrypt — the caller MUST drop it rather than forward a permanently
    /// undecryptable row (sync H2).
    Failed,
}

/// Maximum reassembled image/file plaintext we will re-key onto the wire.
///
/// The P2P transport frames at 16 MiB (`transport.rs`) and the cloud relay
/// caps the request body, so an oversized blob would either be rejected by the
/// transport or land undecryptable on the peer. We enforce the ceiling here so
/// the item is *dropped with a warning* rather than silently corrupting sync.
///
/// Ceiling layering (one blob, four caps — see `defaults.rs::MAX_FILE_SIZE_BYTES`):
///   * STORABLE = 100 MiB — `copypaste_core::MAX_FILE_BYTES`, library hard cap on
///     a locally-stored file item. `max_file_size_bytes` is clamped to this.
///   * SYNC     =   8 MiB — *this* const: the largest plaintext re-keyed onto the
///     wire. Items 8–100 MiB are kept LOCALLY but skipped for sync (warned).
///   * P2P frame =  16 MiB — transport framing cap.
///   * Relay body = 10 MiB — relay request-body cap.
///
/// So a file can be storable yet un-syncable: local storage and sync are
/// deliberately decoupled, and the UI tells the user where the sync line sits.
pub const SYNC_MAX_BLOB_BYTES: usize = 8 * 1024 * 1024;

/// Reassemble an image/file item's at-rest chunk blob back into plaintext.
///
/// The chunks were encrypted under this device's LOCAL v1 seed (`crypto.v1_key`)
/// with the 16-byte `file_id` as AEAD AAD — exactly as `daemon::handle_image`
/// (and the file pipeline) writes them. We parse `file_id` out of the
/// `blob_ref` meta JSON (shared parser for both image and file), deserialize the
/// chunks, and decode:
///   * image → [`copypaste_core::decode_image`] (PNG bytes)
///   * file  → [`copypaste_core::decode_file`] (verbatim bytes)
///
/// Returns the recovered plaintext, or `None` on any parse/decrypt failure
/// (logged). Callers map `None` to `RekeyOutcome::Failed` so a corrupt local
/// row is dropped, never forwarded.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn recover_blob_plaintext(crypto: &SyncCrypto, wire: &WireItem) -> Option<Vec<u8>> {
    let meta_json = wire.blob_ref.as_deref()?;
    let file_id = match crate::ipc::parse_image_file_id(meta_json) {
        Ok(id) => id,
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: blob meta parse failed: {e}");
            return None;
        }
    };
    let content = wire.content.as_deref()?;
    let chunks = match copypaste_core::chunks_from_blob(content) {
        Ok(c) => c,
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: chunks_from_blob failed: {e}");
            return None;
        }
    };
    // Dispatch on the wire item's key_version: v1 rows use the raw local
    // key (v1 seed); v2 rows use derive_v2(seed). After the writer fix
    // (handle_image / handle_file now use derive_v2), all freshly-captured
    // rows are kv=2. Legacy rows stamped kv=2 but encrypted with v1 (the
    // mislabeled rows) are repaired by repair_mislabeled_kv2_blob_rows at
    // startup, so by the time sync runs all kv=2 rows are truly v2.
    let blob_key: &[u8; 32] = if wire.key_version == 1 {
        &crypto.v1_key
    } else {
        &crypto.v2_key
    };
    let decoded = if wire.content_type == "image" {
        copypaste_core::decode_image(&chunks, blob_key, &file_id).map_err(|e| e.to_string())
    } else {
        copypaste_core::decode_file(&chunks, blob_key, &file_id).map_err(|e| e.to_string())
    };
    match decoded {
        Ok(pt) => Some(pt),
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: blob decode failed: {e}");
            None
        }
    }
}

/// Re-key an image/file wire item onto the shared sync key.
///
/// Reassembles the at-rest blob to plaintext ([`recover_blob_plaintext`]),
/// enforces [`SYNC_MAX_BLOB_BYTES`], then replaces `content` with a single
/// shared-key-wrapped blob (`encrypt_for_cloud`, same call the text arm uses),
/// clears `content_nonce` (the unwrap marker) and `blob_ref`, and keeps
/// `content_type`. Mirrors the text arm's `Failed`/`NotApplicable` contract.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
/// Inner implementation for blob (image/file) outbound re-keying under an
/// explicit `SyncKey` (CopyPaste-716: key now passed in by caller rather than
/// fetched from the first-peer-only cache).
pub(super) fn rekey_blob_outbound_with_key(
    crypto: &SyncCrypto,
    shared: &SyncKey,
    wire: &mut WireItem,
) -> RekeyOutcome {
    // A shared key IS present: from here any failure is `Failed` (drop), never
    // a silent forward of an undecryptable at-rest blob (sync H2).
    let Some(plaintext) = recover_blob_plaintext(crypto, wire) else {
        return RekeyOutcome::Failed;
    };
    if plaintext.len() > SYNC_MAX_BLOB_BYTES {
        warn!(
            item_id = %wire.item_id,
            size = plaintext.len(),
            max = SYNC_MAX_BLOB_BYTES,
            "sync_orch: blob exceeds sync ceiling, dropping (not forwarded)"
        );
        return RekeyOutcome::Failed;
    }
    match encrypt_for_cloud(shared, &wire.item_id, &plaintext) {
        Ok(blob) => {
            wire.content = Some(blob);
            // Self-framed blob → no item-level nonce; `None` is the receiver's
            // sync-key-wrapped marker.
            wire.content_nonce = None;
            // For file items: stash filename + mime into the dedicated wire
            // fields BEFORE clearing blob_ref, so the receiver can reconstruct
            // the local file meta JSON with the correct identity. blob_ref
            // itself must not travel (it is a local at-rest artefact; the
            // receiver rebuilds it from recovered plaintext + these fields).
            if wire.content_type == "file" {
                if let Some((fname, fmime)) =
                    wire.blob_ref.as_deref().and_then(parse_file_name_mime)
                {
                    wire.file_name = Some(fname);
                    wire.mime = Some(fmime);
                }
            }
            wire.blob_ref = None;
            RekeyOutcome::Rewrapped
        }
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: blob shared-encrypt failed: {e}");
            RekeyOutcome::Failed
        }
    }
}

pub(super) fn rekey_blob_outbound(crypto: &SyncCrypto, wire: &mut WireItem) -> RekeyOutcome {
    let Some(shared) = crypto.shared_sync_key() else {
        return RekeyOutcome::NotApplicable;
    };
    rekey_blob_outbound_with_key(crypto, &shared, wire)
}

/// Re-encrypt an outgoing item's payload under the shared content sync key so a
/// paired peer can decrypt it (P2P Phase 3).
///
/// Decrypts the row's at-rest ciphertext with this device's local key (by
/// `key_version`), then re-encrypts the plaintext under the shared sync key via
/// [`encrypt_for_cloud`] (XChaCha20-Poly1305, AAD bound to `item_id`). The
/// resulting self-framed blob (its own 24-byte nonce prefix + ciphertext+tag)
/// is placed in `wire.content` and `wire.content_nonce` is cleared to `None`,
/// which the receiver uses as the "sync-key-wrapped" marker.
///
/// Returns [`RekeyOutcome`]:
/// * [`RekeyOutcome::Rewrapped`] — payload re-wrapped, safe to forward.
/// * [`RekeyOutcome::NotApplicable`] — non-text, no content/nonce, or no shared
///   key: the wire item is left UNCHANGED and follows the legacy path.
/// * [`RekeyOutcome::Failed`] — a shared key was present but the crypto step
///   failed; the wire still carries raw at-rest ciphertext the peer cannot
///   decrypt, so the caller must DROP it (sync H2).
///
/// Image and file items are re-keyed by reassembling the at-rest chunk blob
/// into plaintext (decoded with the LOCAL v1 seed + `file_id` AAD), then
/// re-wrapping that whole plaintext under the shared sync key — identical wire
/// shape to text (`content_nonce = None`, `blob_ref = None`, `content_type`
/// preserved). See [`recover_blob_plaintext`] / [`rekey_blob_outbound`].
pub(super) fn rekey_outbound(crypto: &SyncCrypto, wire: &mut WireItem) -> RekeyOutcome {
    if wire.content_type == "image" || wire.content_type == "file" {
        return rekey_blob_outbound(crypto, wire);
    }
    if wire.content_type != "text" {
        return RekeyOutcome::NotApplicable;
    }
    let Some(shared) = crypto.shared_sync_key() else {
        return RekeyOutcome::NotApplicable;
    };
    rekey_outbound_text_with_key(crypto, &shared, wire)
}

/// Inner text re-key under an explicit `SyncKey` (CopyPaste-716: per-peer key).
///
/// Decrypts the at-rest ciphertext under `crypto`'s local key, then
/// re-encrypts under `peer_key`. Caller is responsible for passing the correct
/// per-peer key (via [`SyncCrypto::sync_key_for_peer`]).
pub(super) fn rekey_outbound_text_with_key(
    crypto: &SyncCrypto,
    peer_key: &SyncKey,
    wire: &mut WireItem,
) -> RekeyOutcome {
    let (Some(ciphertext), Some(nonce_vec)) = (wire.content.as_ref(), wire.content_nonce.as_ref())
    else {
        return RekeyOutcome::NotApplicable;
    };
    // From here on a shared key IS present and the item IS a re-key candidate,
    // so any failure must surface as `Failed` (drop), never silent forward.
    let mut nonce = [0u8; NONCE_SIZE];
    if nonce_vec.len() != NONCE_SIZE {
        warn!(item_id = %wire.item_id, "sync_orch: rekey_outbound wrong nonce length, dropping");
        return RekeyOutcome::Failed;
    }
    nonce.copy_from_slice(nonce_vec);

    let plaintext = match decrypt_item_by_version(
        wire.key_version,
        &crypto.v1_key,
        &crypto.v2_key,
        &wire.item_id,
        &nonce,
        ciphertext,
    ) {
        Ok(pt) => pt,
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: rekey_outbound local-decrypt failed: {e}");
            return RekeyOutcome::Failed;
        }
    };

    match encrypt_for_cloud(peer_key, &wire.item_id, &plaintext) {
        Ok(blob) => {
            wire.content = Some(blob);
            // The cloud blob is self-framed (nonce prefix), so there is no
            // separate item-level nonce. `None` is the receiver's unwrap marker.
            wire.content_nonce = None;
            RekeyOutcome::Rewrapped
        }
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: rekey_outbound shared-encrypt failed: {e}");
            RekeyOutcome::Failed
        }
    }
}

/// Re-encrypt an outgoing item under the pairwise sync key for `peer_fingerprint`.
///
/// CopyPaste-716: this is the correct per-peer fanout call. Unlike
/// [`rekey_outbound`] (which uses the first cached key for legacy/catchup
/// compatibility), this function looks up the sync key specific to
/// `peer_fingerprint` from the per-peer cache. The caller (fanout + catchup
/// paths) must clone the `WireItem` before calling so each peer gets its own
/// independently-encrypted copy.
///
/// Returns [`RekeyOutcome`]:
/// * [`RekeyOutcome::Rewrapped`] — payload re-wrapped under the peer's key.
/// * [`RekeyOutcome::NotApplicable`] — peer has no sync key, or item type is
///   not re-keyable (non-text/image/file). Wire item is left unchanged.
/// * [`RekeyOutcome::Failed`] — key present but crypto failed; caller must drop.
pub fn rekey_outbound_for_peer(
    crypto: &SyncCrypto,
    peer_fingerprint: &str,
    wire: &mut WireItem,
) -> RekeyOutcome {
    let Some(peer_key) = crypto.sync_key_for_peer(peer_fingerprint) else {
        return RekeyOutcome::NotApplicable;
    };
    if wire.content_type == "image" || wire.content_type == "file" {
        return rekey_blob_outbound_with_key(crypto, &peer_key, wire);
    }
    if wire.content_type != "text" {
        return RekeyOutcome::NotApplicable;
    }
    rekey_outbound_text_with_key(crypto, &peer_key, wire)
}

/// Inverse of [`rekey_outbound`]: turn a sync-key-wrapped incoming wire item
/// into a [`ClipboardItem`] encrypted under THIS device's local v2 key, plus
/// the recovered plaintext (for FTS indexing).
///
/// Returns `Err(wire)` (handing the item back unchanged) when the item is not
/// sync-key-wrapped or cannot be decrypted, so the caller can fall back to
/// storing it verbatim.
// `WireItem` is ~232 bytes, so a bare `Result<_, WireItem>` trips
// clippy::result_large_err. We box the rarely-taken error payload (the
// hand-back-unchanged path) to keep the common Ok variant small.
#[allow(clippy::result_large_err)]
pub(super) fn rekey_inbound(
    crypto: &SyncCrypto,
    wire: WireItem,
) -> Result<(ClipboardItem, Option<Vec<u8>>), Box<WireItem>> {
    // Marker: a sync-key-wrapped payload carries content but no nonce.
    let is_blob = wire.content_type == "image" || wire.content_type == "file";
    if (wire.content_type != "text" && !is_blob)
        || wire.content_nonce.is_some()
        || wire.content.is_none()
    {
        return Err(Box::new(wire));
    }

    // CopyPaste-kw2 fix: try ALL registered peer keys instead of the arbitrary
    // first entry in the HashMap.  In a 3+-device topology the authenticated
    // mTLS sender fingerprint is dropped before items reach the merge path, so
    // we cannot look up the pairwise key by fingerprint here.  AEAD guarantees
    // that only the correct key (K_sender_this_device) produces a valid tag —
    // trying every key until one succeeds is correct, safe, and O(n) in the
    // number of paired peers (typically 1-3).
    let peer_keys = crypto.all_sync_keys();
    if peer_keys.is_empty() {
        return Err(Box::new(wire));
    }

    if is_blob {
        // For blobs try each key; pass ownership of wire only to the first
        // attempt, hand it back on failure, and on the final failure return.
        let mut wire_box = Box::new(wire);
        for key in &peer_keys {
            match rewrap_inbound_blob(crypto, *wire_box, key) {
                Ok(pair) => return Ok(pair),
                Err(w) => {
                    wire_box = w;
                }
            }
        }
        return Err(wire_box);
    }

    let blob = match wire.content.as_ref() {
        Some(b) => b.clone(),
        None => return Err(Box::new(wire)),
    };

    // Try each pairwise key until AEAD decryption succeeds (CopyPaste-kw2).
    let plaintext = {
        let mut found: Option<Vec<u8>> = None;
        for key in &peer_keys {
            match decrypt_from_cloud(key, &wire.item_id, &blob) {
                Ok(pt) => {
                    found = Some(pt);
                    break;
                }
                Err(_) => continue,
            }
        }
        match found {
            Some(pt) => pt,
            None => {
                warn!(item_id = %wire.item_id, "sync_orch: rekey_inbound: all peer keys failed to decrypt (tried {})", peer_keys.len());
                return Err(Box::new(wire));
            }
        }
    };

    // Re-encrypt under this device's local v2 key + v4 AAD so the stored row is
    // readable by the production read path (`decrypt_item_by_version` at v2).
    let aad = build_item_aad_v2(&wire.item_id, AAD_SCHEMA_VERSION_V4, 2);
    let (nonce, ciphertext) = match encrypt_item_with_aad(&plaintext, &crypto.v2_key, &aad) {
        Ok(out) => out,
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: rekey_inbound local-encrypt failed: {e}");
            return Err(Box::new(wire));
        }
    };

    let mut local = wire_to_local(wire);
    local.content = Some(ciphertext);
    local.content_nonce = Some(nonce.to_vec());
    local.key_version = 2;
    Ok((local, Some(plaintext)))
}

/// Byte ceiling for the small-image fast path in [`rewrap_inbound_blob`].
///
/// Images whose plaintext PNG is ≤ this size skip the full pixel-decode +
/// re-encode cycle (`encode_image_with_limit`) and are stored by encrypting the
/// original PNG bytes directly.  The AEAD AAD (`file_id`, `key_version = 1`) is
/// identical to the full-encode path, so the decode path is unaffected.
///
/// 512 KB covers virtually all macOS screenshot-paste ("grab-a-selection" via
/// ⌘⇧4), which are the dominant tiny-image case that triggers the bug report.
/// Larger images still go through the full normalise → re-encode pipeline.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const SMALL_IMAGE_FAST_PATH_BYTES: usize = 512 * 1024;

/// Read the pixel dimensions of a PNG by parsing its IHDR chunk without
/// decoding the pixel data.
///
/// Used by the small-image fast path in [`rewrap_inbound_blob`] to populate
/// [`copypaste_core::ImageMeta`] cheaply (O(1) bytes read, no heap alloc for
/// the bitmap).  Falls back to `(0, 0)` on any parse error so the caller can
/// proceed with neutral metadata rather than failing the whole re-wrap.
///
/// PNG IHDR layout (RFC 2083 §11.2.2):
///   Offset  Bytes  Field
///    0       8     PNG signature
///    8       4     IHDR length (always 13)
///   12       4     Chunk type ("IHDR")
///   16       4     Width (big-endian u32)
///   20       4     Height (big-endian u32)
///   ...
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn read_png_dimensions(png: &[u8]) -> Option<(u32, u32)> {
    // Minimum valid PNG: 8 (sig) + 4 (len) + 4 (type) + 13 (IHDR) + 4 (crc) = 33 bytes.
    if png.len() < 24 {
        return None;
    }
    // Verify the 8-byte PNG signature so we don't misparse non-PNG data.
    const PNG_SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    if png[..8] != PNG_SIG {
        return None;
    }
    // Width and height are at bytes 16–19 and 20–23 respectively.
    let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    Some((width, height))
}

/// Inverse of [`rekey_blob_outbound`]: unwrap a sync-key-wrapped image/file
/// payload and re-chunk it under THIS device's local v1 seed so the stored row
/// reads back through the production image/file decode path.
///
/// 1. `decrypt_from_cloud(shared, item_id, content)` → plaintext (the original
///    PNG / file bytes).
/// 2. Re-derive `file_id` deterministically from the plaintext content hash so
///    the AEAD AAD matches on both devices and item_id/dedup converge.
/// 3. Re-encode under `crypto.v1_key` (image → [`encode_image_with_limit`] or
///    the small-image fast path, file → `encode_file`) → `chunks_to_blob` →
///    `local.content`; rebuild the meta JSON; set `blob_ref`, `content_type`,
///    `key_version = 1` (chunks are v1-keyed). `fts_plaintext = None` (blobs
///    are not FTS-indexed).
///
/// **Small-image fast path (Fix C):** for images whose plaintext PNG is
/// ≤ [`SMALL_IMAGE_FAST_PATH_BYTES`], the expensive pixel-decode + re-encode
/// step inside `encode_image_with_limit` is skipped.  The PNG bytes are
/// encrypted directly via `encrypt_chunks` and image dimensions are read from
/// the PNG header without decoding the pixel data.  The AEAD keys, AAD, and
/// stored format are identical to the full path.
///
/// Returns `Err(wire)` (hand back unchanged) on any failure so the caller can
/// fall back to verbatim storage.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[allow(clippy::result_large_err)]
pub(super) fn rewrap_inbound_blob(
    crypto: &SyncCrypto,
    wire: WireItem,
    shared: &SyncKey,
) -> Result<(ClipboardItem, Option<Vec<u8>>), Box<WireItem>> {
    // F2: decrypt borrows the at-rest blob in place — no `.clone()` of the
    // (potentially multi-MiB) ciphertext. We still hand `wire` back intact on
    // either failure path so the caller's verbatim-storage fallback keeps the
    // original `content`. The borrow of `wire.content` ends before each
    // `Err(Box::new(wire))` move (NLL), so returning `wire` is sound.
    let plaintext = match wire.content.as_deref() {
        Some(blob) => match decrypt_from_cloud(shared, &wire.item_id, blob) {
            Ok(pt) => pt,
            Err(e) => {
                warn!(item_id = %wire.item_id, "sync_orch: inbound blob shared-decrypt failed: {e}");
                return Err(Box::new(wire));
            }
        },
        None => return Err(Box::new(wire)),
    };

    // Re-derive file_id deterministically from the recovered bytes (same hash
    // the sender used at capture) so item_id and dedup converge across devices.
    let file_id = crate::clipboard::image_content_hash(&plaintext);

    let (chunks_blob, meta_json) = if wire.content_type == "image" {
        // Fix C — small-image fast path: for tiny PNGs skip the full pixel
        // decode+re-encode cycle.  The sender already ran `encode_as_png` before
        // storing, so the plaintext IS a valid PNG; we just re-encrypt it
        // verbatim.  Dimensions are read from the PNG IHDR (cheap — no pixel
        // alloc).  AEAD keys + AAD are identical to the full path.
        if plaintext.len() <= SMALL_IMAGE_FAST_PATH_BYTES {
            let (width, height) = read_png_dimensions(&plaintext).unwrap_or((0, 0));
            let original_size = plaintext.len() as u64;
            match encrypt_chunks(&plaintext, &crypto.v1_key, &file_id, IMAGE_CHUNK_SIZE) {
                Ok(chunks) => {
                    let chunk_count = match u32::try_from(chunks.len()) {
                        Ok(n) => n,
                        Err(_) => {
                            warn!(item_id = %wire.item_id, "sync_orch: inbound image fast-path: chunk count overflow");
                            return Err(Box::new(wire));
                        }
                    };
                    let blob = match copypaste_core::chunks_to_blob(&chunks) {
                        Ok(b) => b,
                        Err(e) => {
                            warn!(item_id = %wire.item_id, "sync_orch: inbound image fast-path: chunks_to_blob failed: {e}");
                            return Err(Box::new(wire));
                        }
                    };
                    let meta = ImageMeta {
                        width,
                        height,
                        original_size,
                        chunk_count,
                        file_id,
                    };
                    let thumb_file_id = crate::clipboard::image_thumb_file_id(&file_id);
                    let meta_json =
                        crate::clipboard::build_image_meta_json(&meta, &thumb_file_id, 0, 0);
                    debug!(
                        item_id = %wire.item_id,
                        size = plaintext.len(),
                        "sync_orch: inbound image stored via small-image fast path (no pixel re-encode)"
                    );
                    (blob, meta_json)
                }
                Err(e) => {
                    warn!(item_id = %wire.item_id, "sync_orch: inbound image fast-path: encrypt_chunks failed: {e}");
                    return Err(Box::new(wire));
                }
            }
        } else {
            // Full encode path for larger images: pixel decode + re-encode to
            // normalise format, then chunk-encrypt.
            match encode_image_with_limit(
                &plaintext,
                &crypto.v1_key,
                &file_id,
                copypaste_core::MAX_IMAGE_BYTES,
                copypaste_core::config::MAX_DECODED_IMAGE_MB,
            ) {
                Ok((meta, chunks)) => {
                    let blob = match copypaste_core::chunks_to_blob(&chunks) {
                        Ok(b) => b,
                        Err(e) => {
                            warn!(item_id = %wire.item_id, "sync_orch: inbound image chunks_to_blob failed: {e}");
                            return Err(Box::new(wire));
                        }
                    };
                    // No thumbnail is synced (regenerated on demand); record a
                    // distinct thumb_file_id with zero dims so the meta shape stays
                    // consistent and get_item_thumbnail returns the null sentinel.
                    let thumb_file_id = crate::clipboard::image_thumb_file_id(&file_id);
                    let meta_json =
                        crate::clipboard::build_image_meta_json(&meta, &thumb_file_id, 0, 0);
                    (blob, meta_json)
                }
                Err(e) => {
                    warn!(item_id = %wire.item_id, "sync_orch: inbound image re-encode failed: {e}");
                    return Err(Box::new(wire));
                }
            }
        }
    } else {
        // File: re-chunk verbatim. Prefer the dedicated wire fields
        // (file_name / mime) stamped by `rekey_blob_outbound`; fall back to
        // parsing blob_ref (pre-21b peers or direct non-rekey paths) and
        // finally to neutral defaults when neither is available.
        let (raw_filename, mime) = if wire.file_name.is_some() || wire.mime.is_some() {
            (
                wire.file_name.clone().unwrap_or_else(|| "file".to_string()),
                wire.mime
                    .clone()
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
            )
        } else {
            wire.blob_ref
                .as_deref()
                .and_then(parse_file_name_mime)
                .unwrap_or_else(|| ("file".to_string(), "application/octet-stream".to_string()))
        };
        // fr44: sanitize the peer-supplied filename before storage — defense in
        // depth against path-traversal and shell-special characters injected by
        // a malicious peer.  The dangerous-extension check is enforced at the
        // open/view layer (Tauri ipc.rs on macOS, HistoryActivity on Android);
        // sanitize_filename here ensures the stored name is always filesystem-safe
        // regardless of which client later opens the item.
        let filename = copypaste_core::sanitize_filename(&raw_filename);
        // B3: this is the INBOUND re-chunk path; the configured per-device
        // capture knob (`max_file_size_bytes`) is NOT threaded this deep (doing
        // so would change `run`'s signature and its daemon.rs call site, which is
        // out of scope here). Using `MAX_FILE_BYTES` is now coherent regardless:
        // `clamp_values` caps the user knob AT `MAX_FILE_BYTES`, so the storable
        // ceiling and this bound are the same number — we accept any item a peer
        // could legitimately have stored, never more.
        match copypaste_core::encode_file(
            &plaintext,
            &filename,
            &mime,
            &crypto.v1_key,
            &file_id,
            copypaste_core::MAX_FILE_BYTES,
        ) {
            Ok((meta, chunks)) => {
                let blob = match copypaste_core::chunks_to_blob(&chunks) {
                    Ok(b) => b,
                    Err(e) => {
                        warn!(item_id = %wire.item_id, "sync_orch: inbound file chunks_to_blob failed: {e}");
                        return Err(Box::new(wire));
                    }
                };
                let meta_json = crate::clipboard::build_file_meta_json(&meta);
                (blob, meta_json)
            }
            Err(e) => {
                warn!(item_id = %wire.item_id, "sync_orch: inbound file re-encode failed: {e}");
                return Err(Box::new(wire));
            }
        }
    };

    let mut local = wire_to_local(wire);
    local.content = Some(chunks_blob);
    local.content_nonce = None;
    local.blob_ref = Some(meta_json);
    // Chunk content is keyed by the LOCAL v1 seed + file_id AAD, NOT the v2
    // item-AAD scheme — the image/file read paths decode with v1.
    local.key_version = 1;
    Ok((local, None))
}

/// Parse `filename` / `mime` out of a file `blob_ref` meta JSON (the shape
/// produced by `clipboard::build_file_meta_json`). Returns `None` if either
/// field is absent so the caller can fall back to neutral defaults.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn parse_file_name_mime(meta_json: &str) -> Option<(String, String)> {
    let value: serde_json::Value = serde_json::from_str(meta_json).ok()?;
    let filename = value.get("filename")?.as_str()?.to_string();
    let mime = value.get("mime")?.as_str()?.to_string();
    Some((filename, mime))
}
