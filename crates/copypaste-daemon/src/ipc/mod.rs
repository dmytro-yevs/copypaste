use crate::protocol::{
    Request, Response, CURRENT_PROTOCOL_VERSION, ERR_CODE_AUTH_FAILED, ERR_CODE_INTERNAL_ERROR,
    ERR_CODE_INVALID_ARGUMENT, ERR_CODE_IPC_NOT_READY, ERR_CODE_NOT_FOUND,
    ERR_CODE_NOT_IMPLEMENTED, ERR_CODE_RATE_LIMITED, ERR_CODE_REQUEST_TOO_LARGE,
    ERR_CODE_VERSION_MISMATCH, MIN_SUPPORTED_PROTOCOL_VERSION,
};
use anyhow::Context as _; // CopyPaste-crh3.90
                          // CopyPaste-merc / CopyPaste-1jms.22: canonical badge-state computation lives in
                          // copypaste-ipc. The `_with_inflight` variant is used so the daemon can emit the
                          // `Syncing` (green-pulse) badge while a round-trip is in progress. Gated on
                          // cloud-sync: the get_sync_status handler is only compiled with that feature, so
                          // the import must match to avoid an unused-import warning (-D warnings).
#[cfg(feature = "cloud-sync")]
use copypaste_ipc::compute_sync_badge_state_with_inflight;
// derive_sync_key / SyncKey are used by both cloud-sync (Supabase) and
// relay-sync. `revoke_and_rotate` / `rotate_sync_key` / `set_sync_passphrase`
// derive a key from a passphrase + the Supabase account id (the single
// per-account-salt derivation); `revoke_peer` uses `SyncKey::random()` for
// automatic no-passphrase rotation (CopyPaste-gbo fix).
#[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
use copypaste_core::derive_sync_key;
#[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
use copypaste_core::SyncKey;
use copypaste_core::{
    bump_item_recency,
    chunks_from_blob,
    count_items,
    decode_file,
    decode_image,
    decrypt_item_by_version,
    derive_v2,
    ensure_revoked_devices_table,
    fetch_text_preview,
    // c4q2.17: get_page removed — the "list" handler that used it is now stubbed.
    fetch_text_previews_batch,
    get_device_names,
    get_item_by_id,
    get_page_pinned_first,
    is_sensitive_for_autowipe,
    pin_item,
    reorder_pinned,
    revoke_device,
    revoke_devices,
    search_items_filtered,
    unpin_item,
    Database,
    DbRead,
    SensitiveDetector,
    V1Key,
    V2Key,
};
// l07l: EncryptError is only matched on the macOS pasteboard decrypt path, so
// gate it to macOS — otherwise it's an unused import on non-macOS (-D warnings).
#[cfg(target_os = "macos")]
use copypaste_core::EncryptError;
// `soft_delete_item` is not yet re-exported from the crate root so we use the
// full module path (the `storage` module is `pub`).
use copypaste_core::storage::items::soft_delete_item;
use copypaste_p2p::pake::{
    channel_confirmation_tag, ConfirmRole, PakeInitiator, PakeResponder, CONFIRM_TAG_LEN,
};
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Submodules (behaviour-preserving split of the original ipc.rs god-file).
// Only clearly-separable, cohesive groups were extracted; everything that
// touches shared state or calls other helpers without a clean boundary stays
// here in mod.rs.  All pub / pub(crate) symbols from the submodules are
// re-exported below so external call sites (`crate::ipc::Foo`) remain valid
// without any modification.
// ---------------------------------------------------------------------------

pub(super) mod config;
pub(super) mod pairing;
pub(super) mod pasteboard;
pub(super) mod socket;

// ── config ─────────────────────────────────────────────────────────────────
// pub / pub(crate) items used by callers outside the ipc module:
pub use config::p2p_enabled_from_config;
pub(crate) use config::read_config;
pub(crate) use config::update_core_config;
pub use config::AppConfig;
// Helpers used directly in impl IpcServer dispatch code (non-test):
use config::{build_config_response, merge_config, write_config};
// Helpers only called from the inline test module; #[cfg(test)] keeps the
// import from triggering -D warnings in non-test compilation.
#[cfg(test)]
pub(crate) use config::config_path;

// ── pairing ────────────────────────────────────────────────────────────────
// pub(crate) items callable from outside ipc:
pub(crate) use pairing::canonical_fingerprint;
pub(crate) use pairing::compute_peer_online;
pub(crate) use pairing::display_fingerprint;
pub(crate) use pairing::peers_file_path;
// Helpers used in impl IpcServer dispatch code (non-test):
use pairing::{
    byte_to_char_offset, encrypt_pake_password_file, extract_uuid_param, is_valid_fingerprint,
    load_peers, paired_ip_hosts, queue_unpair_for_offline_delivery, save_peers, too_large_to_sync,
    PakeSession, StampedPakeSession, MAX_PAKE_SESSIONS, PAKE_SESSION_TTL,
};
// Helpers only called from the inline test module:
#[cfg(test)]
pub(crate) use pairing::{decrypt_pake_password_file, ONLINE_THRESHOLD_SECS};

// ── socket ─────────────────────────────────────────────────────────────────
// bind_with_stale_cleanup is called from impl IpcServer::bind (non-test):
use socket::bind_with_stale_cleanup;
// Helpers only called from the inline test module:
#[cfg(test)]
pub(crate) use socket::{
    is_socket_live, pid_exe_is_copypaste, pid_exe_path, probe_listening_daemon,
};

