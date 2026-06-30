#![allow(clippy::empty_line_after_doc_comments)] // uniffi-generated scaffolding triggers this lint

uniffi::include_scaffolding!("copypaste_android");

pub mod p2p_listener;
pub mod pairing;
pub mod panic_boundary;
pub mod stun;
pub mod version;
pub use p2p_listener::{P2pListenerHandle, PeerSessionKey};
pub use pairing::{DiscoveredPeer, PairStatus};
pub use panic_boundary::PanicError;
pub use version::{
    check_compatibility, core_version, uniffi_abi_version, VersionError, UNIFFI_ABI_VERSION,
};

// ── Split modules ────────────────────────────────────────────────────────────
// Each module contains a cohesive group of UniFFI-exported functions/types that
// were previously inlined in this god file. All public items are re-exported
// from lib.rs so the generated Kotlin bindings are unchanged (UDL references
// the types by name; the scaffolding finds them via `pub use`).
pub mod ffi_cloud_sync;
pub mod ffi_config;
pub mod ffi_crypto;
pub mod ffi_db;
pub mod ffi_error;
pub mod ffi_p2p_session;
pub mod ffi_pairing;
pub mod ffi_revocation;
pub mod ffi_sensitive;
pub mod ffi_system;

pub use ffi_cloud_sync::{
    cloud_decrypt, cloud_encrypt, derive_cloud_sync_key, relay_inbox_id, relay_public_key_b64,
    relay_registration_pop,
};
pub use ffi_config::{
    appconfig_from_config, clamp_config, config_from_appconfig, default_config, Config,
    DEFAULT_IMAGE_MAX_HEIGHT, DEFAULT_MASK_SENSITIVE_CONTENT, DEFAULT_P2P_ENABLED,
};
pub use ffi_crypto::{
    decrypt_text, decrypt_text_batch, encrypt_text, DecryptBatchResult, DecryptedItem,
    EncryptedBlob, EncryptedItem,
};
#[cfg(feature = "android-uniffi-live")]
pub use ffi_db::db_by_path;
pub use ffi_db::{
    add_clipboard_item, close_database, db_handle_to_cache_key, db_vacuum, fts_search,
    get_history_count, get_history_page, key_cache_hash, open_database, store_clipboard_item,
    with_cached_db, HistoryItem, SearchResultItem,
};
pub use ffi_error::CopypasteError;
pub use ffi_p2p_session::{
    canonicalize_fingerprint, is_fingerprint_revoked, poll_p2p_listener,
    shared_sync_key_from_session, start_p2p_listener, stop_p2p_listener, sync_with_peer,
    update_p2p_listener_peers, LocalItem, P2pSyncResult, SyncedItem, P2P_SYNC_KEY_SALT,
    P2P_WIRE_KEY_VERSION,
};
pub use ffi_pairing::{
    bootstrap_pair_initiator, bootstrap_result_from_pairing, build_android_peer_meta,
    build_pairing_qr, confirmed_pairing_from, generate_device_cert, list_discovered, pair_abort,
    pair_confirm_sas, pair_get_sas, pair_reset, pair_with_discovered, parse_pairing_qr,
    start_discovery, stop_discovery, BootstrapResult, DeviceCert, PairingQrPayload, ScannedPairing,
    SyncProvisioning,
};
pub use ffi_revocation::{
    derive_new_sync_key_from_passphrase, list_revoked_fingerprints, list_revoked_peers,
    revoke_device_and_rotate_key, revoke_device_audit, rotate_sync_key, RevokedPeer,
};
pub use ffi_sensitive::{
    byte_to_char_offset_android, detect_sensitive_spans, is_sensitive, sensitive_capture_decision,
    sensitive_expires_at_ms, sensitive_kind, SensitiveCaptureDecision, SensitiveSpan,
};
pub use ffi_system::{
    classify_text_kind, compute_android_sync_badge_state, get_private_mode, resolve_stun_public_ip,
    set_private_mode, sync_badge_recent_ms,
};

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
