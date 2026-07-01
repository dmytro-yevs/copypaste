//! Cross-device content-key context: [`SyncCrypto`] (per-peer sync-key cache)
//! and [`AutoApplyCtx`] (Universal-Clipboard auto-apply state).
//!
//! Split out of the former flat `rekey.rs` (ADR-017, CopyPaste-vp63.9) — moved
//! verbatim, no behavior change.

use std::path::PathBuf;
// l07l: AtomicI64/Ordering are only exercised by the macOS pasteboard
// change-count path; allow them unused on non-macOS so -D warnings stays green.
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
use std::sync::atomic::AtomicI64;
use std::sync::Arc;

use copypaste_core::{derive_v2, SyncKey};

/// Context passed to [`crate::sync_orch::merge::merge_incoming_with_crypto`] to enable the
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
///   K_AC for peer C, etc. — [`copypaste_core::encrypt_for_cloud`], XChaCha20-Poly1305 + per-item-id
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Characterization test (CopyPaste-vp63.9): `load_keys_from_peers` should
    /// populate the cache only from entries carrying a valid `sync_key_b64`,
    /// silently skipping legacy peers that have none.
    #[test]
    fn load_keys_from_peers_skips_legacy_entries_without_sync_key() {
        use base64::Engine as _;
        let dir = std::env::temp_dir().join(format!(
            "copypaste-rekey-crypto-ctx-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let peers_path = dir.join("peers.json");

        let valid_key = [7u8; 32];
        let valid_b64 = base64::engine::general_purpose::STANDARD.encode(valid_key);

        let json = format!(
            r#"[
                {{"fingerprint": "aa:bb:cc:dd", "name": "Legacy Device", "paired_at": 0}},
                {{"fingerprint": "11:22:33:44", "name": "Valid Device", "paired_at": 0, "sync_key_b64": "{valid_b64}"}}
            ]"#
        );
        let mut f = std::fs::File::create(&peers_path).expect("create peers.json");
        f.write_all(json.as_bytes()).expect("write peers.json");
        drop(f);

        let crypto = SyncCrypto::new([1u8; 32], peers_path);
        assert!(crypto.has_cached_sync_key());
        assert_eq!(crypto.sync_key_for_peer("11:22:33:44").is_some(), true);
        assert_eq!(crypto.sync_key_for_peer("aa:bb:cc:dd").is_some(), false);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