// ── pasteboard ─────────────────────────────────────────────────────────────
// pub(crate) items used by external callers:
pub(crate) use pasteboard::parse_file_meta;
pub(crate) use pasteboard::parse_image_file_id;
pub(crate) use pasteboard::parse_image_thumb_file_id;
// ra15.1: the non-test callers of these two helpers live in the macOS-gated
// paste-back path, but the helpers themselves are cross-platform and exercised
// by inline tests on every platform. Allow them unused on the non-macOS lib
// build so -D warnings stays green without breaking the Linux test build.
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
pub(crate) use pasteboard::paste_file_cache_dir;
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
pub(crate) use pasteboard::prune_old_paste_files;
// Helpers used in impl IpcServer dispatch code (non-test):
use pasteboard::{lazy_backfill_thumbnail, parse_image_thumb_dims, PasteboardError};
// ra15.1: `map_content_type_to_uti` is `#[cfg(target_os = "macos")]` (its only
// caller is the macOS paste-back path); gate the import to match so the
// non-macOS (Linux) build resolves (CI E0432).
#[cfg(target_os = "macos")]
use pasteboard::map_content_type_to_uti;

// ── params (CopyPaste-vp63.52 dedup) ────────────────────────────────────────
mod params;
// Helper used in impl IpcServer dispatch code across handlers_items /
// handlers_pairing / handlers_sync (non-test):
use params::extract_str_param;

// ── consts (CopyPaste-vp63.19) ──────────────────────────────────────────────
mod consts;
// pub items re-exported so external `crate::ipc::X` call sites keep resolving
// (these were `pub const` directly in mod.rs before the split):
pub use consts::{
    BUILD_VERSION, DEGRADED_REASON_DB_KEY_MISMATCH, DEGRADED_REASON_KEYCHAIN_LOCKED,
    IPC_READ_TIMEOUT, IPC_WRITE_TIMEOUT, PEER_EVENT_QUEUE_CAP,
};
// Previously bare (module-private) consts — a private import here keeps them
// visible to every descendant of `ipc` (via `use super::*`) without widening
// their reachability beyond that, matching the original scope exactly.
use consts::{
    ERR_IPC_NOT_READY, MAX_CONCURRENT_CONNECTIONS, MAX_IMPORT_ITEM_BYTES, MAX_PAGE,
    MAX_REQUEST_BYTES, SMALL_REQUEST_BYTES,
};

// ── restore (CopyPaste-vp63.19) ─────────────────────────────────────────────
mod restore;
// restore_database_file was module-private (no external callers); a private
// import keeps it reachable from handlers_db.rs / tests.rs via `super::*`.
use restore::restore_database_file;

// ra15.1: dispatch handler groups + helper-method clusters extracted from the
// original ipc god-module. mod.rs keeps the core types, shared consts/helpers,
// and the thin dispatcher that routes to the per-domain handlers below.
mod builder;
mod connection;
mod handlers_config;
mod handlers_db;
mod handlers_items;
mod handlers_items_clipboard;
mod handlers_items_ingest;
mod handlers_items_media;
mod handlers_items_mutate;
mod handlers_items_paste;
mod handlers_items_read;
mod handlers_pairing;
mod handlers_pairing_password;
mod handlers_pairing_qr;
mod handlers_pairing_revoke;
mod handlers_pairing_sas;
mod handlers_peers;
mod handlers_status;
mod handlers_sync;
mod handlers_sync_auth;
mod handlers_sync_keys;
mod handlers_sync_status;
mod handlers_transfer;
mod pairing_ops_flows_discovered;
mod pairing_ops_flows_qr;
mod pairing_ops_persist;
mod pairing_ops_provisioning;
mod pairing_ops_session;

// ── server (CopyPaste-vp63.19) ──────────────────────────────────────────────
// The `IpcServer` struct definition + `PeerEventRecord` live in server.rs; all
// `impl IpcServer` blocks (builder, connection, dispatch, handlers, pairing
// ops) stay in their existing sibling files above and reach the type via this
// re-export, exactly as if it were still declared directly in mod.rs.
mod server;
pub use server::{IpcServer, PeerEventRecord};

impl IpcServer {
    async fn dispatch(&self, line: &str) -> Response {
        let req: Request = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                // CopyPaste-cbfl: echo the request's id back so the CLI's
                // id-matching guard doesn't reject this error response.
                // If the line is valid JSON but not a Request, extract id
                // from the raw Value. Fall back to "?" only when the line
                // is not parseable as JSON at all.
                let echo_id = serde_json::from_str::<serde_json::Value>(line)
                    .ok()
                    .and_then(|v| {
                        v["id"]
                            .as_str()
                            .map(|s| s.to_string())
                            .or_else(|| v["id"].as_i64().map(|n| n.to_string()))
                            .or_else(|| v["id"].as_u64().map(|n| n.to_string()))
                    })
                    .unwrap_or_else(|| "?".to_string());
                return Response::err_with_code(
                    echo_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    format!("parse error: {e}"),
                );
            }
        };

        tracing::Span::current().record("method", req.method.as_str());
        tracing::debug!(method = %req.method, id = %req.id, "IPC request");

        // Protocol-version gate (ADR-007) + readiness gate. ADR-007: the version
        // gate uses ERR_CODE_VERSION_MISMATCH so the CLI can surface the "please
        // upgrade" prompt (P2-ptb8). Shared with the watch_subscribe path via
        // check_request_gates (CopyPaste-crh3.105).
        if let Some(err) = self.check_request_gates(&req, false) {
            return err;
        }

        // ra15.1: route to the per-domain handler chain. Each dispatch_*
        // returns a Response and falls through to the next domain via its
        // `_ =>` arm; the final dispatch_items_extra holds the unknown-method
        // fallback. Behaviour is identical to the original single match.
        self.dispatch_items(req).await
    }
}

#[cfg(test)]
mod tests;
